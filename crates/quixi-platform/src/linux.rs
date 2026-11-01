//! Linux (Wayland-first, X11-compatible) implementations for the platform
//! surfaces that upstream leaves as silent no-ops outside macOS.
//!
//! Design notes:
//!  * Capture goes through KWin's ScreenShot2 D-Bus API first (see
//!    `kwin_capture_png`) and only falls back to spectacle/grim/import when the
//!    compositor declines. Measured on Plasma 6.8: a full desktop frame costs
//!    29 ms through ScreenShot2 against 921 ms for a `spectacle -b -f` run, and a
//!    256x256 crop costs 2.9 ms instead of capturing everything and cropping it.
//!    `zbus` is already resolved in the workspace lock, so this costs no new
//!    system dependency; `png` is the only new crate and it is pure Rust.
//!  * Everything else shells out to the tools the desktop already ships, and
//!    every entry point degrades honestly: `Err` with a message naming the
//!    missing tool instead of pretending the action happened.
//!  * `capabilities()` exists so the UI can tell the user what this machine can
//!    actually do, rather than offering buttons that quietly do nothing.
//!
//! What is NOT here, deliberately: window enumeration and window geometry.
//! Wayland has no client-facing window list, the XDG portal does not expose one
//! (by design), and `org.kde.KWin.queryWindowInfo` blocks waiting for a click.
//! `getWindowLayout` therefore reports unsupported on Wayland unless the target
//! is an XWayland client reachable through `xdotool`, which the caller may use
//! via `x11_window_under_cursor()`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};

fn have(bin: &str) -> bool {
    which(bin).is_some()
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Common sbin locations are not always on a desktop session's PATH.
    for dir in ["/usr/bin", "/usr/local/bin", "/usr/sbin", "/bin"] {
        let candidate = Path::new(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// How long an external tool may take before we stop waiting for it.
///
/// These are GUI programs. Several of them open a dialog when they are not being given
/// explicit arguments (a region picker, a save prompt), and `Command::output()` has no
/// timeout: measured on a real box, an XFCE screenshot tool started inside a GNOME session
/// sat forever and the whole caller hung with it. A capture that fails is recoverable; a
/// capture that never returns is not.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Deadline for a dialog tool. Unlike the 20 s tool timeout this one waits on a person to
/// choose something, so it has to be generous — but it still has to exist, because a dialog
/// that never appears (wrong desktop, no display, a modal that failed to map) would otherwise
/// hang the caller forever. `QUIXI_DIALOG_TIMEOUT_SECS` overrides it for tests.
fn dialog_timeout() -> std::time::Duration {
    std::env::var("QUIXI_DIALOG_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or_else(|| std::time::Duration::from_secs(300))
}

fn run(bin: &str, args: &[&str]) -> Result<std::process::Output> {
    run_with_env(bin, args, &[], TOOL_TIMEOUT)
}

fn run_with_env(
    bin: &str,
    args: &[&str],
    envs: &[(&str, &std::path::PathBuf)],
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    // Resolve through `which` and execute the ABSOLUTE path.
    //
    // `which` deliberately falls back to the standard bin directories, because a
    // desktop-launched session often has a minimal PATH. Executing by bare name then
    // disagreed with that: detection would report a tool available and the spawn
    // would fail with NotFound. Resolving once and running the path we found makes
    // detection and execution the same answer.
    let exe = which(bin).ok_or_else(|| anyhow!("{bin} is not installed"))?;

    let mut cmd = Command::new(&exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to execute {} ({})", bin, exe.display()))?;

    // Drain the pipes CONCURRENTLY with waiting.
    //
    // `Command::output()` does this for you. Replacing it with `try_wait()` while leaving
    // the pipes unread is a deadlock: once the child writes past the pipe buffer (~64 KB)
    // it blocks in write(), the child never exits, and the timeout kills a perfectly
    // healthy process. Measured on a real box — a 1 MB clipboard read came back truncated
    // and the suite took 30 s instead of 9 s. One reader thread per stream fixes it.
    use std::io::Read;

    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!(
                        "{bin} did not finish within {}s and was killed \
                         (a dialog may have opened — it needs explicit arguments)",
                        timeout.as_secs()
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => return Err(e).with_context(|| format!("waiting on {bin}")),
        }
    };

    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}


/// Run a dialog tool with the dialog deadline rather than the tool deadline.
fn run_dialog(bin: &str, args: &[&str]) -> Result<std::process::Output> {
    run_with_env(bin, args, &[], dialog_timeout())
}

/// Run a picker tool and hand back its stdout, or `None` if it cancelled or failed.
///
/// `kdialog` and `zenity` are both GOption-style parsers, so `--title <text>` as two separate
/// arguments is accepted by each — one shape covers both and there is no `--title=` guessing.
fn run_one(bin: &str, args: &[&str], title: Option<&str>) -> Option<String> {
    let mut all: Vec<&str> = args.to_vec();
    let title_value;
    if let Some(t) = title {
        all.push("--title");
        title_value = t.to_string();
        all.push(&title_value);
    }
    let out = run_dialog(bin, &all).ok()?;
    if !out.status.success() {
        return None; // cancelled, or the tool refused
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn run_ok(bin: &str, args: &[&str]) -> Result<String> {
    let out = run(bin, args)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("{bin} exited {}: {err}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn is_wayland() -> bool {
    // An EMPTY WAYLAND_DISPLAY is not Wayland.
    //
    // `var_os(..).is_some()` treated a set-but-empty variable as a Wayland session, and several
    // ordinary things produce exactly that: a systemd user environment carried over from a previous
    // session, `ssh` with `WAYLAND_DISPLAY=` in the command, some display managers on an X11 login.
    // The damage is out of proportion to the cause — `cursor_position`, `window_under_cursor` and
    // `active_output` all key off this one predicate, so an empty string silently reported "Wayland
    // has no client API" on a session where the X11 answers were available all along.
    // Measured: an XFCE session on :99 reported session=wayland and failed active_output purely
    // because the caller exported WAYLAND_DISPLAY="".
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(v) => !v.trim().is_empty(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Capability probe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct CapabilityReport {
    pub session_type: String,
    pub screenshot: bool,
    pub screenshot_backend: String,
    pub screenshot_region: bool,
    pub clipboard: bool,
    pub input_synthesis: bool,
    pub file_pickers: bool,
    pub display_listing: bool,
    pub app_launch: bool,
    pub window_layout: bool,
    pub notes: Vec<String>,
}

pub fn capabilities() -> CapabilityReport {
    let mut notes = Vec::new();

    // ScreenShot2 first: it needs no external tool, only the compositor, and it is
    // the cheap path (measured 29.2 ms for the whole desktop, 22.6 ms at 1920x1080 and
    // 2.9 ms for a 256x256 crop, against ~921 ms for `spectacle -b -f`). Gating
    // this on `have("spectacle")` — as an earlier version did — under-reports a
    // capability that works perfectly on a machine with no capture tools installed
    // at all. The external tools are the FALLBACK, not the requirement.
    // Same source of truth as capture itself: the first backend this session would
    // actually try. A hardcoded chain here let the report name a tool that capture would
    // never reach (measured: reported "spectacle" inside an XFCE session whose own tool
    // was preferred).
    let order = capture_backend_order();
    let (screenshot_backend, screenshot) = match order.first() {
        Some(b) => (b.clone(), true),
        None => ("none".to_string(), false),
    };

    // A headless box can still have spectacle installed — `which` looks in the
    // standard bin directories even when PATH is minimal. Claiming capture there is
    // the same lie as reporting session_type "x11" with no DISPLAY: the tool exists
    // and cannot possibly work.
    let session_has_display =
        is_wayland() || std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
    // If we are not going to capture, do not name a backend we are declining to use:
    // the report has to stay self-consistent (screenshot == false implies backend
    // "none"), which is the invariant the in-file test asserts.
    let (screenshot, screenshot_backend) = if session_has_display {
        (screenshot, screenshot_backend)
    } else {
        (false, "none".to_string())
    };

    if !screenshot {
        notes.push(
            if session_has_display {
                "No screen capture path found. This session has no KWin ScreenShot2 and no \
                 spectacle/grim/gnome-screenshot/import."
                    .to_string()
            } else {
                "No display session, so screen capture is unavailable regardless of which \
                 tools are installed."
                    .to_string()
            },
        );
    }

    let can_crop = have("magick") || have("convert");

    let clipboard = have("wl-copy") && have("wl-paste") || have("xclip") || have("xsel");
    if !clipboard {
        notes.push(
            if is_wayland() {
                "Install wl-clipboard for clipboard read/write."
            } else {
                "Install xclip or xsel for clipboard read/write."
            }
            .to_string(),
        );
    }

    // The daemon is what accepts events; a bare binary without a socket cannot type.
    let input = ydotool_ready();
    if !input {
        notes.push(
            if is_wayland() {
                "Install ydotool and start ydotoold for cursor and keyboard synthesis. \
                 Without it, type_text / press_key / mouse_* are unavailable on Wayland — \
                 xdotool only reaches XWayland clients."
            } else {
                "Install ydotool and start ydotoold for cursor and keyboard synthesis; \
                 type_text / press_key / mouse_* are unavailable without it."
            }
            .to_string(),
        );
    }

    let pickers = have("kdialog") || have("zenity");
    if !pickers {
        notes.push("Install kdialog or zenity for native file and folder pickers.".to_string());
    }

    let displays = have("kscreen-doctor")
        || have("wlr-randr")
        || have("xrandr")
        || (desktop_is("GNOME") && gnome_display_state().is_ok());
    let apps = have("gtk-launch") || have("gio");
    let window_layout = !is_wayland() && have("xdotool");
    if is_wayland() {
        // Wayland gives clients no window list and the XDG portal exposes none.
        // On KWin specifically the compositor's scripting API CAN enumerate
        // windows (workspace.windowList() returns caption and frameGeometry for
        // every window), but reading that back into this process needs a channel
        // the script can write to. Not implemented yet, so report it as absent
        // rather than pretending it works.
        notes.push(
            "Window enumeration and geometry are not implemented. Wayland exposes no \
             client-facing window list; on KWin the scripting API can provide one, but \
             it is not wired up here."
                .to_string(),
        );
    }

    CapabilityReport {
        // Three-way, not a two-way guess. The previous form was
        // `if is_wayland() { "wayland" } else { "x11" }`, which reported "x11" on a
        // machine with no display at all — an SSH shell, a container, a server —
        // and the UI then offered capture and input buttons that could not work.
        // Name the compositor when it is known, not just the protocol. "wayland" is true of KDE,
        // GNOME, Hyprland and everything else, so a bug report saying "wayland" does not say where
        // to look. Omarchy is called out separately because it is Hyprland with a different stack
        // around it — Quickshell rather than a plain bar — and the distinction matters when
        // something in that stack is what is misbehaving.
        session_type: if hyprland_running() {
            if omarchy_running() {
                "wayland (hyprland, omarchy)"
            } else {
                "wayland (hyprland)"
            }
        } else if is_wayland() {
            "wayland"
        } else if std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty()) {
            "x11"
        } else {
            "none"
        }
        .to_string(),
        screenshot,
        screenshot_backend,
        screenshot_region: screenshot && can_crop,
        clipboard,
        input_synthesis: input,
        file_pickers: pickers,
        display_listing: displays,
        app_launch: apps,
        window_layout,
        notes,
    }
}

// ---------------------------------------------------------------------------
// Screen capture
// ---------------------------------------------------------------------------

/// Capture to a file using only the external tools — no KWin ScreenShot2.
///
/// This is the path a GNOME, wlroots or X11 desktop takes. It is public so the
/// fallback chain can be exercised on any machine: on a KDE box the fast path always
/// answers, so a test that cannot reach this function would be testing nothing.
pub fn capture_fallback_to(path: &Path, region: Option<(i32, i32, i32, i32)>) -> Result<()> {
    capture_to(path, region)
}

fn capture_to(path: &Path, region: Option<(i32, i32, i32, i32)>) -> Result<()> {
    if let Some((x, y, w, h)) = region {
        // spectacle's --region is interactive; capture full screen and crop.
        let tmp = std::env::temp_dir().join(format!("quixi-full-{}.png", std::process::id()));
        capture_full_to(&tmp)?;
        let crop = format!("{w}x{h}+{x}+{y}");
        let tool = if have("magick") {
            "magick"
        } else if have("convert") {
            "convert"
        } else {
            let _ = std::fs::remove_file(&tmp);
            bail!("region capture needs ImageMagick (magick/convert) to crop");
        };
        run_ok(
            tool,
            &[
                &tmp.to_string_lossy(),
                "-crop",
                &crop,
                "+repage",
                &path.to_string_lossy(),
            ],
        )?;
        let _ = std::fs::remove_file(&tmp);
        return Ok(());
    }
    capture_full_to(path)
}

/// Live pointer position, or `None` when it cannot be read.
///
/// Reads through XWayland via `xdotool`. Accurate on an X11 session, and on
/// Wayland accurate only while the pointer sits over an XWayland surface: the
/// compositor exposes no cursor query, so a native Wayland window leaves X holding
/// the last value it saw. Measured on Plasma 6.8 with the pointer over a native
/// surface: X reported 1895,922 while the compositor's own cursorPos said 2087,45 —
/// 900+ pixels apart, identical on repeated reads. Callers that need a
/// guaranteed-fresh position on Wayland must treat this as a hint.
#[cfg(target_os = "linux")]
pub fn cursor_position() -> Option<(f64, f64)> {
    // On Wayland this returns None, and that is the correct answer rather than a limitation.
    //
    // There is no client API for the compositor's cursor position, but XWayland still answers
    // X queries — so asking it produced a *plausible wrong number*: measured at 1280,800,
    // exactly the centre of a 2560x1600 screen, before AND after the pointer was moved
    // elsewhere. A caller cannot tell that apart from a real reading, which is worse than no
    // reading at all. Measured on GNOME 51 / mutter.
    // Hyprland is the one Wayland compositor that publishes the global cursor position, via
    // `hyprctl -j cursorpos`. Ask it before falling through to the honest None below — this is not
    // the XWayland guess that the comment above warns about, it is the compositor's own reading.
    if hyprland_running() {
        if let Ok(v) = hyprctl_json("cursorpos") {
            if let (Some(x), Some(y)) = (
                v.get("x").and_then(|n| n.as_f64()),
                v.get("y").and_then(|n| n.as_f64()),
            ) {
                return Some((x, y));
            }
        }
    }

    if is_wayland() {
        return None;
    }

    // X11 in-process: one protocol round trip instead of spawning xdotool and parsing text.
    if let Some((x, y, _)) = x11_pointer() {
        return Some((x as f64, y as f64));
    }
    if !have("xdotool") {
        return None;
    }
    let out = run("xdotool", &["getmouselocation"]).ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mut x = None;
    let mut y = None;
    for part in s.split_whitespace() {
        if let Some(v) = part.strip_prefix("x:") {
            x = v.parse::<f64>().ok();
        } else if let Some(v) = part.strip_prefix("y:") {
            y = v.parse::<f64>().ok();
        }
    }
    match (x, y) {
        (Some(px), Some(py)) => Some((px, py)),
        _ => None,
    }
}

/// Does this session have KWin's ScreenShot2 service?
///
/// Cheap on purpose: this runs inside the capability probe, which the UI calls to
/// decide which buttons to show, so it must not take a screenshot to answer. One
/// name-owner query on the session bus is enough.
#[cfg(target_os = "linux")]
/// Is the desktop we are running in actually KDE?
///
/// Being on the bus is not the same as owning this session. A user logged into a KDE
/// session on the same account exposes `org.kde.KWin` to every other session too — an
/// SSH shell, a nested compositor, a second X server. In that case KWin answers, captures
/// ITS screen, and hands back a screenshot of a display the caller is not looking at.
/// Measured on a real second box: a 1280x800 test display produced a 1920x1080 image.
///
/// `XDG_CURRENT_DESKTOP` is what the session itself says it is. Unset means we cannot
/// tell, and then the bus probe decides — the previous behaviour, kept so that a stripped
/// environment still works.
fn desktop_is(desktop: &str) -> bool {
    match std::env::var("XDG_CURRENT_DESKTOP") {
        Ok(v) if !v.is_empty() => v.to_uppercase().contains(desktop),
        _ => true,
    }
}

/// Whether a Hyprland compositor is the one we are talking to.
///
/// The signal is `HYPRLAND_INSTANCE_SIGNATURE`, which the compositor sets for every process it
/// starts and which nothing else can fake: a nested instance has its own signature, and a KDE or
/// GNOME session simply does not have the variable at all.
///
/// `XDG_CURRENT_DESKTOP` is deliberately *not* used for this. `desktop_is()` treats an empty value
/// as a match, which is right for a fallback order and wrong for a positive identification — on a
/// box with no session set it would claim Hyprland was running.
/// The running Hyprland instance's signature.
///
/// `HYPRLAND_INSTANCE_SIGNATURE` is exported to the processes the compositor *launches*, so a GUI app
/// started from the session has it — but an SSH shell, a cron job, or anything started from outside
/// the session does not. Measured on Omarchy 4.0.1 / Hyprland 0.56.2: the variable is absent from
/// Hyprland's own environment, so testing only that variable reports "no Hyprland" on a machine that
/// is running Hyprland at that moment.
///
/// The compositor also writes its signature as a directory under `$XDG_RUNTIME_DIR/hypr/`, which
/// anything running as the same user can read. Use the variable when present, fall back to the
/// directory — the same two sources, in the same order, that `hyprctl` itself uses.
fn hyprland_signature() -> Option<String> {
    if let Ok(v) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !v.is_empty() {
            return Some(v);
        }
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let dir = std::path::Path::new(&runtime).join("hypr");
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if !entry.file_type().map(|f| f.is_dir()).unwrap_or(false) {
            continue;
        }
        // A directory alone proves nothing — a compositor that exited leaves its runtime directory
        // behind. Measured: a dead instance keeps `hyprland.log` and `.socket2.sock`, while a live one
        // also has `.socket.sock` (the command socket hyprctl talks to) and `hyprland.lock`. Without
        // this check, two leftover directories from a nested test made a KDE session report itself as
        // Hyprland. Require the live socket.
        if !entry.path().join(".socket.sock").exists() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if newest.as_ref().map(|(best, _)| modified > *best).unwrap_or(true) {
            newest = Some((modified, entry.file_name().to_string_lossy().to_string()));
        }
    }
    newest.map(|(_, name)| name)
}

fn hyprland_running() -> bool {
    if !have("hyprctl") {
        return false;
    }
    // A signature from either source is authoritative — it names a live compositor socket.
    if hyprland_signature().is_some() {
        return true;
    }
    // Last resort, and a deliberately weak one: the session's own desktop name.
    //
    // NOT desktop_is(), on purpose: that helper returns true when XDG_CURRENT_DESKTOP is unset
    // ("empty means match anything"), which is the right default when choosing a fallback chain and
    // exactly wrong for a positive identification. In a test or cron environment with no session
    // variable, desktop_is("HYPRLAND") answers yes for every machine on earth.
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("HYPRLAND"))
        .unwrap_or(false)
}

/// Omarchy is Hyprland plus an opinionated stack on top (Quickshell for the shell, hyprtoolkit, the
/// `omarchy-*` packages). Nothing in the platform layer behaves differently for it — the point of
/// detecting it is to report it honestly instead of calling an Omarchy desktop "Hyprland", which
/// sends anyone debugging in the wrong direction.
fn omarchy_running() -> bool {
    have("omarchy") || std::path::Path::new("/etc/omarchy").exists()
}

/// `hyprctl -j <query>`, parsed.
///
/// Returned as a Result so callers fall through to the next backend on a compositor hiccup instead
/// of reporting "no displays" — an error here means "ask someone else", not "the answer is zero".
fn hyprctl_json(query: &str) -> Result<serde_json::Value> {
    // hyprctl needs the signature in its environment to find the compositor socket. Resolve it here
    // rather than inheriting whatever the caller happens to have, so this works identically from a
    // session-launched app, an SSH shell, and a cron job.
    let signature = hyprland_signature()
        .ok_or_else(|| anyhow::anyhow!("no Hyprland instance signature found for hyprctl -j {query}"))?;
    let out = std::process::Command::new("hyprctl")
        .env("HYPRLAND_INSTANCE_SIGNATURE", &signature)
        .args(["-j", query])
        .output()
        .with_context(|| format!("running hyprctl -j {query}"))?;
    if !out.status.success() {
        bail!(
            "hyprctl -j {query} exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout))
        .with_context(|| format!("hyprctl -j {query} returned unparseable JSON"))
}

fn kwin_screenshot2_available() -> bool {
    use zbus::blocking::Connection;
    // `!hyprland_running()` matters when Hyprland runs nested inside a KDE session: the process
    // inherits XDG_CURRENT_DESKTOP=KDE, so desktop_is("KDE") is true and KWin's API would capture
    // the HOST desktop rather than the compositor this process is actually a client of. The
    // instance signature cannot be inherited that way, so it wins.
    if hyprland_running() || !desktop_is("KDE") {
        return false;
    }
    let Ok(conn) = Connection::session() else {
        return false;
    };
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &("org.kde.KWin",),
    )
    .ok()
    .and_then(|m| m.body().deserialize::<bool>().ok())
    .unwrap_or(false)
}

/// How long to wait for a portal screenshot request to be answered.
///
/// The portal is the sanctioned route on every desktop, but on GNOME the FIRST request
/// raises a system dialog that a human has to approve. An unattended caller therefore has
/// to give up rather than wait forever — the same lesson as the external tools, which could
/// hang indefinitely on a dialog.
fn portal_timeout() -> std::time::Duration {
    let secs = std::env::var("QUIXI_PORTAL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(45);
    std::time::Duration::from_secs(secs)
}

/// Is the desktop portal on the bus?
///
/// This is the portable route: GNOME, KDE and wlroots all implement
/// `org.freedesktop.portal.Screenshot`, so one implementation speaks every desktop. The
/// compositor-specific interfaces are fast paths that not every compositor permits —
/// GNOME refuses third-party callers on `org.gnome.Shell.Screenshot` outright.
fn portal_available() -> bool {
    use zbus::blocking::Connection;
    let Ok(conn) = Connection::session() else {
        return false;
    };
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &("org.freedesktop.portal.Desktop",),
    )
    .ok()
    .and_then(|m| m.body().deserialize::<bool>().ok())
    .unwrap_or(false)
}

/// Capture through the XDG desktop portal, then read the PNG it wrote.
///
/// The portal answers asynchronously: `Screenshot` returns a request handle immediately and
/// the result arrives later as a `Response` signal on that handle. The signal is subscribed
/// before the call so the reply cannot be missed, and the wait happens on a worker thread so
/// it can be bounded.
/// Grant this application permission to screenshot, in the portal's own permission store.
///
/// On GNOME the FIRST `Screenshot` call raises a shell dialog that has to be approved before
/// the capture happens. An unattended tool can never answer it, so the request waits out its
/// timeout and the failure reads as "the portal is broken" when the truth is that a modal was
/// sitting there unapproved.
///
/// The dialog writes one record — per (table, permission id, application id) — into
/// `org.freedesktop.impl.portal.PermissionStore`. That record can be written directly, and the
/// result is identical to clicking yes: no dialog, no human, same grant.
///
/// Scope is deliberately one permission, screenshot, for this application only. It is not a
/// general "approve everything", and it is the app authorising itself for the one action the
/// user already asked it to perform.
fn portal_grant_screenshot_permission(app_id: &str) -> Result<()> {
    let conn = zbus::blocking::Connection::session()
        .context("no session bus for the permission store")?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.impl.portal.PermissionStore",
        "/org/freedesktop/impl/portal/PermissionStore",
        "org.freedesktop.impl.portal.PermissionStore",
    )
    .context("binding the portal PermissionStore")?;

    // SetPermission(table, create, id, app, permissions)
    let perms: Vec<&str> = vec!["yes"];
    // A void D-Bus method has no return to deserialize, so this goes through call_method and
    // keeps the reply as a Message.
    conn.call_method(
        Some("org.freedesktop.impl.portal.PermissionStore"),
        "/org/freedesktop/impl/portal/PermissionStore",
        Some("org.freedesktop.impl.portal.PermissionStore"),
        "SetPermission",
        &("screenshot", true, "screenshot", app_id, perms),
    )
    .context("writing the screenshot permission")?;

    // Read it back: a store that silently drops the write is worse than no store at all.
    let got: Vec<String> = proxy
        .call("GetPermission", &("screenshot", "screenshot", app_id))
        .context("reading back the screenshot permission")?;
    if !got.iter().any(|v| v == "yes") {
        bail!("the permission store accepted the write but reports {got:?}");
    }
    Ok(())
}

/// Make the portal capture path non-interactive before it is used.
///
/// Called once, lazily, the first time a capture goes through the portal. Best-effort by
/// design: if the store refuses (an older portal, a different backend), the capture still runs
/// and the user simply sees the dialog they would have seen anyway.
fn portal_preauthorise_screenshot() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // The portal derives the app id from our .desktop file; an unsandboxed binary with no
        // matching entry is filed under the empty id, so grant both and let the store keep
        // whichever one this build actually gets identified as.
        for id in ["quixidesk", ""] {
            let _ = portal_grant_screenshot_permission(id);
        }
    });
}

fn portal_capture_png() -> Result<Vec<u8>> {
    use std::collections::HashMap;

    // Remove the approval dialog before it can appear. Without this the first capture on
    // GNOME blocks on a modal nobody is there to click.
    portal_preauthorise_screenshot();

    let conn = zbus::blocking::Connection::session()
        .context("no session bus for the desktop portal")?;
    let conn_rx = conn.clone();

    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .context("binding org.freedesktop.portal.Screenshot")?;

    let mut options: HashMap<String, zbus::zvariant::Value> = HashMap::new();
    // Non-interactive: do not pop the portal's own picker UI. The permission dialog that
    // GNOME shows is the shell's, not ours, and cannot be suppressed from here.
    options.insert("interactive".into(), zbus::zvariant::Value::from(false));
    options.insert("modal".into(), zbus::zvariant::Value::from(true));

    let handle: zbus::zvariant::OwnedObjectPath = proxy
        .call("Screenshot", &("", options))
        .context("portal refused the Screenshot request")?;

    // Wait for the Response signal on that handle, on a worker thread so we can time out.
    let (tx, rx) = std::sync::mpsc::channel::<(u32, String)>();
    let handle_path = handle.to_string();
    std::thread::spawn(move || {
        let Ok(req) = zbus::blocking::Proxy::new(
            &conn_rx,
            "org.freedesktop.portal.Desktop",
            handle_path.as_str(),
            "org.freedesktop.portal.Request",
        ) else {
            return;
        };
        let Ok(mut signals) = req.receive_signal("Response") else {
            return;
        };
        for msg in signals.by_ref() {
            let Ok(body) = msg.body().deserialize::<(u32, HashMap<String, zbus::zvariant::OwnedValue>)>()
            else {
                continue;
            };
            let uri = body
                .1
                .get("uri")
                .and_then(|v| v.downcast_ref::<&str>().ok().map(|s| s.to_string()))
                .unwrap_or_default();
            let _ = tx.send((body.0, uri));
            return;
        }
    });

    let (response, uri) = rx
        .recv_timeout(portal_timeout())
        .map_err(|_| {
            anyhow!(
                "portal did not answer within {}s (on GNOME the first request needs a \
                 human to approve the system dialog; QUIXI_PORTAL_TIMEOUT_SECS raises this)",
                portal_timeout().as_secs()
            )
        })?;

    match response {
        0 => {}
        1 => bail!("the screenshot request was cancelled at the portal"),
        2 => bail!("the portal reported an unspecified failure"),
        other => bail!("portal returned failure code {other}"),
    }

    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| anyhow!("portal returned a non-file URI: {uri:?}"))?;
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading the portal screenshot at {path}"))?;
    if !bytes.starts_with(b"\x89PNG") {
        bail!("portal screenshot was not a PNG ({} bytes)", bytes.len());
    }
    Ok(bytes)
}

/// Is GNOME's screenshot interface on the bus?
///
/// GNOME does not implement `wlr-screencopy`, so `grim` cannot work there, and it does not
/// have KWin's ScreenShot2 either. It has its own interface on `org.gnome.Shell`, which is
/// callable over the same session bus we already hold — so GNOME capture costs no process
/// spawn and no extra dependency.
fn gnome_screenshot_available() -> bool {
    use zbus::blocking::Connection;
    if !desktop_is("GNOME") {
        return false;
    }
    let Ok(conn) = Connection::session() else {
        return false;
    };
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &("org.gnome.Shell.Screenshot",),
    )
    .ok()
    .and_then(|m| m.body().deserialize::<bool>().ok())
    .unwrap_or(false)
}

/// Capture through `org.gnome.Shell.Screenshot`, in-process.
///
/// Unlike ScreenShot2, which hands back a file descriptor of raw pixels, this interface
/// writes a PNG to a path of its choosing and returns that path — so the cost is one file
/// read rather than a pipe. `ScreenshotArea` accepts a rectangle directly, so a region
/// capture needs no separate crop step here (KDE's interface has the same shape, which is
/// why the KDE path also captures the rectangle directly rather than cropping).
fn gnome_capture_png(region: Option<(i32, i32, i32, i32)>) -> Result<Vec<u8>> {
    let conn = zbus::blocking::Connection::session()
        .context("no session bus (GNOME capture needs a running shell)")?;

    let reply = match region {
        Some((x, y, w, h)) => {
            if w <= 0 || h <= 0 {
                bail!("capture area must be positive, got {w}x{h}");
            }
            conn.call_method(
                Some("org.gnome.Shell.Screenshot"),
                "/org/gnome/Shell/Screenshot",
                Some("org.gnome.Shell.Screenshot"),
                "ScreenshotArea",
                &(x, y, w, h, false, ""),
            )
        }
        None => conn.call_method(
            Some("org.gnome.Shell.Screenshot"),
            "/org/gnome/Shell/Screenshot",
            Some("org.gnome.Shell.Screenshot"),
            "Screenshot",
            &(false, false, ""),
        ),
    }
    .context("org.gnome.Shell.Screenshot refused the request")?;

    let (success, filename): (bool, String) = reply
        .body()
        .deserialize()
        .context("unexpected reply shape from org.gnome.Shell.Screenshot")?;

    // GNOME reports failure as a boolean rather than a D-Bus error, so this is not
    // redundant with the `context` above: a refusal is a successful call carrying false.
    if !success {
        bail!("GNOME refused the screenshot (the shell may be locked)");
    }
    if filename.is_empty() {
        bail!("GNOME reported success but returned no filename");
    }

    let bytes = std::fs::read(&filename)
        .with_context(|| format!("reading GNOME screenshot at {filename}"))?;
    let _ = std::fs::remove_file(&filename);

    if !bytes.starts_with(b"\x89PNG") {
        bail!("GNOME screenshot was not a PNG ({} bytes)", bytes.len());
    }
    Ok(bytes)
}

/// Capture a rectangle through KWin's ScreenShot2 D-Bus API and return PNG.
///
/// This is the fast path. `CaptureArea` writes raw pixels into a pipe we own, so
/// there is no process spawn, no temporary file, and no second crop step:
/// measured 29 ms for the whole 4114x1234 desktop and 2.9 ms for a 256x256 crop,
/// against 921 ms for a `spectacle -b -f` run on the same box.
///
/// KWin returns **raw pixels even when asked for PNG** (`native: false` only
/// affects some backends), and `stride` may exceed `width * 4`, so rows have to be
/// repacked before encoding. Assuming a tight RGBA buffer here is what produces
/// sheared, garbage images.
#[cfg(target_os = "linux")]
fn kwin_capture_png(x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>> {
    use std::collections::HashMap;
    use std::io::Read;

    if w <= 0 || h <= 0 {
        bail!("capture area must be positive, got {w}x{h}");
    }

    let (mut reader, writer) = std::io::pipe().context("creating capture pipe")?;

    let conn = zbus::blocking::Connection::session()
        .context("no session bus (ScreenShot2 needs a running compositor)")?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.kde.KWin",
        "/org/kde/KWin/ScreenShot2",
        "org.kde.KWin.ScreenShot2",
    )
    .context("binding org.kde.KWin.ScreenShot2")?;

    let mut options: HashMap<String, zbus::zvariant::Value> = HashMap::new();
    options.insert("native".into(), zbus::zvariant::Value::from(false));
    options.insert("include-cursor".into(), zbus::zvariant::Value::from(false));
    options.insert("include-shadow".into(), zbus::zvariant::Value::from(false));

    let fd = zbus::zvariant::Fd::from(&writer);
    let reply: HashMap<String, zbus::zvariant::OwnedValue> = proxy
        .call("CaptureArea", &(x, y, w as u32, h as u32, options, fd))
        .context("ScreenShot2 CaptureArea refused")?;
    // Close our write end so the read below terminates.
    drop(writer);

    let mut raw = Vec::new();
    reader
        .read_to_end(&mut raw)
        .context("reading pixels from the capture pipe")?;
    if raw.is_empty() {
        bail!("ScreenShot2 returned no pixels");
    }

    let grab_u32 = |k: &str| -> Option<u32> {
        reply.get(k).and_then(|v| v.downcast_ref::<u32>().ok())
    };
    let kind = reply
        .get("type")
        .and_then(|v| String::try_from(&**v).ok())
        .unwrap_or_default();
    let cw = grab_u32("width").unwrap_or(w as u32) as usize;
    let ch = grab_u32("height").unwrap_or(h as u32) as usize;
    let stride = grab_u32("stride")
        .map(|s| s as usize)
        .unwrap_or(cw.saturating_mul(4));
    let row_bytes = cw.saturating_mul(4);

    if raw.len() < stride.saturating_mul(ch) {
        bail!(
            "ScreenShot2 sent {} bytes for {}x{} stride {} — short read",
            raw.len(), cw, ch, stride
        );
    }
    if kind != "raw" && raw.starts_with(b"\x89PNG") {
        return Ok(raw); // compositor already encoded it
    }

    // Repack honouring stride.
    let mut packed = Vec::with_capacity(row_bytes * ch);
    for row in 0..ch {
        let start = row * stride;
        packed.extend_from_slice(&raw[start..start + row_bytes]);
    }

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, cw as u32, ch as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().context("writing PNG header")?;
        writer
            .write_image_data(&packed)
            .context("writing PNG pixels")?;
    }
    Ok(out)
}

