//! Behavioral integration tests against a separate MCP process/socket peer.
//! Requires Python 3; override its executable with QUIXI_MCP_TEST_PYTHON.
use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn python() -> String {
    std::env::var("QUIXI_MCP_TEST_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into())
}
fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/mcp/fixture_server.py")
        .canonicalize()
        .unwrap()
}
struct Fixture {
    host: Arc<McpHost>,
    root: PathBuf,
    work: PathBuf,
    child: Option<Child>,
}
impl Fixture {
    fn new(scenario: &str) -> Self {
        Self::configured(scenario, "stdio", json!({}))
    }
    fn configured(scenario: &str, transport: &str, extra: Value) -> Self {
        let root = std::env::temp_dir().join(format!(
            "quixi-mcp-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("work")).unwrap();
        let host = Arc::new(McpHost::new_with_data_dir(&root.join("data")));
        let log = root.join("peer.jsonl");
        let pid = root.join("peer.pid");
        let mut child = None;
        let mut entry = if transport == "stdio" {
            json!({"command":python(),"args":[script(),"--scenario",scenario,"--log",log,"--pid-file",pid],"connectTimeoutMs":2000,"discoveryTimeoutMs":2000,"requestTimeoutMs":2000})
        } else {
            let port_file = root.join("port");
            let process = Command::new(python())
                .arg(script())
                .args([
                    "--transport",
                    transport,
                    "--scenario",
                    scenario,
                    "--port-file",
                ])
                .arg(&port_file)
                .arg("--log")
                .arg(&log)
                .arg("--pid-file")
                .arg(pid)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("Python 3 must be installed for MCP integration tests");
            child = Some(process);
            wait_until(
                || {
                    fs::read_to_string(&port_file)
                        .ok()
                        .and_then(|s| s.trim().parse::<u16>().ok())
                        .is_some_and(|p| p > 0)
                },
                "HTTP fixture did not start",
            );
            let port = fs::read_to_string(port_file).unwrap();
            json!({"transport":transport,"url":format!("http://127.0.0.1:{}/{}",port.trim(),if transport=="sse"{"sse"}else{"mcp"}),"connectTimeoutMs":2000,"discoveryTimeoutMs":2000,"requestTimeoutMs":2000})
        };
        entry
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        host.write_config(&json!({"fixture":entry})).unwrap();
        let work = root.join("work");
        Self {
            host,
            root,
            work,
            child,
        }
    }
    fn cwd(&self) -> &Path {
        &self.work
    }
    fn discover(&self) -> anyhow::Result<Value> {
        self.host.discover_tools(
            Some("fixture"),
            None,
            0,
            self.cwd(),
            &AtomicBool::new(false),
        )
    }
    fn call(&self, name: &str, arguments: Value) -> anyhow::Result<Value> {
        self.host.call_tool_cancellable(
            "fixture",
            name,
            &arguments,
            self.cwd(),
            &AtomicBool::new(false),
        )
    }
    fn status(&self) -> Value {
        self.host
            .list_local_mcp_servers()
            .into_iter()
            .find(|s| s["name"] == "fixture")
            .unwrap()
    }
    fn log(&self) -> Vec<Value> {
        fs::read_to_string(self.root.join("peer.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
    fn count_requests(&self, method: &str) -> usize {
        self.log()
            .iter()
            .filter(|v| v["event"] == "request" && v["method"] == method)
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.host.disconnect_server("fixture");
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn wait_until(mut predicate: impl FnMut() -> bool, message: &str) {
    let end = Instant::now() + Duration::from_secs(8);
    while !predicate() {
        assert!(Instant::now() < end, "{message}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn names(catalog: &Value) -> Vec<String> {
    catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn ordered_handshake_persistent_process_and_full_schema() {
    let f = Fixture::new("normal");
    assert_eq!(f.status()["status"], "configured");
    assert_eq!(f.status()["isConnected"], false);
    let catalog = f.discover().unwrap();
    let echo = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "echo")
        .unwrap();
    assert_eq!(echo["inputSchema"]["properties"]["text"]["minLength"], 1);
    assert_eq!(echo["_meta"]["fixture/metadata"], "preserve-me");
    let a = f.call("counter", json!({})).unwrap();
    let b = f.call("counter", json!({})).unwrap();
    assert_eq!(a["structuredContent"]["count"], 1);
    assert_eq!(b["structuredContent"]["count"], 2);
    assert_eq!(a["structuredContent"]["pid"], b["structuredContent"]["pid"]);
    assert_eq!(f.count_requests("initialize"), 1);
    assert!(!f.log().iter().any(|v| v["event"] == "order_violation"));
    assert_eq!(f.status()["status"], "ready");
    assert_eq!(f.status()["toolsEnumerated"], true);
}
#[test]
fn discovers_every_page_and_rejects_incomplete_catalogs() {
    let f = Fixture::new("paginated");
    let catalog = f.discover().unwrap();
    assert_eq!(names(&catalog).len(), 7);
    assert!(names(&catalog).contains(&"request_error".into()));
    assert_eq!(f.count_requests("tools/list"), 2);
    for scenario in ["cursor_loop", "duplicate_tools"] {
        let bad = Fixture::new(scenario);
        assert!(bad.discover().is_err(), "{scenario}");
        assert_ne!(bad.status()["status"], "ready");
        assert_ne!(bad.status()["toolsEnumerated"], true);
    }
}
#[test]
fn missing_tools_capability_and_empty_catalog_are_distinct_from_connection_failure() {
    for scenario in ["missing_tools_capability", "zero_tools"] {
        let f = Fixture::new(scenario);
        assert!(names(&f.discover().unwrap()).is_empty());
        assert_eq!(f.status()["status"], "ready");
        assert_eq!(f.status()["isConnected"], true);
        if scenario == "missing_tools_capability" {
            assert_eq!(f.count_requests("tools/list"), 0);
        }
    }
}
#[test]
fn invalid_initialization_never_reaches_discovery() {
    for scenario in ["unsupported_version", "invalid_initialize"] {
        let f = Fixture::new(scenario);
        assert!(f.discover().is_err(), "{scenario}");
        assert_eq!(f.count_requests("tools/list"), 0);
        assert_ne!(f.status()["status"], "ready");
    }
}
#[test]
fn discovery_failure_is_reported_without_success_or_inventory() {
    let f = Fixture::new("discovery_error");
    let error = f.discover().unwrap_err().to_string();
    assert!(
        error.contains("discovery") || error.contains("Deliberate"),
        "{error}"
    );
    assert_ne!(f.status()["isConnected"], true);
    assert_ne!(f.status()["toolsEnumerated"], true);
}
#[test]
fn unknown_server_and_unknown_tool_never_fall_back_to_builtin_execution() {
    let f = Fixture::new("normal");
    assert!(f
        .host
        .call_tool_cancellable(
            "not-configured",
            "tools/list",
            &json!({}),
            f.cwd(),
            &AtomicBool::new(false)
        )
        .is_err());
    f.discover().unwrap();
    assert!(f.call("tools/list", json!({})).is_err());
    assert_eq!(f.count_requests("tools/call"), 0);
}
#[test]
fn tool_errors_preserve_failure_and_invalid_arguments_never_reach_server() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let failed = f.call("fail", json!({})).unwrap();
    assert_eq!(failed["isError"], true);
    let rpc_failure = f.call("request_error", json!({}));
    assert!(rpc_failure.is_err() || rpc_failure.unwrap()["isError"] == true);
    let calls = f.count_requests("tools/call");
    assert!(f.call("echo", json!({"text":12})).is_err());
    assert_eq!(f.count_requests("tools/call"), calls);
}
#[test]
fn invalid_schema_and_invalid_structured_output_are_errors() {
    let f = Fixture::new("invalid_schema");
    assert!(f.discover().is_err());
    let f = Fixture::new("invalid_output");
    f.discover().unwrap();
    let result = f.call("counter", json!({}));
    assert!(result.is_err() || result.unwrap()["isError"] == true);
}
#[test]
fn notification_refresh_reaches_the_catalog_without_reconnecting() {
    let f = Fixture::new("list_change");
    assert!(!names(&f.discover().unwrap()).contains(&"newly_available".into()));
    f.call("mutate_catalog", json!({})).unwrap();
    wait_until(
        || {
            f.status()["tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|t| t["name"] == "newly_available"))
        },
        "list_changed did not refresh catalog",
    );
    assert!(f.call("newly_available", json!({})).is_ok());
    assert_eq!(f.count_requests("initialize"), 1);
}
#[test]
fn stopping_request_cancels_it_without_destroying_the_connection() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let host = f.host.clone();
    let cwd = f.cwd().to_owned();
    let worker = std::thread::spawn(move || {
        host.call_tool_cancellable("fixture", "delay", &json!({"seconds":5}), &cwd, &signal)
    });
    wait_until(
        || f.count_requests("tools/call") > 0,
        "delayed call never started",
    );
    let start = Instant::now();
    stop.store(true, Ordering::SeqCst);
    assert!(worker.join().unwrap().is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
    wait_until(
        || f.log().iter().any(|v| v["event"] == "cancelled"),
        "cancel notification not received",
    );
    assert!(f.call("echo", json!({"text":"still connected"})).is_ok());
    assert_eq!(f.count_requests("initialize"), 1);
}
#[test]
fn initialization_discovery_and_tool_deadlines_are_bounded() {
    for (scenario, extra) in [
        ("initialize_timeout", json!({"connectTimeoutMs":200})),
        ("discovery_timeout", json!({"discoveryTimeoutMs":200})),
    ] {
        let f = Fixture::configured(scenario, "stdio", extra);
        let start = Instant::now();
        assert!(f.discover().is_err());
        assert!(start.elapsed() < Duration::from_secs(4));
    }
    let f = Fixture::configured("normal", "stdio", json!({"requestTimeoutMs":200}));
    f.discover().unwrap();
    let start = Instant::now();
    assert!(f.call("delay", json!({"seconds":5})).is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
}
#[test]
fn concurrent_discovery_joins_one_connection_attempt() {
    let f = Fixture::new("normal");
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let host = f.host.clone();
            let cwd = f.cwd().to_owned();
            std::thread::spawn(move || {
                host.discover_tools(Some("fixture"), None, 0, &cwd, &AtomicBool::new(false))
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap().unwrap();
    }
    assert_eq!(f.count_requests("initialize"), 1);
}
#[test]
fn explicit_disconnect_requires_reconnect_and_process_state_resets() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let before = f.call("counter", json!({})).unwrap();
    f.host.disconnect_server("fixture").unwrap();
    assert_eq!(f.status()["status"], "disconnected");
    assert!(f.call("counter", json!({})).is_err());
    f.host.reconnect_server("fixture", f.cwd()).unwrap();
    wait_until(|| f.status()["status"] == "ready", "reconnect never ready");
    let after = f.call("counter", json!({})).unwrap();
    assert_eq!(after["structuredContent"]["count"], 1);
    assert_ne!(
        before["structuredContent"]["pid"],
        after["structuredContent"]["pid"]
    );
}
#[test]
fn removing_config_prevents_late_reconnection_and_invalid_save_preserves_config() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let before = fs::read(&f.host.config_path).unwrap();
    assert!(f
        .host
        .write_config(&json!({"broken":{"command":4}}))
        .is_err());
    assert_eq!(fs::read(&f.host.config_path).unwrap(), before);
    f.host.write_config(&json!({})).unwrap();
    assert!(f.call("echo", json!({"text":"removed"})).is_err());
    assert!(!f
        .host
        .list_local_mcp_servers()
        .iter()
        .any(|s| s["name"] == "fixture"));
}
#[test]
fn streamable_http_discovers_calls_and_refreshes() {
    let f = Fixture::configured("paginated", "http", json!({}));
    assert_eq!(names(&f.discover().unwrap()).len(), 7);
    assert_eq!(
        f.call("counter", json!({})).unwrap()["structuredContent"]["count"],
        1
    );
    f.call("mutate_catalog", json!({})).unwrap();
    wait_until(
        || {
            f.status()["tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|t| t["name"] == "newly_available"))
        },
        "HTTP list_changed not reflected",
    );
}
#[test]
fn legacy_sse_discovers_and_calls() {
    let f = Fixture::configured("normal", "sse", json!({}));
    assert_eq!(names(&f.discover().unwrap()).len(), 7);
    assert_eq!(
        f.call("echo", json!({"text":"SSE works"})).unwrap()["content"][0]["text"],
        "SSE works"
    );
}
#[test]
fn http_auth_failure_is_needs_auth_and_configured_credentials_can_connect() {
    let f = Fixture::configured("http_401", "http", json!({}));
    assert!(f.discover().is_err());
    assert_eq!(f.status()["status"], "needs_auth");
    let f = Fixture::configured(
        "http_401",
        "http",
        json!({"headers":{"Authorization":"Bearer fixture-token"}}),
    );
    assert!(f.discover().is_ok());
    assert!(!f.status().to_string().contains("fixture-token"));
}

#[test]
fn page_limit_is_an_error_instead_of_silently_truncated_success() {
    let f = Fixture::configured("paginated", "stdio", json!({"maxPages":1}));
    assert!(f.discover().is_err());
    assert_ne!(f.status()["status"], "ready");
    assert_ne!(f.status()["toolsEnumerated"], true);
}
#[test]
fn failed_refresh_marks_old_inventory_stale_and_does_not_invent_tools() {
    let f = Fixture::new("refresh_error");
    f.discover().unwrap();
    f.call("mutate_catalog", json!({})).unwrap();
    wait_until(
        || f.status()["catalogStale"] == true && f.status()["status"] == "failed",
        "failed refresh was not recorded",
    );
    assert_ne!(f.status()["status"], "ready");
    assert!(f.call("newly_available", json!({})).is_err());
}
#[test]
fn crash_fails_once_and_explicit_reconnect_does_not_replay_the_call() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    assert!(f.call("crash", json!({})).is_err());
    wait_until(
        || f.status()["isConnected"] == false,
        "crashed process still connected",
    );
    assert_eq!(f.log().iter().filter(|v| v["event"] == "crash").count(), 1);
    f.host.reconnect_server("fixture", f.cwd()).unwrap();
    wait_until(
        || f.status()["status"] == "ready",
        "reconnect after crash never ready",
    );
    assert_eq!(
        f.call("counter", json!({})).unwrap()["structuredContent"]["count"],
        1
    );
    assert_eq!(f.log().iter().filter(|v| v["event"] == "crash").count(), 1);
}

#[test]
fn schemas_support_local_refs_and_reject_external_resolution() {
    let f = Fixture::new("complex_schema");
    f.discover().unwrap();
    assert!(f.call("echo", json!({"text":"allowed-123"})).is_ok());
    let calls = f.count_requests("tools/call");
    assert!(f.call("echo", json!({"text":"wrong-pattern"})).is_err());
    assert_eq!(f.count_requests("tools/call"), calls);
    let f = Fixture::new("external_schema");
    let start = Instant::now();
    assert!(f.discover().is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_ne!(f.status()["status"], "ready");
}

#[test]
fn catalog_limits_and_disabled_servers_never_publish_partial_success() {
    let f = Fixture::configured("normal", "stdio", json!({"maxTools":1}));
    assert!(f.discover().is_err());
    assert_ne!(f.status()["status"], "ready");
    let f = Fixture::configured("normal", "stdio", json!({"enabled":false}));
    assert_eq!(f.status()["status"], "disabled");
    assert!(f.discover().is_err());
    assert!(f.call("echo", json!({"text":"never execute"})).is_err());
    assert_eq!(f.count_requests("initialize"), 0);
}

fn process_is_running(pid: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }
    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args([
                "/FI",
                &format!("PID eq {}", pid.trim()),
                "/FO",
                "CSV",
                "/NH",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).contains(&format!("\",\"{}\",", pid.trim()))
    }
}

#[test]
fn disconnect_reaps_the_owned_server_process() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let pid = fs::read_to_string(f.root.join("peer.pid")).unwrap();
    assert!(process_is_running(&pid));
    f.host.disconnect_server("fixture").unwrap();
    wait_until(
        || !process_is_running(&pid),
        "disconnect left the MCP server process running",
    );
}

#[test]
fn shutdown_reaps_all_owned_workspace_processes_before_returning() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let first = f.call("counter", json!({})).unwrap()["structuredContent"]["pid"].to_string();
    let other = f.root.join("shutdown-other-workspace");
    fs::create_dir_all(&other).unwrap();
    let second = f
        .host
        .call_tool_cancellable(
            "fixture",
            "counter",
            &json!({}),
            &other,
            &AtomicBool::new(false),
        )
        .unwrap()["structuredContent"]["pid"]
        .to_string();
    assert_ne!(first, second);
    assert!(process_is_running(&first) && process_is_running(&second));
    let started = Instant::now();
    f.host.shutdown();
    assert!(started.elapsed() < Duration::from_secs(6));
    assert!(
        !process_is_running(&first),
        "shutdown returned with first server still alive"
    );
    assert!(
        !process_is_running(&second),
        "shutdown returned with second server still alive"
    );
}

#[test]
fn legacy_sse_rejects_cross_origin_endpoint_and_preserves_http_auth_errors() {
    let f = Fixture::configured(
        "sse_cross_origin",
        "sse",
        json!({"headers":{"Authorization":"Bearer fixture-token"}}),
    );
    assert!(f.discover().is_err());
    assert_eq!(f.count_requests("initialize"), 0);
    assert_ne!(f.status()["status"], "ready");
    for scenario in ["http_401", "sse_post_401"] {
        let f = Fixture::configured(scenario, "sse", json!({}));
        assert!(f.discover().is_err());
        assert_eq!(
            f.status()["status"],
            "needs_auth",
            "{scenario}: {}",
            f.status()
        );
    }
}

#[test]
fn resources_and_prompts_use_the_established_server_session() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    assert!(f.status()["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["uri"] == "fixture://readme"));
    assert!(f.status()["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "fixture_prompt"));
    let resource = f
        .host
        .read_resource_checked(
            "fixture",
            "fixture://readme",
            f.cwd(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(resource["contents"][0]["text"], "Fixture resource");
    let prompt = f
        .host
        .get_prompt(
            "fixture",
            "fixture_prompt",
            &json!({}),
            f.cwd(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(
        prompt["messages"][0]["content"]["text"],
        "Fixture prompt body"
    );
    assert_eq!(f.count_requests("initialize"), 1);
    assert_eq!(f.count_requests("resources/read"), 1);
    assert_eq!(f.count_requests("prompts/get"), 1);
    assert!(f
        .host
        .get_prompt(
            "fixture",
            "not_discovered",
            &json!({}),
            f.cwd(),
            &AtomicBool::new(false)
        )
        .is_err());
    assert_eq!(f.count_requests("prompts/get"), 1);
}

#[test]
fn modern_discovery_and_explicit_legacy_fallback_use_correct_lifecycle() {
    let f = Fixture::configured("modern", "stdio", json!({"lifecycle":"discover"}));
    f.discover().unwrap();
    assert_eq!(
        f.call("echo", json!({"text":"modern works"})).unwrap()["content"][0]["text"],
        "modern works"
    );
    assert_eq!(f.count_requests("server/discover"), 1);
    assert_eq!(f.count_requests("initialize"), 0);
    assert_eq!(f.count_requests("notifications/initialized"), 0);
    let tool_request = f
        .log()
        .into_iter()
        .find(|r| r["method"] == "tools/call")
        .unwrap();
    assert_eq!(
        tool_request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
        "2026-07-28"
    );
    let f = Fixture::configured("normal", "stdio", json!({"lifecycle":"auto"}));
    f.discover().unwrap();
    assert_eq!(f.count_requests("server/discover"), 1);
    assert_eq!(f.count_requests("initialize"), 1);
    assert_eq!(f.count_requests("notifications/initialized"), 1);
}

#[test]
fn modern_capability_rejection_does_not_fall_back_to_legacy() {
    let f = Fixture::configured("modern_rejection", "stdio", json!({"lifecycle":"auto"}));
    assert!(f.discover().is_err());
    assert_eq!(f.count_requests("server/discover"), 1);
    assert_eq!(f.count_requests("initialize"), 0);
}

#[test]
fn resource_and_prompt_pagination_is_complete_and_failures_are_explicit() {
    let f = Fixture::new("resource_prompt_pages");
    f.discover().unwrap();
    assert_eq!(f.status()["resources"].as_array().unwrap().len(), 2);
    assert_eq!(f.status()["prompts"].as_array().unwrap().len(), 2);
    assert_eq!(f.count_requests("resources/list"), 2);
    assert_eq!(f.count_requests("prompts/list"), 2);
    let f = Fixture::new("resources_error");
    assert!(f.discover().is_err());
    assert_ne!(f.status()["status"], "ready");
    assert_ne!(f.status()["toolsEnumerated"], true);
}

#[test]
fn idle_connections_park_with_stale_catalog_and_reconnect_on_use() {
    let f = Fixture::configured("normal", "stdio", json!({"idleTimeoutMs":300}));
    f.discover().unwrap();
    let first = f.call("counter", json!({})).unwrap();
    wait_until(
        || f.status()["error"]["kind"] == "idle",
        "idle connection was not parked",
    );
    assert_eq!(f.status()["status"], "disconnected");
    assert_eq!(f.status()["catalogStale"], true);
    assert!(!f.status()["tools"].as_array().unwrap().is_empty());
    let next = f.call("counter", json!({})).unwrap();
    assert_eq!(next["structuredContent"]["count"], 1);
    assert_ne!(
        next["structuredContent"]["pid"],
        first["structuredContent"]["pid"]
    );
}

#[test]
fn active_requests_are_not_parked_after_idle_deadline() {
    let f = Fixture::configured("normal", "stdio", json!({"idleTimeoutMs":300}));
    f.discover().unwrap();
    f.call("delay", json!({"seconds":1.0})).unwrap();
    assert_eq!(f.status()["status"], "ready");
    assert_eq!(f.count_requests("initialize"), 1);
    assert_eq!(
        f.call("echo", json!({"text":"still connected"})).unwrap()["content"][0]["text"],
        "still connected"
    );
}

#[test]
fn different_workspaces_do_not_share_process_state() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    let first = f.call("counter", json!({})).unwrap();
    let other = f.root.join("other-workspace");
    fs::create_dir_all(&other).unwrap();
    let second = f
        .host
        .call_tool_cancellable(
            "fixture",
            "counter",
            &json!({}),
            &other,
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_ne!(
        first["structuredContent"]["pid"],
        second["structuredContent"]["pid"]
    );
    assert_eq!(second["structuredContent"]["count"], 1);
    assert_eq!(
        f.call("counter", json!({})).unwrap()["structuredContent"]["count"],
        2
    );
    assert_eq!(f.count_requests("initialize"), 2);
}

#[test]
fn reconnect_monitor_generations_cannot_overwrite_the_current_ready_session() {
    let f = Fixture::new("normal");
    f.discover().unwrap();
    for _ in 0..6 {
        f.host.reconnect_server("fixture", f.cwd()).unwrap();
        wait_until(
            || f.status()["status"] == "ready",
            "reconnection did not become ready",
        );
        f.call("echo", json!({"text":"generation ready"})).unwrap();
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(f.status()["status"], "ready");
    assert_eq!(f.status()["catalogStale"], false);
    assert_eq!(f.count_requests("initialize"), 7);
    assert_eq!(
        f.call("counter", json!({})).unwrap()["structuredContent"]["count"],
        1
    );
}

#[test]
fn builtin_discovery_reports_disabled_and_disconnected_servers_as_errors() {
    let f = Fixture::new("normal");
    let entry = json!({"command":"quixi-mcp-builtin","args":["--workspace"],"enabled":false});
    f.host
        .write_config(&json!({"quixi-workspace":entry}))
        .unwrap();
    let discover = || {
        f.host.discover_tools(
            Some("quixi-workspace"),
            None,
            0,
            f.cwd(),
            &AtomicBool::new(false),
        )
    };
    assert!(discover().is_err());
    f.host
        .write_config(
            &json!({"quixi-workspace":{"command":"quixi-mcp-builtin","args":["--workspace"]}}),
        )
        .unwrap();
    assert!(discover().is_ok());
    f.host.disconnect_server("quixi-workspace").unwrap();
    assert!(discover().is_err());
}

#[test]
fn discovery_pages_bound_output_without_truncating_large_schemas() {
    let f = Fixture::new("large_catalog");
    let mut cursor = 0;
    let mut all = Vec::new();
    loop {
        let page = f
            .host
            .discover_tools(
                Some("fixture"),
                None,
                cursor,
                f.cwd(),
                &AtomicBool::new(false),
            )
            .unwrap();
        let tools = page["tools"].as_array().unwrap();
        assert!(!tools.is_empty() && tools.len() <= 20);
        if serde_json::to_vec(tools).unwrap().len() > 65536 {
            assert_eq!(
                tools.len(),
                1,
                "an oversized individual schema must have a dedicated page"
            );
        }
        all.extend(tools.iter().cloned());
        let Some(next) = page["nextCursor"].as_u64() else {
            break;
        };
        assert!(next as usize > cursor);
        cursor = next as usize;
    }
    assert_eq!(all.len(), 45);
    assert_eq!(
        all[0]["inputSchema"]["properties"]["text"]["description"]
            .as_str()
            .unwrap()
            .len(),
        70000
    );
    assert_eq!(all[44]["name"], "item_044");
    assert_eq!(f.count_requests("initialize"), 1);
    assert_eq!(f.count_requests("tools/list"), 1);
}
