// Shared helpers for the input-touching test binaries.
//
// The recurring failure mode in this suite is not broken code: synthetic input
// goes to whatever window has focus, and a test earlier in the run (a mouse
// click, another binary's click) moves that focus somewhere useless. The result
// reads as "the input layer is broken" when delivery was perfect.
//
// So the tests focus their own target instead of trusting whatever the operator
// left behind. This is KDE-specific and entirely best-effort: on any other
// desktop it prints a warning and the caller carries on.

#![allow(dead_code)]

use std::process::Command;

/// Tests that need a live graphical session (a display, a capture path, a clipboard tool) are
/// opt-in, like the input tests. Without this, `cargo test --workspace` fails on every headless
/// box (CI, SSH, containers) for reasons that are about the machine, not the code.
///
/// Run them on a desktop, or under Xvfb:
///   QUIXI_LIVE_DESKTOP_TEST=1 cargo test -p quixi-platform -- --test-threads=1
pub fn live_desktop_armed(test: &str) -> bool {
    let armed = matches!(
        std::env::var("QUIXI_LIVE_DESKTOP_TEST").ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    );
    if !armed {
        eprintln!("{test}: skipped; set QUIXI_LIVE_DESKTOP_TEST=1 on a live desktop session to run it");
    }
    armed
}

/// Activate the first Konsole window, if we can. Returns true when a focus
/// attempt was made and reported success.
pub fn focus_terminal() -> bool {
    // Only do this when the operator has armed input tests at all.
    if !matches!(
        std::env::var("QUIXI_INPUT_TEST").ok().as_deref(),
        Some("1") | Some("true") | Some("yes")
    ) {
        return false;
    }
    if Command::new("qdbus6").arg("--help").output().is_err() {
        eprintln!("focus_terminal: qdbus6 not available, relying on the operator's focus");
        return false;
    }

    // No `return` inside the loop: a top-level `return` in a KWin script is a
    // syntax error and the whole script silently never runs.
    let script = r#"
var ws = workspace.windowList ? workspace.windowList() : [];
for (var i = 0; i < ws.length; i++) {
  var w = ws[i];
  if (!w) continue;
  var cap = String(w.caption).toLowerCase();
  if (cap.indexOf("konsole") === -1 && cap.indexOf("terminal") === -1) continue;
  try { w.activate(); print("FOCUSED " + w.caption); } catch (e) { print("ERR " + e); }
}
"#;
    let path = format!("/tmp/quixi-focus-{}.js", std::process::id());
    if std::fs::write(&path, script).is_err() {
        return false;
    }
    let loaded = Command::new("qdbus6")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
            &path,
            "quixi-focus",
        ])
        .output();
    let started = Command::new("qdbus6")
        .args(["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"])
        .output();
    let _ = std::fs::remove_file(&path);
    let ok = matches!(loaded, Ok(ref o) if o.status.success())
        && matches!(started, Ok(ref o) if o.status.success());
    if ok {
        std::thread::sleep(std::time::Duration::from_millis(400));
    } else {
        eprintln!("focus_terminal: KWin scripting unavailable, relying on the operator's focus");
    }
    ok
}
