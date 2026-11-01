// Stress and soak tests for the Linux ComputerUse layer.
//
// These answer three questions the acceptance tests do not: does throughput hold
// up, do resources leak, and does anything degrade after repetition. Every case
// prints its measured numbers so a regression is visible as a number rather than
// a vibe.
//
//   cargo test --release -p quixi-platform --test stress -- --test-threads=1 --nocapture
//
// Input cases need QUIXI_INPUT_TEST=1 and a focused throwaway terminal.

#![cfg(target_os = "linux")]

mod common;

use quixi_platform::linux;
use std::time::{Duration, Instant};

fn input_armed() -> bool {
    matches!(
        std::env::var("QUIXI_INPUT_TEST").ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

fn open_fds() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count())
        .unwrap_or(0)
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|n| n.to_string()))
        })
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Clipboard throughput and leak behaviour
// ---------------------------------------------------------------------------

#[test]
fn clipboard_large_payloads_roundtrip_exactly() {
    if !common::live_desktop_armed("clipboard_large_payloads_roundtrip_exactly") {
        return;
    }
    // 64 KiB is the practical ceiling for a selection on most compositors; go
    // well past it to find the cliff rather than assume one.
    for size in [1_000usize, 64 * 1024, 1024 * 1024] {
        let payload: String = (0..size)
            .map(|i| (b'a' + (i % 26) as u8) as char)
            .collect();
        let t = Instant::now();
        linux::clipboard_write(&payload).expect("clipboard_write must not fail");
        let w = t.elapsed();
        let t = Instant::now();
        let back = linux::clipboard_read().unwrap_or_default();
        let r = t.elapsed();
        println!(
            "clipboard {:>8} bytes  write={:>7.1}ms  read={:>7.1}ms  match={}",
            size,
            w.as_secs_f64() * 1000.0,
            r.as_secs_f64() * 1000.0,
            back.len() == payload.len() && back == payload
        );
        assert_eq!(back.len(), payload.len(), "clipboard truncated {size} bytes");
        assert_eq!(back, payload, "clipboard corrupted {size} bytes");
        // A write that blocks is the historical deadlock; cap it hard.
        assert!(w < Duration::from_secs(10), "clipboard_write blocked: {w:?}");
    }
}

#[test]
fn clipboard_repeated_use_does_not_leak() {
    if !common::live_desktop_armed("clipboard_repeated_use_does_not_leak") {
        return;
    }
    let fds_before = open_fds();
    let rss_before = rss_kb();
    for i in 0..50 {
        let s = format!("stress-{i}");
        linux::clipboard_write(&s).expect("write must not fail");
        let _ = linux::clipboard_read();
    }
    let fds_after = open_fds();
    let rss_after = rss_kb();
    println!(
        "clipboard x50  fd {}->{}  rss {}->{} KiB",
        fds_before, fds_after, rss_before, rss_after
    );
    assert!(
        fds_after <= fds_before + 5,
        "file descriptor leak: {fds_before} -> {fds_after}"
    );
}

// ---------------------------------------------------------------------------
// Screenshot throughput
// ---------------------------------------------------------------------------

#[test]
fn screenshots_sustain_throughput_without_leaking() {
    if !common::live_desktop_armed("screenshots_sustain_throughput_without_leaking") {
        return;
    }
    let fds_before = open_fds();
    let n = 25;
    let t = Instant::now();
    let mut bytes = 0usize;
    for _ in 0..n {
        let png = linux::screenshot_png().expect("screenshot must succeed");
        bytes += png.len();
    }
    let elapsed = t.elapsed();
    let fds_after = open_fds();
    println!(
        "screenshots x{}  total={:.1}ms  per={:.1}ms  avg_size={} KiB  fd {}->{}",
        n,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / n as f64,
        bytes / n / 1024,
        fds_before,
        fds_after
    );
    assert!(fds_after <= fds_before + 5, "fd leak capturing screenshots");
    let per_capture = elapsed.as_secs_f64() / n as f64;
    assert!(
        per_capture < 5.0,
        "a screenshot taking >5s is a hang, not slowness"
    );
}

#[test]
fn region_capture_is_cheaper_than_full_frame() {
    if !common::live_desktop_armed("region_capture_is_cheaper_than_full_frame") {
        return;
    }
    let t = Instant::now();
    let full = linux::screenshot_png().expect("full capture");
    let full_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let small = linux::screenshot_region_png(0, 0, 320, 200).expect("region capture");
    let small_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "capture  full={:.0}ms ({} KiB)  region320x200={:.0}ms ({} KiB)",
        full_ms,
        full.len() / 1024,
        small_ms,
        small.len() / 1024
    );
    assert!(
        small.len() < full.len(),
        "a 320x200 region should not produce a larger payload than the full frame"
    );
}

