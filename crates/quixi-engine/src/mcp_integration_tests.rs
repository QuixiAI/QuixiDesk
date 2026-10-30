//! Exercise the provider -> discovery -> external transport -> tool result path.
use super::*;
use std::io::{BufRead, Read, Write};
use std::time::{Duration, Instant};

// Compiled by the installed Rust toolchain, so these integration tests need no
// Python/Node installation and use the same fixture on Windows, Linux and macOS.
const MCP_FIXTURE: &str = r###"
use std::io::{BufRead, Write};
fn main() {
    let log = std::env::args().nth(1).unwrap();
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap();
    writeln!(file,"spawn").unwrap();
    let mut initialized = false;
    let mut calls = 0;
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let Some((_,id)) = line.split_once("\"id\":") else {
            if line.contains("notifications/initialized") { initialized = true; writeln!(file,"initialized").unwrap(); }
            continue;
        };
        let id = id.trim_start().split(|c| c == ',' || c == '}').next().unwrap();
        let result = if line.contains("\"initialize\"") {
            writeln!(file,"initialize").unwrap();
            r#"{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":true}},"serverInfo":{"name":"engine-fixture","version":"1"},"instructions":"Return real fixture results; never invent completion."}"#.to_owned()
        } else if line.contains("\"tools/list\"") && initialized {
            writeln!(file,"tools/list").unwrap();
            r#"{"tools":[{"name":"read_item","description":"Read the fixture token","inputSchema":{"type":"object","properties":{"key":{"type":"string","enum":["token"]}},"required":["key"],"additionalProperties":false},"outputSchema":{"type":"object","properties":{"value":{"type":"integer"},"calls":{"type":"integer"}},"required":["value","calls"]},"annotations":{"readOnlyHint":true}}]}"#.to_owned()
        } else if line.contains("\"tools/call\"") && initialized {
            calls += 1;
            writeln!(file,"tools/call {calls}").unwrap();
            format!(r#"{{"content":[{{"type":"text","text":"PROJECT TOKEN 42; call {calls}"}}],"structuredContent":{{"value":42,"calls":{calls}}},"isError":false}}"#)
        } else {
            println!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":-32600,\"message\":\"Unexpected request or handshake order\"}}}}");
            std::io::stdout().flush().unwrap(); continue;
        };
        println!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
        std::io::stdout().flush().unwrap();
    }
}
"###;

struct Fixture {
    engine: Arc<QuixiEngine>,
    root: PathBuf,
    session: String,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!("quixi-mcp-engine-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("work")).unwrap();
        let source = root.join("server.rs");
        fs::write(&source, MCP_FIXTURE).unwrap();
        let executable = root.join(if cfg!(windows) {
            "server.exe"
        } else {
            "server"
        });
        let built = Command::new("rustc")
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Rust toolchain is required for MCP integration fixtures");
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let engine = QuixiEngine::new_with_data_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            root.join("data"),
        );
        engine
            .mcp_host
            .write_config(&json!({"engine-fixture":{
                "command":executable,"args":[root.join("events.log")]
            }}))
            .unwrap();
        let session =
            engine.start_agent_session(&json!({"cwd":root.join("work"),"permissionMode":mode}));
        Self {
            engine,
            root,
            session,
        }
    }
    fn cwd(&self) -> String {
        self.root.join("work").to_string_lossy().into_owned()
    }
    fn events(&self) -> String {
        fs::read_to_string(self.root.join("events.log")).unwrap_or_default()
    }
}

