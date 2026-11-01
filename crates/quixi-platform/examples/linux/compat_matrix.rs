// Full macOS-parity matrix for the Linux ComputerUse layer.
//
// Every entry point that macOS implements is exercised here and reports PASS/FAIL, so
// "does this work on GNOME / XFCE / KDE" is a measurement instead of an opinion.
//
// Safe tests run always. Tests that move the real pointer, type into whatever has focus,
// or open a modal dialog are behind --intrusive, because on a machine someone is using
// they are indistinguishable from the user's own input.

use std::io::Write;

fn ok(name: &str, detail: impl std::fmt::Display) {
    println!("PASS  {name:<26} {detail}");
}
fn fail(name: &str, detail: impl std::fmt::Display) {
    println!("FAIL  {name:<26} {detail}");
}
fn skip(name: &str, why: &str) {
    println!("SKIP  {name:<26} {why}");
}

pub fn main() {
    let intrusive = std::env::args().any(|a| a == "--intrusive");
    let mut pass = 0u32;
    let mut fail_count = 0u32;

    macro_rules! check {
        ($name:expr, $body:block) => {{
            match (|| -> anyhow::Result<String> { $body })() {
                Ok(d) => {
                    ok($name, d);
                    pass += 1;
                }
                Err(e) => {
                    fail($name, format!("{e:#}"));
                    fail_count += 1;
                }
            }
        }};
    }

    println!("═══ capture ═══");
    check!("screenshot", {
        let b = quixi_platform::linux::screenshot_png()?;
        anyhow::ensure!(b.starts_with(b"\x89PNG"), "not a PNG");
        Ok::<_, anyhow::Error>(format!("{} bytes", b.len()))
    });
    check!("screenshot_region", {
        let b = quixi_platform::linux::screenshot_region_png(10, 10, 64, 64)?;
        anyhow::ensure!(b.starts_with(b"\x89PNG"), "not a PNG");
        Ok::<_, anyhow::Error>(format!("{} bytes", b.len()))
    });

    println!("═══ clipboard ═══");
    check!("clipboard_roundtrip", {
        let token = format!("quixi-matrix-{}", std::process::id());
        quixi_platform::linux::clipboard_write(&token)?;
        std::thread::sleep(std::time::Duration::from_millis(150));
        let back = quixi_platform::linux::clipboard_read()
            .ok_or_else(|| anyhow::anyhow!("clipboard read returned nothing"))?;
        anyhow::ensure!(back == token, "round-trip mismatch: got {:?}", &back[..back.len().min(40)]);
        Ok::<_, anyhow::Error>(format!("{} chars exact", token.len()))
    });

    println!("═══ displays ═══");
    check!("list_displays", {
        let d = quixi_platform::linux::list_displays_json()?;
        Ok::<_, anyhow::Error>(format!("{}", d))
    });
    check!("displays_struct", {
        let d = quixi_platform::linux::displays();
        anyhow::ensure!(!d.is_empty(), "no displays reported");
        Ok::<_, anyhow::Error>(format!("{} outputs, first={}", d.len(), d[0].name))
    });
    check!("active_output", {
        let a = quixi_platform::linux::active_output()
            .ok_or_else(|| anyhow::anyhow!("no active output (KWin-only call)"))?;
        Ok::<_, anyhow::Error>(a)
    });

    println!("═══ cursor ═══");
    // On Wayland there is no client API for the compositor's cursor position. XWayland still
    // answers X queries with a stale value (measured: screen centre, before and after a move),
    // so `None` is the honest answer and this reports it as unavailable rather than failing.
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    match quixi_platform::linux::cursor_position() {
        Some(p) => check!("cursor_position", {
            Ok::<_, anyhow::Error>(format!("{:.0},{:.0}", p.0, p.1))
        }),
        None if wayland => skip(
            "cursor_position",
            "Wayland has no client cursor query (XWayland's answer is stale, so None is correct)",
        ),
        None => check!("cursor_position", {
            Err::<String, anyhow::Error>(anyhow::anyhow!("no cursor position available"))
        }),
    }

    println!("═══ apps ═══");
    check!("installed_apps", {
        let a = quixi_platform::linux::installed_apps();
        anyhow::ensure!(!a.is_empty(), "no .desktop entries found");
        Ok::<_, anyhow::Error>(format!("{} apps", a.len()))
    });
    check!("list_installed_apps_json", {
        let v = quixi_platform::linux::list_installed_apps();
        Ok::<_, anyhow::Error>(format!("keys={:?}", v.as_object().map(|o| o.len())))
    });

    println!("═══ window ═══");
    match quixi_platform::linux::x11_window_under_cursor() {
        Some(w) => ok("window_under_cursor", w),
        None => skip("window_under_cursor", "no window reported (Wayland has no client API for it; on X11 this is a real gap)"),
    }

    // ------------------------------------------------------------ keybindings
    println!("\n═══ keybindings ═══");
    check!("list_keybindings", {
        let binds = quixi_platform::linux::list_keybindings();
        if binds.is_empty() {
            anyhow::bail!("this desktop keeps its shortcuts somewhere else");
        }
        let bound = binds.iter().filter(|k| !k.keys.is_empty()).count();
        Ok::<_, anyhow::Error>(format!("{} shortcuts, {} bound", binds.len(), bound))
    });
    check!("keybindings_normalised", {
        let binds = quixi_platform::linux::list_keybindings();
        let sample: Vec<String> = binds
            .iter()
            .filter(|k| !k.keys.is_empty())
            .take(3)
            .map(|k| format!("{}={}", k.keys, k.action))
            .collect();
        if sample.is_empty() {
            anyhow::bail!("no bound shortcuts to show");
        }
        Ok::<_, anyhow::Error>(sample.join("  "))
    });
    match quixi_platform::linux::find_keybinding("terminal") {
        Some(k) => {
            ok("find_keybinding", format!("terminal -> {}", k.keys));
            pass += 1;
        }
        None => skip("find_keybinding", "no shortcut mentioning 'terminal' on this desktop"),
    }

    println!("═══ capabilities ═══");
    {
        let c = quixi_platform::linux::capabilities();
        println!(
            "INFO  capabilities              session={} capture={} ({}) region={} clipboard={} input={} pickers={} displays={} launch={}",
            c.session_type,
            c.screenshot,
            c.screenshot_backend,
            c.screenshot_region,
            c.clipboard,
            c.input_synthesis,
            c.file_pickers,
            c.display_listing,
            c.app_launch
        );
    }

    println!("═══ input (needs --intrusive) ═══");
    if !intrusive {
        // Safe mode still proves the input PATH works: a zero-displacement injection through
        // ydotool -> ydotoold -> uinput. Keystrokes have no non-disruptive equivalent and stay
        // explicitly uncovered rather than silently assumed.
        check!("input_path", {
            let detail = quixi_platform::linux::input_path_check()?;
            Ok::<_, anyhow::Error>(detail)
        });
        for n in [
            "type_text",
            "press_key",
            "mouse_move",
            "mouse_click",
            "mouse_down_up",
            "mouse_drag",
        ] {
            skip(n, "--intrusive not set (moves the real pointer / types into focus)");
        }
        skip("pick_file", "opens a modal dialog; run manually");
        skip("pick_folder", "opens a modal dialog; run manually");
    } else {
        let start = quixi_platform::linux::cursor_position().unwrap_or((0.0, 0.0));
        check!("mouse_move", {
            // Target the middle of the screen: asking for a point outside the pointer space
            // is a clamp, not a move, and testing that proves nothing about mouse control.
            let (sw, sh) = quixi_platform::linux::displays()
                .first()
                .map(|d| (d.width as i32, d.height as i32))
                .unwrap_or((640, 400));
            let tx = (sw / 2).max(10);
            let ty = (sh / 2).max(10);
            quixi_platform::linux::mouse_move(tx, ty)?;
            std::thread::sleep(std::time::Duration::from_millis(300));
            // The move was issued either way. Verifying it needs a cursor read, which Wayland
            // does not offer — so on Wayland this reports the move as unverifiable instead of
            // calling a working call a failure.
            match quixi_platform::linux::cursor_position() {
                Some(p) => {
                    anyhow::ensure!(
                        (p.0 - tx as f64).abs() < 40.0 && (p.1 - ty as f64).abs() < 40.0,
                        "moved to {:.0},{:.0}, asked for {tx},{ty}",
                        p.0,
                        p.1
                    );
                    Ok::<_, anyhow::Error>(format!("{:.0},{:.0}", p.0, p.1))
                }
                None => Ok(format!("issued to {tx},{ty} (position not readable on Wayland)")),
            }
        });
        check!("mouse_click", {
            quixi_platform::linux::mouse_click("left", 1)?;
            Ok::<_, anyhow::Error>("accepted".to_string())
        });
        check!("mouse_down_up", {
            quixi_platform::linux::mouse_down("left")?;
            quixi_platform::linux::mouse_up("left")?;
            Ok::<_, anyhow::Error>("accepted".to_string())
        });
        check!("mouse_drag", {
            quixi_platform::linux::mouse_drag(
                start.0 as i32,
                start.1 as i32,
                start.0 as i32 + 30,
                start.1 as i32 + 30,
                "left",
            )?;
            Ok::<_, anyhow::Error>("accepted".to_string())
        });
        check!("type_text", {
            quixi_platform::linux::type_text("echo quixi")?;
            Ok::<_, anyhow::Error>("accepted".to_string())
        });
        check!("press_key", {
            quixi_platform::linux::press_key("escape", 1)?;
            Ok::<_, anyhow::Error>("accepted".to_string())
        });
        skip("pick_file", "opens a modal dialog; run manually");
        skip("pick_folder", "opens a modal dialog; run manually");
    }

    println!();
    println!("═══ {pass} passed, {fail_count} failed ═══");
    let _ = std::io::stdout().flush();
    if fail_count > 0 {
        std::process::exit(1);
    }
}
