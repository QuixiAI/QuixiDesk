//! Process ownership and recovery checks using disposable remote MCP fixtures.
use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
struct Fixture {
    host: McpHost,
    root: tempfile::TempDir,
    server: Child,
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < end,
            "Timed out waiting for worker lifecycle"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }
    #[cfg(windows)]
    {
        Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",&format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}")]).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap().success()
    }
}
fn kill(pid: u32) {
    #[cfg(unix)]
    let mut command = Command::new("kill");
    #[cfg(unix)]
    command.args(["-9", &pid.to_string()]);
    #[cfg(windows)]
    let mut command = Command::new("taskkill.exe");
    #[cfg(windows)]
    command.args(["/F", "/T", "/PID", &pid.to_string()]);
    assert!(command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
}
impl Fixture {
    fn new(transport: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let port = root.path().join("port");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/mcp")
            .join(if transport == "http" {
                "worker_fixture.py"
            } else {
                "fixture_server.py"
            });
        let python = std::env::var("QUIXI_MCP_TEST_PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
        let mut command = Command::new(python);
        command.arg(script).arg("--port-file").arg(&port);
        if transport == "sse" {
            command.args(["--transport", "sse"]);
        } else {
            command.arg("--closed-file").arg(root.path().join("closed"));
        }
        let server = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait(|| {
            fs::read_to_string(&port)
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .is_some()
        });
        let host = McpHost::new_with_data_dir(&root.path().join("data"));
        host.write_config(&json!({"fixture":{"transport":transport,"url":format!("http://127.0.0.1:{}/{}",fs::read_to_string(port).unwrap(),if transport=="sse"{"sse"}else{"mcp"}),"headers":{"X-Private-Test":"private-bootstrap-marker"},"connectTimeoutMs":3000,"discoveryTimeoutMs":3000,"requestTimeoutMs":3000}})).unwrap();
        host.discover_tools(
            Some("fixture"),
            None,
            0,
            root.path(),
            &AtomicBool::new(false),
        )
        .unwrap();
        Self { host, root, server }
    }
    fn status(&self) -> Value {
        self.host
            .list_local_mcp_servers()
            .into_iter()
            .find(|v| v["name"] == "fixture")
            .unwrap()
    }
    fn pid(&self) -> u32 {
        self.status()["workerPid"].as_u64().unwrap() as u32
    }
    fn call(&self, name: &str) -> anyhow::Result<Value> {
        self.host
            .call_tool("fixture", name, &json!({}), self.root.path())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.host.shutdown();
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}
#[test]
fn both_remote_transports_have_owned_processes_and_shutdown_reaps_them() {
    for transport in ["http", "sse"] {
        let f = Fixture::new(transport);
        let pid = f.pid();
        assert_ne!(pid, std::process::id());
        assert_ne!(pid, f.server.id());
        assert!(alive(pid));
        #[cfg(unix)]
        {
            let args = Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "command="])
                .output()
                .unwrap();
            let args = String::from_utf8_lossy(&args.stdout);
            assert!(args.contains("--mcp-transport-worker"));
            assert!(!args.contains("private-bootstrap-marker"));
            assert!(!args.contains("127.0.0.1"));
        }
        f.host.shutdown();
        wait(|| !alive(pid));
        if transport == "http" {
            assert!(
                f.root.path().join("closed").exists(),
                "Worker must close its HTTP session before exit"
            );
        }
    }
}
#[test]
fn worker_crash_recovers_catalog_in_fresh_process_and_explicit_disconnect_stays_closed() {
    let f = Fixture::new("http");
    let old = f.pid();
    kill(old);
    wait(|| f.status()["status"] == "ready" && f.pid() != old);
    let recovered = f.pid();
    assert!(alive(recovered));
    assert_eq!(
        f.call("status").unwrap()["structuredContent"]["mutations"],
        0
    );
    f.host.disconnect_server("fixture").unwrap();
    wait(|| !alive(recovered));
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(f.status()["status"], "disconnected");
    assert!(f.call("status").is_err());
}
#[test]
fn expired_session_fails_mutation_once_then_recovers_without_replay() {
    let f = Fixture::new("http");
    let old = f.pid();
    let error = f.call("mutate").unwrap_err();
    assert!(format!("{error:#}").contains("Session expired"));
    wait(|| f.status()["status"] == "ready" && f.pid() != old);
    wait(|| !alive(old));
    assert_eq!(
        f.call("status").unwrap()["structuredContent"]["mutations"],
        1
    );
    assert_eq!(f.status()["catalogStale"], false);
}
