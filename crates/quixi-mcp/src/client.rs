//! Persistent MCP clients. Catalogs are published atomically only after complete discovery.
use super::McpServerConfigEntry;
use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use rmcp::{
    model::*,
    service::{
        ClientLifecycleMode, ClientServiceExt, NotificationContext, Peer, PeerRequestOptions,
        RequestContext,
    },
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
        TokioChildProcess,
    },
    ClientHandler, RoleClient,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, OnceLock, Weak,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

pub(crate) fn init_tls() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("quixi-mcp")
            .build()
            .expect("MCP runtime")
    })
}
pub(crate) fn block_on<F: Future + Send>(future: F) -> F::Output
where
    F::Output: Send,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::scope(|s| {
            s.spawn(|| runtime().block_on(future))
                .join()
                .expect("MCP worker")
        })
    } else {
        runtime().block_on(future)
    }
}
fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn deadline(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[derive(Clone)]
pub(crate) struct Live {
    id: u64,
    pub peer: Peer<RoleClient>,
    pub stop: CancellationToken,
    closed: CancellationToken,
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub status: String,
    pub tools: Vec<Value>,
    pub resources: Vec<Value>,
    pub prompts: Vec<Value>,
    pub capabilities: Value,
    pub server_info: Value,
    pub instructions: String,
    pub error: Option<Value>,
    pub stale: bool,
    pub enumerated: bool,
    pub last_success: Option<u128>,
    pub generation: u64,
    pub stderr: String,
    pub updated: u128,
    pub auth_url: Option<String>,
    pub auth_attempt: Option<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: "configured".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            capabilities: json!({}),
            server_info: Value::Null,
            instructions: String::new(),
            error: None,
            stale: false,
            enumerated: false,
            last_success: None,
            generation: 0,
            stderr: String::new(),
            updated: now(),
            auth_url: None,
            auth_attempt: None,
        }
    }
}
pub(crate) struct Entry {
    pub profile: PathBuf,
    auth: Mutex<Option<CancellationToken>>,
    auth_challenge: Mutex<Option<String>>,
    active: AtomicUsize,
    last_used: AtomicU64,
    connection: AtomicU64,
    connection_stop: Mutex<CancellationToken>,
    authenticated: AtomicBool,
    pub name: String,
    pub config: McpServerConfigEntry,
    pub cwd: PathBuf,
    pub snapshot: Mutex<Snapshot>,
    live: tokio::sync::Mutex<Option<Live>>,
    operation: tokio::sync::Mutex<()>,
    pub retired: CancellationToken,
    changed: AtomicU64,
    refreshed: AtomicU64,
}
impl Entry {
    fn remember_challenge(&self, error: &anyhow::Error) {
        for e in error.chain() {
            if let Some(e) = e.downcast_ref::<rmcp::service::ClientInitializeError>() {
                if let Some(challenge) = e.auth_challenge() {
                    *self.auth_challenge.lock() = Some(challenge.into());
                }
            }
            if let Some(e) =
                e.downcast_ref::<rmcp::transport::streamable_http_client::AuthRequiredError>()
            {
                *self.auth_challenge.lock() = Some(e.www_authenticate_header.clone());
            }
            if let Some(e) =
                e.downcast_ref::<rmcp::transport::streamable_http_client::InsufficientScopeError>()
            {
                *self.auth_challenge.lock() = Some(e.www_authenticate_header.clone());
            }
        }
    }

