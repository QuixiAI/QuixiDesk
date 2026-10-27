//! `quixi-disclaimer` — Native Rust replacement for `Contents/Helpers/disclaimer`
//! Spawns a target command after setting `responsibility_spawnattrs_setdisclaim(attr, 1)`
//! so macOS TCC prompts attribute permissions to the child binary directly.

use quixi_platform::SecurityAndSpawn;
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: quixi-disclaimer <program> [args...]");
        std::process::exit(64);
    }
    let program = &args[0];
    let child_args = &args[1..];
    match SecurityAndSpawn::spawn_disclaimed(program, child_args, None) {
        Ok(mut child) => {
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
            std::process::exit(code);
        }
        Err(err) => {
            eprintln!("quixi-disclaimer failed: {}", err);
            std::process::exit(1);
        }
    }
}
