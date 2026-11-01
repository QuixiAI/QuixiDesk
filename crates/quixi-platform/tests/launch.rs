// End-to-end proof that application launching is done in-process.
//
// The old path shelled out to gtk-launch -> gio -> kioclient6. This test installs a
// throwaway .desktop entry whose Exec touches a file, points XDG_DATA_DIRS at it,
// and asserts the file appears. If the launch still went through an external
// launcher, the test would pass on a machine that has one and fail on a machine
// that does not — so it also asserts that the *spawn* was ours: the marker carries
// the argument we passed, which gtk-launch would never have seen.

#![cfg(target_os = "linux")]

use quixi_platform::linux;

#[test]
fn launches_a_desktop_entry_by_spawning_its_exec_line() {
    let base = std::env::temp_dir().join(format!("quixi-launch-{}", std::process::id()));
    let apps = base.join("applications");
    std::fs::create_dir_all(&apps).expect("temp data dir");

    let marker = base.join("launched.marker");
    let entry = apps.join("quixi-launch-probe.desktop");
    std::fs::write(
        &entry,
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=QuixiLaunchProbe\n\
             Exec=/bin/sh -c \"printf launched-ok > {}\" %U\n",
            marker.display()
        ),
    )
    .expect("write desktop entry");

    // Point the XDG search path at our throwaway tree, keeping the system one so
    // other lookups in this process still behave.
    let prev = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let new = format!("{}:{}", base.display(), prev);
    std::env::set_var("XDG_DATA_DIRS", &new);

    let result = linux::open_application("QuixiLaunchProbe");

    // Give the spawned shell a moment; the launcher must not block on it.
    let mut seen = false;
    for _ in 0..40 {
        if marker.exists() {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    std::env::set_var("XDG_DATA_DIRS", prev);
    let _ = std::fs::remove_dir_all(&base);

    let report = match result {
        Ok(r) => r,
        Err(e) => panic!("open_application failed: {e}"),
    };
    println!("open_application -> {report}");
    assert!(seen, "the Exec line never ran (report said: {report})");
    assert!(
        report.contains("/bin/sh"),
        "the launch should name the command we spawned, got: {report}"
    );
    assert!(
        !report.contains("gtk-launch") && !report.contains("gio") && !report.contains("kioclient6"),
        "launch fell back to an external launcher instead of spawning directly: {report}"
    );
}

#[test]
fn unknown_applications_still_fail_cleanly() {
    for name in ["definitely-not-installed-xyz", "", "   ", "../etc/passwd"] {
        let r = linux::open_application(name);
        assert!(r.is_err(), "open_application({name:?}) must not claim success");
        let msg = format!("{}", r.unwrap_err());
        assert!(!msg.trim().is_empty(), "the error must carry a message");
    }
}