    fn state_for(&self, id: u64, state: &str) -> Result<()> {
        let mut s = self.snapshot.lock();
        if self.connection.load(Ordering::SeqCst) != id || self.retired.is_cancelled() {
            bail!("MCP connection was superseded");
        }
        s.status = state.into();
        s.updated = now();
        Ok(())
    }
    fn fail_for(&self, id: u64, kind: &str, message: impl ToString) {
        let message = self.redact(&message.to_string());
        let mut s = self.snapshot.lock();
        if self.connection.load(Ordering::SeqCst) != id || self.retired.is_cancelled() {
            return;
        }
        s.status = if kind == "auth" {
            "needs_auth"
        } else {
            "failed"
        }
        .into();
        s.error = Some(json!({"kind":kind,"message":message}));
        s.stale = s.enumerated;
        s.updated = now();
    }
    fn redact(&self, message: &str) -> String {
        let mut secrets: Vec<String> = self
            .config
            .env
            .values()
            .chain(self.config.headers.values())
            .cloned()
            .collect();
        for v in self.config.headers.values() {
            if let Some((_, token)) = v.split_once(' ') {
                secrets.push(token.into());
            }
        }
        if let Some(url) = self
            .config
            .url
            .as_deref()
            .and_then(|v| url::Url::parse(v).ok())
        {
            secrets.extend(url.query_pairs().map(|(_, v)| v.into_owned()));
        }
        redact(message, &secrets)
    }
    pub fn status(&self) -> Value {
        let s = self.snapshot.lock();
        json!({"name":self.name,"wireKey":self.name,"uuid":self.name,"transport":self.config.transport_kind(),"builtin":false,"isInternal":false,"serverSource":"user_config","status":s.status,"isConnected":matches!(s.status.as_str(),"ready"|"refreshing"),"toolsEnumerated":s.enumerated,"catalogStale":s.stale,"tools":s.tools,"resources":s.resources,"prompts":s.prompts,"capabilities":s.capabilities,"serverInfo":s.server_info,"instructions":s.instructions,"error":s.error,"lastSuccessAt":s.last_success,"generation":s.generation,"stderr":s.stderr,"cwd":self.cwd,"updatedAt":s.updated,"authorizationUrl":s.auth_url,"authAttempt":s.auth_attempt,"hasAuth":self.authenticated.load(Ordering::SeqCst)})
    }
}
pub(crate) struct Manager {
    profile: PathBuf,
    entries: Mutex<HashMap<(String, PathBuf), Arc<Entry>>>,
    disabled: Mutex<HashSet<String>>,
}
impl Drop for Manager {
    fn drop(&mut self) {
        for entry in self.entries.get_mut().values() {
            entry.retired.cancel();
        }
    }
}
impl Manager {
    pub fn new(profile: &Path) -> Self {
        Self {
            profile: profile.into(),
            entries: Mutex::new(HashMap::new()),
            disabled: Mutex::new(HashSet::new()),
        }
    }
    pub fn cancel_auth(&self, name: &str) {
        for e in self.entries.lock().values().filter(|e| e.name == name) {
            if let Some(c) = e.auth.lock().take() {
                c.cancel();
            }
            let mut s = e.snapshot.lock();
            if s.status == "authenticating" {
                s.status = "needs_auth".into();
                s.auth_url = None;
                s.auth_attempt = None;
            }
        }
    }
    pub fn authenticate(entry: Arc<Entry>) -> Value {
        if let Some(c) = entry.auth.lock().take() {
            c.cancel();
        }
        let token = entry.retired.child_token();
        *entry.auth.lock() = Some(token.clone());
        let attempt = uuid::Uuid::new_v4().to_string();
        {
            let mut s = entry.snapshot.lock();
            entry.connection.fetch_add(1, Ordering::SeqCst);
            entry.connection_stop.lock().cancel();
            s.status = "authenticating".into();
            s.auth_attempt = Some(attempt.clone());
            s.auth_url = None;
            s.error = None;
            s.stale = s.enumerated;
            s.updated = now();
        }
        runtime().spawn(async move {
            init_tls();
            {
                let _g = entry.operation.lock().await;
                if token.is_cancelled() {
                    return;
                }
                if let Some(live) = entry.live.lock().await.take() {
                    live.stop.cancel();
                }
            }
            let result = async {
                let challenge = entry.auth_challenge.lock().clone();
                let pending = super::oauth::begin(
                    &entry.profile,
                    &entry.name,
                    entry.config.url.as_deref().unwrap_or(""),
                    entry
                        .config
                        .extra
                        .get("oauthClientId")
                        .and_then(Value::as_str),
                    challenge.as_deref(),
                    token.clone(),
                )
                .await?;
                {
                    let mut s = entry.snapshot.lock();
                    if token.is_cancelled() {
                        bail!("Sign-in cancelled");
                    }
                    s.auth_url = Some(pending.authorization_url.clone());
                }
                pending.wait().await
            }
            .await;
            let success = result.is_ok();
            {
                let mut s = entry.snapshot.lock();
                if token.is_cancelled() {
                    return;
                }
                s.auth_url = None;
                s.auth_attempt = None;
                match result {
                    Ok(()) => s.status = "configured".into(),
                    Err(e) => {
                        s.status = "needs_auth".into();
                        s.error =
                            Some(json!({"kind":"auth","message":entry.redact(&format!("{e:#}"))}));
                    }
                }
            }
            if success {
                let _ = Self::ensure(&entry, &AtomicBool::new(false)).await;
            }
        });
        json!({"status":"authenticating","authAttempt":attempt})
    }