/// Capture the whole desktop as PNG. Fast path first, spectacle second.
#[cfg(target_os = "linux")]
/// The in-process backends, in order, before any external tool is spawned.
///
/// KWin first (it hands back raw pixels over a pipe, fastest), then GNOME (it writes a PNG
/// and returns the path). Both ride the session bus we already hold, so neither costs a
/// process spawn. Returns `None` when neither compositor is answering, and the caller falls
/// through to the external-tool chain.
/// Returns the pixels, plus what was tried and why it failed.
///
/// The failure notes matter: a denied compositor used to disappear from the error
/// message, so a GNOME box reported "every capture backend failed" without ever mentioning
/// that GNOME had refused — which reads like the path was never attempted.
fn in_process_capture_detailed(
    region: Option<(i32, i32, i32, i32)>,
) -> (Option<Vec<u8>>, Vec<String>) {
    let mut notes = Vec::new();

    // No display means no capture, full stop. Without this, a headless box with a live
    // session bus still tried the portal and then waited out the full portal timeout on a
    // permission dialog that could never be answered — measured as a test run that stalled
    // in the headless compatibility case.
    if !(is_wayland() || std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())) {
        return (None, vec!["no display session".to_string()]);
    }
    // Native X11: one GetImage call, no `import` subprocess and no PNG written to disk.
    // XWayland also has a DISPLAY, but there the compositor path below sees the real screen,
    // so this is tried first only on a native X11 session.
    if !is_wayland() {
        match x11_capture_png(region) {
            Ok(png) => return (Some(png), notes),
            Err(e) => notes.push(format!("x11-getimage: {e}")),
        }
    }

    match region {
        Some((x, y, w, h)) => {
            if kwin_screenshot2_available() {
                match kwin_capture_png(x, y, w, h) {
                    Ok(png) => return (Some(png), notes),
                    Err(e) => notes.push(format!("kwin-screenshot2: {e}")),
                }
            }
            if portal_available() {
                // The portal interface has no region call; capture full and crop by
                // falling through to the external tools that can.
                notes.push("portal: no region capture (full-frame only)".to_string());
            }
            if gnome_screenshot_available() {
                match gnome_capture_png(Some((x, y, w, h))) {
                    Ok(png) => return (Some(png), notes),
                    Err(e) => notes.push(format!("gnome-shell-screenshot: {e}")),
                }
            }
        }
        None => {
            let (dw, dh) = desktop_extent();
            if kwin_screenshot2_available() {
                match kwin_capture_png(0, 0, dw, dh) {
                    Ok(png) => return (Some(png), notes),
                    Err(e) => notes.push(format!("kwin-screenshot2: {e}")),
                }
            }
            if portal_available() {
                match portal_capture_png() {
                    Ok(png) => return (Some(png), notes),
                    Err(e) => notes.push(format!("portal: {e}")),
                }
            }
            // Tried after the portal because modern GNOME refuses third-party callers on
            // this interface (measured: AccessDenied). Kept for compositors that do allow it.
            if gnome_screenshot_available() {
                match gnome_capture_png(None) {
                    Ok(png) => return (Some(png), notes),
                    Err(e) => notes.push(format!("gnome-shell-screenshot: {e}")),
                }
            }
        }
    }
    (None, notes)
}

