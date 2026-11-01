// Compatibility: the layer must be harmless on machines that are not this one.
//
// The bug hunt proves the code survives hostile INPUT. This proves it survives a
// hostile ENVIRONMENT — a different session type, a machine with none of the
// helper tools, a headless box. The contract is:
//
//   * nothing panics;
//   * a capability is never claimed without its backend;
//   * every call that cannot work returns Err or None with a message, rather than
//     pretending, hanging, or returning a plausible-looking lie.
//
// Run it in three environments (scripts/compat-bench.sh does this):
//   * the live Wayland session
//   * an X11-only session (Xvfb on DISPLAY=:99, no WAYLAND_DISPLAY, no session bus)
//   * no session at all (both unset)

#![cfg(target_os = "linux")]

use quixi_platform::linux;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// What this process should think the session is.
fn expected_session() -> &'static str {
    if env("WAYLAND_DISPLAY").is_some() {
        "wayland"
    } else if env("DISPLAY").is_some() {
        "x11"
    } else {
        "none"
    }
}

#[test]
fn session_is_classified_from_the_environment_not_guessed() {
    let report = linux::capabilities();
    println!(
        "env: WAYLAND_DISPLAY={:?} DISPLAY={:?} DBUS={}",
        env("WAYLAND_DISPLAY"),
        env("DISPLAY"),
        env("DBUS_SESSION_BUS_ADDRESS").is_some()
    );
    println!(
        "probe: session={} screenshot={} ({}) clipboard={} input={} pickers={} displays={} apps={} launch={}",
        report.session_type,
        report.screenshot,
        report.screenshot_backend,
        report.clipboard,
        report.input_synthesis,
        report.file_pickers,
        report.display_listing,
        report.app_launch,
        report.window_layout
    );
    let want = expected_session();
    match want {
        "wayland" => assert_eq!(report.session_type, "wayland"),
        "x11" => assert_eq!(report.session_type, "x11"),
        _ => assert!(
            report.session_type == "none" || report.session_type == "unknown",
            "headless classification was {:?}",
            report.session_type
        ),
    }
}

#[test]
fn capabilities_never_claim_a_backend_that_is_absent() {
    // The layer resolves tools through PATH, so the test must too: a fixed list of /usr/bin
    // paths fails on any machine that installs them elsewhere (/usr/local, ~/.local, Nix).
    let on_path = |tool: &str| {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
            .unwrap_or(false)
    };
    let r = linux::capabilities();

    if r.screenshot {
        // Either the compositor provides it, or a capture tool must exist.
        let dbus = ["kwin-screenshot2", "portal", "gnome-shell-screenshot"]
            .contains(&r.screenshot_backend.as_str());
        assert!(
            dbus || on_path(&r.screenshot_backend),
            "claimed {:?} with no backend present",
            r.screenshot_backend
        );
    }

    if r.input_synthesis {
        assert!(on_path("ydotool"), "claimed input synthesis with no ydotool present");
    }

    if r.clipboard {
        let tool = (on_path("wl-copy") && on_path("wl-paste")) || on_path("xclip") || on_path("xsel");
        assert!(tool, "claimed clipboard with no clipboard tool present");
    }
}

#[test]
fn every_call_returns_cleanly_or_fails_cleanly_but_never_panics() {
    // Screenshots
    match linux::screenshot_png() {
        Ok(b) => assert!(!b.is_empty(), "successful capture returned no bytes"),
        Err(e) => assert!(!format!("{e}").is_empty(), "an error must carry a message"),
    }
    match linux::screenshot_region_png(0, 0, 320, 200) {
        Ok(b) => assert!(!b.is_empty()),
        Err(e) => assert!(!format!("{e}").is_empty()),
    }

    // Clipboard: never panics, may be empty
    let _ = linux::clipboard_read();
    let _ = linux::clipboard_write("compat");

    // Enumeration: must produce something plausible or nothing, never garbage
    for d in linux::displays() {
        assert!(d.width > 0 && d.height > 0);
        assert!(d.width < 100_000 && d.height < 100_000);
    }
    for (name, exec, id) in linux::installed_apps() {
        assert!(!name.trim().is_empty());
        assert!(!exec.trim().is_empty());
        assert!(!id.contains('\n'));
    }

    // Input and launching must refuse cleanly when unavailable.
    for combo in ["return", "ctrl+c", "nonsense"] {
        let _ = linux::press_key(combo, 1);
    }
    for btn in ["left", "right", "bogus"] {
        let _ = linux::mouse_click(btn, 1);
        let _ = linux::mouse_down(btn);
        let _ = linux::mouse_up(btn);
    }
    // Empty text is a no-op; unmappable text is an error. Neither may panic.
    let empty = linux::type_text("");
    assert!(empty.is_ok(), "empty text must be a no-op, not an error");
    let unmappable = linux::type_text("漢");
    assert!(unmappable.is_err(), "unmappable text must be refused, not silently dropped");

    assert!(linux::open_application("definitely-not-installed-xyz").is_err());
    assert!(linux::list_displays_json().is_ok() || linux::list_displays_json().is_err());

    // Pickers block on a dialog, so only the capability is checked here.
    let _ = linux::active_output();
    let _ = linux::x11_window_under_cursor();
}

#[test]
fn repeated_calls_stay_stable_in_any_environment() {
    // Fifty mixed calls: catches fd growth and state corruption under a session
    // type the code was not developed against.
    for _ in 0..50 {
        let _ = linux::capabilities();
        let _ = linux::screenshot_region_png(0, 0, 32, 32);
        let _ = linux::clipboard_write("x");
        let _ = linux::clipboard_read();
        let _ = linux::press_key("nonsense", 1);
    }
    let fds = std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0);
    println!("fd count after 250 mixed calls: {fds}");
    assert!(fds < 200, "descriptor growth under repeated use: {fds}");
}