    pub fn reconcile(&self, configs: &HashMap<String, McpServerConfigEntry>) {
        let mut entries = self.entries.lock();
        entries.retain(|(name, _), entry| {
            let keep = configs
                .get(name)
                .is_some_and(|c| c == &entry.config && c.enabled);
            if !keep {
                entry.retired.cancel();
            }
            keep
        });
        self.disabled
            .lock()
            .retain(|name| configs.contains_key(name));
    }
    pub fn entry(
        &self,
        name: &str,
        config: &McpServerConfigEntry,
        cwd: &Path,
    ) -> Result<Arc<Entry>> {
        if !config.enabled {
            bail!("MCP server '{name}' is disabled");
        }
        if self.disabled.lock().contains(name) {
            bail!("MCP server '{name}' is disconnected. Connect it to use its tools.");
        }
        let cwd = config
            .cwd
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.to_path_buf())
            .canonicalize()
            .context("MCP workspace does not exist")?;
        let key = (name.to_owned(), cwd.clone());
        let mut entries = self.entries.lock();
        if let Some(old) = entries.get(&key) {
            if old.config == *config && !old.retired.is_cancelled() {
                return Ok(old.clone());
            }
            old.retired.cancel();
        }
        let entry = Arc::new(Entry {
            profile: self.profile.clone(),
            auth: Mutex::new(None),
            auth_challenge: Mutex::new(None),
            active: AtomicUsize::new(0),
            last_used: AtomicU64::new(now() as u64),
            connection: AtomicU64::new(0),
            connection_stop: Mutex::new(CancellationToken::new()),
            authenticated: AtomicBool::new(false),
            name: name.into(),
            config: config.clone(),
            cwd,
            snapshot: Mutex::new(Snapshot::default()),
            live: tokio::sync::Mutex::new(None),
            operation: tokio::sync::Mutex::new(()),
            retired: CancellationToken::new(),
            changed: AtomicU64::new(0),
            refreshed: AtomicU64::new(0),
        });
        entries.insert(key, entry.clone());
        Ok(entry)
    }
    pub fn statuses(&self) -> HashMap<String, Value> {
        let mut rows: HashMap<String, Value> = HashMap::new();
        for e in self.entries.lock().values() {
            let v = e.status();
            if rows.get(&e.name).is_none_or(|old| {
                old["updatedAt"].as_u64().unwrap_or(0) < v["updatedAt"].as_u64().unwrap_or(0)
            }) {
                rows.insert(e.name.clone(), v);
            }
        }
        rows
    }
    pub fn is_disconnected(&self, name: &str) -> bool {
        self.disabled.lock().contains(name)
    }
    pub fn shutdown(&self) {
        let entries: Vec<_> = self.entries.lock().values().cloned().collect();
        for e in &entries {
            e.retired.cancel();
        }
        block_on(async {
            let cleanup = async {
                for e in entries {
                    let _g = e.operation.lock().await;
                    if let Some(live) = e.live.lock().await.as_ref() {
                        live.stop.cancel();
                        live.closed.cancelled().await;
                    }
                }
            };
            let _ = tokio::time::timeout(Duration::from_secs(5), cleanup).await;
        });
    }
    pub fn disconnect(&self, name: &str) {
        self.disabled.lock().insert(name.into());
        for e in self.entries.lock().values().filter(|e| e.name == name) {
            e.retired.cancel();
            let mut s = e.snapshot.lock();
            s.status = "disconnected".into();
            s.stale = s.enumerated;
            s.updated = now();
        }
    }
    pub fn enable(&self, name: &str) {
        self.disabled.lock().remove(name);
    }
    pub fn reconnect(&self, name: &str) {
        self.disabled.lock().remove(name);
        self.entries.lock().retain(|(n, _), e| {
            if n == name {
                e.retired.cancel();
                false
            } else {
                true
            }
        });
    }
    pub fn start(entry: Arc<Entry>, refresh: bool) {
        runtime().spawn(async move {
            let never = AtomicBool::new(false);
            if let Ok(live) = Self::ensure(&entry, &never).await {
                if refresh {
                    let _ = Self::refresh(&entry, &live, &never).await;
                }
            }
        });
    }
    pub async fn ensure(entry: &Arc<Entry>, cancelled: &AtomicBool) -> Result<Live> {
        let _guard = tokio::select! {g=entry.operation.lock()=>g,_=stop_requested(cancelled)=>bail!("MCP connection cancelled"),_=entry.retired.cancelled()=>bail!("MCP configuration was disconnected or replaced")};
        if entry.retired.is_cancelled() {
            bail!("MCP connection was disconnected or replaced");
        }
        if entry.snapshot.lock().status == "authenticating" {
            bail!("MCP sign-in is still waiting for completion");
        }
        if let Some(live) = entry.live.lock().await.as_ref() {
            if !live.peer.is_transport_closed() && !live.stop.is_cancelled() {
                if entry.snapshot.lock().status == "ready" {
                    return Ok(live.clone());
                }
            }
        }
        let generation = {
            let mut s = entry.snapshot.lock();
            if s.status == "authenticating" {
                bail!("MCP sign-in is still waiting for completion");
            }
            let generation = entry.connection.fetch_add(1, Ordering::SeqCst) + 1;
            let mut stop = entry.connection_stop.lock();
            stop.cancel();
            *stop = entry.retired.child_token();
            s.status = "starting".into();
            s.updated = now();
            generation
        };
        if let Some(old) = entry.live.lock().await.take() {
            old.stop.cancel();
        }
        entry.state_for(generation, "starting")?;
        let outcome = tokio::select! {r=connect(entry,generation)=>r,_=stop_requested(cancelled)=>Err(anyhow!("MCP initialization cancelled"))};
        if entry.connection.load(Ordering::SeqCst) != generation {
            bail!("MCP connection attempt was superseded");
        }
        let live = match outcome {
            Ok(l) => l,
            Err(e) => {
                let msg = format!("{e:#}");
                entry.remember_challenge(&e);
                entry.fail_for(
                    generation,
                    if is_auth(&msg) { "auth" } else { "connection" },
                    &msg,
                );
                return Err(e);
            }
        };
        entry.last_used.store(now() as u64, Ordering::SeqCst);
        if entry.retired.is_cancelled() {
            live.stop.cancel();
            bail!("MCP connection was replaced during initialization");
        }
        *entry.live.lock().await = Some(live.clone());
        entry.state_for(live.id, "discovering")?;
        if let Err(e) = refresh_catalog(entry, &live, cancelled).await {
            live.stop.cancel();
            if entry.connection.load(Ordering::SeqCst) == live.id {
                entry.fail_for(live.id, "discovery", &e);
            }
            return Err(e);
        }
        let weak = Arc::downgrade(entry);
        let watch = live.clone();
        runtime().spawn(async move{
            loop {tokio::time::sleep(Duration::from_millis(100)).await;let Some(e)=weak.upgrade()else{watch.stop.cancel();break};
                if e.connection.load(Ordering::SeqCst)!=watch.id {watch.stop.cancel();break;}
                if e.retired.is_cancelled(){watch.stop.cancel();break;}
                if watch.stop.is_cancelled()||watch.peer.is_transport_closed(){let _g=e.operation.lock().await;if e.connection.load(Ordering::SeqCst)!=watch.id{break;}let mut s=e.snapshot.lock();if matches!(s.status.as_str(),"ready"|"refreshing"){s.status="disconnected".into();s.stale=s.enumerated;s.error=Some(json!({"kind":"transport","message":"MCP server connection closed. Reconnect to continue."}));s.updated=now();}break;}
                if e.config.idle_timeout_ms>0&&e.active.load(Ordering::SeqCst)==0&&(now() as u64).saturating_sub(e.last_used.load(Ordering::SeqCst))>e.config.idle_timeout_ms {
                    let _g=e.operation.lock().await;
                    if e.connection.load(Ordering::SeqCst)==watch.id&&e.active.load(Ordering::SeqCst)==0&&(now() as u64).saturating_sub(e.last_used.load(Ordering::SeqCst))>e.config.idle_timeout_ms{watch.stop.cancel();let mut s=e.snapshot.lock();s.status="disconnected".into();s.stale=s.enumerated;s.error=Some(json!({"kind":"idle","message":"Idle connection parked; it reconnects when used."}));break;}
                }
                if e.changed.load(Ordering::SeqCst)!=e.refreshed.load(Ordering::SeqCst){let _=Self::refresh(&e,&watch,&AtomicBool::new(false)).await;}
            }
        });
        Ok(live)
    }
    async fn refresh(entry: &Arc<Entry>, live: &Live, cancelled: &AtomicBool) -> Result<()> {
        let _g = tokio::select! {g=entry.operation.lock()=>g,_=stop_requested(cancelled)=>bail!("MCP refresh cancelled"),_=entry.retired.cancelled()=>bail!("MCP connection closed"),_=tokio::time::sleep(deadline(entry.config.discovery_timeout_ms))=>bail!("MCP refresh queue timed out")};
        if entry.retired.is_cancelled()
            || entry.connection.load(Ordering::SeqCst) != live.id
            || live.stop.is_cancelled()
        {
            bail!("MCP connection closed or superseded");
        }
        entry.state_for(live.id, "refreshing")?;
        let result = refresh_catalog(entry, live, cancelled).await;
        if let Err(e) = &result {
            if entry.connection.load(Ordering::SeqCst) != live.id {
                return result;
            }
            entry.fail_for(live.id, "discovery", e);
            entry
                .refreshed
                .store(entry.changed.load(Ordering::SeqCst), Ordering::SeqCst);
        }
        result
    }
    pub async fn discover(entry: &Arc<Entry>, cancelled: &AtomicBool) -> Result<()> {
        let _activity = Activity::new(entry);
        let live = Self::ensure(entry, cancelled).await?;
        if entry.changed.load(Ordering::SeqCst) != entry.refreshed.load(Ordering::SeqCst) {
            Self::refresh(entry, &live, cancelled).await?;
        }
        Ok(())
    }
    pub async fn call(
        entry: &Arc<Entry>,
        tool: &str,
        args: &Value,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        let _activity = Activity::new(entry);
        let live = Self::ensure(entry, cancelled).await?;
        if entry.changed.load(Ordering::SeqCst) != entry.refreshed.load(Ordering::SeqCst) {
            Self::refresh(entry, &live, cancelled).await?;
        }
        let definition = entry
            .snapshot
            .lock()
            .tools
            .iter()
            .find(|t| t["name"] == tool)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "Unknown MCP tool '{tool}' on '{}'. Discover its current tools first.",
                    entry.name
                )
            })?;
        validate_instance(&definition["inputSchema"], args)
            .context("MCP tool arguments do not match inputSchema")?;
        let result = rpc(
            &live.peer,
            "tools/call",
            json!({"name":tool,"arguments":args}),
            entry.config.request_timeout_ms,
            cancelled,
            &entry.retired,
        )
        .await;
        if entry.retired.is_cancelled() || entry.connection.load(Ordering::SeqCst) != live.id {
            bail!("MCP connection changed while tool was running; completion was not established");
        }
        let result = match result {
            Ok(v) => v,
            Err(e) => {
                entry.remember_challenge(&e);
                if is_auth(&format!("{e:#}")) {
                    entry.fail_for(live.id, "auth", format!("{e:#}"));
                    live.stop.cancel();
                } else if live.peer.is_transport_closed() {
                    entry.fail_for(live.id, "transport", &e);
                }
                return Err(e);
            }
        };
        // Deserialize the protocol shape before trusting a server's successful result.
        let _: CallToolResult =
            serde_json::from_value(result.clone()).context("Invalid MCP tools/call result")?;
        if result["isError"] != true {
            if let Some(schema) = definition.get("outputSchema") {
                let output = result.get("structuredContent").ok_or_else(|| {
                    anyhow!("MCP result omitted structuredContent required by outputSchema")
                })?;
                validate_instance(schema, output)
                    .context("MCP tool result does not match outputSchema")?;
            }
        }
        Ok(result)
    }
    pub async fn resource(
        entry: &Arc<Entry>,
        method: &str,
        params: Value,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        let _activity = Activity::new(entry);
        Self::discover(entry, cancelled).await?;
        if method == "prompts/get" {
            let prompt = entry
                .snapshot
                .lock()
                .prompts
                .iter()
                .find(|p| p["name"] == params["name"])
                .cloned()
                .ok_or_else(|| anyhow!("Unknown MCP prompt"))?;
            let args = params["arguments"]
                .as_object()
                .ok_or_else(|| anyhow!("Prompt arguments must be an object"))?;
            if args.values().any(|v| !v.is_string()) {
                bail!("MCP prompt arguments must be strings");
            }
            for a in prompt["arguments"].as_array().into_iter().flatten() {
                if a["required"] == true && !args.contains_key(a["name"].as_str().unwrap_or("")) {
                    bail!("Missing required prompt argument {}", a["name"]);
                }
            }
        }
        let live = Self::ensure(entry, cancelled).await?;
        rpc(
            &live.peer,
            method,
            params,
            entry.config.request_timeout_ms,
            cancelled,
            &entry.retired,
        )
        .await
    }
}
struct Activity(Arc<Entry>);
impl Activity {
    fn new(e: &Arc<Entry>) -> Self {
        e.active.fetch_add(1, Ordering::SeqCst);
        e.last_used.store(now() as u64, Ordering::SeqCst);
        Self(e.clone())
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        self.0.last_used.store(now() as u64, Ordering::SeqCst);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
fn is_auth(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    s.contains("401")
        || s.contains("403")
        || [
            "auth required",
            "authorization required",
            "insufficient scope",
            "invalid_grant",
            "credential store",
            "token expired",
        ]
        .iter()
        .any(|p| s.contains(p))
}
async fn stop_requested(cancelled: &AtomicBool) {
    while !cancelled.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn rpc(
    peer: &Peer<RoleClient>,
    method: &str,
    params: Value,
    timeout: u64,
    cancelled: &AtomicBool,
    retired: &CancellationToken,
) -> Result<Value> {
    if cancelled.load(Ordering::SeqCst) {
        bail!("MCP request cancelled");
    }
    let request: ClientRequest = serde_json::from_value(json!({"method":method,"params":params}))?;
    let handle = tokio::select! {
        r=peer.send_cancellable_request(request,PeerRequestOptions::with_timeout(deadline(timeout)))=>r?,
        _=stop_requested(cancelled)=>bail!("MCP request submission cancelled; completion was not established"),
        _=retired.cancelled()=>bail!("MCP connection replaced before request submission"),
        _=tokio::time::sleep(deadline(timeout))=>bail!("MCP request submission timed out")
    };
    let id = handle.id.clone();
    let response = handle.await_response();
    tokio::pin!(response);
    tokio::select! {
        result=&mut response=>Ok(serde_json::to_value(result?)?),
        _=stop_requested(cancelled)=>{let _=tokio::time::timeout(Duration::from_millis(100),peer.notify_cancelled(CancelledNotificationParam::new(Some(id),Some("User stopped the task".into())))).await;bail!("MCP request cancelled; completion was not established")},
        _=retired.cancelled()=>{let _=tokio::time::timeout(Duration::from_millis(100),peer.notify_cancelled(CancelledNotificationParam::new(Some(id),Some("Connection configuration changed".into())))).await;bail!("MCP connection disconnected or replaced")}
    }
}
struct Handler {
    entry: Weak<Entry>,
    connection: u64,
}
impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        serde_json::from_value(json!({"protocolVersion":"2025-11-25","capabilities":{"roots":{"listChanged":true}},"clientInfo":{"name":"QuixiDesk","version":env!("CARGO_PKG_VERSION")}})).expect("client info")
    }
    #[allow(deprecated)]
    async fn list_roots(
        &self,
        _: RequestContext<RoleClient>,
    ) -> std::result::Result<ListRootsResult, ErrorData> {
        let roots = self
            .entry
            .upgrade()
            .and_then(|e| url::Url::from_directory_path(&e.cwd).ok())
            .map(|uri| vec![json!({"uri":uri.as_str(),"name":"Selected workspace"})])
            .unwrap_or_default();
        Ok(serde_json::from_value(json!({"roots":roots})).unwrap())
    }
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.invalidate();
    }
    async fn on_resource_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.invalidate();
    }
    async fn on_prompt_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.invalidate();
    }
}
impl Handler {
    fn invalidate(&self) {
        if let Some(e) = self.entry.upgrade() {
            let mut s = e.snapshot.lock();
            if e.connection.load(Ordering::SeqCst) != self.connection || e.retired.is_cancelled() {
                return;
            }
            e.changed.fetch_add(1, Ordering::SeqCst);
            s.stale = s.enumerated;
        }
    }
}
async fn connect(entry: &Arc<Entry>, connection_id: u64) -> Result<Live> {
    init_tls();
    let handler = Handler {
        entry: Arc::downgrade(entry),
        connection: connection_id,
    };
    let stop = entry.connection_stop.lock().child_token();
    entry.state_for(connection_id, "initializing")?;
    let lifecycle = match entry.config.lifecycle.as_str() {
        "discover" => ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
        "auto" => ClientLifecycleMode::Auto {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            legacy_version: Some(ProtocolVersion::V_2025_11_25),
        },
        _ => ClientLifecycleMode::Initialize,
    };
    let future = async {
        let service = if entry.config.transport_kind() == "stdio" {
            let executable = resolve_executable(&entry.config.command, &entry.config.env);
            let mut command = tokio::process::Command::new(&executable);
            command
                .args(&entry.config.args)
                .current_dir(&entry.cwd)
                .envs(&entry.config.env)
                .kill_on_drop(true);
            if !entry.config.env.contains_key("PATH") {
                command.env("PATH", runtime_path());
            }
            let mut wrapped = process_wrap::tokio::CommandWrap::from(command);
            #[cfg(unix)]
            wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
            #[cfg(windows)]
            wrapped.wrap(process_wrap::tokio::JobObject);
            let (transport, stderr) = TokioChildProcess::builder(wrapped)
                .stderr(Stdio::piped())
                .spawn()
                .with_context(|| {
                    format!("Could not start MCP command '{}'", entry.config.command)
                })?;
            if let Some(mut stderr) = stderr {
                let weak = Arc::downgrade(entry);
                runtime().spawn(async move {
                    let mut buf = [0; 2048];
                    let mut raw = Vec::new();
                    while let Ok(n) = stderr.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let Some(e) = weak.upgrade() else { break };
                        raw.extend_from_slice(&buf[..n]);
                        if raw.len() > 16384 {
                            raw.drain(..raw.len() - 8192);
                        }
                        e.snapshot.lock().stderr = e.redact(&String::from_utf8_lossy(&raw));
                    }
                });
            }

            handler
                .serve_with_lifecycle(transport, lifecycle.clone())
                .await
                .context("MCP initialization failed")?
        } else if entry.config.transport_kind() == "http" {
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in &entry.config.headers {
                headers.insert(
                    reqwest::header::HeaderName::from_bytes(k.as_bytes())?,
                    reqwest::header::HeaderValue::from_str(v)?,
                );
            }
            let client = reqwest::Client::builder()
                .default_headers(headers)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .build()?;
            let config = StreamableHttpClientTransportConfig::with_uri(
                entry.config.url.clone().unwrap_or_default(),
            )
            .max_sse_event_size(16 * 1024 * 1024)
            .reinit_on_expired_session(false);
            if let Some(authenticated) = super::oauth::authenticated_client(
                &entry.profile,
                &entry.name,
                entry.config.url.as_deref().unwrap_or(""),
                client.clone(),
            )
            .await?
            {
                entry.authenticated.store(true, Ordering::SeqCst);
                handler
                    .serve_with_lifecycle(
                        StreamableHttpClientTransport::with_client(authenticated, config),
                        lifecycle.clone(),
                    )
                    .await
                    .context("MCP authenticated HTTP initialization failed")?
            } else {
                handler
                    .serve_with_lifecycle(
                        StreamableHttpClientTransport::with_client(client, config),
                        lifecycle.clone(),
                    )
                    .await
                    .context("MCP HTTP initialization failed")?
            }
        } else {
            let transport =
                super::sse::connect(&entry.profile, &entry.name, &entry.config, &stop).await?;
            handler
                .serve_with_lifecycle(transport, lifecycle.clone())
                .await
                .context("MCP SSE initialization failed")?
        };
        let peer = service.peer().clone();
        let version = peer
            .peer_info()
            .map(|i| i.protocol_version.to_string())
            .unwrap_or_default();
        if ![
            "2024-11-05",
            "2025-03-26",
            "2025-06-18",
            "2025-11-25",
            "2026-07-28",
        ]
        .contains(&version.as_str())
        {
            bail!("Unsupported MCP protocol version '{version}'");
        }
        let service_stop = service.cancellation_token();
        let token = stop.clone();
        let closed = CancellationToken::new();
        let finished = closed.clone();
        runtime().spawn(async move{
            let waiting=service.waiting();tokio::pin!(waiting);
            tokio::select!{_=&mut waiting=>{},_=token.cancelled()=>{service_stop.cancel();let _=tokio::time::timeout(Duration::from_secs(4),&mut waiting).await;}}
            finished.cancel();
        });
        Ok::<_, anyhow::Error>(Live {
            id: connection_id,
            peer,
            stop: stop.clone(),
            closed,
        })
    };
    tokio::select! {r=tokio::time::timeout(deadline(entry.config.connect_timeout_ms),future)=>r.context("MCP initialization timed out")?,_=stop.cancelled()=>bail!("MCP initialization cancelled")}
}
fn runtime_path() -> std::ffi::OsString {
    let mut paths: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(home) = dirs::home_dir() {
        paths.extend([
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            home.join(".nvm/current/bin"),
        ]);
    }
    #[cfg(unix)]
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    std::env::join_paths(paths).unwrap_or_default()
}
fn resolve_executable(command: &str, env: &HashMap<String, String>) -> PathBuf {
    if Path::new(command).components().count() > 1 {
        return command.into();
    }
    let path = env
        .get("PATH")
        .map(std::ffi::OsString::from)
        .unwrap_or_else(runtime_path);
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return candidate;
        }
        #[cfg(windows)]
        for extension in ["exe", "cmd", "bat"] {
            let p = candidate.with_extension(extension);
            if p.is_file() {
                return p;
            }
        }
    }
    command.into()
}
async fn refresh_catalog(entry: &Arc<Entry>, live: &Live, cancelled: &AtomicBool) -> Result<()> {
    let generation = entry.changed.load(Ordering::SeqCst);
    let info = live
        .peer
        .peer_info()
        .ok_or_else(|| anyhow!("MCP initialized without server info"))?;
    let info = serde_json::to_value(info.as_ref())?;
    let caps = info["capabilities"].clone();
    let future = async {
        let mut collections = Vec::new();
        for collection in ["tools", "resources", "prompts"] {
            let mut items = Vec::new();
            let mut cursor: Option<String> = None;
            let mut seen = HashSet::new();
            let mut names = HashSet::new();
            let mut total_bytes = 0;
            if caps.get(collection).is_some() {
                for page in 0..entry.config.max_pages {
                    let params = cursor
                        .as_ref()
                        .map(|c| json!({"cursor":c}))
                        .unwrap_or_else(|| json!({}));
                    let response = rpc(
                        &live.peer,
                        &format!("{collection}/list"),
                        params,
                        entry.config.discovery_timeout_ms,
                        cancelled,
                        &entry.retired,
                    )
                    .await?;
                    let rows = response[collection].as_array().ok_or_else(|| {
                        anyhow!("Invalid {collection}/list result: missing array")
                    })?;
                    for item in rows {
                        let key = item[if collection == "resources" {
                            "uri"
                        } else {
                            "name"
                        }]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("Invalid {collection} item identity"))?;
                        if !names.insert(key.to_owned()) {
                            bail!("Duplicate {collection} identity '{key}' in MCP catalog");
                        }
                        if collection == "tools" {
                            validate_schema(&item["inputSchema"])
                                .with_context(|| format!("Invalid inputSchema for {key}"))?;
                            if let Some(schema) = item.get("outputSchema") {
                                validate_schema(schema)
                                    .with_context(|| format!("Invalid outputSchema for {key}"))?;
                            }
                        }
                        total_bytes += serde_json::to_vec(item)?.len();
                        if items.len() >= entry.config.max_tools || total_bytes > 16 * 1024 * 1024 {
                            bail!("MCP {collection} catalog exceeds configured size limit; no partial catalog published");
                        }
                        items.push(item.clone());
                    }
                    cursor = match response.get("nextCursor") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(c)) => Some(c.clone()),
                        _ => bail!("Invalid pagination cursor"),
                    };
                    let Some(c) = &cursor else { break };
                    if !seen.insert(c.clone()) {
                        bail!("MCP {collection} pagination repeated cursor; catalog is incomplete");
                    }
                    if page + 1 == entry.config.max_pages {
                        bail!(
                            "MCP {collection} pagination exceeded {} pages; catalog is incomplete",
                            entry.config.max_pages
                        );
                    }
                }
            }
            collections.push(items);
        }
        Ok::<_, anyhow::Error>(collections)
    };
    let mut cols = tokio::time::timeout(deadline(entry.config.discovery_timeout_ms), future)
        .await
        .context("MCP discovery timed out")??;
    let mut s = entry.snapshot.lock();
    if entry.retired.is_cancelled()
        || entry.connection.load(Ordering::SeqCst) != live.id
        || live.stop.is_cancelled()
    {
        bail!("MCP configuration replaced during discovery");
    }
    s.tools = cols.remove(0);
    s.resources = cols.remove(0);
    s.prompts = cols.remove(0);
    s.capabilities = caps;
    s.server_info = info["serverInfo"].clone();
    s.instructions = info["instructions"].as_str().unwrap_or("").to_string();
    s.status = "ready".into();
    s.enumerated = true;
    s.stale = generation != entry.changed.load(Ordering::SeqCst);
    s.error = None;
    s.last_success = Some(now());
    s.updated = now();
    s.generation += 1;
    entry.refreshed.store(generation, Ordering::SeqCst);
    Ok(())
}
fn validate_schema(schema: &Value) -> Result<()> {
    if !schema.is_object() {
        bail!("Schema must be a JSON object");
    }
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|e| anyhow!("{e}"))
}
pub(crate) fn validate_instance(schema: &Value, value: &Value) -> Result<()> {
    let validator = jsonschema::validator_for(schema).map_err(|e| anyhow!("{e}"))?;
    validator.validate(value).map_err(|e| anyhow!("{e}"))
}

