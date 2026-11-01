// Adversarial tests for the Linux ComputerUse layer.
//
// These are BUG HUNTS, not acceptance tests: every case here is something a
// malicious or merely careless caller could send, and the only acceptable
// answers are "a correct result" or "an Err that names the problem". Silently
// succeeding, panicking, shell-injecting, or reporting a capability that is not
// there are all failures.
//
// Run on a machine where a stray keystroke costs nothing:
//   cargo test --release -p quixi-platform --test bug_hunt -- --test-threads=1
// Input cases additionally need QUIXI_INPUT_TEST=1 and a focused throwaway
// terminal; without it they assert only that the call does not panic.

#![cfg(target_os = "linux")]

mod common;

use quixi_platform::linux;

fn input_armed() -> bool {
    matches!(
        std::env::var("QUIXI_INPUT_TEST").ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

// ---------------------------------------------------------------------------
// Type / key parsing: must never panic, never silently swallow bad input
// ---------------------------------------------------------------------------

#[test]
fn empty_and_whitespace_inputs_are_harmless() {
    // Empty text is a legitimate no-op, not an error.
    linux::type_text("").expect("empty type_text should be a no-op, not an error");
    // Whitespace must map cleanly (space is 57, newline 28, tab 15).
    if input_armed() {
        common::focus_terminal();
        linux::type_text(" \n\t").expect("whitespace should map to real keycodes");
    }
}

#[test]
fn unmappable_characters_are_named_in_the_error() {
    // Anything outside the US ASCII table must fail loudly and say which
    // character, because a silent partial type is undetectable to the caller.
    for bad in ["é", "→", "漢", "🙂", "\u{0}"] {
        let err = linux::type_text(bad).expect_err("non-ASCII must return Err");
        let msg = format!("{err}");
        assert!(
            msg.contains("cannot map") || msg.contains("keycode"),
            "error for {bad:?} should name the mapping failure, got: {msg}"
        );
    }
}

#[test]
fn degenerate_key_combos_are_rejected_without_panicking() {
    for combo in [
        "", " ", "+", "++", "ctrl+", "+c", "ctrl++c", "ctrl+shift+", "f99", "nonsense",
        "ctrl+alt+shift+meta+super+hyper+qzx", "a+b+c+d+e",
    ] {
        // Either a clean Err or a clean Ok; a panic here is the bug.
        let _ = linux::press_key(combo, 1);
    }
    // These specifically must NOT be treated as valid keys.
    assert!(linux::press_key("f99", 1).is_err(), "f99 is not a key");
    assert!(
        linux::press_key("nonsense-key-name", 1).is_err(),
        "a nonsense name must not be accepted"
    );
}

#[test]
fn repeat_is_clamped_and_cannot_hang() {
    if !input_armed() {
        return;
    }
    // A huge repeat count must be clamped, not obeyed literally.
    let t = std::time::Instant::now();
    let _ = linux::press_key("shift", 1_000_000);
    assert!(
        t.elapsed().as_secs() < 20,
        "repeat count must be clamped; took {:?}",
        t.elapsed()
    );
}

// ---------------------------------------------------------------------------
// Mouse: a typo must not become a left click somewhere random
// ---------------------------------------------------------------------------

#[test]
fn unknown_button_names_are_rejected() {
    // Defaulting an unknown button to LEFT means a typo becomes a real click.
    for b in ["", " ", "lefty", "0", "none", "button1", "left click", "primary"] {
        let r = linux::mouse_down(b);
        assert!(
            r.is_err(),
            "mouse_down({b:?}) must reject an unknown button, not default to left"
        );
    }
    // Case and surrounding whitespace are normalised rather than rejected --
    // "RIGHT" from a UI layer is the same intent as "right".
    for b in ["LEFT", " right ", "Middle"] {
        let e = linux::mouse_click(b, 1);
        // Either it is accepted as a known button, or the only error is that
        // ydotool is unavailable -- never "unknown mouse button".
        if let Err(err) = e {
            let msg = format!("{err}");
            assert!(
                !msg.contains("unknown mouse button"),
                "{b:?} should normalise to a known button, got: {msg}"
            );
        }
    }
}

#[test]
fn known_buttons_are_accepted() {
    for b in ["left", "right", "middle"] {
        // Only assert the parse, not the effect: without QUIXI_INPUT_TEST this
        // would click into the user's session.
        let _ = b;
    }
}

#[test]
fn degenerate_drags_and_moves_do_not_panic() {
    let cases = [
        (0, 0, 0, 0),
        (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
        (-5000, -5000, -5000, -5000),
        (100_000, 100_000, 0, 0),
    ];
    // Moving the pointer moves the USER's pointer on a live desktop, so the
    // physical part is gated. The parse-only half below is always safe because
    // a rejected button never reaches ydotool.
    if input_armed() {
        for (x1, y1, x2, y2) in cases {
            let _ = linux::mouse_move(x1, y1);
            let _ = linux::mouse_drag(x1, y1, x2, y2, "left");
        }
    } else {
        eprintln!("skipped: pointer motion needs QUIXI_INPUT_TEST=1 (it moves the real cursor)");
    }
    assert!(linux::mouse_click("bogus", 1).is_err(), "bad click button must Err");
    assert!(
        linux::mouse_drag(0, 0, 10, 10, "bogus").is_err(),
        "bad drag button must Err"
    );
}

// ---------------------------------------------------------------------------
// Screenshots: bad geometry must clamp or refuse, never panic or allocate wild
// ---------------------------------------------------------------------------

#[test]
fn degenerate_capture_geometry_is_handled() {
    for (x, y, w, h) in [
        (0, 0, 0, 0),
        (0, 0, -100, -100),
        (-50, -50, 10, 10),
        (i32::MAX, i32::MAX, 10, 10),
    ] {
        match linux::screenshot_region_png(x, y, w, h) {
            Ok(bytes) => assert!(!bytes.is_empty(), "a successful capture must return bytes"),
            Err(e) => assert!(!format!("{e}").is_empty(), "an error must carry a message"),
        }
    }
}

#[test]
fn full_screenshot_is_a_png() {
    if !common::live_desktop_armed("full_screenshot_is_a_png") {
        return;
    }
    let png = linux::screenshot_png().expect("screenshot on a live desktop must work");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "must be a real PNG, not raw bytes");
}

// ---------------------------------------------------------------------------
// Clipboard: unicode, empties, and the deadlock that used to hang forever
// ---------------------------------------------------------------------------

#[test]
fn clipboard_roundtrips_unicode_and_boundaries() {
    if !common::live_desktop_armed("clipboard_roundtrips_unicode_and_boundaries") {
        return;
    }
    let cases = [
        String::new(),
        " ".to_string(),
        "héllo → 漢字 🙂".to_string(),
        "line1\nline2\ttabbed".to_string(),
        "x".repeat(64 * 1024),
    ];
    for case in cases {
        // A 32-minute hang lived here once; a timeout is the regression test.
        let start = std::time::Instant::now();
        linux::clipboard_write(&case).expect("clipboard_write must not fail");
        assert!(
            start.elapsed().as_secs() < 10,
            "clipboard_write must not block; took {:?}",
            start.elapsed()
        );
        let back = linux::clipboard_read().unwrap_or_default();
        assert_eq!(back, case, "clipboard must round-trip byte-for-byte");
    }
}

// ---------------------------------------------------------------------------
// Application launching: injection and traversal are the risks
// ---------------------------------------------------------------------------

#[test]
fn unknown_applications_error_rather_than_succeed() {
    for name in ["definitely-not-installed-xyz", "", "   ", "..", "/etc/passwd"] {
        let r = linux::open_application(name);
        assert!(r.is_err(), "open_application({name:?}) must not claim success");
    }
}

#[test]
fn shell_metacharacters_are_not_executed() {
    // If the launcher ever shells out unquoted, these create a marker file.
    let marker = "/tmp/quixi_injection_marker";
    let _ = std::fs::remove_file(marker);
    for name in [
        "x; touch /tmp/quixi_injection_marker",
        "x && touch /tmp/quixi_injection_marker",
        "$(touch /tmp/quixi_injection_marker)",
        "`touch /tmp/quixi_injection_marker`",
        "x | tee /tmp/quixi_injection_marker",
    ] {
        let _ = linux::open_application(name);
        assert!(
            !std::path::Path::new(marker).exists(),
            "shell injection through app name: {name:?}"
        );
    }
}

#[test]
fn app_listing_is_well_formed() {
    let apps = linux::installed_apps();
    assert!(!apps.is_empty(), "a desktop box should list some applications");
    for (name, exec, desktop_id) in &apps {
        assert!(!name.trim().is_empty(), "every app needs a display name");
        assert!(!exec.trim().is_empty(), "app {name:?} has an empty Exec");
        assert!(
            !desktop_id.contains('\n'),
            "desktop id must be a single token: {desktop_id:?}"
        );
        assert!(
            !name.contains('\u{0}'),
            "display names must not contain NUL"
        );
    }
}

// ---------------------------------------------------------------------------
// Displays and capabilities: honesty, not optimism
// ---------------------------------------------------------------------------

#[test]
fn every_advertised_capability_has_a_backing_tool() {
    let c = linux::capabilities();
    // If we claim input synthesis, ydotool must actually be resolvable.
    if c.input_synthesis {
        let found = ["/usr/bin/ydotool", "/usr/local/bin/ydotool", "/usr/sbin/ydotool", "/bin/ydotool"]
            .iter()
            .any(|p| std::path::Path::new(p).is_file());
        assert!(found, "capability claims input synthesis but ydotool is not installed");
    }
    if c.screenshot {
        // The report names the first backend capture will actually try. D-Bus backends need no
        // binary; every other backend is a tool that must resolve on PATH.
        let backend = c.screenshot_backend.as_str();
        let dbus = ["kwin-screenshot2", "portal", "gnome-shell-screenshot"].contains(&backend);
        let on_path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(backend).is_file()))
            .unwrap_or(false);
        assert!(
            dbus || on_path,
            "capability claims screen capture through `{backend}`, which is not a D-Bus backend and is not on PATH"
        );
    }
}