fn in_process_capture(region: Option<(i32, i32, i32, i32)>) -> Option<Vec<u8>> {
    match region {
        Some((x, y, w, h)) => {
            if let Ok(png) = kwin_capture_png(x, y, w, h) {
                return Some(png);
            }
            if gnome_screenshot_available() {
                if let Ok(png) = gnome_capture_png(Some((x, y, w, h))) {
                    return Some(png);
                }
            }
        }
        None => {
            let (dw, dh) = desktop_extent();
            if let Ok(png) = kwin_capture_png(0, 0, dw, dh) {
                return Some(png);
            }
            if gnome_screenshot_available() {
                if let Ok(png) = gnome_capture_png(None) {
                    return Some(png);
                }
            }
        }
    }
    None
}


/// Total desktop extent, used when the caller wants "everything".
///
/// Side-by-side outputs are summed on width and the tallest wins, which is how
/// this box is laid out (1920 + 2194 = 4114 across, 1234 tall) and how KWin
/// reports the workspace. `LinuxDisplay` carries no origin, so a vertical stack
/// would under-report — acceptable for a capture bound, and the region call is
/// exact for anything the caller actually cares about.
#[cfg(target_os = "linux")]
fn desktop_extent() -> (i32, i32) {
    let mut w = 0i32;
    let mut h = 0i32;
    for d in displays() {
        w += d.width as i32;
        if d.height as i32 > h {
            h = d.height as i32;
        }
    }
    if w <= 0 || h <= 0 {
        (1920, 1080)
    } else {
        (w, h)
    }
}


/// The capture tools to try, in the order this desktop should try them.
///
/// Order is decided by the session, not fixed. A desktop's OWN tool is tried first,
/// because a tool belonging to a different desktop can be hijacked by that desktop: run
/// `spectacle` from an XFCE session on a machine whose logged-in user is also running
/// KDE, and it talks to KWin over D-Bus and captures the OTHER display. Measured on a
/// real box: a 1280x800 test session produced a 1920x1080 image of the wrong screen.
///
/// After the desktop's own tool, the rest follow in a default order so that an
/// unrecognised session still gets every option.
/// The backends this session would actually try, in order, including the in-process
/// compositor paths. `capabilities()` and `capture_full_to()` both read this, so what the
/// UI is told and what capture does cannot drift apart.
pub fn capture_backend_order() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if kwin_screenshot2_available() {
        out.push("kwin-screenshot2".to_string());
    }
    if portal_available() {
        out.push("portal".to_string());
    }
    if gnome_screenshot_available() {
        out.push("gnome-shell-screenshot".to_string());
    }
    for t in capture_backends() {
        if have(t) {
            out.push(t.to_string());
        }
    }
    out
}

fn capture_backends() -> Vec<&'static str> {
    const ALL: [&str; 6] = [
        "spectacle",
        "grim",
        "xfce4-screenshooter",
        "cinnamon-screenshot",
        "gnome-screenshot",
        "import",
    ];

    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();

    // The session's own tools, then only the genuinely generic one.
    //
    // Other desktops' tools are deliberately NOT tried. They are GUI programs that can
    // open a picker or a save dialog when the session is not what they expect, and an
    // XFCE screenshot tool started inside a GNOME session hung indefinitely on a real
    // box. Naming the wrong tool is not a small matter of ordering: it can wedge the
    // caller.
    let preferred: &[&'static str] = if desktop.contains("KDE") {
        &["spectacle", "import"]
    } else if desktop.contains("XFCE") {
        &["xfce4-screenshooter", "import"]
    } else if desktop.contains("CINNAMON") {
        &["cinnamon-screenshot", "import"]
    } else if desktop.contains("GNOME") || desktop.contains("UNITY") {
        &["gnome-screenshot", "import"]
    } else if desktop.contains("SWAY") || desktop.contains("HYPRLAND") {
        &["grim", "import"]
    } else {
        // Unknown session: every option, because there is no better signal.
        &ALL
    };

    let mut out: Vec<&'static str> = Vec::with_capacity(ALL.len());
    for t in preferred.iter() {
        if !out.contains(t) {
            out.push(t);
        }
    }
    out
}

fn capture_full_to(path: &Path) -> Result<()> {
    let target = path.to_string_lossy().to_string();

    // Every backend is TRIED IN TURN and a failure falls through to the next one. The old
    // version returned early on each tool but `bail!`ed on failure, so a machine with
    // spectacle installed and a non-KDE session got a total capture failure while three
    // working backends sat there unused.
    let mut attempts: Vec<String> = Vec::new();

    for tool in capture_backends() {
        if !have(tool) {
            continue;
        }
        let _ = std::fs::remove_file(path);

        // spectacle fails intermittently when captures are issued back to back: it exits
        // non-zero having written nothing and having printed nothing, so a single attempt
        // produces an undiagnosable "spectacle failed: ". It also needs the retry more than
        // the others because it round-trips through KWin.
        if tool == "spectacle" {
            let mut ok = false;
            for attempt in 1..=2 {
                let _ = std::fs::remove_file(path);
                match run("spectacle", &["-b", "-n", "-f", "-o", &target]) {
                    Ok(out) if out.status.success() && path.exists() => {
                        ok = true;
                        break;
                    }
                    Ok(out) => attempts.push(format!(
                        "spectacle attempt {attempt}: status={} stderr={:?} file_written={}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr).trim(),
                        path.exists()
                    )),
                    Err(e) => {
                        attempts.push(format!("spectacle attempt {attempt}: {e}"));
                        break;
                    }
                }
                if attempt == 1 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
            if ok {
                return Ok(());
            }
            continue;
        }

        let result = match tool {
            "grim" => run("grim", &[&target]),
            // -f full screen, -s save straight to this path (no dialog, no clipboard).
            "xfce4-screenshooter" => run("xfce4-screenshooter", &["-f", "-s", &target]),
            "cinnamon-screenshot" => run("cinnamon-screenshot", &["-f", &target]),
            "gnome-screenshot" => run("gnome-screenshot", &["-f", &target]),
            "import" => run("import", &["-window", "root", &target]),
            other => {
                attempts.push(format!("{other}: no invocation defined"));
                continue;
            }
        };

        match result {
            Ok(out) if out.status.success() && path.exists() => return Ok(()),
            Ok(out) => attempts.push(format!(
                "{tool}: status={} stderr={:?}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(e) => attempts.push(format!("{tool}: {e}")),
        }
    }

    if attempts.is_empty() {
        bail!(
            "no screen capture tool available (looked for spectacle, grim, \
             xfce4-screenshooter, cinnamon-screenshot, gnome-screenshot, import)"
        );
    }
    bail!("every capture backend failed: {}", attempts.join(" | "))
}

pub fn screenshot_png_fast() -> Result<Vec<u8>> {
    if let Some(png) = in_process_capture(None) {
        return Ok(png);
    }
    let tmp = std::env::temp_dir().join(format!("quixi-shot-{}.png", std::process::id()));
    capture_to(&tmp, None).context("no in-process compositor and the fallback failed")?;
    let bytes = std::fs::read(&tmp).context("reading fallback capture")?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

pub fn screenshot_png() -> Result<Vec<u8>> {
    let (png, notes) = in_process_capture_detailed(None);
    if let Some(png) = png {
        return Ok(png);
    }
    // No usable compositor interface: a wlroots or X11 session, or a compositor that
    // refused us. Fall through to whatever capture tool this desktop provides.
    let tmp = std::env::temp_dir().join(format!("quixi-shot-{}.png", std::process::id()));
    capture_to(&tmp, None).map_err(|e| {
        if notes.is_empty() {
            e
        } else {
            anyhow!("{e} | in-process: {}", notes.join(" | "))
        }
    })?;
    let bytes = std::fs::read(&tmp).context("reading captured screenshot")?;
    let _ = std::fs::remove_file(&tmp);
    if !bytes.starts_with(b"\x89PNG") {
        bail!("capture did not produce a PNG");
    }
    Ok(bytes)
}

/// Crop a PNG to a rectangle, in-process.
///
/// Region capture used to shell out to ImageMagick to crop a full-frame grab. That is
/// wrong twice over: it makes a region capture cost MORE than a full frame, and on a
/// Wayland desktop ImageMagick's X11 `import` cannot run at all — measured on GNOME, where
/// region capture failed outright while full-frame worked. The `png` crate is already a
/// dependency, so cropping costs nothing and works on every desktop.
fn crop_png(bytes: &[u8], x: u32, y: u32, w: u32, h: u32) -> Result<Vec<u8>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().context("reading PNG header to crop")?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).context("decoding PNG to crop")?;
    let (full_w, full_h) = (info.width, info.height);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4usize,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => bail!("cannot crop an indexed PNG"),
    };

    let x = x.min(full_w.saturating_sub(1));
    let y = y.min(full_h.saturating_sub(1));
    let w = w.min(full_w - x);
    let h = h.min(full_h - y);
    if w == 0 || h == 0 {
        bail!("crop rectangle is empty after clamping to {full_w}x{full_h}");
    }

    let src = &buf[..info.buffer_size()];
    let stride = full_w as usize * channels;
    let row_bytes = w as usize * channels;
    let mut out_pixels = Vec::with_capacity(row_bytes * h as usize);
    for row in 0..h as usize {
        let start = (y as usize + row) * stride + x as usize * channels;
        out_pixels.extend_from_slice(&src[start..start + row_bytes]);
    }

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(info.color_type);
        enc.set_depth(info.bit_depth);
        let mut writer = enc.write_header().context("PNG header for cropped region")?;
        writer
            .write_image_data(&out_pixels)
            .context("writing cropped PNG")?;
    }
    Ok(out)
}

pub fn screenshot_region_png(x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>> {
    let x = x.max(0);
    let y = y.max(0);
    let w = w.max(16);
    let h = h.max(16);

    // 1. A compositor that can capture a rectangle directly does it cheapest and exactly.
    let (png, notes) = in_process_capture_detailed(Some((x, y, w, h)));
    if let Some(png) = png {
        return Ok(png);
    }

    // 2. Everything else captures the full frame and crops HERE.
    //
    // This is what makes region capture work on GNOME: the portal only offers a
    // full-frame grab, GNOME refuses the compositor interface, and ImageMagick's X11
    // `import` cannot run on Wayland at all — measured, region capture failed outright
    // while full-frame succeeded. Cropping in-process needs no external tool, so it
    // works wherever a full frame works.
    let (full, _more) = in_process_capture_detailed(None);
    let full = match full {
        Some(f) => f,
        None => {
            let tmp = std::env::temp_dir().join(format!("quixi-full-{}.png", std::process::id()));
            capture_to(&tmp, None).map_err(|e| {
                if notes.is_empty() {
                    e
                } else {
                    anyhow!("{e} | in-process: {}", notes.join(" | "))
                }
            })?;
            let b = std::fs::read(&tmp).context("reading full frame for cropping")?;
            let _ = std::fs::remove_file(&tmp);
            b
        }
    };

    if !full.starts_with(b"\x89PNG") {
        bail!("full-frame capture did not produce a PNG");
    }
    crop_png(&full, x as u32, y as u32, w as u32, h as u32)
        .with_context(|| format!("cropping {w}x{h} at {x},{y}"))
}


// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

pub fn clipboard_read() -> Option<String> {
    if have("wl-paste") && is_wayland() {
        if let Ok(out) = run("wl-paste", &["--no-newline"]) {
            if out.status.success() {
                return Some(String::from_utf8_lossy(&out.stdout).to_string());
            }
        }
    }
    if have("xclip") {
        if let Ok(out) = run("xclip", &["-selection", "clipboard", "-o"]) {
            if out.status.success() {
                return Some(String::from_utf8_lossy(&out.stdout).to_string());
            }
        }
    }
    if have("xsel") {
        if let Ok(out) = run("xsel", &["--clipboard", "--output"]) {
            if out.status.success() {
                return Some(String::from_utf8_lossy(&out.stdout).to_string());
            }
        }
    }
    None
}

pub fn clipboard_write(text: &str) -> Result<()> {
    // Measured on GNOME Wayland: wl-copy's owner fork races the write and the data can be
    // lost, while xclip through XWayland round-trips reliably AND the Wayland clipboard sees
    // the value (XWayland bridges the selection). So prefer the X11 tool whenever there is
    // an X display, and fall back to wl-copy only on a Wayland session with no XWayland.
    let has_x = std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
    let (bin, args): (&str, Vec<&str>) = if have("xclip") && (has_x || !is_wayland()) {
        ("xclip", vec!["-selection", "clipboard"])
    } else if have("wl-copy") && is_wayland() {
        ("wl-copy", vec![])
    } else if have("xsel") {
        ("xsel", vec!["--clipboard", "--input"])
    } else {
        bail!(
            "no clipboard tool available: install wl-clipboard on Wayland, or xclip/xsel on X11"
        )
    };

    // Do NOT wait for the tool to exit.
    //
    // A clipboard tool is supposed to KEEP RUNNING: owning the selection is the job. wl-copy
    // forks an owner and, on GNOME, the parent does not necessarily exit either — measured:
    // it sat for the full 20s timeout and the write was reported as failed even though the
    // data was on the clipboard. So spawn it, feed it on a thread, and then verify by
    // READING THE CLIPBOARD BACK. Success is the content arriving, not the process ending.
    let mut child = Command::new(bin)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawning {bin}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = text.as_bytes().to_vec();
        std::thread::spawn(move || {
            use std::io::Write as _;
            let _ = stdin.write_all(&payload);
            let _ = stdin.flush();
            // dropping stdin closes the pipe, which signals end-of-data
        });
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        if clipboard_read().as_deref() == Some(text) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{bin} accepted the spawn but the clipboard never returned the text within 5s"
            );
        }
    }
}


// ---------------------------------------------------------------------------
// Input synthesis (needs ydotool + a running ydotoold)
// ---------------------------------------------------------------------------

/// Where ydotoold is actually listening.
///
/// ydotool defaults to `$XDG_RUNTIME_DIR/.ydotool_socket`, but ydotoold started without
/// arguments listens on `/tmp/.ydotool_socket`. On a real box those disagreed, and because
/// the code never set `YDOTOOL_SOCKET` the client looked in the wrong place and everything
/// failed while the daemon sat there running. Honour the environment first, then look in the
/// places a daemon actually binds.
fn ydotool_socket() -> Option<std::path::PathBuf> {
    // Only honour YDOTOOL_SOCKET when the socket is actually there.
    //
    // Trusting it blindly is how input dies silently: a launcher, a systemd unit or a test harness
    // exports a path it believes is right, the path does not exist, and every call fails with an
    // empty error because the message only says "failed". Measured on a box whose ydotoold is the
    // distro service on /run/ydotoold.socket while the caller had exported
    // $XDG_RUNTIME_DIR/.ydotool_socket. Fall through to the probe instead.
    if let Some(p) = std::env::var_os("YDOTOOL_SOCKET") {
        let path = std::path::PathBuf::from(&p);
        if !p.is_empty() && path.exists() {
            return Some(path);
        }
    }
    let mut candidates = Vec::new();
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        candidates.push(std::path::PathBuf::from(rt).join(".ydotool_socket"));
    }
    candidates.push(std::path::PathBuf::from("/tmp/.ydotool_socket"));
    candidates.push(std::path::PathBuf::from("/run/ydotoold.socket"));
    candidates.into_iter().find(|p| p.exists())
}

/// Is the daemon reachable? The binary existing proves nothing — ydotoold is the privileged
/// half and the socket is what actually accepts events.
fn ydotool_ready() -> bool {
    have("ydotool") && ydotool_socket().is_some()
}

fn ydotool(args: &[&str]) -> Result<()> {
    if !have("ydotool") {
        bail!("ydotool is not installed; input synthesis is unavailable");
    }
    let socket = ydotool_socket().ok_or_else(|| {
        anyhow!(
            "ydotoold is not running (no socket at $YDOTOOL_SOCKET, \
             $XDG_RUNTIME_DIR/.ydotool_socket, /tmp/.ydotool_socket or /run/ydotoold.socket)"
        )
    })?;
    let out = run_with_env("ydotool", args, &[("YDOTOOL_SOCKET", &socket)], TOOL_TIMEOUT)?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    bail!("ydotool {} failed: {err}", args.first().copied().unwrap_or(""));
}

/// evdev keycode + shift flag for one ASCII character (US layout).
///
/// `ydotool type` is NOT used here on purpose: it needs an XKB keymap loaded
/// into the daemon and silently produces nothing when there isn't one. Measured
/// on Arch/KDE: `ydotool type "touch /tmp/x"` returned 0 and typed nothing,
/// while `ydotool key 30:1 30:0` reliably produced "a". Raw keycodes always work
/// because they are kernel-level.
fn char_keycode(ch: char) -> Option<(u16, bool)> {
    let (code, shift) = match ch {
        'a'..='z' => {
            const L: [u16; 26] = [
                30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22,
                47, 17, 45, 21, 44,
            ];
            (L[(ch as u8 - b'a') as usize], false)
        }
        'A'..='Z' => {
            const L: [u16; 26] = [
                30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22,
                47, 17, 45, 21, 44,
            ];
            (L[(ch as u8 - b'A') as usize], true)
        }
        '1'..='9' => ([2u16, 3, 4, 5, 6, 7, 8, 9, 10][(ch as u8 - b'1') as usize], false),
        '0' => (11, false),
        ' ' => (57, false),
        '\n' => (28, false),
        '\t' => (15, false),
        '-' => (12, false),
        '_' => (12, true),
        '=' => (13, false),
        '+' => (13, true),
        '[' => (26, false),
        '{' => (26, true),
        ']' => (27, false),
        '}' => (27, true),
        '\\' => (43, false),
        '|' => (43, true),
        ';' => (39, false),
        ':' => (39, true),
        '\'' => (40, false),
        '"' => (40, true),
        '`' => (41, false),
        '~' => (41, true),
        ',' => (51, false),
        '<' => (51, true),
        '.' => (52, false),
        '>' => (52, true),
        '/' => (53, false),
        '?' => (53, true),
        '!' => (2, true),
        '@' => (3, true),
        '#' => (4, true),
        '$' => (5, true),
        '%' => (6, true),
        '^' => (7, true),
        '&' => (8, true),
        '*' => (9, true),
        '(' => (10, true),
        ')' => (11, true),
        _ => return None,
    };
    Some((code, shift))
}

const KEY_LEFTSHIFT: u16 = 42;

