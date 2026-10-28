//! `quixi-ssh-askpass` — Native SSH password prompt helper.
//! Prompts the user or queries the parent QuixiDesk IPC socket when `SSH_ASKPASS` is invoked.

use std::env;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let prompt = env::args()
        .nth(1)
        .unwrap_or_else(|| "Enter SSH passphrase for QuixiDesk:".to_string());

    if let Ok(pass) = env::var("QUIXI_SSH_PASS") {
        println!("{}", pass);
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "Tell application \"System Events\" to display dialog {:?} default answer \"\" with hidden answer with title \"QuixiDesk SSH\"\ntext returned of result",
            prompt
        );
        if let Ok(out) = Command::new("/usr/bin/osascript").arg("-e").arg(script).output() {
            if out.status.success() {
                print!("{}", String::from_utf8_lossy(&out.stdout));
                return Ok(());
            }
        }
    }

    let _ = prompt;
    std::process::exit(1);
}
