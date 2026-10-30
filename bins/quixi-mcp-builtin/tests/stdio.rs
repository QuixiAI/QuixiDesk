use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "quixi-builtin-stdio-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("token.txt"), "REAL WORKSPACE TOKEN").unwrap();
        Self(root)
    }
    fn run(&self, mode: &str, requests: Vec<Value>, malformed: bool) -> Vec<Value> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_quixi-mcp-builtin"))
            .arg(mode)
            .current_dir(&self.0)
            .env("QUIXI_TEST_PROFILE", self.0.join("profile"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        for request in requests {
            writeln!(input, "{request}").unwrap();
        }
        if malformed {
            writeln!(input, "not json").unwrap();
        }
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn initialize() -> Value {
    request(
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    )
}
fn initialized() -> Value {
    json!({"jsonrpc":"2.0","method":"notifications/initialized"})
}

#[test]
fn real_stdio_requires_ordered_handshake_and_negotiates_supported_versions() {
    let fixture = Fixture::new();
    let responses = fixture.run(
        "--workspace",
        vec![
            request(10, "tools/list", json!({})),
            initialized(),
            request(
                11,
                "initialize",
                json!({"protocolVersion":"unsupported-future-version","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
            ),
            request(12, "tools/list", json!({})),
            initialized(),
            request(13, "tools/list", json!({})),
            request(14, "initialize", json!({"protocolVersion":"2025-11-25"})),
            request(15, "unknown/method", json!({})),
            request(16, "ping", json!({})),
        ],
        true,
    );
    assert_eq!(responses.len(), 8);
    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[1]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        responses[1]["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(responses[1]["result"]["capabilities"]
        .get("resources")
        .is_none());
    assert_eq!(responses[2]["error"]["code"], -32002);
    assert!(responses[3]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "read_file"));
    assert_eq!(responses[4]["error"]["code"], -32600);
    assert_eq!(responses[5]["error"]["code"], -32601);
    assert_eq!(responses[6]["result"], json!({}));
    assert_eq!(responses[7]["error"]["code"], -32700);
}

#[test]
fn real_stdio_enforces_mode_catalog_and_schemas_before_side_effects() {
    let fixture = Fixture::new();
    let responses=fixture.run("--workspace",vec![initialize(),initialized(),
        request(2,"tools/call",json!({"name":"read_file","arguments":{"path":"token.txt"}})),
        request(3,"tools/call",json!({"name":"left_click","arguments":{"coordinate":[0,0]}})),
        request(4,"tools/call",json!({"name":"write_file","arguments":{"path":"must-not-exist.txt"}})),
        request(5,"tools/call",json!({"name":"write_file","arguments":{"path":"written.txt","content":"ACTUALLY WRITTEN"}})),
    ],false);
    assert_eq!(responses.len(), 5);
    assert!(responses[1]["result"]
        .to_string()
        .contains("REAL WORKSPACE TOKEN"));
    assert_eq!(responses[2]["error"]["code"], -32602);
    assert_eq!(responses[3]["result"]["isError"], true);
    assert!(!fixture.0.join("must-not-exist.txt").exists());
    assert_eq!(
        fs::read_to_string(fixture.0.join("written.txt")).unwrap(),
        "ACTUALLY WRITTEN"
    );
    let denied = fixture.run(
        "--computer-use",
        vec![
            initialize(),
            initialized(),
            request(
                2,
                "tools/call",
                json!({"name":"write_file","arguments":{"path":"outside-mode.txt","content":"NO"}}),
            ),
        ],
        false,
    );
    assert_eq!(denied[1]["error"]["code"], -32602);
    assert!(!fixture.0.join("outside-mode.txt").exists());
}

#[test]
fn unavailable_legacy_helpers_expose_no_fake_tools() {
    for mode in ["--github", "--office365", "--pdf"] {
        let fixture = Fixture::new();
        let replies = fixture.run(
            mode,
            vec![
                initialize(),
                initialized(),
                request(2, "tools/list", json!({})),
                request(
                    3,
                    "tools/call",
                    json!({"name":"read_file","arguments":{"path":"token.txt"}}),
                ),
            ],
            false,
        );
        assert_eq!(replies[1]["result"]["tools"], json!([]), "{mode}");
        assert_eq!(replies[2]["error"]["code"], -32602, "{mode}");
    }
}

#[test]
fn in_app_browser_and_memory_flags_use_their_own_real_catalogs() {
    for (mode, expected) in [
        ("--in-app-browser", "tabs_create"),
        ("--memory", "recall_memory"),
    ] {
        let fixture = Fixture::new();
        let replies = fixture.run(
            mode,
            vec![
                initialize(),
                initialized(),
                request(2, "tools/list", json!({})),
            ],
            false,
        );
        let tools = replies[1]["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|tool| tool["name"] == expected), "{mode}");
        assert!(!tools
            .iter()
            .any(|tool| tool["name"] == "show_widget" || tool["name"] == "gif_creator"));
    }
}