// ---------------------------------------------------------------------------
// Probe / enumeration soak
// ---------------------------------------------------------------------------

#[test]
fn capability_probe_sustains_and_does_not_leak() {
    let fds_before = open_fds();
    let t = Instant::now();
    let n = 200;
    for _ in 0..n {
        let c = linux::capabilities();
        assert!(!c.session_type.is_empty());
    }
    let elapsed = t.elapsed();
    let fds_after = open_fds();
    println!(
        "capabilities x{}  total={:.1}ms  per={:.2}ms  fd {}->{}",
        n,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / n as f64,
        fds_before,
        fds_after
    );
    assert!(fds_after <= fds_before + 5, "fd leak probing capabilities");
}

#[test]
fn app_enumeration_is_stable_across_repeats() {
    let first = linux::installed_apps().len();
    for _ in 0..10 {
        let n = linux::installed_apps().len();
        assert_eq!(n, first, "app enumeration is not stable across calls");
    }
    println!("app enumeration stable at {} entries x10", first);
}

#[test]
fn display_enumeration_is_stable_and_fast() {
    let t = Instant::now();
    let n = 50;
    let mut counts = std::collections::BTreeSet::new();
    for _ in 0..n {
        counts.insert(linux::displays().len());
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("displays x{}  per={:.2}ms  distinct counts={:?}", n, per, counts);
    assert_eq!(counts.len(), 1, "display count changed between calls: {counts:?}");
    assert!(per < 500.0, "display enumeration at {per:.0}ms per call is too slow");
}

// ---------------------------------------------------------------------------
// Input soak (armed only)
// ---------------------------------------------------------------------------

#[test]
fn large_text_block_is_typed_completely() {
    if !input_armed() {
        eprintln!("skipped: needs QUIXI_INPUT_TEST=1 and a focused terminal");
        return;
    }
    // Type a long line and prove every character landed by having the shell
    // echo it back into a file whose size we can measure.
    // Focus our own target: an earlier test's click may have stolen focus, and
    // the symptom of that is indistinguishable from a broken input layer.
    common::focus_terminal();
    let marker = "/tmp/quixi_stress_len";
    let _ = std::fs::remove_file(marker);
    let payload = "x".repeat(1200);
    let cmd = format!("printf %s {payload} > {marker}");
    let t = Instant::now();
    linux::type_text(&cmd).expect("type_text must not fail");
    linux::press_key("return", 1).expect("press_key must not fail");
    let typed_ms = t.elapsed().as_secs_f64() * 1000.0;

    for _ in 0..60 {
        if std::path::Path::new(marker).exists() {
            let len = std::fs::metadata(marker).map(|m| m.len()).unwrap_or(0);
            println!(
                "typed {} chars in {:.0}ms ({:.0} chars/s), shell received {} bytes",
                cmd.len(),
                typed_ms,
                cmd.len() as f64 / (typed_ms / 1000.0),
                len
            );
            assert_eq!(len as usize, payload.len(), "keystrokes were dropped or duplicated");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("large typed command never executed");
}

#[test]
fn repeated_key_presses_do_not_degrade() {
    if !input_armed() {
        eprintln!("skipped: needs QUIXI_INPUT_TEST=1");
        return;
    }
    common::focus_terminal();
    let n = 100;
    let t = Instant::now();
    for _ in 0..n {
        linux::press_key("shift", 1).expect("press_key must not fail");
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("press_key x{}  per={:.2}ms", n, per);
    assert!(per < 200.0, "key press at {per:.0}ms per call is too slow");
}

#[test]
fn rapid_pointer_motion_is_accepted() {
    if !input_armed() {
        eprintln!("skipped: needs QUIXI_INPUT_TEST=1");
        return;
    }
    let t = Instant::now();
    let n = 200;
    for i in 0..n {
        let x = 100 + (i % 1200);
        let y = 100 + (i % 800);
        linux::mouse_move(x, y).expect("mouse_move must not fail");
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("mouse_move x{}  per={:.2}ms", n, per);
}

// ---------------------------------------------------------------------------
// Error-path soak: repeated failures must not leak
// ---------------------------------------------------------------------------

#[test]
fn repeated_failures_do_not_leak() {
    let fds_before = open_fds();
    for _ in 0..500 {
        assert!(linux::open_application("no-such-app-xyz").is_err());
        assert!(linux::press_key("nonsense-key", 1).is_err());
        assert!(linux::type_text("é").is_err());
    }
    let fds_after = open_fds();
    println!("error path x500  fd {}->{}", fds_before, fds_after);
    assert!(fds_after <= fds_before + 5, "fd leak on error paths");
}