fn tool(id: &str, name: &str, args: Value) -> Value {
    json!({"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]})
}

fn provider(engine: &QuixiEngine, replies: Vec<Value>) -> std::thread::JoinHandle<Vec<Value>> {
    provider_format(engine, replies, "openai")
}
fn provider_format(
    engine: &QuixiEngine,
    replies: Vec<Value>,
    format: &'static str,
) -> std::thread::JoinHandle<Vec<Value>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    {
        let mut state = engine.provider_manager.state.write();
        state.active_provider = "custom".into();
        state.active_model = "mcp-fixture-model".into();
        let p = state
            .providers
            .iter_mut()
            .find(|p| p.id == "custom")
            .unwrap();
        p.configured = true;
        p.selected_model = "mcp-fixture-model".into();
        p.base_url = format!("http://{addr}/v1");
        p.api_format = format.into();
        p.auth_mode = "local".into();
        p.models.clear();
    }
    std::thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(Instant::now() < deadline, "provider request timed out");
                std::thread::sleep(Duration::from_millis(10));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            requests.push(serde_json::from_slice(&body).unwrap());
            if let Some(status) = reply["_httpStatus"].as_u64() {
                let body = reply["error"].to_string();
                write!(stream,"HTTP/1.1 {status} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                continue;
            }
            let finish = if reply.get("tool_calls").is_some() {
                "tool_calls"
            } else {
                "stop"
            };
            let body = if format == "anthropic" {
                let block = if let Some(call) =
                    reply["tool_calls"].as_array().and_then(|v| v.first())
                {
                    json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":{}})
                } else {
                    json!({"type":"text","text":""})
                };
                let delta = if block["type"] == "tool_use" {
                    json!({"type":"input_json_delta","partial_json":reply["tool_calls"][0]["function"]["arguments"]})
                } else {
                    json!({"type":"text_delta","text":reply["content"]})
                };
                [json!({"type":"message_start","message":{"role":"assistant","content":[]}}),json!({"type":"content_block_start","index":0,"content_block":block}),json!({"type":"content_block_delta","index":0,"delta":delta}),json!({"type":"content_block_stop","index":0}),json!({"type":"message_delta","delta":{"stop_reason":if finish=="tool_calls"{"tool_use"}else{"end_turn"}}}),json!({"type":"message_stop"})].iter().map(|event|format!("data: {event}\n\n")).collect::<String>()
            } else {
                format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    json!({"choices":[{"delta":reply,"finish_reason":finish}]})
                )
            };
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    })
}

