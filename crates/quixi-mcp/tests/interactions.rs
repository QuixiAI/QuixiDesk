//! End-to-end server-to-user requests through the actual SDK stdio transport.
use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    host: Arc<McpHost>,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "quixi-interactions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let host = Arc::new(McpHost::new_with_data_dir(&root.join("data")));
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/mcp/interaction_fixture.py")
            .canonicalize()
            .unwrap();
        let python = std::env::var("QUIXI_MCP_TEST_PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
        host.write_config(&json!({"fixture":{"command":python,"args":[script],"connectTimeoutMs":2000,"discoveryTimeoutMs":2000,"requestTimeoutMs":5000}})).unwrap();
        host.discover_tools(Some("fixture"), None, 0, &root, &AtomicBool::new(false))
            .unwrap();
        Self { host, root }
    }
    fn start(
        &self,
        name: &str,
        cancel: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<anyhow::Result<Value>> {
        let host = self.host.clone();
        let root = self.root.clone();
        let name = name.to_owned();
        std::thread::spawn(move || {
            host.call_tool_cancellable("fixture", &name, &json!({}), &root, &cancel)
        })
    }
    fn pending(&self) -> Value {
        wait(|| !self.host.pending_interactions().is_empty());
        self.host.pending_interactions().into_iter().next().unwrap()
    }
    fn call(&self, name: &str) -> anyhow::Result<Value> {
        self.host.call_tool("fixture", name, &json!({}), &self.root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.host.disconnect_server("fixture");
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn wait(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(8);
    while !predicate() {
        assert!(
            Instant::now() < end,
            "Timed out waiting for MCP interaction"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn server_form_is_explicit_validated_and_reaches_real_peer() {
    let f = Fixture::new();
    let call = f.start("ask_form", Arc::new(AtomicBool::new(false)));
    let row = f.pending();
    let id = row["id"].as_str().unwrap();
    assert_eq!(row["server"], "fixture");
    assert_eq!(row["mode"], "form");
    assert!(f
        .host
        .respond_interaction(id, "accept", Some(json!({"nickname":"x"})))
        .is_err());
    assert_eq!(f.host.pending_interactions().len(), 1);
    f.host
        .respond_interaction(id, "accept", Some(json!({"nickname":"Sam"})))
        .unwrap();
    assert_eq!(
        call.join().unwrap().unwrap()["structuredContent"]["response"],
        json!({"action":"accept","content":{"nickname":"Sam"}})
    );
    assert!(f.host.pending_interactions().is_empty());
    assert!(f
        .host
        .respond_interaction(id, "accept", Some(json!({"nickname":"Again"})))
        .is_err());
}
#[test]
fn form_decline_and_cancel_send_no_values() {
    let f = Fixture::new();
    for action in ["decline", "cancel"] {
        let call = f.start("ask_form", Arc::new(AtomicBool::new(false)));
        let row = f.pending();
        f.host
            .respond_interaction(
                row["id"].as_str().unwrap(),
                action,
                Some(json!({"nickname":"ignored"})),
            )
            .unwrap();
        assert_eq!(
            call.join().unwrap().unwrap()["structuredContent"]["response"],
            json!({"action":action})
        );
    }
}
#[test]
fn stopping_tool_and_disconnecting_cancel_pending_forms() {
    let f = Fixture::new();
    let stop = Arc::new(AtomicBool::new(false));
    let call = f.start("ask_form", stop.clone());
    let _ = f.pending();
    stop.store(true, Ordering::SeqCst);
    assert!(call.join().unwrap().is_err());
    wait(|| f.host.pending_interactions().is_empty());
    let call = f.start("ask_form", Arc::new(AtomicBool::new(false)));
    let _ = f.pending();
    f.host.disconnect_server("fixture").unwrap();
    assert!(call.join().unwrap().is_err());
    wait(|| f.host.pending_interactions().is_empty());
}
#[test]
fn url_consent_waits_for_scoped_completion_and_does_not_submit_content() {
    let f = Fixture::new();
    let call = f.start("ask_url", Arc::new(AtomicBool::new(false)));
    let row = f.pending();
    let id = row["id"].as_str().unwrap();
    assert_eq!(row["host"], "example.com");
    assert_eq!(row["status"], "pending");
    let response = f
        .host
        .respond_interaction(id, "accept", Some(json!({"ignored":"value"})))
        .unwrap();
    assert_eq!(response["url"], "https://example.com/fixture");
    assert_eq!(
        call.join().unwrap().unwrap()["structuredContent"]["response"],
        json!({"action":"accept"})
    );
    assert_eq!(f.pending()["status"], "waiting_external");
    f.call("complete_url").unwrap();
    wait(|| {
        f.host
            .pending_interactions()
            .iter()
            .any(|v| v["status"] == "completed")
    });
    assert!(f.pending().get("url").is_none());
}
#[test]
fn url_required_error_is_actionable_without_automatic_tool_replay() {
    let f = Fixture::new();
    assert!(f.call("url_required").is_err());
    let row = f.pending();
    assert_eq!(row["requiresRetry"], true);
    f.host
        .respond_interaction(row["id"].as_str().unwrap(), "accept", None)
        .unwrap();
    assert_eq!(
        f.call("count_calls").unwrap()["structuredContent"]["calls"],
        2
    );
    assert_eq!(f.pending()["status"], "waiting_external");
}
#[test]
fn secret_forms_and_unsafe_urls_never_reach_ui() {
    let f = Fixture::new();
    for name in ["secret_form", "unsafe_url"] {
        let result = f.call(name).unwrap();
        assert!(result["structuredContent"]["error"].is_object());
        assert!(f.host.pending_interactions().is_empty());
    }
}

#[test]
fn received_constraints_survive_sdk_deserialization_without_weakening() {
    let f = Fixture::new();
    let call = f.start("ask_constraints", Arc::new(AtomicBool::new(false)));
    let row = f.pending();
    let id = row["id"].as_str().unwrap();
    assert_eq!(
        row["requestedSchema"]["properties"]["code"]["pattern"],
        "^[A-Z]{3}$"
    );
    assert_eq!(
        row["requestedSchema"]["properties"]["choices"]["uniqueItems"],
        true
    );
    assert_eq!(row["requestedSchema"]["additionalProperties"], false);
    for invalid in [
        json!({"code":"bad","choices":["one"],"count":2}),
        json!({"code":"ABC","choices":["one","one"],"count":2}),
        json!({"code":"ABC","choices":["one"],"count":3}),
    ] {
        assert!(f
            .host
            .respond_interaction(id, "accept", Some(invalid))
            .is_err());
    }
    let valid = json!({"code":"ABC","choices":["one","two"],"count":4});
    f.host
        .respond_interaction(id, "accept", Some(valid.clone()))
        .unwrap();
    assert_eq!(
        call.join().unwrap().unwrap()["structuredContent"]["response"]["content"],
        valid
    );
    let result = f.call("unsupported_form").unwrap();
    assert!(result["structuredContent"]["error"].is_object());
    assert!(f.host.pending_interactions().is_empty());
}

#[test]
fn native_oauth_sign_in_authenticates_http_mcp_and_survives_host_restart() {
    use std::process::{Child, Command, Stdio};
    struct Server(Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = tempfile::tempdir().unwrap();
    // Headless integration runs must use the isolated test credential store,
    // not require an unlocked platform keychain over SSH.
    quixi_proto::paths::configure_test_profile(root.path()).unwrap();
    let port_file = root.path().join("port");
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/mcp/oauth_gui_fixture.py");
    let python = std::env::var("QUIXI_MCP_TEST_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
    let _server = Server(
        Command::new(python)
            .arg(script)
            .arg("--port-file")
            .arg(&port_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait(|| {
        fs::read_to_string(&port_file)
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .is_some()
    });
    let base = format!(
        "http://127.0.0.1:{}",
        fs::read_to_string(&port_file).unwrap()
    );
    let profile = root.path().join("data");
    let host = McpHost::new_with_data_dir(&profile);
    host.write_config(&json!({"oauth":{"transport":"http","url":format!("{base}/mcp"),"connectTimeoutMs":2000,"discoveryTimeoutMs":2000,"requestTimeoutMs":2000}})).unwrap();
    assert!(host
        .discover_tools(Some("oauth"), None, 0, root.path(), &AtomicBool::new(false))
        .is_err());
    assert_eq!(host.list_local_mcp_servers()[0]["status"], "needs_auth");
    host.authenticate_server("oauth", root.path()).unwrap();
    wait(|| host.list_local_mcp_servers()[0]["authorizationUrl"].is_string());
    let authorize = host.list_local_mcp_servers()[0]["authorizationUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let http = reqwest::Client::new();
        let page = http
            .get(&authorize)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(page.contains("Approve connection"));
        let ticket = page
            .split("name=\"ticket\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let reply = http
            .post(format!("{base}/authorize"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("ticket", ticket)
                    .append_pair("decision", "approve")
                    .finish(),
            )
            .send()
            .await
            .unwrap();
        let status = reply.status();
        let body = reply.text().await.unwrap();
        assert!(
            status.is_success(),
            "OAuth callback returned {status}: {body}"
        );
    });
    wait(|| host.list_local_mcp_servers()[0]["status"] == "ready");
    let response = host
        .call_tool(
            "oauth",
            "echo",
            &json!({"text":"native sign-in works"}),
            root.path(),
        )
        .unwrap();
    assert_eq!(response["content"][0]["text"], "native sign-in works");
    host.disconnect_server("oauth").unwrap();
    drop(host);
    let restored = McpHost::new_with_data_dir(&profile);
    restored
        .discover_tools(Some("oauth"), None, 0, root.path(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        restored
            .list_local_mcp_servers()
            .into_iter()
            .find(|row| row["name"] == "oauth")
            .unwrap()["status"],
        "ready"
    );
    restored.clear_server_auth("oauth").unwrap();
    assert!(restored
        .discover_tools(Some("oauth"), None, 0, root.path(), &AtomicBool::new(false))
        .is_err());
    assert_eq!(
        restored
            .list_local_mcp_servers()
            .into_iter()
            .find(|row| row["name"] == "oauth")
            .unwrap()["status"],
        "needs_auth"
    );
}