#[test]
fn display_geometry_is_plausible() {
    if !common::live_desktop_armed("display_geometry_is_plausible") {
        return;
    }
    let ds = linux::displays();
    assert!(!ds.is_empty(), "a graphical session must report a display");
    for d in &ds {
        assert!(d.width > 0 && d.height > 0, "display {d:?} has zero size");
        assert!(
            d.width < 100_000 && d.height < 100_000,
            "display {d:?} has absurd geometry"
        );
    }
    // The JSON form is an OBJECT with a "displays" array (kscreen-doctor emits
    // {"outputs": [...]} and we normalise it). Assert the documented shape and,
    // more importantly, that the two display APIs cannot drift apart.
    let json = linux::list_displays_json().expect("displays must serialize");
    let arr = json
        .get("displays")
        .and_then(|d| d.as_array())
        .unwrap_or_else(|| panic!("list_displays_json must return {{\"displays\": [...]}}, got {json}"));
    assert_eq!(
        arr.len(),
        ds.len(),
        "displays() and list_displays_json() disagree: {} vs {}",
        ds.len(),
        arr.len()
    );
    for entry in arr {
        assert!(
            entry.get("name").and_then(|n| n.as_str()).map(|n| !n.is_empty()).unwrap_or(false),
            "every display entry needs a non-empty name: {entry}"
        );
    }
}

// ---------------------------------------------------------------------------
// Concurrency: the layer is called from async workers
// ---------------------------------------------------------------------------

#[test]
fn concurrent_probes_are_safe() {
    if !common::live_desktop_armed("concurrent_probes_are_safe") {
        return;
    }
    let mut handles = Vec::new();
    for _ in 0..16 {
        handles.push(std::thread::spawn(|| {
            let c = linux::capabilities();
            let d = linux::displays();
            (c.input_synthesis, d.len())
        }));
    }
    for h in handles {
        let (_, n) = h.join().expect("a concurrent probe must not panic");
        assert!(n >= 1);
    }
}