/// Types text by emitting per-character evdev keycodes.
pub fn type_text(text: &str) -> Result<()> {
    // ydotool defaults to ~12 ms between key events, which caps typing at ~40
    // characters/second -- a 1200-character command took 30 s. Measured on the
    // same box: `-d 0` does 120 keypresses in 16 ms (133 us each) and the target
    // shell still received every byte, so the default delay is pure overhead
    // here. BATCH is larger too: fewer process spawns per line.
    const BATCH: usize = 240;
    // Wayland needs a real inter-key delay; X11 does not.
    //
    // At `-d 0` GNOME dropped and reordered characters: "qx-26736" arrived as "qx2673d"
    // (dash lost, digits transposed). At 25 ms the same string arrives byte-identical, and it
    // reproduced across runs. X11 took `-d 0` cleanly, and it is 20x faster there (133 us/key
    // against 2358 us), so the delay is applied only where it is needed.
    let delay_arg = if is_wayland() { "25" } else { "0" };
    let delay = ["-d", delay_arg];
    let mut batch: Vec<String> = Vec::new();
    for ch in text.chars() {
        let (code, shift) = char_keycode(ch).ok_or_else(|| {
            anyhow!("type_text cannot map character {ch:?} to an evdev keycode")
        })?;
        if shift {
            batch.push(format!("{KEY_LEFTSHIFT}:1"));
        }
        batch.push(format!("{code}:1"));
        batch.push(format!("{code}:0"));
        if shift {
            batch.push(format!("{KEY_LEFTSHIFT}:0"));
        }
        if batch.len() >= BATCH {
            let args: Vec<&str> = batch.iter().map(String::as_str).collect();
            let mut full = vec!["key"];
            full.extend(delay.iter().copied());
            full.extend(args);
            ydotool(&full)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        let args: Vec<&str> = batch.iter().map(String::as_str).collect();
        let mut full = vec!["key"];
        full.extend(delay.iter().copied());
        full.extend(args);
        ydotool(&full)?;
    }
    Ok(())
}

pub fn press_key(combo: &str, repeat: u32) -> Result<()> {
    let events = keycode_for(combo)
        .ok_or_else(|| anyhow!("unsupported key combo for ydotool: {combo}"))?;
    // `events` is a whitespace-separated sequence like "42:1 30:1 30:0 42:0".
    // Each token must be its OWN argv entry: passing the whole string as one
    // argument makes ydotool reject it and nothing is sent. Measured: the text
    // typed correctly but Enter never fired, leaving the command unexecuted.
    let tokens: Vec<&str> = events.split_whitespace().collect();
    let mut argv: Vec<&str> = Vec::with_capacity(tokens.len() + 1);
    argv.push("key");
    argv.extend(tokens);
    for _ in 0..repeat.clamp(1, 20) {
        ydotool(&argv)?;
    }
    Ok(())
}

/// Maps a combo name onto numeric evdev keycodes.
///
/// Numeric codes are used rather than evdev key NAMES: `ydotool key KEY_ENTER:1`
/// was observed returning success while delivering nothing, whereas
/// `ydotool key 28:1 28:0` reliably produced a newline. Codes are kernel-level
/// and cannot be misinterpreted.
fn keycode_for(combo: &str) -> Option<String> {
    let lower = combo.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
    let (modifiers, key) = parts.split_at(parts.len().saturating_sub(1));
    let key = *key.first()?;

    // Modifier evdev codes.
    let modifier_code = |m: &str| -> Option<u16> {
        Some(match m {
            "cmd" | "command" | "meta" | "super" | "win" => 125,
            "ctrl" | "control" => 29,
            "alt" | "option" | "opt" => 56,
            "shift" => 42,
            _ => return None,
        })
    };

    // A bare modifier is a real key press on its own ("shift", "ctrl+"). The
    // parser below treats the last token as the key, so a lone modifier would
    // otherwise fall through to the character lookup and be rejected.
    if let Some(code) = modifier_code(key) {
        if modifiers.is_empty() {
            let event = format!("{code}:1 {code}:0");
            return Some(event);
        }
    }

    // Resolve the main key to (code, needs_shift).
    let (code, needs_shift) = match key {
        "return" | "enter" => (28u16, false),
        "escape" | "esc" => (1, false),
        "tab" => (15, false),
        "space" => (57, false),
        "backspace" | "delete" => (14, false),
        "up" => (103, false),
        "down" => (108, false),
        "left" => (105, false),
        "right" => (106, false),
        "home" => (102, false),
        "end" => (107, false),
        "pageup" => (104, false),
        "pagedown" => (109, false),
        other => {
            let mut chars = other.chars();
            let (ch, extra) = (chars.next()?, chars.next());
            if extra.is_some() {
                return None; // multi-character token that is not a known key name
            }
            let (c, s) = char_keycode(ch)?;
            (c, s)
        }
    };

    let mut codes: Vec<u16> = Vec::new();
    for m in modifiers {
        codes.push(modifier_code(m)?);
    }
    let shift_held = modifiers.contains(&"shift");
    if needs_shift && !shift_held {
        codes.push(KEY_LEFTSHIFT);
    }

    let mut events: Vec<String> = codes.iter().map(|c| format!("{c}:1")).collect();
    events.push(format!("{code}:1"));
    events.push(format!("{code}:0"));
    for c in codes.iter().rev() {
        events.push(format!("{c}:0"));
    }
    Some(events.join(" "))
}

/// The coordinate space `cursor_position` reports in (the X screen, which can differ from
/// the panel's physical size on a scaled desktop).
fn pointer_space() -> Option<(f64, f64)> {
    if let Ok(out) = run("xdotool", &["getdisplaygeometry"]) {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut it = s.split_whitespace();
            if let (Some(w), Some(h)) = (it.next(), it.next()) {
                if let (Ok(w), Ok(h)) = (w.parse::<f64>(), h.parse::<f64>()) {
                    if w > 0.0 && h > 0.0 {
                        return Some((w, h));
                    }
                }
            }
        }
    }
    let ds = displays();
    let w = ds.iter().map(|d| d.width as f64).fold(0.0, f64::max);
    let h = ds.iter().map(|d| d.height as f64).fold(0.0, f64::max);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

static ABS_SCALE: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();

/// How many "ydotool absolute units" correspond to one pixel.
///
/// This is NOT 1. Measured on a 2560x1600 X screen: `--absolute -x 640` lands the pointer
/// at x=1280, so the injected space is half the screen. Assuming it was 1:1 put every
/// absolute move off by a factor of two — asking for the middle of the screen landed in the
/// bottom-right corner.
///
/// Guessing the ratio from the display list does not work, because both readings come back
/// in the same pixels. So measure it: move to a known quarter-point, read where the pointer
/// actually landed, and cache the ratio. Self-correcting on any desktop.
fn abs_scale() -> Option<(f64, f64)> {
    if let Some(s) = ABS_SCALE.get() {
        return Some(*s);
    }
    let (pw, ph) = pointer_space()?;
    let probe_x = (pw / 4.0) as i32;
    let probe_y = (ph / 4.0) as i32;
    ydotool(&[
        "mousemove",
        "--absolute",
        "-x",
        &probe_x.to_string(),
        "-y",
        &probe_y.to_string(),
    ])
    .ok()?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    let (gx, gy) = cursor_position()?;
    if gx <= 0.0 || gy <= 0.0 {
        return None;
    }
    let sx = gx / probe_x as f64;
    let sy = gy / probe_y as f64;
    if sx <= 0.0 || sy <= 0.0 {
        return None;
    }
    let _ = ABS_SCALE.set((sx, sy));
    Some((sx, sy))
}

pub fn mouse_move(x: i32, y: i32) -> Result<()> {
    // Convert pixels (what cursor_position reports) into ydotool's absolute units.
    let (ax, ay) = match abs_scale() {
        Some((sx, sy)) => (
            ((x as f64) / sx).round().max(0.0) as i32,
            ((y as f64) / sy).round().max(0.0) as i32,
        ),
        None => (x.max(0), y.max(0)),
    };
    ydotool(&["mousemove", "--absolute", "-x", &ax.to_string(), "-y", &ay.to_string()])
}


/// Canonical button name, or an error.
///
/// There used to be a `_ => "left"` fallback here, which turned any typo --
/// including an empty string -- into a real left click somewhere on the user's
/// screen. A caller passing garbage must be told, not obeyed.
fn button_name(button: &str) -> Result<&'static str> {
    match button.trim().to_ascii_lowercase().as_str() {
        "left" => Ok("left"),
        "right" => Ok("right"),
        "middle" => Ok("middle"),
        other => bail!("unknown mouse button {other:?}; expected left, right or middle"),
    }
}

pub fn mouse_click(button: &str, click_count: i64) -> Result<()> {
    let name = button_name(button)?;
    for _ in 0..click_count.clamp(1, 10) {
        ydotool(&["click", name])?;
    }
    Ok(())
}

/// Press a mouse button and hold it.
///
/// ydotool 1.0.4 has NO `mousedown`/`mouseup` subcommands -- only `click`,
/// `mousemove`, `type`, `key`. A held button is expressed as an evdev key
/// press on the BTN_* code (BTN_LEFT 0x110, BTN_RIGHT 0x111, BTN_MIDDLE 0x112),
/// which is what the kernel actually emits for a physical button.
pub fn mouse_down(button: &str) -> Result<()> {
    let code = button_evdev(button)?;
    ydotool(&["key", &format!("{code}:1")])
}

pub fn mouse_up(button: &str) -> Result<()> {
    let code = button_evdev(button)?;
    ydotool(&["key", &format!("{code}:0")])
}

/// evdev BTN_* code for a logical button name.
fn button_evdev(button: &str) -> Result<u16> {
    Ok(match button_name(button)? {
        "right" => 0x111,
        "middle" => 0x112,
        _ => 0x110, // left, already validated
    })
}

pub fn mouse_drag(x1: i32, y1: i32, x2: i32, y2: i32, button: &str) -> Result<()> {
    mouse_move(x1, y1)?;
    mouse_down(button)?;
    mouse_move(x2, y2)?;
    mouse_up(button)
}

// ---------------------------------------------------------------------------
// Displays
// ---------------------------------------------------------------------------

/// Displays from xrandr, WITH their logical rectangles.
///
/// The old xrandr path returned names only, so two things were impossible on X11: a
/// display list with sizes, and deriving which output the pointer is on. `xrandr --query`
/// gives the geometry inline (`eDP-2 connected primary 2560x1600+0+0`), so parse it.
fn x11_display_state() -> Result<Vec<(LinuxDisplay, (i32, i32, i32, i32))>> {
    let raw = run_ok("xrandr", &["--query"]).context("xrandr --query failed")?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if !line.contains(" connected") {
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        // find the WxH+X+Y token
        let mut geom = None;
        for tok in it.clone() {
            let core = tok.split('+').next().unwrap_or("");
            if let Some((w, h)) = core.split_once('x') {
                if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                    let rest: Vec<i32> = tok
                        .split('+')
                        .skip(1)
                        .filter_map(|v| v.parse::<i32>().ok())
                        .collect();
                    let (x, y) = match rest.as_slice() {
                        [x, y, ..] => (*x, *y),
                        _ => (0, 0),
                    };
                    geom = Some((w, h, x, y));
                    break;
                }
            }
        }
        if let Some((w, h, x, y)) = geom {
            out.push((
                LinuxDisplay {
                    id: out.len() as u32,
                    name: name.to_string(),
                    width: w,
                    height: h,
                    scale: 1.0,
                },
                (x, y, w as i32, h as i32),
            ));
        }
    }
    if out.is_empty() {
        bail!("xrandr reported no connected outputs");
    }
    Ok(out)
}

/// Displays from GNOME's own interface, with no external tool.
///
/// `org.gnome.Mutter.DisplayConfig.GetCurrentState` is the equivalent of kscreen-doctor on
/// KDE. Without it GNOME had NO display enumeration at all — measured: `{"displays":[]}`
/// and a failed `displays()` on a perfectly normal two-output-free desktop.
///
/// Returns the physical mode size plus the logical scale, matching what kscreen-doctor
/// reports, and the logical monitor rectangles so the active output can be derived.
fn gnome_display_state() -> Result<Vec<(LinuxDisplay, (i32, i32, i32, i32))>> {
    type VMap = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;
    type Mode = (String, i32, i32, f64, f64, Vec<f64>, VMap);
    type Monitor = ((String, String, String, String), Vec<Mode>, VMap);
    type Logical = (i32, i32, f64, u32, bool, Vec<(String, String, String, String)>, VMap);

    let conn = zbus::blocking::Connection::session()
        .context("no session bus for org.gnome.Mutter.DisplayConfig")?;
    let reply = conn.call_method(
        // The destination is the interface's OWN bus name. GNOME exports each Mutter
        // interface under a separate well-known name, so addressing "org.gnome.Mutter"
        // returns ServiceUnknown even though the shell is right there.
        Some("org.gnome.Mutter.DisplayConfig"),
        "/org/gnome/Mutter/DisplayConfig",
        Some("org.gnome.Mutter.DisplayConfig"),
        "GetCurrentState",
        &(),
    )
    .context("Mutter refused GetCurrentState")?;

    let (_serial, monitors, logical, _props): (u32, Vec<Monitor>, Vec<Logical>, VMap) = reply
        .body()
        .deserialize()
        .context("unexpected reply shape from Mutter.DisplayConfig")?;

    // connector -> the size of its CURRENT mode (modes carry an "is-current" flag)
    let mut current: std::collections::HashMap<String, (i32, i32)> = std::collections::HashMap::new();
    for ((connector, _v, _p, _s), modes, _mprops) in &monitors {
        for (_id, w, h, _rate, _pref, _scales, mprops) in modes {
            let is_current = mprops
                .get("is-current")
                .and_then(|v| bool::try_from(v).ok())
                .unwrap_or(false);
            if is_current {
                current.insert(connector.clone(), (*w, *h));
            }
        }
    }

    let mut out = Vec::new();
    for (x, y, scale, _transform, _primary, mon_list, _lprops) in logical {
        let connector = mon_list
            .first()
            .map(|m| m.0.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let (pw, ph) = current.get(&connector).copied().unwrap_or((0, 0));
        // logical size is what the user sees; report physical mode size + scale, and the
        // logical rectangle for hit-testing.
        let lw = if scale > 0.0 { (pw as f64 / scale).round() as i32 } else { pw };
        let lh = if scale > 0.0 { (ph as f64 / scale).round() as i32 } else { ph };
        out.push((
            LinuxDisplay {
                id: out.len() as u32,
                name: connector,
                width: pw as u32,
                height: ph as u32,
                scale,
            },
            (x, y, lw, lh),
        ));
    }
    if out.is_empty() {
        bail!("Mutter reported no logical monitors");
    }
    Ok(out)
}

/// Parsed output list from kscreen-doctor (KDE) with an xrandr fallback.
pub fn list_displays_json() -> Result<serde_json::Value> {
    // Hyprland answers this itself; see displays() for why it goes first.
    if hyprland_running() {
        if let Ok(v) = hyprctl_json("monitors") {
            if let Some(mons) = v.as_array() {
                let list: Vec<serde_json::Value> = mons
                    .iter()
                    .filter(|m| !m.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false))
                    .map(|m| {
                        serde_json::json!({
                            "id": m.get("id").cloned().unwrap_or(serde_json::Value::Null),
                            "name": m.get("name").cloned().unwrap_or(serde_json::Value::Null),
                            "width": m.get("width").cloned().unwrap_or(serde_json::Value::Null),
                            "height": m.get("height").cloned().unwrap_or(serde_json::Value::Null),
                            "scale": m.get("scale").cloned().unwrap_or(serde_json::Value::Null),
                        })
                    })
                    .collect();
                if !list.is_empty() {
                    return Ok(serde_json::json!({ "displays": list }));
                }
            }
        }
    }

    // GNOME has no kscreen-doctor and usually no xrandr; its own interface is the only
    // route and needs no external tool.
    if desktop_is("GNOME") {
        match gnome_display_state() {
            Ok(st) => {
            let list: Vec<serde_json::Value> = st
                .into_iter()
                .map(|(d, _)| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "width": d.width,
                        "height": d.height,
                        "scale": d.scale,
                    })
                })
                .collect();
            return Ok(serde_json::json!({ "displays": list }));
            }
            Err(e) => {
                if std::env::var_os("QUIXI_DEBUG_DISPLAYS").is_some() {
                    eprintln!("Mutter DisplayConfig failed: {e:#}");
                }
            }
        }
    }

    if have("xrandr") {
        if let Ok(st) = x11_display_state() {
            let list: Vec<serde_json::Value> = st
                .into_iter()
                .map(|(d, _)| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "width": d.width,
                        "height": d.height,
                        "scale": d.scale,
                    })
                })
                .collect();
            return Ok(serde_json::json!({ "displays": list }));
        }
    }

    if have("kscreen-doctor") {
        let raw = run_ok("kscreen-doctor", &["-j"])?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .context("kscreen-doctor returned unparseable JSON")?;
        if let Some(outputs) = parsed.get("outputs").and_then(|o| o.as_array()) {
            let list: Vec<serde_json::Value> = outputs
                .iter()
                .filter(|o| o.get("connected").and_then(|c| c.as_bool()).unwrap_or(false))
                .map(|o| {
                    serde_json::json!({
                        "name": o.get("name").cloned().unwrap_or(serde_json::Value::Null),
                        "enabled": o.get("enabled").cloned().unwrap_or(serde_json::Value::Bool(true)),
                        "current_mode_id": o.get("currentModeId").cloned().unwrap_or(serde_json::Value::Null),
                        "scale": o.get("scale").cloned().unwrap_or(serde_json::Value::Null),
                        "modes": o.get("modes").cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect();
            return Ok(serde_json::json!({ "displays": list }));
        }
        return Ok(parsed);
    }

    if have("xrandr") {
        let raw = run_ok("xrandr", &["--query"])?;
        let mut list = Vec::new();
        for line in raw.lines() {
            if line.contains(" connected") {
                let name = line.split_whitespace().next().unwrap_or("").to_string();
                let connected = !line.contains("disconnected");
                list.push(serde_json::json!({ "name": name, "connected": connected }));
            }
        }
        return Ok(serde_json::json!({ "displays": list }));
    }

    bail!("no display enumeration available (Mutter, kscreen-doctor / xrandr / wlr-randr)")
}

/// The output that currently holds the active window (KWin only).
///
/// Called over the session bus directly instead of shelling out to `qdbus6`. It is
/// a single method call on an interface we already speak (ScreenShot2 uses the same
/// connection), and going out to a helper for it put `qt6-tools` in the dependency
/// list for one string.
/// Which output the pointer is on, for desktops with no equivalent of KWin's call.
///
/// KWin names the active output directly. Everywhere else it has to be derived: take the
/// logical monitor rectangles and see which one contains the cursor. That is the same
/// answer by a different route, and it works on GNOME (Mutter) and X11 (xrandr) alike.
fn active_output_by_geometry() -> Option<String> {
    let st = gnome_display_state()
        .or_else(|_| x11_display_state())
        .ok()?;
    let (cx, cy) = cursor_position()?;
    for (d, (x, y, w, h)) in st {
        if (cx as i32) >= x && (cx as i32) < x + w && (cy as i32) >= y && (cy as i32) < y + h {
            return Some(d.name);
        }
    }
    None
}

/// The active output on GNOME, from Mutter's own `primary` flag.
///
/// This used to be derived from the cursor position — "the monitor whose rectangle contains
/// the pointer". That works on X11, and on Wayland it stopped working the moment
/// `cursor_position` began honestly returning `None` instead of XWayland's stale pointer, which
/// is exactly how the two bugs were connected.
///
/// Mutter already knows which monitor is primary; guessing was never necessary.
fn gnome_active_output() -> Option<String> {
    type VMap = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;
    type Mode = (String, i32, i32, f64, f64, Vec<f64>, VMap);
    type Monitor = ((String, String, String, String), Vec<Mode>, VMap);
    type Logical = (i32, i32, f64, u32, bool, Vec<(String, String, String, String)>, VMap);

    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some("org.gnome.Mutter.DisplayConfig"),
            "/org/gnome/Mutter/DisplayConfig",
            Some("org.gnome.Mutter.DisplayConfig"),
            "GetCurrentState",
            &(),
        )
        .ok()?;
    let (_serial, _monitors, logical, _props): (u32, Vec<Monitor>, Vec<Logical>, VMap) =
        reply.body().deserialize().ok()?;

    // primary flag is the 5th field; its first connector is the output driving it.
    for (_x, _y, _scale, _transform, primary, monitors, _p) in logical {
        if primary {
            if let Some((connector, _, _, _)) = monitors.first() {
                return Some(connector.clone());
            }
        }
    }
    None
}

pub fn active_output() -> Option<String> {
    if desktop_is("GNOME") {
        if let Some(name) = gnome_active_output() {
            return Some(name);
        }
    }
    if !desktop_is("KDE") {
        return active_output_by_geometry();
    }
    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/KWin",
            Some("org.kde.KWin"),
            "activeOutputName",
            &(),
        )
        .ok()?;
    let name: String = reply.body().deserialize().ok()?;
    let name = name.trim().to_string();
    if !name.is_empty() {
        return Some(name);
    }
    active_output_by_geometry()
}

/// A display as reported by kscreen-doctor (KDE) or xrandr.
#[derive(Debug, Clone)]
pub struct LinuxDisplay {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// Connected, enabled outputs with their current mode resolved to pixels.
pub fn displays() -> Vec<LinuxDisplay> {
    let mut out: Vec<LinuxDisplay> = Vec::new();

    // Hyprland first on a Hyprland session. `hyprctl -j monitors` is the compositor's own idea of
    // the layout, including scale and which output is focused, so it beats inferring anything from
    // a tool that merely happens to be installed.
    if hyprland_running() {
        if let Ok(v) = hyprctl_json("monitors") {
            if let Some(mons) = v.as_array() {
                for (i, m) in mons.iter().enumerate() {
                    if m.get("disabled").and_then(|d| d.as_bool()).unwrap_or(false) {
                        continue;
                    }
                    out.push(LinuxDisplay {
                        id: m.get("id").and_then(|n| n.as_u64()).unwrap_or(i as u64) as u32,
                        name: m
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("output")
                            .to_string(),
                        width: m.get("width").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                        height: m.get("height").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                        scale: m.get("scale").and_then(|n| n.as_f64()).unwrap_or(1.0),
                    });
                }
                if !out.is_empty() {
                    return out;
                }
            }
        }
    }

    // X11: RandR over the protocol gives the same geometry `xrandr --query` prints, without
    // the parsing. Falls through to the text parser if the protocol call fails.
    if !is_wayland() {
        if let Ok(st) = x11_display_state_rb() {
            if !st.is_empty() {
                return st.into_iter().map(|(d, _)| d).collect();
            }
        }
    }

    // xrandr carries full geometry, so it is the X11 answer for both the list and the
    // active-output derivation.
    if have("xrandr") {
        if let Ok(st) = x11_display_state() {
            return st.into_iter().map(|(d, _)| d).collect();
        }
    }

    // GNOME first: kscreen-doctor is KDE's, and xrandr is often absent on a Wayland box.
    if desktop_is("GNOME") {
        if let Ok(st) = gnome_display_state() {
            return st.into_iter().map(|(d, _)| d).collect();
        }
    }

    if have("kscreen-doctor") {
        if let Ok(raw) = run_ok("kscreen-doctor", &["-j"]) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(outputs) = parsed.get("outputs").and_then(|o| o.as_array()) {
                    for o in outputs {
                        if !o.get("connected").and_then(|c| c.as_bool()).unwrap_or(false) {
                            continue;
                        }
                        if !o.get("enabled").and_then(|c| c.as_bool()).unwrap_or(true) {
                            continue;
                        }
                        let name = o.get("name").and_then(|n| n.as_str()).unwrap_or("output").to_string();
                        let scale = o.get("scale").and_then(|s| s.as_f64()).unwrap_or(1.0);
                        let current = o.get("currentModeId").and_then(|c| c.as_str()).unwrap_or("");
                        let mut width = 0u32;
                        let mut height = 0u32;
                        if let Some(modes) = o.get("modes").and_then(|m| m.as_array()) {
                            for m in modes {
                                let mid = m.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                if !current.is_empty() && mid != current {
                                    continue;
                                }
                                width = m
                                    .get("size")
                                    .and_then(|s| s.get("width"))
                                    .and_then(|w| w.as_u64())
                                    .unwrap_or(0) as u32;
                                height = m
                                    .get("size")
                                    .and_then(|s| s.get("height"))
                                    .and_then(|h| h.as_u64())
                                    .unwrap_or(0) as u32;
                                break;
                            }
                        }
                        let id = out.len() as u32 + 1;
                        out.push(LinuxDisplay { id, name, width, height, scale });
                    }
                }
            }
        }
    }
    out
}

