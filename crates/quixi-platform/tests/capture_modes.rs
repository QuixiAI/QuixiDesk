// Prove the two capture modes QuixiDesk offers, on this desktop.
//
//   whole frame   -> screenshot_png()                  (the "whole page" option)
//   fine-tuned    -> screenshot_region_png(x,y,w,h)    (the "just what's in the box"
//                                                       option)
//
// Both must go through KWin's ScreenShot2 fast path, both must produce a valid
// PNG, and the region must be cheaper than the full frame — the old
// capture-everything-then-crop path made a small region MORE expensive than the
// whole screen, which is backwards for the one case people use constantly.

#![cfg(target_os = "linux")]

mod common;

use quixi_platform::linux;
use std::time::Instant;

fn png_dims(bytes: &[u8]) -> (u32, u32) {
    // PNG IHDR: 8-byte signature, 4-byte length, "IHDR", then w and h big-endian.
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}

#[test]
fn whole_frame_and_region_are_both_fast_and_valid() {
    if !common::live_desktop_armed("whole_frame_and_region_are_both_fast_and_valid") {
        return;
    }
    // warm up: first D-Bus call pays connection setup
    let _ = linux::screenshot_png();

    let mut full_times = Vec::new();
    let mut full_bytes = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let png = linux::screenshot_png().expect("full capture");
        full_times.push(t.elapsed().as_secs_f64() * 1000.0);
        full_bytes.push(png.len());
    }
    let full_ms = median(&full_times);
    let fw_fh = png_dims(&linux::screenshot_png().expect("full capture"));

    let mut reg_times = Vec::new();
    let mut reg_sizes = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let png = linux::screenshot_region_png(400, 300, 256, 256).expect("region capture");
        reg_times.push(t.elapsed().as_secs_f64() * 1000.0);
        reg_sizes.push(png.len());
    }
    let reg_ms = median(&reg_times);
    let rw_rh = png_dims(&linux::screenshot_region_png(400, 300, 256, 256).expect("region"));

    println!(
        "whole frame : {:>7.1} ms  {:>8} bytes  {}x{}",
        full_ms,
        full_bytes.iter().max().unwrap(),
        fw_fh.0,
        fw_fh.1
    );
    println!(
        "256x256 crop: {:>7.1} ms  {:>8} bytes  {}x{}",
        reg_ms,
        reg_sizes.iter().max().unwrap(),
        rw_rh.0,
        rw_rh.1
    );
    println!("speedup     : {:.1}x", full_ms / reg_ms.max(0.001));

    assert_eq!(rw_rh, (256, 256), "region must be exactly the rectangle asked for");
    assert!(reg_ms < full_ms, "region ({reg_ms:.1}ms) should beat full ({full_ms:.1}ms)");
    assert!(full_ms < 200.0, "full frame at {full_ms:.0}ms is not the fast path");
    assert!(reg_ms < 100.0, "region at {reg_ms:.0}ms is not the fast path");
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[s.len() / 2]
}
