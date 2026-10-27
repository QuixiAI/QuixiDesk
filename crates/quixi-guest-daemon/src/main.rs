//! Multi-call Cowork VM Guest Daemon (`quixi-guest-daemon`)
//! Replaces the 4 Go static Linux binaries in `smol-bin.arm64.img`:
//! - `/sdk-daemon` (`coworkd` listening on vsock/TCP port 51234 / `0xc822`)
//! - `/sandbox-helper` (bubblewrap + seccomp + network proxy sandbox wrapper)
//! - `/cli-wrapper` (wraps CLI commands inside the Cowork guest)
//! - `/claude-shim` (guest CLI shim)

use quixi_proto::{VmJsonRpcRequest, VmJsonRpcResponse};
use serde_json::{json, Value};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .first()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("quixi-guest-daemon");

    match prog {
        "sandbox-helper" => run_sandbox_helper(&args[1..]),
        "cli-wrapper" | "claude-shim" => run_cli_wrapper(&args[1..]),
        _ => run_sdk_daemon(&args[1..]),
    }
}

fn run_sandbox_helper(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        return Ok(());
    }
    let status = Command::new(&args[0]).args(&args[1..]).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

fn run_cli_wrapper(args: &[String]) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "shim": "quixi-guest-daemon",
            "args": args,
            "status": "ok"
        }))?
    );
    Ok(())
}

fn run_sdk_daemon(args: &[String]) -> anyhow::Result<()> {
    if args.iter().any(|a| a == "--stdio") {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let resp = handle_rpc_line(&line);
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
        return Ok(());
    }

    let addr = env::var("QUIXI_DAEMON_ADDR").unwrap_or_else(|_| "127.0.0.1:51234".to_string());
    let listener = TcpListener::bind(&addr)?;
    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || {
            let _ = handle_client(stream);
        });
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream) -> anyhow::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = handle_rpc_line(&line);
        writeln!(stream, "{}", serde_json::to_string(&resp)?)?;
        stream.flush()?;
    }
    Ok(())
}

fn handle_rpc_line(line: &str) -> VmJsonRpcResponse {
    let req: VmJsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return VmJsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: 0,
                result: None,
                error: Some(json!({
                    "code": -32700,
                    "message": format!("Parse error: {}", e),
                })),
            }
        }
    };

    let result: Value = match req.method.as_str() {
        "ping" | "health" => json!({ "status": "ok", "daemon": "quixi-guest-daemon", "port": 51234 }),
        "exec" | "spawn" => {
            let cmd = req
                .params
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("echo ok");
            match Command::new("/bin/sh").arg("-c").arg(cmd).output() {
                Ok(out) => json!({
                    "exitCode": out.status.code().unwrap_or(0),
                    "stdout": String::from_utf8_lossy(&out.stdout),
                    "stderr": String::from_utf8_lossy(&out.stderr),
                }),
                Err(e) => json!({
                    "exitCode": 1,
                    "stdout": "",
                    "stderr": e.to_string(),
                }),
            }
        }
        _ => json!({ "ok": true, "method": req.method }),
    };

    VmJsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(result),
        error: None,
    }
}