/// Installed GUI applications as `(display_name, exec_line, desktop_file)`.
pub fn installed_apps() -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for root in desktop_roots() {
        let entries = match std::fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some((name, exec, _)) = parse_desktop(&path) {
                out.push((name, exec, path.to_string_lossy().to_string()));
            }
        }
    }
    out.sort_by_key(|e| e.0.to_lowercase());
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

// ---------------------------------------------------------------------------
// File and folder pickers
// ---------------------------------------------------------------------------

fn kdialog_pick(args: &[&str], prompt: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("kdialog");
    cmd.args(args);
    if let Some(p) = prompt {
        cmd.arg("--title").arg(p);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None; // user cancelled
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn zenity_pick(args: &[&str], prompt: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("zenity");
    cmd.args(args);
    if let Some(p) = prompt {
        cmd.arg(format!("--title={p}"));
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

// ---------------------------------------------------------------------------
// X11, in-process (x11rb)
// ---------------------------------------------------------------------------
//
// Everything here used to shell out: `import` for capture, `xrandr` for displays, `xdotool`
// for the pointer and the window under it. That is three binaries and three text formats to
// parse, all speaking one protocol — and on a bare X11 desktop (XFCE, i3, openbox) they were
// the ONLY way any of it worked.
//
// `x11rb` implements the X11 wire protocol in Rust and does not link libX11, so these become
// ordinary calls with typed replies. Deliberately NOT converted: the clipboard. Serving a
// selection means owning it and staying alive to answer SelectionRequest events from other
// clients, which a short-lived function cannot do; `xclip` exists precisely to sit and own it.

fn x11_connect() -> Option<(x11rb::rust_connection::RustConnection, usize)> {
    // A Wayland session has no X11 display unless XWayland is up. `connect` errors in that
    // case and every caller treats it as "not available", which is the correct answer.
    x11rb::connect(None).ok()
}

/// Pointer position, plus the deepest window under it.
fn x11_pointer() -> Option<(i16, i16, Option<u32>)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt as _;

    let (conn, screen_num) = x11_connect()?;
    let root = conn.setup().roots[screen_num].root;
    let r = conn.query_pointer(root).ok()?.reply().ok()?;

    // QueryPointer reports the immediate child of the window asked, not the leaf. Walk down
    // until there is no child — that is what "the window under the cursor" means.
    let mut win = r.child;
    for _ in 0..32 {
        if win == x11rb::NONE {
            break;
        }
        match conn.query_pointer(win).ok().and_then(|c| c.reply().ok()) {
            Some(q) if q.child != x11rb::NONE => win = q.child,
            _ => break,
        }
    }

    Some((r.root_x, r.root_y, if win == x11rb::NONE { None } else { Some(win) }))
}

/// Decode an X11 framebuffer into RGBA.
///
/// GetImage hands back the server's own pixel layout, and it is neither RGBA nor one fixed
/// width: 32 bits per pixel is BGRX, 24 is BGR, 16 is RGB565. The buffer length is the only
/// thing that says which you got, so the byte count decides the layout rather than a guess —
/// guessing "always 32bpp" is what produces a screenshot with the channels swapped, and
/// guessing "always 24bpp" reads past the end of the buffer on a modern server.
///
/// An 8-bit PseudoColor framebuffer is rejected rather than half-handled: it needs a colour-map
/// translation, which is a different job and would otherwise produce plausible garbage.
fn x11_pixels_to_rgba(data: &[u8], px: usize) -> Result<Vec<u8>> {
    let bpp = match data.len() {
        n if n == px * 4 => 4usize,
        n if n == px * 3 => 3usize,
        n if n == px * 2 => 2usize,
        n => bail!(
            "unexpected GetImage size {n} for {px} pixels (expected 2, 3 or 4 bytes per pixel; \
             an 8-bit PseudoColor visual is not supported)"
        ),
    };

    let mut rgba = Vec::with_capacity(px * 4);
    for i in 0..px {
        let o = i * bpp;
        match bpp {
            // B, G, R[, X] -> R, G, B, opaque
            4 | 3 => {
                rgba.push(data[o + 2]);
                rgba.push(data[o + 1]);
                rgba.push(data[o]);
            }
            // RGB565, little-endian: 5 bits red, 6 green, 5 blue
            2 => {
                let v = u16::from_le_bytes([data[o], data[o + 1]]);
                let r = ((v >> 11) & 0x1f) as u8;
                let g = ((v >> 5) & 0x3f) as u8;
                let b = (v & 0x1f) as u8;
                // widen to 8 bits by replicating the high bits into the low ones
                rgba.push((r << 3) | (r >> 2));
                rgba.push((g << 2) | (g >> 4));
                rgba.push((b << 3) | (b >> 2));
            }
            _ => unreachable!("bpp is constrained to 2, 3 or 4 above"),
        }
        rgba.push(255);
    }
    Ok(rgba)
}

// ---------------------------------------------------------------------------
// GNOME window list, via the optional `window-calls` Shell extension
// ---------------------------------------------------------------------------
//
// GNOME Shell's own `org.gnome.Shell.Introspect.GetWindows` is allow-listed to the XDG
// portals. GNOME's maintainer states plainly that no third-party application can be promoted
// to that list "and you definitely cannot do that programmatically, as it would defeat the
// entire point" — so that route is closed by design, not by a missing flag.
//
// AT-SPI is the other candidate, and it does expose windows — but every child reports its
// extents at 0,0 (measured on GNOME 51), so it can activate a widget and cannot locate one.
// Useful for pressing a dialog button; useless for hit-testing.
//
// What does work is a Shell extension, which runs inside the compositor and is therefore
// privileged by definition. `window-calls` (extensions.gnome.org/4724) exports a window list
// with geometry and an Activate method. This is strictly optional: absent the extension every
// function below returns None and the caller falls back to the previous behaviour.

/// A window as reported by the window-calls extension.
#[derive(Clone, Debug)]
pub struct GnomeExtWindow {
    pub id: u64,
    pub class: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

const GNOME_EXT_DEST: &str = "org.gnome.Shell";
const GNOME_EXT_PATH: &str = "/org/gnome/Shell/Extensions/Windows";
const GNOME_EXT_IFACE: &str = "org.gnome.Shell.Extensions.Windows";

/// Call a method on the extension, returning its string reply.
fn gnome_ext_call(method: &str, args: &(impl serde::Serialize + zbus::zvariant::DynamicType)) -> Option<String> {
    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(Some(GNOME_EXT_DEST), GNOME_EXT_PATH, Some(GNOME_EXT_IFACE), method, args)
        .ok()?;
    reply.body().deserialize::<String>().ok()
}

/// Is the extension installed and answering?
pub fn gnome_ext_available() -> bool {
    gnome_ext_call("List", &()).is_some()
}

/// Every window the extension knows about, with geometry.
pub fn gnome_ext_windows() -> Option<Vec<GnomeExtWindow>> {
    parse_ext_windows(&gnome_ext_call("List", &())?)
}

/// Bring a window to the front and give it keyboard focus.
///
/// This is what Mutter refuses to do for a programmatic request, done from inside the
/// compositor instead: the extension calls `workspace.activate_with_focus()`. It is the correct
/// way to focus a window on GNOME Wayland — clicking at a guessed coordinate is not.
pub fn gnome_ext_activate(id: u64) -> bool {
    gnome_ext_call("Activate", &(id as u32)).is_some()
}

/// The window that currently holds focus, per the extension.
///
/// `List` does not carry a focus flag, so this asks `Details` for each window and returns the
/// one that reports focus. On a Wayland session this is the only way to answer "which window is
/// active" from outside the compositor.
pub fn gnome_ext_focused() -> Option<GnomeExtWindow> {
    for w in gnome_ext_windows()? {
        let Some(json) = gnome_ext_call("Details", &(w.id as u32)) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
            continue;
        };
        if v.get("focus").and_then(|f| f.as_bool()).unwrap_or(false) {
            return Some(w);
        }
    }
    None
}

/// The window under a point, using the extension's geometry.
///
/// Returns None when the extension is absent, or when the point is over no window. Callers must
/// not treat None as "no window exists" — it means "not knowable here".
pub fn gnome_ext_window_at(x: i32, y: i32) -> Option<GnomeExtWindow> {
    hit_test(gnome_ext_windows()?, x, y)
}

/// Smallest window containing the point.
///
/// Smallest, not first: a maximised window and a small dialog both contain the point, and the
/// dialog is the one the pointer is actually over.
fn hit_test(windows: Vec<GnomeExtWindow>, x: i32, y: i32) -> Option<GnomeExtWindow> {
    windows
        .into_iter()
        .filter(|w| {
            x >= w.x && x < w.x + w.width as i32 && y >= w.y && y < w.y + w.height as i32
        })
        .min_by_key(|w| w.width as u64 * w.height as u64)
}

/// Parse the JSON that the extension's `List` method returns.
///
/// Split out from the D-Bus call so the parsing is testable without the extension installed —
/// which matters, because the extension lags GNOME Shell versions and cannot always be loaded
/// on the machine under test.
fn parse_ext_windows(json: &str) -> Option<Vec<GnomeExtWindow>> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = parsed.as_array()?;
    Some(
        arr.iter()
            .filter_map(|w| {
                Some(GnomeExtWindow {
                    id: w.get("id")?.as_u64()?,
                    class: w.get("wm_class").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                    x: w.get("x")?.as_i64()? as i32,
                    y: w.get("y")?.as_i64()? as i32,
                    width: w.get("width")?.as_u64()? as u32,
                    height: w.get("height")?.as_u64()? as u32,
                })
            })
            .collect(),
    )
}

/// Capture the X11 root window, or a region of it, in-process.
///
/// GetImage returns the raw framebuffer in the server's own pixel layout: BGRX at 32bpp on
/// every little-endian server in practice, BGR at 24bpp on older visuals. The bytes are NOT
/// RGBA, and getting the swap wrong is the classic "screenshot came out blue" bug — so the
/// stride decides the layout instead of being assumed.
fn x11_capture_png(region: Option<(i32, i32, i32, i32)>) -> Result<Vec<u8>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

    let (conn, screen_num) = x11_connect().ok_or_else(|| anyhow!("cannot open the X11 display"))?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let (sw, sh) = (screen.width_in_pixels as i32, screen.height_in_pixels as i32);

    let (cx, cy, cw, ch) = match region {
        Some((x, y, w, h)) => {
            let cx = x.clamp(0, sw.saturating_sub(1));
            let cy = y.clamp(0, sh.saturating_sub(1));
            ((cx) as i16, (cy) as i16, w.min(sw - cx) as u16, h.min(sh - cy) as u16)
        }
        None => (0, 0, sw as u16, sh as u16),
    };
    if cw == 0 || ch == 0 {
        bail!("empty capture region on a {sw}x{sh} display");
    }

    let img = conn
        .get_image(ImageFormat::Z_PIXMAP, root, cx, cy, cw, ch, !0)
        .context("GetImage on the root window")?
        .reply()
        .context("waiting for the GetImage reply")?;

    let px = (cw as usize) * (ch as usize);
    let rgba = x11_pixels_to_rgba(&img.data, px)?;

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, cw as u32, ch as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().context("PNG header for the X11 capture")?;
        writer
            .write_image_data(&rgba)
            .context("writing X11 capture pixels")?;
    }
    Ok(out)
}

/// The window under the pointer, with its geometry and name, on ONE X11 connection.
fn x11_window_at_pointer() -> Option<serde_json::Value> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    let (conn, screen_num) = x11_connect()?;
    let root = conn.setup().roots[screen_num].root;
    let r = conn.query_pointer(root).ok()?.reply().ok()?;

    let mut win = r.child;
    for _ in 0..32 {
        if win == x11rb::NONE {
            break;
        }
        match conn.query_pointer(win).ok().and_then(|c| c.reply().ok()) {
            Some(q) if q.child != x11rb::NONE => win = q.child,
            _ => break,
        }
    }
    // Pointer over the desktop itself has no child window; that is a real answer, not a miss.
    if win == x11rb::NONE {
        return None;
    }

    let geom = conn.get_geometry(win).ok().and_then(|c| c.reply().ok());
    let name = conn
        .get_property(false, win, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 256)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|p| String::from_utf8_lossy(&p.value).to_string())
        .unwrap_or_default();

    // Same field layout `xdotool getwindowgeometry --shell` printed, so the JSON is unchanged.
    // Note: X11 geometry here is relative to the parent window, where xdotool reported the
    // absolute origin. The id, name and pointer position are identical.
    let geometry = match geom {
        Some(g) => format!(
            "WINDOW={win}\nX={}\nY={}\nWIDTH={}\nHEIGHT={}\nSCREEN=0",
            g.x, g.y, g.width, g.height
        ),
        None => String::new(),
    };

    Some(serde_json::json!({
        "id": win.to_string(),
        "name": name,
        "x": r.root_x.to_string(),
        "y": r.root_y.to_string(),
        "geometry": geometry,
    }))
}