#[test]
fn provider_discovers_real_schemas_then_calls_persistent_external_server() {
    let fixture = Fixture::new("auto");
    let worker = provider(
        &fixture.engine,
        vec![
            tool(
                "discover",
                "McpDiscover",
                json!({"server":"engine-fixture"}),
            ),
            tool(
                "read-one",
                "McpCall",
                json!({"server":"engine-fixture","tool":"read_item","arguments":{"key":"token"}}),
            ),
            tool(
                "read-two",
                "McpCall",
                json!({"server":"engine-fixture","tool":"read_item","arguments":{"key":"token"}}),
            ),
            json!({"content":"The real server returned token 42 twice."}),
        ],
    );
    let (answer, _) = fixture.engine.generate_reply_with_tools_streaming(
        "Discover engine-fixture, inspect its schema, and read the token twice.",
        &fixture.cwd(),
        &[],
        Some("mcp-fixture-model"),
        "auto",
        &fixture.session,
        &AtomicBool::new(false),
        |_| {},
        |_| {},
    );
    assert_eq!(answer, "The real server returned token 42 twice.");
    let requests = worker.join().unwrap();
    let discovery = requests[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap();
    let discovery: Value = serde_json::from_str(discovery["content"].as_str().unwrap()).unwrap();
    let schema = discovery["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "read_item")
        .unwrap();
    assert_eq!(
        schema["inputSchema"]["properties"]["key"]["enum"],
        json!(["token"])
    );
    assert_eq!(schema["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        schema["outputSchema"]["required"],
        json!(["value", "calls"])
    );
    assert!(requests[3].to_string().contains("call 2"));
    let events = fixture.events();
    assert_eq!(events.lines().filter(|l| *l == "spawn").count(), 1);
    assert_eq!(events.lines().filter(|l| *l == "initialize").count(), 1);
    assert!(
        events.contains("initialize\ninitialized\ntools/list"),
        "{events}"
    );
    assert!(events.contains("tools/call 1\ntools/call 2"), "{events}");
}

#[test]
fn discovery_is_manual_safe_but_server_readonly_annotation_does_not_grant_approval() {
    let fixture = Fixture::new("default");
    let discovered = fixture.engine.execute_tool_with_approval(
        &fixture.session,
        "discover",
        "McpDiscover",
        &json!({"server":"engine-fixture"}),
        &fixture.cwd(),
        "default",
        &AtomicBool::new(false),
    );
    assert!(!discovered.1, "{}", discovered.0);
    assert!(fixture
        .engine
        .pending_tool_approvals(&fixture.session)
        .as_array()
        .unwrap()
        .is_empty());
    let engine = fixture.engine.clone();
    let session = fixture.session.clone();
    let cwd = fixture.cwd();
    let signal = fixture.engine.begin_turn(&fixture.session);
    let worker = std::thread::spawn(move || {
        engine.execute_tool_with_approval(
            &session,
            "external-read",
            "McpCall",
            &json!({"server":"engine-fixture","tool":"read_item","arguments":{"key":"token"}}),
            &cwd,
            "default",
            &signal,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if !fixture
            .engine
            .pending_tool_approvals(&fixture.session)
            .as_array()
            .unwrap()
            .is_empty()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "external tool did not request approval"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!fixture.events().contains("tools/call"));
    fixture.engine.cancel_turn(&fixture.session);
    assert!(worker.join().unwrap().1);
    assert!(!fixture.events().contains("tools/call"));
}

#[test]
fn discovery_does_not_launch_without_server_and_protocol_methods_are_rejected() {
    let fixture = Fixture::new("auto");
    let (out, failed) = agent_tools::execute(
        "McpDiscover",
        &json!({}),
        &fixture.cwd(),
        "default",
        &fixture.engine.mcp_host,
    );
    assert!(!failed, "{out}");
    assert!(out.contains("engine-fixture"));
    assert!(fixture.events().is_empty());
    let (out, failed) = agent_tools::execute(
        "McpCall",
        &json!({"server":"engine-fixture","tool":"tools/list"}),
        &fixture.cwd(),
        "auto",
        &fixture.engine.mcp_host,
    );
    assert!(failed);
    assert!(out.contains("McpDiscover"));
    assert!(fixture.events().is_empty());
    let (out, failed) = agent_tools::execute_cancellable(
        "McpDiscover",
        &json!({"server":"engine-fixture"}),
        &fixture.cwd(),
        "auto",
        &fixture.engine.mcp_host,
        &AtomicBool::new(true),
    );
    assert!(failed);
    assert!(out.contains("stopped"));
    assert!(fixture.events().is_empty());
}

#[test]
fn external_override_of_builtin_name_cannot_bypass_manual_approval() {
    let fixture = Fixture::new("default");
    fixture
        .engine
        .mcp_host
        .write_config(&json!({"computer-use":{
            "command":"not-the-built-in-helper","args":[]
        }}))
        .unwrap();
    let input = json!({"server":"computer-use","tool":"screenshot"});
    assert!(agent_tools::requires_approval_with_host(
        "McpCall",
        &input,
        "default",
        &fixture.engine.mcp_host
    ));
    let (message, failed) = agent_tools::execute(
        "McpCall",
        &input,
        &fixture.cwd(),
        "default",
        &fixture.engine.mcp_host,
    );
    assert!(failed);
    assert!(message.contains("Approval required"));
    assert!(fixture.events().is_empty());
}

#[test]
fn pending_approval_cannot_authorize_a_replacement_server_configuration() {
    let fixture = Fixture::new("default");
    let engine = fixture.engine.clone();
    let session = fixture.session.clone();
    let cwd = fixture.cwd();
    let cancelled = fixture.engine.begin_turn(&fixture.session);
    let worker = std::thread::spawn(move || {
        engine.execute_tool_with_approval(
            &session,
            "bound-approval",
            "McpCall",
            &json!({"server":"engine-fixture","tool":"read_item","arguments":{"key":"token"}}),
            &cwd,
            "default",
            &cancelled,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let approval = loop {
        let pending = fixture.engine.pending_tool_approvals(&fixture.session);
        if let Some(request) = pending.as_array().unwrap().first() {
            break request.clone();
        }
        assert!(Instant::now() < deadline, "MCP approval did not appear");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(approval["mcpServerIdentity"].is_string());
    let mut config = fixture.engine.mcp_host.read_config_checked().unwrap();
    config
        .mcp_servers
        .get_mut("engine-fixture")
        .unwrap()
        .env
        .insert("CHANGED_TARGET".into(), "replacement".into());
    fixture
        .engine
        .mcp_host
        .write_config(&serde_json::to_value(config.mcp_servers).unwrap())
        .unwrap();
    assert_eq!(
        fixture.engine.resolve_tool_approval(
            &fixture.session,
            approval["requestId"].as_str().unwrap(),
            "allow_once"
        )["ok"],
        true
    );
    let (message, failed) = worker.join().unwrap();
    assert!(failed, "{message}");
    assert!(message.to_lowercase().contains("chang"), "{message}");
    assert!(
        fixture.events().is_empty(),
        "A replacement target must not launch under stale consent"
    );
}

#[test]
fn discovered_mcp_tools_are_native_functions_on_both_provider_protocols() {
    for format in ["openai", "anthropic"] {
        let fixture = Fixture::new("auto");
        let name = crate::mcp_registry::alias("engine-fixture", "read_item");
        let worker = provider_format(
            &fixture.engine,
            vec![
                tool(
                    "discover",
                    "McpDiscover",
                    json!({"server":"engine-fixture"}),
                ),
                tool("direct", &name, json!({"key":"token"})),
                json!({"content":"Token 42 from the real server."}),
            ],
            format,
        );
        let (answer, _) = fixture.engine.generate_reply_with_tools_streaming(
            "Inspect the fixture token.",
            &fixture.cwd(),
            &[],
            Some("mcp-fixture-model"),
            "auto",
            &fixture.session,
            &AtomicBool::new(false),
            |_| {},
            |_| {},
        );
        assert_eq!(answer, "Token 42 from the real server.");
        let requests = worker.join().unwrap();
        let lookup = |request: &Value| -> Option<Value> {
            request["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| {
                    if format == "anthropic" {
                        t["name"] == name
                    } else {
                        t["function"]["name"] == name
                    }
                })
                .cloned()
        };
        assert!(
            lookup(&requests[0]).is_none(),
            "Exposure must not start servers"
        );
        let exposed = lookup(&requests[1]).expect("Next round must refresh the catalog");
        let schema = if format == "anthropic" {
            &exposed["input_schema"]
        } else {
            &exposed["function"]["parameters"]
        };
        assert_eq!(schema["properties"]["key"]["enum"], json!(["token"]));
        assert_eq!(schema["additionalProperties"], false);
        assert!(requests[2].to_string().contains("PROJECT TOKEN 42"));
        assert_eq!(
            fixture
                .events()
                .lines()
                .filter(|l| *l == "tools/call 1")
                .count(),
            1
        );
        if format == "anthropic" {
            assert!(requests[2]["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|b| b["type"] == "tool_result" && b["tool_use_id"] == "direct")));
        }
    }
}

#[test]
fn disabling_an_advertised_tool_removes_it_and_blocks_both_routes() {
    let fixture = Fixture::new("auto");
    let discover = json!({"server":"engine-fixture"});
    assert!(
        !fixture
            .engine
            .execute_tool_with_approval(
                &fixture.session,
                "discover",
                "McpDiscover",
                &discover,
                &fixture.cwd(),
                "auto",
                &AtomicBool::new(false)
            )
            .1
    );
    let registry = crate::mcp_registry::Registry::build(
        fixture
            .engine
            .mcp_host
            .ready_tools(Path::new(&fixture.cwd())),
        &[],
        Default::default(),
    );
    let alias = crate::mcp_registry::alias("engine-fixture", "read_item");
    let route = registry.route(&alias).unwrap();
    let mut config = fixture.engine.mcp_host.read_config_value_checked().unwrap();
    config["mcpServers"]["engine-fixture"]["disabledTools"] = json!(["read_item"]);
    fixture
        .engine
        .mcp_host
        .write_config(&config["mcpServers"])
        .unwrap();
    let fresh = crate::mcp_registry::Registry::build(
        fixture
            .engine
            .mcp_host
            .ready_tools(Path::new(&fixture.cwd())),
        &[],
        Default::default(),
    );
    assert!(fresh.route(&alias).is_none());
    let input = route.input(&json!({"key":"token"}));
    assert!(
        fixture
            .engine
            .execute_tool_with_approval_and_mcp_identity(
                &fixture.session,
                "direct",
                "McpCall",
                &input,
                &fixture.cwd(),
                "auto",
                &AtomicBool::new(false),
                Some(&route.identity)
            )
            .1
    );
    assert!(
        fixture
            .engine
            .execute_tool_with_approval(
                &fixture.session,
                "generic",
                "McpCall",
                &input,
                &fixture.cwd(),
                "auto",
                &AtomicBool::new(false)
            )
            .1
    );
    assert!(!fixture.events().contains("tools/call"));
}

#[test]
fn colliding_display_names_route_to_distinct_real_server_processes() {
    let fixture = Fixture::new("auto");
    let config = fixture.engine.mcp_host.read_config_value_checked().unwrap();
    let first = config["mcpServers"]["engine-fixture"].clone();
    let mut second = first.clone();
    second["args"][0] = json!(fixture.root.join("second.log"));
    fixture
        .engine
        .mcp_host
        .write_config(&json!({"a b":first,"a_b":second}))
        .unwrap();
    for server in ["a b", "a_b"] {
        assert!(
            !fixture
                .engine
                .execute_tool_with_approval(
                    &fixture.session,
                    server,
                    "McpDiscover",
                    &json!({"server":server}),
                    &fixture.cwd(),
                    "auto",
                    &AtomicBool::new(false)
                )
                .1
        );
    }
    let first = crate::mcp_registry::alias("a b", "read_item");
    let second = crate::mcp_registry::alias("a_b", "read_item");
    assert_ne!(first, second);
    let worker = provider(
        &fixture.engine,
        vec![
            tool("first", &first, json!({"key":"token"})),
            tool("second", &second, json!({"key":"token"})),
            json!({"content":"Both servers returned their own result."}),
        ],
    );
    let (answer, _) = fixture.engine.generate_reply_with_tools_streaming(
        "Read both tokens.",
        &fixture.cwd(),
        &[],
        Some("mcp-fixture-model"),
        "auto",
        &fixture.session,
        &AtomicBool::new(false),
        |_| {},
        |_| {},
    );
    assert_eq!(answer, "Both servers returned their own result.");
    let requests = worker.join().unwrap();
    for name in [&first, &second] {
        assert!(requests[0]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == name.as_str()));
    }
    for log in [
        fixture.events(),
        fs::read_to_string(fixture.root.join("second.log")).unwrap(),
    ] {
        assert!(log.contains("tools/call 1"));
        assert!(!log.contains("tools/call 2"));
    }
}

#[test]
fn provider_schema_rejection_defers_whole_catalog_without_losing_generic_tools() {
    let fixture = Fixture::new("auto");
    let worker = provider(
        &fixture.engine,
        vec![
            tool(
                "discover",
                "McpDiscover",
                json!({"server":"engine-fixture"}),
            ),
            json!({"_httpStatus":400,"error":{"message":"Unsupported tool schema keyword"}}),
            tool(
                "generic",
                "McpCall",
                json!({"server":"engine-fixture","tool":"read_item","arguments":{"key":"token"}}),
            ),
            json!({"content":"Recovered with the full schema."}),
        ],
    );
    let (answer, _) = fixture.engine.generate_reply_with_tools_streaming(
        "Read the fixture token.",
        &fixture.cwd(),
        &[],
        Some("mcp-fixture-model"),
        "auto",
        &fixture.session,
        &AtomicBool::new(false),
        |_| {},
        |_| {},
    );
    assert_eq!(answer, "Recovered with the full schema.");
    let requests = worker.join().unwrap();
    let alias = crate::mcp_registry::alias("engine-fixture", "read_item");
    assert!(requests[1]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["function"]["name"] == alias));
    assert!(!requests[2]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["function"]["name"] == alias));
    for name in ["McpDiscover", "McpCall"] {
        assert!(requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == name));
    }
    assert_eq!(
        fixture
            .events()
            .lines()
            .filter(|l| l.starts_with("tools/call"))
            .count(),
        1
    );
}
