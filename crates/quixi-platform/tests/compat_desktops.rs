// Desktop-environment compatibility, proven without those desktops installed.
//
// A GNOME or Sway user does not have KDE's ScreenShot2, and may not have the tool
// this chain reaches for first. What actually breaks them is *selection*: picking a
// backend that cannot work, or giving up on the first failure while another backend
// sits there unused. So each test builds a PATH containing exactly the tools that
// desktop ships and checks the outcome.
//
// Run with --test-threads=1: these mutate PATH and the process environment.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Every test here rewrites PATH and the session variables, which are process-wide. libtest runs
/// tests in parallel by default (and `cargo test --workspace` gives no way to pass
/// `--test-threads=1` to just this binary), so each test holds this lock for its whole body.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    // A panicking test poisons the lock; the next test still owns the environment exclusively.
    ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A stand-in executable that behaves like the real tool for our purposes: it either
/// writes a valid PNG to the path it was given, or fails the way the real one does.
fn make_tool(dir: &Path, name: &str, writes_png: bool) {
    let body = if writes_png {
        format!(
            "#!/bin/sh\n# pretend {name}: last argument is the output path\nfor a in \"$@\"; do out=\"$a\"; done\n\
             printf '\\211PNG\\r\\n\\032\\n' > \"$out\"\n\
             dd if=/dev/zero bs=1 count=64 >> \"$out\" 2>/dev/null\nexit 0\n"
        )
    } else {
        format!("#!/bin/sh\necho \"{name}: cannot open display\" >&2\nexit 1\n")
    };
    let p = dir.join(name);
    std::fs::write(&p, body).expect("write tool");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Point PATH at exactly `dir` and make sure KDE's ScreenShot2 is unreachable, so the
/// fallback chain is what runs.
fn isolate(dir: &Path, desktop: &str) {
    std::env::set_var("PATH", dir);
    // Backend selection is session-aware now, so a test MUST say which desktop it is
    // simulating. Inheriting the runner's XDG_CURRENT_DESKTOP made every scenario look
    // like KDE and produced five false failures.
    std::env::set_var("XDG_CURRENT_DESKTOP", desktop);
    std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
    std::env::remove_var("XDG_RUNTIME_DIR");

    // The session TYPE has to be simulated too.
    //
    // This used to inherit whatever the runner had, so a simulated Sway session on a Wayland machine
    // looked like Wayland (grim was tried, test passed) while the same test under Xvfb looked like X11
    // (grim never tried, test failed). A test that only passes on the author's desktop is not a test.
    let wayland = matches!(desktop, "GNOME" | "Sway" | "sway" | "KDE");
    if wayland {
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
    } else {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    // DISPLAY is deliberately left alone: it points at whatever server is actually running (an Xvfb
    // instance under CI is not :0), and overwriting it breaks the tests that do real X11 capture.
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("quixi-de-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

#[test]
fn gnome_session_captures_with_gnome_screenshot() {
    let _env = env_lock();
    let d = scratch("gnome");
    make_tool(&d, "gnome-screenshot", true);
    isolate(&d, "GNOME");
    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&d);
    r.expect("a GNOME session must capture via gnome-screenshot");
    assert!(bytes.starts_with(b"\x89PNG"), "must be a PNG");
}

#[test]
fn wlroots_session_captures_with_grim() {
    let _env = env_lock();
    let d = scratch("sway");
    make_tool(&d, "grim", true);
    isolate(&d, "Sway");
    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&d);
    r.expect("a wlroots session must capture via grim");
    assert!(bytes.starts_with(b"\x89PNG"), "must be a PNG");
}

/// Cinnamon ships neither spectacle nor grim — it has its own screenshot tool. It is an
/// X11 desktop (Muffin, a Mutter fork), so this is the same class as XFCE/i3, and the
/// backend just has to be recognised by name.
#[test]
fn cinnamon_session_captures_with_cinnamon_screenshot() {
    let _env = env_lock();
    let d = scratch("cinnamon");
    make_tool(&d, "cinnamon-screenshot", true);
    isolate(&d, "Cinnamon");
    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&d);
    r.expect("a Cinnamon session must capture via cinnamon-screenshot");
    assert!(bytes.starts_with(b"\x89PNG"), "must be a PNG");
}

/// XFCE is X11 like Cinnamon, but it ships `xfce4-screenshooter` and a bare XFCE has
/// neither imagemagick nor gnome-screenshot. Without its own backend the desktop we claim
/// to support cannot capture at all.
#[test]
fn xfce_session_captures_with_xfce4_screenshooter() {
    let _env = env_lock();
    let d = scratch("xfce");
    make_tool(&d, "xfce4-screenshooter", true);
    isolate(&d, "XFCE");
    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&d);
    r.expect("an XFCE session must capture via xfce4-screenshooter");
    assert!(bytes.starts_with(b"\x89PNG"), "must be a PNG");
}