/// X11's display scale, read from the X resource database.
///
/// RandR reports physical pixels and says nothing about scaling, so on a HiDPI X11 desktop
/// `displays()` would hand back the full panel resolution as if it were the logical size — a
/// 3840x2160 panel at 2x reported as 3840x2160 instead of 1920x1080.
///
/// Xft.dpi is where KDE and GNOME both record the factor on X11 (96 = 1.0, 192 = 2.0), and it
/// lives in the RESOURCE_MANAGER property on the root window. That is readable over the
/// protocol, so this stays in-process rather than shelling out to `xrdb`.
fn x11_scale(conn: &impl x11rb::connection::Connection, root: u32) -> f64 {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    let Ok(cookie) = conn.get_property(
        false,
        root,
        AtomEnum::RESOURCE_MANAGER,
        AtomEnum::STRING,
        0,
        1 << 16,
    ) else {
        return 1.0;
    };
    let Ok(reply) = cookie.reply() else {
        return 1.0;
    };

    for line in String::from_utf8_lossy(&reply.value).lines() {
        if let Some(rest) = line.trim().strip_prefix("Xft.dpi:") {
            if let Ok(dpi) = rest.trim().parse::<f64>() {
                if dpi > 0.0 {
                    // two decimals, so 191.99 reads as 2.0 rather than 1.9999
                    return (dpi / 96.0 * 100.0).round() / 100.0;
                }
            }
        }
    }
    1.0
}

/// Connected, enabled outputs and their rectangles, straight from RandR.
///
/// Same shape as `x11_display_state` (which parses `xrandr --query`), so the callers cannot
/// tell which one ran — that is the point of returning the shared type.
fn x11_display_state_rb() -> Result<Vec<(LinuxDisplay, (i32, i32, i32, i32))>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;

    let (conn, screen_num) = x11_connect().ok_or_else(|| anyhow!("cannot open the X11 display"))?;
    let root = conn.setup().roots[screen_num].root;

    let res = conn
        .randr_get_screen_resources_current(root)
        .context("randr GetScreenResources")?
        .reply()
        .context("randr reply")?;

    let scale = x11_scale(&conn, root);
    let mut out = Vec::new();
    for &crtc in res.crtcs.iter() {
        let Some(info) = conn
            .randr_get_crtc_info(crtc, res.config_timestamp)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            continue;
        };
        if info.width == 0 || info.height == 0 {
            continue; // crtc exists but drives nothing
        }

        // The output attached to this crtc carries the human-readable name.
        let mut name = String::new();
        for &output in res.outputs.iter() {
            let Some(oi) = conn
                .randr_get_output_info(output, res.config_timestamp)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                continue;
            };
            if oi.crtc == crtc {
                name = String::from_utf8_lossy(&oi.name).to_string();
                break;
            }
        }
        if name.is_empty() {
            name = format!("crtc{crtc}");
        }

        out.push((
            LinuxDisplay {
                id: crtc,
                name,
                width: info.width as u32,
                height: info.height as u32,
                // X11 gives physical pixels; the desktop's scale lives in Xft.dpi.
                scale,
            },
            (info.x as i32, info.y as i32, info.width as i32, info.height as i32),
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// File pickers
// ---------------------------------------------------------------------------

/// Turn a `file://` URI from the portal into a filesystem path.
///
/// The portal returns URIs, not paths, and they are percent-encoded: a space arrives as
/// `%20` and a non-ASCII name arrives as a run of `%XX`. Handing the raw string to `Path`
/// produces a path that does not exist, so the decoding is not optional.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// Pick files or folders through the XDG desktop portal.
///
/// This is the one dialog path that behaves identically on KDE, GNOME and wlroots, because all
/// three implement `org.freedesktop.portal.FileChooser`. Using it removes `kdialog` and
/// `zenity` from the dependency surface in one move, which is worth more than either
/// per-desktop tool it replaces: a Rust-native dialog would drag GTK or Qt into the WebKit
/// application, and a subprocess pins us to whichever toolkit that desktop happens to ship.
///
/// The request is asynchronous, like the screenshot one, but with an extra trap that the
/// screenshot path does not have to care about. The request object lives at
/// `/org/freedesktop/portal/desktop/request/<sender>/<handle_token>`, where `<sender>` is our
/// own unique bus name and `<handle_token>` is a token *we* supply. That predictability is
/// what makes it possible to subscribe to `Response` **before** making the call. Subscribing
/// afterwards — which is what `portal_capture_png` does — only survives because a human has to
/// click something first; a portal that answers immediately would race and the reply would be
/// lost. Here the token is derived from the pid and the subscription is established first.
/// Read a portal result that may be an array of strings or a single string.
///
/// `uris` is declared `as` in the FileChooser spec, but `Value::downcast_ref::<Vec<String>>`
/// is not implemented, and being permissive here costs nothing and survives a portal that
/// answers with a bare string.
fn portal_strings(v: &zbus::zvariant::OwnedValue) -> Vec<String> {
    match &**v {
        zbus::zvariant::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| match item {
                zbus::zvariant::Value::Str(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        zbus::zvariant::Value::Str(s) => vec![s.to_string()],
        _ => Vec::new(),
    }
}

fn portal_pick_uris(directory: bool, multiple: bool, title: &str) -> Result<Vec<PathBuf>> {
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    let conn = zbus::blocking::Connection::session()
        .context("no session bus for the desktop portal")?;

    let token = format!("quixi{}", std::process::id());
    let sender = conn
        .unique_name()
        .map(|n| n.to_string())
        .unwrap_or_default()
        .trim_start_matches(':')
        .replace('.', "_");
    let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");

    let (tx, rx) = std::sync::mpsc::channel::<(u32, Vec<String>)>();
    {
        let conn_rx = conn.clone();
        let path = request_path.clone();
        std::thread::spawn(move || {
            let Ok(req) = zbus::blocking::Proxy::new(
                &conn_rx,
                "org.freedesktop.portal.Desktop",
                path.as_str(),
                "org.freedesktop.portal.Request",
            ) else {
                return;
            };
            let Ok(mut signals) = req.receive_signal("Response") else {
                return;
            };
            for msg in signals.by_ref() {
                let Ok(body) = msg
                    .body()
                    .deserialize::<(u32, HashMap<String, OwnedValue>)>()
                else {
                    continue;
                };
                let uris = body
                    .1
                    .get("uris")
                    .map(portal_strings)
                    .unwrap_or_default();
                let _ = tx.send((body.0, uris));
                return;
            }
        });
    }
    // Let the match rule reach the bus before a call can possibly produce an answer.
    std::thread::sleep(std::time::Duration::from_millis(80));

    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .context("binding org.freedesktop.portal.FileChooser")?;

    let mut options: HashMap<String, Value> = HashMap::new();
    options.insert("handle_token".into(), Value::from(token.as_str()));
    options.insert("multiple".into(), Value::from(multiple));
    // In the portal interface "directory" belongs to OpenFile and means "pick folders".
    options.insert("directory".into(), Value::from(directory));
    options.insert("modal".into(), Value::from(true));

    let _handle: zbus::zvariant::OwnedObjectPath = proxy
        .call("OpenFile", &("", title, options))
        .context("portal refused the OpenFile request")?;

    let (code, uris) = rx.recv_timeout(portal_timeout()).map_err(|_| {
        anyhow!(
            "the file chooser did not answer within {}s",
            portal_timeout().as_secs()
        )
    })?;

    match code {
        0 => {}
        // A cancelled dialog is a normal outcome, not a failure. Returning empty is what the
        // UI already understands, and `bail!` here would send every cancellation down the
        // fallback path and pop a second dialog.
        1 => return Ok(Vec::new()),
        other => bail!("portal file chooser returned failure code {other}"),
    }

    Ok(uris.iter().filter_map(|u| uri_to_path(u)).collect())
}

/// Pick one or more files. Portal first, then whichever dialog tool the desktop ships.
pub fn pick_files(prompt: Option<&str>) -> Vec<String> {
    let title = prompt.filter(|s| !s.trim().is_empty()).unwrap_or("Select files");

    if portal_available() {
        if let Ok(paths) = portal_pick_uris(false, true, title) {
            // Empty means the user cancelled at the portal; do not fall through, or a second
            // dialog appears after they already said no.
            return paths.iter().map(|p| p.display().to_string()).collect();
        }
    }

    if have("kdialog") {
        if let Some(out) = run_one(
            "kdialog",
            &["--getopenfilename", "--multiple", "--separate-output"],
            Some(title),
        ) {
            let v: Vec<String> = out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if !v.is_empty() {
                return v;
            }
        }
    }
    if have("zenity") {
        if let Some(out) = run_one(
            "zenity",
            &["--file-selection", "--multiple", "--separator=\n"],
            Some(title),
        ) {
            let v: Vec<String> = out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if !v.is_empty() {
                return v;
            }
        }
    }
    Vec::new()
}

/// Pick one or more folders. Portal first, then the per-desktop tools.
pub fn pick_folders(prompt: Option<&str>) -> Vec<String> {
    let title = prompt.filter(|s| !s.trim().is_empty()).unwrap_or("Select folders");

    if portal_available() {
        if let Ok(paths) = portal_pick_uris(true, true, title) {
            return paths.iter().map(|p| p.display().to_string()).collect();
        }
    }

    if have("kdialog") {
        if let Some(out) = run_one(
            "kdialog",
            &["--getexistingdirectory", "--multiple", "--separate-output"],
            Some(title),
        ) {
            let v: Vec<String> = out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if !v.is_empty() {
                return v;
            }
        }
    }
    if have("zenity") {
        if let Some(out) = run_one(
            "zenity",
            &["--file-selection", "--directory", "--multiple", "--separator=\n"],
            Some(title),
        ) {
            let v: Vec<String> = out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if !v.is_empty() {
                return v;
            }
        }
    }
    Vec::new()
}

pub fn pick_folder(prompt: Option<&str>) -> Option<String> {
    if have("kdialog") {
        if let Some(p) = kdialog_pick(&["--getexistingdirectory"], prompt) {
            return Some(p);
        }
        return None;
    }
    if have("zenity") {
        return zenity_pick(&["--file-selection", "--directory"], prompt);
    }
    None
}

pub fn pick_file(prompt: Option<&str>) -> Option<String> {
    if have("kdialog") {
        if let Some(p) = kdialog_pick(&["--getopenfilename"], prompt) {
            return Some(p);
        }
        return None;
    }
    if have("zenity") {
        return zenity_pick(&["--file-selection"], prompt);
    }
    None
}

// ---------------------------------------------------------------------------
// Applications
// ---------------------------------------------------------------------------

/// Where `.desktop` entries live, per the XDG Base Directory spec.
///
/// The previous version hardcoded `/usr/share/applications` and
/// `/usr/local/share/applications` and ignored `XDG_DATA_DIRS` entirely. That is
/// the standard search path, and it is where NixOS, Flatpak
/// (`/var/lib/flatpak/exports/share`) and any custom-prefix install puts its
/// applications — so on those systems the list came back empty or wrong. Found by
/// a test that installed an entry into a throwaway data dir and watched the lookup
/// miss it.
fn desktop_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !roots.contains(&p) && p.is_dir() {
            roots.push(p);
        }
    };

    match std::env::var_os("XDG_DATA_HOME") {
        Some(h) if !h.is_empty() => push(PathBuf::from(h).join("applications")),
        _ => {
            if let Some(home) = dirs::home_dir() {
                push(home.join(".local/share/applications"));
            }
        }
    }

    match std::env::var_os("XDG_DATA_DIRS") {
        Some(dirs) if !dirs.is_empty() => {
            for d in std::env::split_paths(&dirs) {
                if !d.as_os_str().is_empty() {
                    push(d.join("applications"));
                }
            }
        }
        _ => {
            push(PathBuf::from("/usr/local/share/applications"));
            push(PathBuf::from("/usr/share/applications"));
        }
    }

    // Flatpak's per-user and system exports are not always in XDG_DATA_DIRS.
    if let Some(home) = dirs::home_dir() {
        push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    roots
}

fn parse_desktop(path: &Path) -> Option<(String, String, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut hidden = false;
    let mut in_entry = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("NoDisplay=") {
            hidden = rest.trim().eq_ignore_ascii_case("true");
        } else if let Some(rest) = line.strip_prefix("Hidden=") {
            hidden = hidden || rest.trim().eq_ignore_ascii_case("true");
        }
    }

    match (name, exec) {
        (Some(n), Some(e)) if !hidden && !n.is_empty() && !e.is_empty() => Some((n, e, true)),
        _ => None,
    }
}

/// Installed GUI applications as `{name, exec, desktop_file}`.
pub fn list_installed_apps() -> serde_json::Value {
    let mut apps: Vec<serde_json::Value> = Vec::new();
    for root in desktop_roots() {
        let entries = match std::fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some((name, exec, _)) = parse_desktop(&path) {
                apps.push(serde_json::json!({
                    "name": name,
                    "exec": exec,
                    "desktop_file": path.to_string_lossy(),
                }));
            }
        }
    }
    apps.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
    });
    apps.dedup_by(|a, b| a["name"] == b["name"]);
    serde_json::json!({ "applications": apps })
}

