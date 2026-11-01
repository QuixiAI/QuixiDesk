// Input-synthesis tests. These type and click into whatever window has focus,
// so they are NOT part of the default suite: they only run when
// QUIXI_INPUT_TEST=1 and the operator has already focused a safe target (a
// throwaway terminal). Run them on a machine where a stray keystroke costs
// nothing — never on the user's working desktop.
//
// Sequence:
//   1. open a terminal and focus it (KWin script or by hand)
//   2. QUIXI_INPUT_TEST=1 cargo test --test input_synthesis -- --test-threads=1
//
// ORDER MATTERS and the names encode it. libtest runs single-threaded tests in
// ALPHABETICAL order, not definition order, so the typing test is named `t1_`
// and the mouse test `t4_`. The mouse test CLICKS, and a click focuses whatever
// window is under the pointer -- on an unattended box that was System Settings,
// so the typing test that followed typed a shell command into the System
// Settings search field and looked like a total failure of the input layer when
// nothing was actually broken. Anything that moves the pointer must run LAST.

#![cfg(target_os = "linux")]

use quixi_platform::linux;
use std::path::Path;
use std::time::Duration;

fn armed() -> bool {
    match std::env::var("QUIXI_INPUT_TEST").ok().as_deref() {
        Some("1") | Some("true") | Some("yes") => true,
        _ => {
            eprintln!("skipped: set QUIXI_INPUT_TEST=1 and focus a scratch terminal first");
            false
        }
    }
}

/// Proves type_text + press_key reach a real application: the typed command
/// creates a file, and the file existing means a shell actually received it.
#[test]
fn t1_typing_reaches_the_focused_application() {
    if !armed() {
        return;
    }
    let proof = "/tmp/quixi_input_proof";
    let _ = std::fs::remove_file(proof);

    linux::type_text(&format!("touch {proof}")).expect("type_text must not error");
    linux::press_key("return", 1).expect("press_key must not error");

    // Give the shell time to run the command.
    for _ in 0..40 {
        if Path::new(proof).exists() {
            println!("input reached the application: {proof} created");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("typed command never executed — keystrokes did not reach the focused window");
}

/// Proves the mouse can be moved to an absolute position and clicked without
/// erroring. It does not assert on cursor position: Wayland gives clients no
/// way to read it back, so that would be a test that cannot observe its subject.
#[test]
fn t4_mouse_move_and_click_report_success() {
    if !armed() {
        return;
    }
    linux::mouse_move(400, 300).expect("mouse_move must not error");
    std::thread::sleep(Duration::from_millis(100));
    linux::mouse_click("left", 1).expect("mouse_click must not error");
    linux::mouse_drag(400, 300, 600, 400, "left").expect("mouse_drag must not error");
}

/// Combos go through the module's own keycode builder, so this exercises the
/// real mapping path rather than a shelled-out shortcut.
#[test]
fn t2_modifier_combo_is_accepted() {
    if !armed() {
        return;
    }
    // ctrl+a: select-all in a terminal. Harmless, and exercises a modifier pair.
    linux::press_key("ctrl+a", 1).expect("ctrl+a must map and send");
}

/// A combo the module cannot map must error rather than silently doing nothing.
#[test]
fn t3_unmappable_combo_is_an_error() {
    let err = linux::press_key("hyper+meta+qzx", 1).unwrap_err();
    println!("unmappable combo rejected as expected: {err}");
}