/// The regression this file exists for: ONE failing backend must not abort the chain.
///
/// The original bug was a `bail!` after spectacle failed, so a machine with spectacle
/// installed got a total capture failure while working backends sat unused. Scoping changed
/// WHICH backend is next — inside a KDE session the fallback is `import`, not another
/// desktop's tool — but the contract being tested is unchanged: keep going.
#[test]
fn a_failing_backend_falls_through_to_the_next_one() {
    let _env = env_lock();
    let d = scratch("fallthru");
    make_tool(&d, "spectacle", false); // present, always fails
    make_tool(&d, "import", true); // the same session's next option, works
    isolate(&d, "KDE");
    // Pin this scenario to X11 KDE. `import` is ImageMagick — an X11 tool — so it is only in the chain
    // for an X11 session. Left to inherit the session type it passed on a Wayland box and failed
    // elsewhere, because on Wayland KDE the next option after a failing spectacle is not `import`.
    std::env::remove_var("WAYLAND_DISPLAY");
    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&d);
    r.expect("a failing spectacle must fall through to the next backend, not give up");
    assert!(bytes.starts_with(b"\x89PNG"), "must be a PNG");
}

#[test]
fn a_bare_session_reports_every_backend_it_tried() {
    let _env = env_lock();
    let d = scratch("bare");
    isolate(&d, "");
    // Unknown session: every backend is a candidate, so this exercises the full chain.
    std::env::remove_var("XDG_CURRENT_DESKTOP");
    // "Bare" has to mean bare: no desktop, no tools AND no display.
    //
    // The premise used to be "no capture tool means no screenshot", which stopped being true once
    // X11 capture moved in-process via x11rb — with a display present it succeeds and needs no tool at
    // all, so the test failed on any machine that had one. Remove the display too, then put it back,
    // because these tests share one process and this one sorts first.
    let saved_display = std::env::var("DISPLAY").ok();
    let saved_wl = std::env::var("WAYLAND_DISPLAY").ok();
    std::env::remove_var("DISPLAY");
    std::env::remove_var("WAYLAND_DISPLAY");

    let out = d.join("shot.png");
    let r = quixi_platform::linux::capture_fallback_to(&out, None);

    if let Some(v) = saved_display {
        std::env::set_var("DISPLAY", v);
    }
    if let Some(v) = saved_wl {
        std::env::set_var("WAYLAND_DISPLAY", v);
    }

    let _ = std::fs::remove_dir_all(&d);
    let err = format!("{}", r.expect_err("with no display and no tools there is no capture path"));
    // `which` falls back to the standard bin directories, so a tool that is installed
    // on the host is still attempted here — what matters is that the failure names
    // what was tried instead of returning a bare "capture failed".
    // Any of these may be the one this host actually has. What the contract requires is that the
    // error names what was tried instead of a bare "capture failed" — naming only spectacle/grim
    // made this pass on a desktop and fail in a container, which is the same host assumption the
    // rest of this patch exists to remove.
    const TRIED: [&str; 9] = [
        "kwin-screenshot2", "portal", "gnome-shell-screenshot", "spectacle", "grim",
        "xfce4-screenshooter", "cinnamon-screenshot", "gnome-screenshot", "import",
    ];
    assert!(
        err.contains("capture") && TRIED.iter().any(|b| err.contains(b)),
        "the error should name the backends it tried; got: {err}"
    );
}

#[test]
fn capabilities_report_no_capture_when_no_backend_exists() {
    let _env = env_lock();
    let d = scratch("caps");
    isolate(&d, "");
    std::env::remove_var("DISPLAY");
    std::env::remove_var("WAYLAND_DISPLAY");
    std::env::remove_var("XDG_SESSION_TYPE");
    let caps = quixi_platform::linux::capabilities();
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        !caps.screenshot && !caps.screenshot_region,
        "capabilities must not claim capture with no backend present: {caps:?}"
    );
    assert_eq!(caps.session_type, "none", "no DISPLAY and no WAYLAND_DISPLAY");
}