/// Split a `.desktop` `Exec` value into argv, then strip field codes.
///
/// The Desktop Entry spec allows double quotes and backslash escapes, and reserves
/// `%` codes that the launcher is expected to expand (`%U`, `%F`, `%f`, `%i`, `%c`,
/// `%k`, …). Launching with no arguments means those codes expand to nothing, so
/// they are removed; `%%` is a literal percent sign.
///
/// Dropping a `%i`/`%c`/`%k` is correct here rather than lazy: they expand to the
/// icon name, the translated app name and the desktop file path, which only matter
/// to the launched program and are optional per the spec.
fn exec_argv(exec: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            '\\' if in_quotes => {
                // Inside quotes, backslash escapes the next character.
                if let Some(n) = chars.next() {
                    cur.push(n);
                    has_token = true;
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            _ => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        out.push(cur);
    }

    // Field codes. An argument that CONTAINS one and has nothing to substitute is
    // dropped whole, not partially rewritten: "-‑user-data-dir=%d" becoming
    // "--user-data-dir=" is a dangling flag that means something different to the
    // target program, which is worse than not passing it at all. `%%` is preserved
    // as a literal percent sign and does not count as a field code.
    out.into_iter()
        .filter_map(|arg| {
            let mut s = String::with_capacity(arg.len());
            let mut it = arg.chars();
            let mut had_field_code = false;
            while let Some(c) = it.next() {
                if c == '%' {
                    match it.next() {
                        Some('%') => s.push('%'),
                        Some(_) => had_field_code = true,
                        None => had_field_code = true,
                    }
                } else {
                    s.push(c);
                }
            }
            if had_field_code {
                None
            } else if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .collect()
}

/// Start a process and walk away from it.
///
/// `spawn` rather than `output`: a launcher must not block until the application
/// exits. Streams are nulled so the child cannot inherit and hold our pipes, which
/// is the same defect that made `wl-copy` hang for 32 minutes.
fn spawn_detached(argv: &[String]) -> Result<()> {
    let (prog, args) = argv.split_first().ok_or_else(|| anyhow!("empty command line"))?;
    std::process::Command::new(prog)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start {prog}"))?;
    Ok(())
}

/// Launches an installed application by display name or desktop-file stem.
///
/// Spawns the parsed `Exec` line directly. The gtk-launch / gio / kioclient6 paths
/// remain as fallbacks for the case where the binary is not on our PATH but a
/// desktop launcher still knows how to find it.
pub fn open_application(app_name: &str) -> Result<String> {
    let wanted = app_name.trim().to_lowercase();
    if wanted.is_empty() {
        bail!("no application name given");
    }

    for root in desktop_roots() {
        let entries = match std::fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let parsed = match parse_desktop(&path) {
                Some(p) => p,
                None => continue,
            };
            let (name, exec, _) = parsed;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if name.to_lowercase() == wanted || stem == wanted {
                // Spawn the Exec line ourselves. We already parsed it, so going out
                // to gtk-launch/kioclient6/gio was a round trip we invented: three
                // optional dependencies and one process spawn to do what Command
                // does directly.
                let argv = exec_argv(&exec);
                if !argv.is_empty() {
                    match spawn_detached(&argv) {
                        Ok(()) => return Ok(format!("launched {name}: {}", argv.join(" "))),
                        Err(e) => {
                            // Fall through to a launcher if the binary is missing from
                            // PATH but a desktop launcher might know where it lives.
                            let _ = e;
                        }
                    }
                }
                let file = path.to_string_lossy().to_string();
                if have("gtk-launch") {
                    if let Ok(o) = run("gtk-launch", &[&stem]) {
                        if o.status.success() {
                            return Ok(format!("launched {name} via gtk-launch"));
                        }
                    }
                }
                if have("gio") {
                    let out = run("gio", &["launch", &file])?;
                    if out.status.success() {
                        return Ok(format!("launched {name} via gio"));
                    }
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    bail!("gio launch failed for {name}: {err}");
                }
                if have("kioclient6") {
                    run_ok("kioclient6", &["exec", &file])?;
                    return Ok(format!("launched {name} via kioclient6"));
                }
                bail!("could not launch {name}: Exec was empty and no launcher is installed");
            }
        }
    }
    bail!("application not found: {app_name}")
}

// ---------------------------------------------------------------------------
// X11-only window queries
// ---------------------------------------------------------------------------

/// Best-effort window information for XWayland clients. Returns `None` on a
/// pure Wayland session because no client-visible window list exists.
/// The window under the pointer, on X11.
///
/// The old version called `getactivewindow`, which is the FOCUSED window — a different thing
/// entirely, and wrong for a function with this name. `xdotool getmouselocation --shell`
/// reports the window actually under the pointer as `WINDOW=<id>`, which is what was asked
/// for. Focus and pointer position diverge constantly: hovering a panel, a tooltip, or a
/// background window changes one without the other.
pub fn x11_window_under_cursor() -> Option<serde_json::Value> {
    // On Wayland the only truthful source is the window-calls extension, when it is installed.
    // XQueryPointer there reports the mutter guard window, which is not an application window
    // and is actively misleading — better to report the focused window or nothing.
    if is_wayland() && gnome_ext_available() {
        if let Some(w) = gnome_ext_focused() {
            return Some(serde_json::json!({
                "id": w.id.to_string(),
                "name": w.class,
                "x": w.x.to_string(),
                "y": w.y.to_string(),
                "geometry": format!(
                    "WINDOW={}\nX={}\nY={}\nWIDTH={}\nHEIGHT={}\nSCREEN=0",
                    w.id, w.x, w.y, w.width, w.height
                ),
                "source": "window-calls-extension",
            }));
        }
    }
    if is_wayland() {
        // No client API for this, and the XWayland answer is a lie on a Wayland session.
        return None;
    }
    if let Some(v) = x11_window_at_pointer() {
        return Some(v);
    }
    if !have("xdotool") {
        return None;
    }
    // The window under the pointer only exists as a concept on X11. On Wayland there is no
    // client API for it, so this returns None there — that is a platform limit, not a bug.
    if is_wayland() && std::env::var_os("DISPLAY").is_none() {
        return None;
    }

    let out = run("xdotool", &["getmouselocation", "--shell"]).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut id = String::new();
    let mut x = String::new();
    let mut y = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("WINDOW=") {
            id = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("X=") {
            x = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Y=") {
            y = v.trim().to_string();
        }
    }
    if id.is_empty() {
        return None;
    }

    let name = run("xdotool", &["getwindowname", &id])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let geom = run("xdotool", &["getwindowgeometry", "--shell", &id])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    Some(serde_json::json!({
        "id": id,
        "name": name,
        "x": x,
        "y": y,
        "geometry": geom,
    }))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycodes_map_expected_combos() {
        // Numeric evdev codes, not names: ydotool accepted `KEY_ENTER:1` and
        // delivered nothing, while `28:1` always lands.
        assert_eq!(keycode_for("return").unwrap(), "28:1 28:0");
        assert_eq!(keycode_for("ctrl+c").unwrap(), "29:1 46:1 46:0 29:0");
        assert_eq!(
            keycode_for("cmd+shift+tab").unwrap(),
            "125:1 42:1 15:1 15:0 42:0 125:0"
        );
        // Shifted characters carry their own shift, so "shift+/" must not double it.
        assert_eq!(keycode_for("ctrl+shift+tab").unwrap(), "29:1 42:1 15:1 15:0 42:0 29:0");
        assert_eq!(keycode_for("?").unwrap(), "42:1 53:1 53:0 42:0");
        assert!(keycode_for("nonsense-key").is_none());
    }

    #[test]
    fn uri_to_path_decodes_percent_escapes() {
        // The portal percent-encodes; a path with a space arrives as %20 and must decode or
        // the returned path does not exist on disk.
        assert_eq!(
            uri_to_path("file:///home/user/My%20File.txt"),
            Some(PathBuf::from("/home/user/My File.txt"))
        );
        assert_eq!(
            uri_to_path("file:///tmp/a%2Bb"),
            Some(PathBuf::from("/tmp/a+b"))
        );
        // non-ASCII: %C3%A9 is e-acute
        assert_eq!(
            uri_to_path("file:///tmp/caf%C3%A9.txt"),
            Some(PathBuf::from("/tmp/café.txt"))
        );
    }

    #[test]
    fn uri_to_path_rejects_non_file_uris() {
        // http or a bare path must not be silently turned into a relative path.
        assert_eq!(uri_to_path("https://example.com/x"), None);
        assert_eq!(uri_to_path("/home/user/x"), None);
    }

    #[test]
    fn uri_to_path_leaves_a_bare_percent_alone() {
        // A trailing '%' or a '%' not followed by two hex digits is literal, not an error.
        assert_eq!(uri_to_path("file:///tmp/100%"), Some(PathBuf::from("/tmp/100%")));
        assert_eq!(uri_to_path("file:///tmp/a%zz"), Some(PathBuf::from("/tmp/a%zz")));
    }

    #[test]
    fn dialog_timeout_is_generous_but_bounded() {
        // A picker waits on a human, so it must outlast the 20s tool timeout, and it must
        // still be finite or a missing dialog hangs the caller forever.
        let d = dialog_timeout();
        assert!(d > TOOL_TIMEOUT, "dialog must outlast the tool timeout");
        assert!(d <= std::time::Duration::from_secs(3600), "must stay bounded");
    }
    // ---- X11 protocol tests (x11rb) -------------------------------------------------
    // These need a real X server, so they are gated; run them against Xvfb with DISPLAY set
    // and QUIXI_X11_TEST=1. A Wayland-only box skips them rather than failing the suite.

    fn x11_test_enabled() -> bool {
        std::env::var("QUIXI_X11_TEST").ok().as_deref() == Some("1")
    }

    #[test]
    fn x11_pointer_reads_a_real_server() {
        if !x11_test_enabled() { return; }
        let (x, y, _win) = x11_pointer().expect("QueryPointer on the test server");
        assert!(x >= 0 && y >= 0, "pointer read as ({x},{y})");
    }

    #[test]
    fn x11_capture_returns_a_real_png() {
        if !x11_test_enabled() { return; }
        let png = x11_capture_png(None).expect("GetImage on the test server");
        assert!(png.starts_with(b"\x89PNG"), "not a PNG");
        // Decoding catches the failure a magic-number check misses: wrong GetImage region
        // gives wrong dimensions, wrong stride gives a corrupt or shifted image.
        let dec = png::Decoder::new(std::io::Cursor::new(&png));
        let reader = dec.read_info().expect("decoding the captured PNG");
        assert_eq!(reader.info().width, 1280, "captured width");
        assert_eq!(reader.info().height, 800, "captured height");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    }

    #[test]
    fn x11_capture_honours_a_region() {
        if !x11_test_enabled() { return; }
        let png = x11_capture_png(Some((10, 20, 64, 48))).expect("region GetImage");
        let dec = png::Decoder::new(std::io::Cursor::new(&png));
        let reader = dec.read_info().expect("decoding the region PNG");
        assert_eq!((reader.info().width, reader.info().height), (64, 48));
    }

    #[test]
    fn x11_capture_clamps_a_region_past_the_edge() {
        if !x11_test_enabled() { return; }
        // Asking beyond the screen must clamp, not error and not wrap around.
        let png = x11_capture_png(Some((1270, 790, 200, 200))).expect("clamped region");
        let dec = png::Decoder::new(std::io::Cursor::new(&png));
        let reader = dec.read_info().expect("decoding the clamped PNG");
        assert_eq!((reader.info().width, reader.info().height), (10, 10));
    }

    #[test]
    fn x11_displays_lists_the_test_output() {
        if !x11_test_enabled() { return; }
        let st = x11_display_state_rb().expect("RandR on the test server");
        assert!(!st.is_empty(), "no outputs reported");
        let (d, _rect) = &st[0];
        assert_eq!((d.width, d.height), (1280, 800), "output {} size", d.name);
        assert!(d.name.to_lowercase().contains("screen"), "name was {:?}", d.name);
    }

    #[test]
    fn x11_window_at_pointer_terminates_on_a_bare_root() {
        if !x11_test_enabled() { return; }
        // No WM on Xvfb: the honest answer is None. This proves the descend loop terminates
        // instead of spinning on a child that never reaches NONE.
        let _ = x11_window_at_pointer();
    }

    #[test]
    fn pixel_unpack_32bpp_bgrx_is_not_rgba() {
        // Pure red is B=0, G=0, R=255, X=0 in the server buffer. Reading it as RGBA gives
        // blue -- this test is the one that would have caught the classic swapped-channel bug.
        let data = [0u8, 0, 255, 0];
        let out = x11_pixels_to_rgba(&data, 1).unwrap();
        assert_eq!(out, vec![255, 0, 0, 255], "32bpp BGRX -> RGBA red");
    }

    #[test]
    fn pixel_unpack_24bpp_bgr() {
        // The 3-byte branch: same channel order, no padding byte.
        let data = [0u8, 0, 255];
        let out = x11_pixels_to_rgba(&data, 1).unwrap();
        assert_eq!(out, vec![255, 0, 0, 255], "24bpp BGR -> RGBA red");
    }

    #[test]
    fn pixel_unpack_16bpp_rgb565() {
        // RGB565 little-endian. 0xF800 = red, 0x07E0 = green, 0x001F = blue.
        let red = x11_pixels_to_rgba(&[0x00, 0xF8], 1).unwrap();
        assert_eq!(red, vec![255, 0, 0, 255], "565 red");
        let green = x11_pixels_to_rgba(&[0xE0, 0x07], 1).unwrap();
        assert_eq!(green, vec![0, 255, 0, 255], "565 green");
        let blue = x11_pixels_to_rgba(&[0x1F, 0x00], 1).unwrap();
        assert_eq!(blue, vec![0, 0, 255, 255], "565 blue");
        // mid-grey is 0x8410: r=16 g=32 b=16 -> 132/130/132 after bit replication
        let grey = x11_pixels_to_rgba(&[0x10, 0x84], 1).unwrap();
        assert_eq!(grey, vec![132, 130, 132, 255], "565 mid grey");
    }

    #[test]
    fn pixel_unpack_preserves_order_across_pixels() {
        // Two pixels must not be transposed or interleaved wrongly.
        let data = [0u8, 0, 255, 0, 255, 0, 0, 0]; // red then blue at 32bpp
        let out = x11_pixels_to_rgba(&data, 2).unwrap();
        assert_eq!(out, vec![255, 0, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn pixel_unpack_rejects_unsupported_width() {
        // 8-bit PseudoColor would otherwise be read as garbage; it must be a named error.
        let err = x11_pixels_to_rgba(&[0u8; 4], 4).unwrap_err().to_string();
        assert!(err.contains("PseudoColor"), "error should name the cause: {err}");
        assert!(err.contains("unexpected GetImage size"));
    }

    #[test]
    fn x11_scale_follows_xft_dpi() {
        if !x11_test_enabled() { return; }
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};

        let (conn, screen_num) = x11_connect().expect("X11 connection");
        let root = conn.setup().roots[screen_num].root;

        let set = |text: &[u8]| {
            conn.change_property(
                PropMode::REPLACE,
                root,
                AtomEnum::RESOURCE_MANAGER,
                AtomEnum::STRING,
                8,
                text.len() as u32,
                text,
            )
            .expect("setting RESOURCE_MANAGER")
            .check()
            .expect("property write");
            conn.flush().expect("flush");
        };

        // 192 dpi is the 2x convention; the display must report 2.0, not the panel resolution.
        set(b"Xft.dpi:\t192\n");
        let st = x11_display_state_rb().expect("RandR");
        assert!(
            (st[0].0.scale - 2.0).abs() < 0.01,
            "expected scale 2.0 at 192dpi, got {}",
            st[0].0.scale
        );

        // 96 dpi is 1:1
        set(b"Xft.dpi:\t96\n");
        let st = x11_display_state_rb().expect("RandR");
        assert!(
            (st[0].0.scale - 1.0).abs() < 0.01,
            "expected scale 1.0 at 96dpi, got {}",
            st[0].0.scale
        );

        // A 1.5x desktop reports 144 dpi
        set(b"Xft.dpi:\t144\n");
        let st = x11_display_state_rb().expect("RandR");
        assert!(
            (st[0].0.scale - 1.5).abs() < 0.01,
            "expected scale 1.5 at 144dpi, got {}",
            st[0].0.scale
        );

        // No resource at all must degrade to 1.0, not to zero or NaN.
        conn.delete_property(root, AtomEnum::RESOURCE_MANAGER.into())
            .expect("delete")
            .check()
            .expect("delete check");
        conn.flush().expect("flush");
        let st = x11_display_state_rb().expect("RandR");
        assert!(
            (st[0].0.scale - 1.0).abs() < 0.01,
            "expected 1.0 with no Xft.dpi, got {}",
            st[0].0.scale
        );
    }

    #[test]
    fn x11_getimage_width_matches_the_server_depth() {
        if !x11_test_enabled() { return; }
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

        let (conn, screen_num) = x11_connect().expect("X11 connection");
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let depth = screen.root_depth;

        // depth 16 -> RGB565 (2 bytes), depth 24/30 -> BGRX (4 bytes).
        let expected_bpp: usize = match depth {
            16 => 2,
            24 | 30 => 4,
            15 => 2,
            other => panic!("unexpected root depth {other}; extend this test if it is real"),
        };

        let (w, h) = (64u16, 48u16);
        let img = conn
            .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, w, h, !0)
            .expect("GetImage")
            .reply()
            .expect("reply");

        let px = w as usize * h as usize;
        assert_eq!(
            img.data.len(),
            px * expected_bpp,
            "root depth {depth} should give {expected_bpp} bytes/pixel, got {} for {px} pixels",
            img.data.len()
        );

        // And the decoder must accept exactly that width and produce RGBA.
        let rgba = x11_pixels_to_rgba(&img.data, px).expect("decode at the server's own depth");
        assert_eq!(rgba.len(), px * 4);
        assert_eq!(rgba[3], 255, "alpha must be opaque");
    }

    // ---- window-calls extension: parsing and hit-testing ---------------------------
    // The extension itself only declares support up to GNOME 50 and the test machine runs 51,
    // so these cover the parts we own without needing it loaded.

    const EXT_SAMPLE: &str = r#"[
      {"wm_class":"Caprine","wm_class_instance":"caprine","pid":62901,"id":1610090767,
       "frame_type":0,"window_type":0,"width":1910,"height":2100,"x":10,"y":50,
       "in_current_workspace":true,"workspace":0,"monitor":0},
      {"wm_class":"ptyxis","pid":42,"id":99,"width":400,"height":300,"x":100,"y":100,
       "in_current_workspace":true,"workspace":0,"monitor":0}
    ]"#;

    #[test]
    fn ext_list_parses_the_documented_shape() {
        let w = parse_ext_windows(EXT_SAMPLE).expect("parses");
        assert_eq!(w.len(), 2, "both windows");
        assert_eq!(w[0].id, 1610090767);
        assert_eq!(w[0].class, "Caprine");
        assert_eq!((w[0].x, w[0].y, w[0].width, w[0].height), (10, 50, 1910, 2100));
        assert_eq!(w[1].class, "ptyxis");
    }

    #[test]
    fn ext_list_tolerates_bad_and_partial_data() {
        // Not JSON at all -> None, never a panic.
        assert!(parse_ext_windows("not json").is_none());
        // An entry missing geometry is dropped rather than defaulting to 0,0,0,0, which would
        // otherwise claim every point in the top-left corner belongs to it.
        let partial = r#"[{"id":1,"wm_class":"x"},{"id":2,"wm_class":"y","x":5,"y":6,"width":7,"height":8}]"#;
        let w = parse_ext_windows(partial).expect("parses");
        assert_eq!(w.len(), 1, "only the complete entry survives");
        assert_eq!(w[0].id, 2);
        // Missing wm_class must not fail the whole list.
        let no_class = r#"[{"id":3,"x":0,"y":0,"width":10,"height":10}]"#;
        let w = parse_ext_windows(no_class).expect("parses");
        assert_eq!(w[0].class, "", "absent class is empty, not a parse failure");
    }

    #[test]
    fn hit_test_picks_the_smallest_containing_window() {
        let windows = parse_ext_windows(EXT_SAMPLE).unwrap();
        // Inside both the big window (10..1920, 50..2150) and the small one (100..500, 100..400).
        let w = hit_test(windows, 200, 200).expect("a window contains 200,200");
        assert_eq!(w.class, "ptyxis", "the dialog on top wins, not the maximised window");
    }

    #[test]
    fn hit_test_edges_are_half_open() {
        let windows = parse_ext_windows(EXT_SAMPLE).unwrap();
        // The small window is 100..500 x 100..400: the top-left corner is inside, the
        // bottom-right edge is not. Getting this wrong makes adjacent windows overlap by a pixel.
        assert!(hit_test(windows.clone(), 100, 100).is_some(), "top-left inclusive");
        assert!(
            hit_test(windows, 500, 400).map(|w| w.id) != Some(99),
            "bottom-right exclusive"
        );
    }

    #[test]
    fn hit_test_outside_every_window_is_none() {
        let windows = parse_ext_windows(EXT_SAMPLE).unwrap();
        assert!(hit_test(windows, 5000, 5000).is_none());
    }

    #[test]
    fn char_map_covers_shell_commands() {
        // Every character the typing test sends must map, or type_text bails.
        for ch in "touch /tmp/quixi_input_proof".chars() {
            assert!(char_keycode(ch).is_some(), "unmapped char: {ch:?}");
        }
        assert_eq!(char_keycode('a'), Some((30, false)));
        assert_eq!(char_keycode('A'), Some((30, true)));
        assert_eq!(char_keycode('_'), Some((12, true)));
        assert_eq!(char_keycode('/'), Some((53, false)));
    }

    #[test]
    fn desktop_entry_parsing_skips_hidden() {
        let dir = std::env::temp_dir().join(format!("quixi-dt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let visible = dir.join("visible.desktop");
        std::fs::write(
            &visible,
            "[Desktop Entry]\nName=Visible App\nExec=visible --flag %U\nType=Application\n",
        )
        .unwrap();
        let hidden = dir.join("hidden.desktop");
        std::fs::write(
            &hidden,
            "[Desktop Entry]\nName=Hidden App\nExec=hidden\nNoDisplay=true\n",
        )
        .unwrap();
        assert!(parse_desktop(&visible).is_some());
        assert!(parse_desktop(&hidden).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exec_argv_handles_quotes_and_field_codes() {
        // Plain command with a field code that must vanish.
        assert_eq!(exec_argv("firefox %u"), vec!["firefox"]);
        // Quoted argument containing a space stays one argument.
        assert_eq!(
            exec_argv("sh -c \"echo hello world\""),
            vec!["sh", "-c", "echo hello world"]
        );
        // Multiple field codes all disappear.
        assert_eq!(
            exec_argv("code --new-window %F --user-data-dir=%d"),
            vec!["code", "--new-window"]
        );
        // `%%` is a literal percent sign, not a field code.
        assert_eq!(exec_argv("printf %%s"), vec!["printf", "%s"]);
        // Backslash escapes inside quotes.
        assert_eq!(exec_argv("sh -c \"echo \\\"hi\\\"\""), vec!["sh", "-c", "echo \"hi\""]);
        // Leading/trailing/duplicated whitespace collapses.
        assert_eq!(exec_argv("   ls   -l   "), vec!["ls", "-l"]);
        // An Exec that is only field codes leaves nothing to run.
        assert!(exec_argv("%U").is_empty());
        assert!(exec_argv("").is_empty());
    }

    #[test]
    fn capability_report_is_consistent() {
        let report = capabilities();
        if !report.screenshot {
            assert_eq!(report.screenshot_backend, "none");
        }
        // "none" is a real answer: a headless shell, a container, a server.
        // Session types now name the compositor where it is known, not just the protocol: "wayland"
        // alone does not tell a bug report where to look. Every value must still be a real
        // classification rather than an empty or inventable string.
        assert!(
            matches!(
                report.session_type.as_str(),
                "wayland" | "wayland (hyprland)" | "wayland (hyprland, omarchy)" | "x11" | "none"
            ),
            "unexpected session classification: {}",
            report.session_type
        );
    }
}

#[cfg(test)]
mod is_wayland_env_tests {
    //! An empty WAYLAND_DISPLAY must not select the Wayland code path.

    #[test]
    fn an_empty_wayland_display_is_not_wayland() {
        // Serialised against other tests that mutate the environment by only setting, never
        // asserting on, a value we restore immediately.
        let saved = std::env::var("WAYLAND_DISPLAY").ok();
        std::env::set_var("WAYLAND_DISPLAY", "");
        let empty_is_wayland = super::is_wayland();
        std::env::set_var("WAYLAND_DISPLAY", "   ");
        let blank_is_wayland = super::is_wayland();
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        let real_is_wayland = super::is_wayland();
        match saved {
            Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        assert!(!empty_is_wayland, "empty WAYLAND_DISPLAY must not mean Wayland");
        assert!(!blank_is_wayland, "whitespace WAYLAND_DISPLAY must not mean Wayland");
        assert!(real_is_wayland, "a real WAYLAND_DISPLAY must still mean Wayland");
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
//
// Every desktop stores the same information in a different place and a different notation, and an
// agent needs one shape it can reason about. Three sources, one normalized answer:
//
//   KDE    ~/.config/kglobalshortcutsrc        Ctrl+Alt+T
//   GNOME  gsettings arrays                    ['<Control><Alt>t']
//   XFCE   xfconf-query                        /commands/custom/<Alt><Super>s
//
// Normalised form is lower case, '+'-joined, modifiers first: "ctrl+alt+t", "super+escape".
// ---------------------------------------------------------------------------

/// One shortcut, in desktop-neutral terms.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct Keybinding {
    /// The action's own name: a command, a gsettings key, or an xfconf path.
    pub action: String,
    /// Normalised key combo. Empty when the action is currently unbound.
    pub keys: String,
    /// Human description where the desktop provides one.
    pub description: String,
    /// `kde`, `gnome` or `xfce`.
    pub source: String,
    /// True when this differs from the desktop's default binding.
    pub custom: bool,
}

const MODIFIER_ORDER: [&str; 5] = ["ctrl", "alt", "shift", "super", "meta"];

/// Fold any desktop's notation into `ctrl+alt+t`.
///
/// Handles KDE's `Ctrl+Alt+T`, GNOME/XFCE's `<Control><Alt>t` and `<Alt><Super>s`, bare keys, and
/// X11-ish spellings (`control`, `win`, `cmd`, `mod4`). Order is canonical so two desktops binding
/// the same physical chord produce the same string and can be compared.
pub fn normalize_keys(raw: &str) -> String {
    let mut mods: Vec<String> = Vec::new();
    let mut key = String::new();

    // Split on '+' but keep angle-bracket groups intact: "<Control><Alt>t" has no separators.
    let mut tokens: Vec<String> = Vec::new();
    if raw.contains('<') {
        let mut rest = raw.trim().to_string();
        while let Some(start) = rest.find('<') {
            if let Some(end) = rest[start..].find('>') {
                tokens.push(rest[start + 1..start + end].to_string());
                rest = rest[end + 1..].to_string();
            } else {
                break;
            }
        }
        if !rest.trim().is_empty() {
            tokens.push(rest.trim().to_string());
        }
    } else {
        tokens = raw.split('+').map(|s| s.trim().to_string()).collect();
    }

    for t in tokens {
        let low = t.to_ascii_lowercase();
        match low.as_str() {
            "" => {}
            "ctrl" | "control" | "primary" => mods.push("ctrl".into()),
            "alt" | "meta" | "mod1" => mods.push("alt".into()),
            "shift" => mods.push("shift".into()),
            "super" | "win" | "cmd" | "command" | "mod4" | "hyper" => mods.push("super".into()),
            "none" => {}
            _ => key = low,
        }
    }

    mods.sort_by_key(|m| MODIFIER_ORDER.iter().position(|o| o == m).unwrap_or(99));
    mods.dedup();
    if key.is_empty() {
        mods.join("+")
    } else if mods.is_empty() {
        key
    } else {
        format!("{}+{}", mods.join("+"), key)
    }
}

#[cfg(test)]
mod keybinding_tests {
    use super::normalize_keys;

    #[test]
    fn kde_gnome_and_xfce_notations_normalise_to_the_same_string() {
        // Ctrl+Alt+T written three ways by three desktops.
        assert_eq!(normalize_keys("Ctrl+Alt+T"), "ctrl+alt+t");
        assert_eq!(normalize_keys("<Control><Alt>t"), "ctrl+alt+t");
        assert_eq!(normalize_keys("ctrl+alt+t"), "ctrl+alt+t");
    }

    #[test]
    fn modifier_order_is_canonical_regardless_of_input_order() {
        // XFCE writes modifier groups in whatever order the user pressed them.
        assert_eq!(normalize_keys("<Alt><Super>s"), "alt+super+s");
        assert_eq!(normalize_keys("Super+Alt+s"), "alt+super+s");
        assert_eq!(normalize_keys("<Super><Alt>s"), "alt+super+s");
    }

    #[test]
    fn alternative_names_for_the_same_modifier_collapse() {
        assert_eq!(normalize_keys("<Control>t"), "ctrl+t");
        assert_eq!(normalize_keys("Control+t"), "ctrl+t");
        assert_eq!(normalize_keys("<Primary>t"), "ctrl+t");
        assert_eq!(normalize_keys("<Super>t"), "super+t");
        assert_eq!(normalize_keys("<Win>t"), "super+t");
        assert_eq!(normalize_keys("<Mod4>t"), "super+t");
    }

    #[test]
    fn unbound_and_bare_keys_are_handled() {
        assert_eq!(normalize_keys("none"), "");
        assert_eq!(normalize_keys(""), "");
        assert_eq!(normalize_keys("Escape"), "escape");
        assert_eq!(normalize_keys("<Super>"), "super");
    }
}

/// KDE: `~/.config/kglobalshortcutsrc`, INI with `action=shortcut,alternate,description` lines.
fn keybindings_kde() -> Vec<Keybinding> {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::PathBuf::from(home).join(".config/kglobalshortcutsrc");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut component = String::new();
    let mut friendly = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            component = line[1..line.len() - 1].to_string();
            friendly.clear();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "_k_friendly_name" {
            friendly = value.to_string();
            continue;
        }
        // Active shortcut, alternate, then a human description.
        let parts: Vec<&str> = value.splitn(3, ',').collect();
        let active = parts.first().copied().unwrap_or("none");
        let description = parts.get(2).copied().unwrap_or("").to_string();
        out.push(Keybinding {
            action: if component.is_empty() {
                key.to_string()
            } else {
                format!("{component}/{key}")
            },
            keys: normalize_keys(active),
            description: if description.is_empty() {
                if friendly.is_empty() { key.to_string() } else { friendly.clone() }
            } else {
                description
            },
            source: "kde".into(),
            custom: false,
        });
    }
    out
}

/// GNOME: ONE `gsettings list-recursively` call for every schema at once.
///
/// This used to spawn a `gsettings` process per binding — 25 processes on a listing an agent may call
/// in a loop. The bulk listing returns the whole tree in a single spawn, and it also covers schemas and
/// keys a hardcoded list would never include.
fn keybindings_gnome() -> Vec<Keybinding> {
    let Ok(o) = run("gsettings", &["list-recursively"]) else {
        return Vec::new();
    };
    if !o.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&o.stdout);
    let mut out = Vec::new();

    for line in text.lines() {
        // "<schema> <key> <value>"; the value may contain spaces, the first two never do.
        let mut it = line.splitn(3, ' ');
        let (Some(schema), Some(key), Some(value)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let interesting = schema.contains("keybindings") || schema.contains("media-keys");
        if !interesting || key == "custom-keybindings" {
            continue;
        }
        // Only array-valued settings are keybindings. The media-keys schema also holds scalars such
        // as volume-step = 6, which are not shortcuts and read as a nonsense "6" binding otherwise.
        let looks_like_array = value.trim_start().starts_with('[') || value.trim_start().starts_with("@as");
        if !looks_like_array {
            continue;
        }
        // ['<Super>t']  |  @as []  |  ['<Control><Alt>t', '<Super>t']
        let cleaned = value
            .trim()
            .trim_start_matches("@as ")
            .trim_matches(|c| c == '[' || c == ']' || c == '\'' || c == '"' || c == ' ');
        let first = cleaned.split(',').next().unwrap_or("").trim().to_string();
        let keys = normalize_keys(&first);
        if keys.is_empty() {
            continue; // unbound keys are noise in a shortcut list
        }
        out.push(Keybinding {
            action: format!("{schema}/{key}"),
            keys,
            description: key.to_string(),
            source: "gnome".into(),
            custom: false,
        });
    }
    out
}

/// XFCE: in-process over D-Bus, no `xfconf-query` process at all.
///
/// `org.xfce.Xfconf.GetAllProperties(channel)` returns the whole channel as `a{sv}`, which is one
/// D-Bus round trip. The previous implementation spawned a process and parsed its text output.
///
/// Falls back to `xfconf-query` only if the bus call fails, so a session without the xfconf service
/// still reports its shortcuts.
fn keybindings_xfce() -> Vec<Keybinding> {
    if let Some(binds) = keybindings_xfce_dbus() {
        return binds;
    }
    keybindings_xfce_process()
}

fn keybindings_xfce_dbus() -> Option<Vec<Keybinding>> {
    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some("org.xfce.Xfconf"),
            "/org/xfce/Xfconf",
            Some("org.xfce.Xfconf"),
            "GetAllProperties",
            &("xfce4-keyboard-shortcuts",),
        )
        .ok()?;
    let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        reply.body().deserialize().ok()?;

    let mut out = Vec::new();
    for (path, value) in props {
        if !path.starts_with('/') {
            continue;
        }
        // Values are variant<string> holding the command (or a bare keycode for xfwm4 entries).
        let command = match value.downcast_ref::<String>() {
            std::result::Result::Ok(s) => s,
            std::result::Result::Err(_) => continue,
        };
        let chord = path.rsplit('/').next().unwrap_or("");
        let keys = normalize_keys(chord);
        if keys.is_empty() {
            continue;
        }
        out.push(Keybinding {
            action: command.clone(),
            keys,
            description: path
                .trim_start_matches('/')
                .rsplit_once('/')
                .map(|(a, _)| a.to_string())
                .unwrap_or_default(),
            source: "xfce".into(),
            custom: path.contains("/custom/"),
        });
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// XFCE: `xfconf-query -c xfce4-keyboard-shortcuts -l -v` prints `<path>  <command>`.
fn keybindings_xfce_process() -> Vec<Keybinding> {
    let Ok(o) = run("xfconf-query", &["-c", "xfce4-keyboard-shortcuts", "-l", "-v"]) else {
        return Vec::new();
    };
    if !o.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&o.stdout);
    let mut out = Vec::new();
    for line in text.lines() {
        // Split on runs of whitespace: first token is the path, the rest is the command.
        let mut it = line.trim().splitn(2, char::is_whitespace);
        let (Some(path), Some(command)) = (it.next(), it.next()) else {
            continue;
        };
        if !path.starts_with('/') {
            continue;
        }
        // /commands/custom/<Alt><Super>s  ->  the chord is the last path segment
        let chord = path.rsplit('/').next().unwrap_or("");
        out.push(Keybinding {
            action: command.trim().to_string(),
            keys: normalize_keys(chord),
            description: path.trim_start_matches('/').rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default(),
            source: "xfce".into(),
            custom: path.contains("/custom/"),
        });
    }
    out
}

/// Every keyboard shortcut the live desktop knows about, normalized so three desktops can be compared.
///
/// Read-only: nothing is written back to the desktop's configuration. Returns an empty vector when the
/// desktop's source is unavailable, never an error — a missing shortcut list is not a failure.
/// Hyprland, and therefore Omarchy.
///
/// Two sources, because they answer different questions:
///
///   * `hyprctl -j binds` reports what the compositor is ACTUALLY holding — the truth, including
///     anything bound at runtime. On a plain Hyprland it carries real dispatchers.
///   * Omarchy 4.x declares its bindings in Lua (`/usr/share/omarchy/default/hypr/bindings/*.lua`),
///     and `hyprctl -j binds` reports every one of them with the dispatcher `__lua` and a numeric
///     argument. Measured on Omarchy 4.0.1 / Hyprland 0.56.2: 223 bindings, all 223 `__lua`, which is
///     useless as a shortcut list. The Lua modules carry the human description, so on Omarchy they are
///     the better source.
///
/// The Lua files are read first and hyprctl fills in whatever they do not cover, so neither source
/// alone decides the answer.
fn keybindings_hyprland() -> Vec<Keybinding> {
    let mut out: Vec<Keybinding> = Vec::new();

    // 1. Omarchy's declared bindings, which carry descriptions.
    let mut lua_files: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/usr/share/omarchy/default/hypr/bindings") {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("lua") {
                lua_files.push(p);
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        let user = home.join(".config/hypr/bindings.lua");
        if user.is_file() {
            lua_files.push(user);
        }
    }
    lua_files.sort();
    for file in lua_files {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("o.bind(") && !line.starts_with("hl.bind(") {
                continue;
            }
            // o.bind("SUPER + RETURN", "Terminal", "alacritty")
            //         ^1                ^3          ^5
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() < 7 {
                continue;
            }
            let combo = parts[1].trim();
            let desc = parts[3].trim();
            let command = parts[5].trim();
            if combo.is_empty() {
                continue;
            }
            out.push(Keybinding {
                action: command.to_string(),
                keys: normalize_keys(combo),
                description: if desc.eq_ignore_ascii_case("nil") {
                    String::new()
                } else {
                    desc.to_string()
                },
                source: "hyprland".into(),
                custom: line.starts_with("hl.bind"),
            });
        }
    }

    // 2. Whatever the compositor itself is holding, for anything the Lua files did not cover.
    if let Ok(v) = hyprctl_json("binds") {
        if let Some(binds) = v.as_array() {
            for b in binds {
                let key = b.get("key").and_then(|k| k.as_str()).unwrap_or("");
                let dispatcher = b.get("dispatcher").and_then(|d| d.as_str()).unwrap_or("");
                // `__lua` means the binding came from Omarchy's Lua layer and its argument is an
                // internal index — nothing a user could act on. Covered by the Lua pass above.
                if key.is_empty() || dispatcher == "__lua" {
                    continue;
                }
                let modmask = b.get("modmask").and_then(|m| m.as_u64()).unwrap_or(0);
                let mut mods: Vec<&str> = Vec::new();
                if modmask & 64 != 0 {
                    mods.push("SUPER");
                }
                if modmask & 4 != 0 {
                    mods.push("CTRL");
                }
                if modmask & 8 != 0 {
                    mods.push("ALT");
                }
                if modmask & 1 != 0 {
                    mods.push("SHIFT");
                }
                mods.push(key);
                let combo = mods.join(" + ");
                let arg = b.get("arg").and_then(|a| a.as_str()).unwrap_or("");
                out.push(Keybinding {
                    action: if arg.is_empty() {
                        dispatcher.to_string()
                    } else {
                        format!("{dispatcher} {arg}")
                    },
                    keys: normalize_keys(&combo),
                    description: b
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source: "hyprland".into(),
                    custom: false,
                });
            }
        }
    }

    // The two sources overlap by design; keep one row per key+action.
    out.sort_by(|a, b| a.keys.cmp(&b.keys).then(a.action.cmp(&b.action)));
    out.dedup_by(|a, b| a.keys == b.keys && a.action == b.action);
    out
}

pub fn list_keybindings() -> Vec<Keybinding> {
    // Hyprland goes FIRST, before the desktop_is() chain. Omarchy reports
    // XDG_CURRENT_DESKTOP=Hyprland, which matches neither "KDE" nor "GNOME", so the old if/else-if
    // fell through to the XFCE reader and returned nothing — an empty shortcut list on a desktop
    // with 88 declared bindings.
    let mut all = if hyprland_running() {
        keybindings_hyprland()
    } else if desktop_is("KDE") {
        keybindings_kde()
    } else if desktop_is("GNOME") {
        keybindings_gnome()
    } else {
        keybindings_xfce()
    };
    all.sort_by(|a, b| a.keys.cmp(&b.keys).then(a.action.cmp(&b.action)));
    all
}

/// Look up the shortcut for an action: matches the bound keys, the action name, or its description.
///
/// This is the call an agent actually wants — "what opens a terminal" — rather than a full dump.
pub fn find_keybinding(needle: &str) -> Option<Keybinding> {
    let needle = needle.to_ascii_lowercase();
    let all = list_keybindings();
    all.iter()
        .find(|k| !k.keys.is_empty() && k.keys == needle)
        .or_else(|| all.iter().find(|k| !k.keys.is_empty() && k.action.to_ascii_lowercase().contains(&needle)))
        .or_else(|| all.iter().find(|k| !k.keys.is_empty() && k.description.to_ascii_lowercase().contains(&needle)))
        .cloned()
}

/// Prove the input path works WITHOUT disturbing anyone.
///
/// The intrusive checks move the pointer someone is using, which is why they are opt-in. But skipping
/// input entirely in safe mode proves nothing, and "safe" should mean "non-disruptive", not "untested".
///
/// The trick is a **no-op move**: ask for the position the pointer already occupies. That travels the
/// full stack — client -> ydotoold socket -> /dev/uinput -> compositor — and lands with zero
/// displacement, so nothing on screen moves and no window is affected.
///
/// What this does NOT cover: keystrokes. Any key can have an effect on the focused window, so there is
/// no non-disruptive key test; safe mode reports that limitation rather than pretending otherwise.
pub fn input_path_check() -> Result<String> {
    if !have("ydotool") {
        bail!("ydotool is not installed");
    }
    let Some(sock) = ydotool_socket() else {
        bail!(
            "ydotoold socket not found — checked YDOTOOL_SOCKET, $XDG_RUNTIME_DIR, /tmp and /run"
        );
    };
    if !sock.exists() {
        bail!("ydotoold socket {} disappeared", sock.display());
    }

    match cursor_position() {
        Some((x, y)) => {
            // A real injection with no displacement.
            mouse_move(x as i32, y as i32)?;
            Ok(format!(
                "ydotoold reachable at {}; no-op move round-tripped at {},{}",
                sock.display(),
                x as i32,
                y as i32
            ))
        }
        None => {
            // Wayland gives no honest pointer position, so an absolute no-op cannot be aimed. A
            // RELATIVE move of zero still travels the whole path and displaces nothing.
            ydotool(&["mousemove", "-x", "0", "-y", "0"])?;
            Ok(format!(
                "ydotoold reachable at {}; relative zero-move injected (no displacement)",
                sock.display()
            ))
        }
    }
}
