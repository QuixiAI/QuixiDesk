//! MCP protocol features exercised against an independent stdio process.
use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SERVER: &str = r#"
import json,sys,os
log=sys.argv[1]
def record(v):
    with open(log,'a') as f: f.write(json.dumps(v)+'\n')
def send(v): print(json.dumps(dict(jsonrpc='2.0',**v)),flush=True)
def roots(): send(dict(id='roots-snapshot',method='roots/list',params={}))
record(dict(event='spawn'))
for line in sys.stdin:
    request=json.loads(line);method=request.get('method');params=request.get('params',{})
    record(request)
    if method=='notifications/initialized': roots();continue
    if method=='notifications/roots/list_changed': roots();continue
    if 'id' not in request or not method: continue
    result={}
    if method=='initialize':
        if os.path.exists(log+'.unavailable'): send(dict(id=request['id'],error=dict(code=-32000,message='503 temporarily unavailable')));continue
        result=dict(protocolVersion='2025-11-25',capabilities=dict(tools=dict(listChanged=True),resources=dict(subscribe=True,listChanged=True),prompts=dict(listChanged=True)),serverInfo=dict(name='parity-fixture',version='1'))
    elif method=='tools/list':
        result=dict(tools=[dict(name=name,description=name,inputSchema=dict(type='object',properties={},additionalProperties=False),**({'_meta':{'ui':{'visibility':['app']}}} if name=='app_only' else {})) for name in ['visible','app_only','disabled','crash']])
    elif method=='tools/call':
        if params['name']=='crash': sys.exit(0)
        result=dict(content=[dict(type='text',text='actual '+params['name'])],isError=False)
    elif method=='resources/list': result=dict(resources=[dict(name='document',uri='fixture://documents/one',mimeType='text/plain')])
    elif method=='resources/templates/list': result=dict(resourceTemplates=[dict(name='by-id',uriTemplate='fixture://documents/{id}{?view,tags*}',mimeType='text/plain')])
    elif method=='resources/read': result=dict(contents=[dict(uri=params['uri'],mimeType='text/plain',text='actual resource contents')])
    elif method in ['resources/subscribe','resources/unsubscribe']:
        if os.path.exists(log+'.subscription-error'): send(dict(id=request['id'],error=dict(code=-32602,message='Resource removed')));continue
    elif method=='prompts/list': result=dict(prompts=[dict(name='summarize',arguments=[dict(name='topic',required=True)])])
    elif method=='prompts/get': result=dict(messages=[dict(role='user',content=dict(type='text',text='Summarize '+params['arguments']['topic']))])
    else: send(dict(id=request['id'],error=dict(code=-32601,message='Unknown method')));continue
    send(dict(id=request['id'],result=result))
    if method=='resources/subscribe': send(dict(method='notifications/resources/updated',params=dict(uri=params['uri'])))