fn redact(message: &str, secrets: &[String]) -> String {
    let mut result = message.to_owned();
    for secret in secrets.iter().filter(|s| s.len() >= 4) {
        result = result.replace(secret, "[redacted]");
        // Mask an incomplete secret at either edge of a bounded stderr buffer.
        for (n, _) in secret.char_indices().skip(1) {
            if n < 4 {
                continue;
            }
            if result.ends_with(&secret[..n]) {
                result.truncate(result.len() - n);
                result.push_str("[redacted]");
            }
            if result.starts_with(&secret[n..]) && secret.len() - n >= 4 {
                result.replace_range(..secret.len() - n, "[redacted]");
            }
        }
    }
    result
        .chars()
        .rev()
        .take(8192)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}
#[cfg(test)]
mod diagnostic_tests {
    #[test]
    fn redacts_credentials_across_chunks_and_retained_buffer_boundaries() {
        let secret = vec!["sensitive-fixture-token".to_string()];
        assert_eq!(
            super::redact("error sensitive-fixture", &secret),
            "error [redacted]"
        );
        assert_eq!(
            super::redact("error sensitive-fixture-token done", &secret),
            "error [redacted] done"
        );
        assert_eq!(
            super::redact("fixture-token done", &secret),
            "[redacted] done"
        );
    }
}