"#;
struct Fixture {
    host: McpHost,
    root: PathBuf,
    work: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "quixi-mcp-parity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let work = root.join("work");
        fs::create_dir_all(&work).unwrap();
        let script = root.join("peer.py");
        fs::write(&script, SERVER).unwrap();
        let host = McpHost::new_with_data_dir(&root.join("data"));
        let python = std::env::var("QUIXI_MCP_TEST_PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
        host.write_config(&json!({"fixture":{"command":python,"args":[script,root.join("events.jsonl")],"connectTimeoutMs":2000,"discoveryTimeoutMs":2000,"requestTimeoutMs":2000,"disabledTools":["disabled"]}})).unwrap();
        Self { host, root, work }
    }
    fn discover(&self) -> Value {
        self.host
            .discover_tools(
                Some("fixture"),
                None,
                0,
                &self.work,
                &AtomicBool::new(false),
            )
            .unwrap()
    }
    fn call(&self, name: &str) -> anyhow::Result<Value> {
        self.host.call_tool("fixture", name, &json!({}), &self.work)
    }
    fn events(&self) -> Vec<Value> {
        fs::read_to_string(self.root.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.host.shutdown();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn wait(mut check: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(6);
    while !check() {
        assert!(
            Instant::now() < until,
            "Timed out waiting for fixture protocol event"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn invalid_entry_is_isolated_and_raw_configuration_survives_healthy_edit() {
    let f = Fixture::new();
    f.discover();
    let mut config = f.host.read_config_value_checked().unwrap();
    config["mcpServers"]["broken"] = json!({"command":12,"future":{"keep":"verbatim"}});
    config["futureTopLevel"] = json!({"keep":42});
    fs::write(&f.host.config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(f
        .host
        .read_config_checked()
        .unwrap()
        .invalid_servers
        .contains_key("broken"));
    assert!(f
        .call("visible")
        .unwrap()
        .to_string()
        .contains("actual visible"));
    let statuses = f.host.list_local_mcp_servers();
    assert!(statuses
        .iter()
        .any(|s| s["name"] == "broken" && s["status"] == "failed"));
    config["mcpServers"]["fixture"]["description"] = json!("healthy edit");
    f.host.write_config(&config["mcpServers"]).unwrap();
    let saved = f.host.read_config_value_checked().unwrap();
    assert_eq!(
        saved["mcpServers"]["broken"],
        config["mcpServers"]["broken"]
    );
    assert_eq!(saved["futureTopLevel"], config["futureTopLevel"]);
    assert!(f.call("visible").is_ok());
}

#[test]
fn app_only_and_disabled_tools_are_hidden_from_models_and_never_execute() {
    let f = Fixture::new();
    let catalog = f.discover();
    for forbidden in ["app_only", "disabled"] {
        assert!(!catalog["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == forbidden));
        assert!(!f
            .host
            .ready_tools(&f.work)
            .iter()
            .any(|t| t["name"] == forbidden));
        assert!(f.call(forbidden).is_err());
    }
    assert!(f.call("visible").is_ok());
    let calls: Vec<_> = f
        .events()
        .into_iter()
        .filter(|e| e["method"] == "tools/call")
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["params"]["name"], "visible");
    let status = f
        .host
        .list_local_mcp_servers()
        .into_iter()
        .find(|s| s["name"] == "fixture")
        .unwrap();
    assert!(status["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "app_only" && t["modelVisible"] == false));
}

#[test]
fn resource_templates_prompts_and_subscription_notifications_roundtrip() {
    let f = Fixture::new();
    let catalog = f.host.browse_resources("fixture", &f.work).unwrap();
    assert_eq!(catalog["resources"][0]["uri"], "fixture://documents/one");
    assert_eq!(catalog["prompts"][0]["name"], "summarize");
    let expanded = f
        .host
        .expand_resource_template(
            "fixture",
            "fixture://documents/{id}{?view,tags*}",
            &json!({"id":"a b","view":"compact","tags":["x","y"]}),
            &f.work,
        )
        .unwrap();
    assert_eq!(
        expanded["uri"],
        "fixture://documents/a%20b?view=compact&tags=x&tags=y"
    );
    assert!(f
        .host
        .expand_resource_template(
            "fixture",
            "fixture://unadvertised/{id}",
            &json!({"id":"secret"}),
            &f.work
        )
        .is_err());
    let uri = expanded["uri"].as_str().unwrap();
    let result = f
        .host
        .read_resource_checked("fixture", uri, &f.work, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result["contents"][0]["text"], "actual resource contents");
    let prompt = f
        .host
        .get_prompt(
            "fixture",
            "summarize",
            &json!({"topic":"clubs"}),
            &f.work,
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(prompt["messages"][0]["content"]["text"], "Summarize clubs");
    f.host
        .subscribe_resource("fixture", uri, &f.work, true)
        .unwrap();
    wait(|| {
        f.host
            .resource_updates()
            .iter()
            .any(|u| u["uri"] == uri && u["version"] == 1)
    });
    f.host
        .subscribe_resource("fixture", uri, &f.work, false)
        .unwrap();
    assert!(f
        .events()
        .iter()
        .any(|e| e["method"] == "resources/unsubscribe" && e["params"]["uri"] == uri));
}

#[test]
fn roots_are_reported_notified_and_survive_transport_recovery_with_subscriptions() {
    let f = Fixture::new();
    f.discover();
    wait(|| {
        f.events()
            .iter()
            .any(|e| e["id"] == "roots-snapshot" && e["result"]["roots"].is_array())
    });
    let other = f.root.join("extra root");
    fs::create_dir_all(&other).unwrap();
    f.host
        .set_roots("fixture", &f.work, &[f.work.clone(), other.clone()])
        .unwrap();
    wait(|| {
        f.events().iter().any(|e| {
            e["id"] == "roots-snapshot"
                && e["result"]["roots"]
                    .as_array()
                    .is_some_and(|v| v.len() == 2)
        })
    });
    let uri = "fixture://documents/one";
    f.host
        .subscribe_resource("fixture", uri, &f.work, true)
        .unwrap();
    assert!(f.call("crash").is_err());
    assert!(f.call("visible").is_ok());
    wait(|| {
        f.events()
            .iter()
            .filter(|e| e["method"] == "resources/subscribe")
            .count()
            == 2
    });
    let roots = f
        .events()
        .into_iter()
        .filter(|e| e["id"] == "roots-snapshot" && e["result"]["roots"].is_array())
        .last()
        .unwrap();
    assert_eq!(roots["result"]["roots"].as_array().unwrap().len(), 2);
    assert!(f
        .host
        .set_roots("fixture", &f.work, &[f.root.join("absent")])
        .is_err());
}

#[test]
fn explicit_reconnect_restores_context_and_restart_restores_selected_roots() {
    let f = Fixture::new();
    f.discover();
    let other = f.root.join("other");
    fs::create_dir_all(&other).unwrap();
    f.host
        .set_roots("fixture", &f.work, &[other.clone()])
        .unwrap();
    f.host
        .subscribe_resource("fixture", "fixture://documents/one", &f.work, true)
        .unwrap();
    // A subscription acknowledgment precedes its notification on the wire.
    // Observe the old version before retiring that connection.
    wait(|| {
        f.host
            .resource_updates()
            .iter()
            .any(|u| u["uri"] == "fixture://documents/one" && u["version"] == 1)
    });
    f.host.reconnect_server("fixture", &f.work).unwrap();
    f.discover();
    wait(|| {
        f.events()
            .iter()
            .filter(|e| e["method"] == "resources/subscribe")
            .count()
            == 2
    });
    wait(|| {
        f.host.resource_updates().iter().any(|u| {
            u["uri"] == "fixture://documents/one" && u["version"].as_u64().unwrap_or(0) >= 2
        })
    });
    let catalog = f.host.browse_resources("fixture", &f.work).unwrap();
    assert_eq!(
        catalog["roots"][0]["path"],
        json!(other.canonicalize().unwrap())
    );
    fs::write(f.root.join("events.jsonl.subscription-error"), "").unwrap();
    f.host.reconnect_server("fixture", &f.work).unwrap();
    f.discover();
    assert!(
        f.call("visible").is_ok(),
        "Lost subscription cannot disable tools"
    );
    assert!(f
        .host
        .resource_updates()
        .iter()
        .any(|u| u["error"].is_string()));
    f.host.shutdown();
    let restored = McpHost::new_with_data_dir(&f.root.join("data"));
    let catalog = restored.browse_resources("fixture", &f.work).unwrap();
    assert_eq!(
        catalog["roots"][0]["path"],
        json!(other.canonicalize().unwrap())
    );
    restored.shutdown();
}

#[test]
fn crashed_transport_retries_boundedly_without_replaying_failed_action() {
    let f = Fixture::new();
    f.discover();
    fs::write(f.root.join("events.jsonl.unavailable"), "").unwrap();
    assert!(f.call("crash").is_err());
    wait(|| {
        f.host
            .list_server_statuses()
            .iter()
            .any(|s| s["name"] == "fixture" && s["circuitOpen"] == true && s["nextRetryAt"] == 0)
    });
    let spawns = f.events().iter().filter(|e| e["event"] == "spawn").count();
    assert_eq!(spawns, 4, "Initial process plus exactly three retries");
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        f.events().iter().filter(|e| e["event"] == "spawn").count(),
        spawns
    );
    assert_eq!(
        f.events()
            .iter()
            .filter(|e| e["method"] == "tools/call" && e["params"]["name"] == "crash")
            .count(),
        1
    );
    fs::remove_file(f.root.join("events.jsonl.unavailable")).unwrap();
    f.host.reconnect_server("fixture", &f.work).unwrap();
    assert!(f.call("visible").is_ok());
}

#[test]
fn root_config_rejects_invalid_protocol_shapes() {
    let f = Fixture::new();
    for roots in [
        json!({"uri":"file:///tmp"}),
        json!([{"uri":"https://example.com"}]),
        json!([{"uri":"file:///tmp","name":3}]),
    ] {
        assert!(f
            .host
            .write_config(&json!({"bad":{"command":"python3","roots":roots}}))
            .is_err());
    }
}
