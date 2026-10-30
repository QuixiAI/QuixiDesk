//! HTTP/SSE transports run in a disposable process. The parent owns MCP state.
//! Bootstrap secrets travel over private stdin, never arguments or environment.
use crate::McpServerConfigEntry;
use anyhow::{bail, Context, Result};
use futures_util::{stream::FuturesUnordered, StreamExt};
use rmcp::{
    model::{ElicitRequestParams, JsonRpcMessage, ServerRequest},
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
        Transport,
    },
    RoleClient,
};
use serde_json::{json, Value};
use std::{
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, Mutex},
};
use tokio_util::sync::CancellationToken;
const MAX_FRAME: usize = 16 * 1024 * 1024;
const WORKER_ERROR: i32 = -32098;

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Value>> {
    let mut header = [0; 4];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME {
        bail!("MCP worker frame exceeds 16 MiB limit");
    }
    let mut body = vec![0; size];
    reader.read_exact(&mut body).await?;
    Ok(Some(serde_json::from_slice(&body)?))
}
async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_FRAME {
        bail!("MCP worker frame exceeds 16 MiB limit");
    }
    writer.write_all(&(body.len() as u32).to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}
fn executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("QUIXI_MCP_WORKER") {
        let path = PathBuf::from(path);
        if !path.is_absolute() || !path.is_file() {
            bail!("QUIXI_MCP_WORKER must name an existing absolute executable");
        }
        return Ok(path);
    }
    let current = std::env::current_exe()?;
    let stem = current.file_stem().and_then(|v| v.to_str()).unwrap_or("");
    if matches!(stem, "QuixiDesk" | "quixi-app" | "quixi-mcp-worker") {
        return Ok(current);
    }
    let name = if cfg!(windows) {
        "quixi-mcp-worker.exe"
    } else {
        "quixi-mcp-worker"
    };
    let directory = current.parent().context("No executable directory")?;
    for path in [
        directory.join(name),
        directory.parent().unwrap_or(directory).join(name),
    ] {
        if path.is_file() {
            return Ok(path);
        }
    }
    bail!("MCP transport helper is missing. Rebuild or reinstall QuixiDesk (cargo build -p quixi-mcp --bin quixi-mcp-worker for tests).")
}

pub(crate) struct WorkerTransport {
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    messages: mpsc::Receiver<RxJsonRpcMessage<RoleClient>>,
    stop: CancellationToken,
    finished: CancellationToken,
    pid: u32,
    authenticated: Arc<AtomicBool>,
}
impl WorkerTransport {
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn authenticated(&self) -> Arc<AtomicBool> {
        self.authenticated.clone()
    }
}
impl Drop for WorkerTransport {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
impl Transport<RoleClient> for WorkerTransport {
    type Error = io::Error;
    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = io::Result<()>> + Send + 'static {
        let writer = self.writer.clone();
        let stop = self.stop.clone();
        async move {
            if stop.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "MCP transport worker stopped",
                ));
            }
            // Complete each frame even when the request waiting for this send is cancelled.
            tokio::spawn(async move {
                let mut writer = writer.lock().await;
                write_frame(&mut *writer, &json!({"kind":"message","message":item}))
                    .await
                    .map_err(io::Error::other)
            })
            .await
            .map_err(io::Error::other)?
        }
    }
    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        self.messages.recv().await
    }
    async fn close(&mut self) -> io::Result<()> {
        // Allow the child to close its remote session (including HTTP DELETE),
        // then enforce a hard process deadline even if the remote peer stalls.
        let _ = tokio::time::timeout(Duration::from_millis(100), async {
            let mut writer = self.writer.lock().await;
            write_frame(&mut *writer, &json!({"kind":"close"})).await
        })
        .await;
        if tokio::time::timeout(Duration::from_millis(2500), self.finished.cancelled())
            .await
            .is_err()
        {
            self.stop.cancel();
        }
        let _ = tokio::time::timeout(Duration::from_millis(500), self.finished.cancelled()).await;
        Ok(())
    }
}

pub(crate) async fn connect(
    profile: &Path,
    server: &str,
    config: &McpServerConfigEntry,
    cwd: &Path,
) -> Result<WorkerTransport> {
    let mut command = tokio::process::Command::new(executable()?);
    command
        .arg("--mcp-transport-worker")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut wrapped = process_wrap::tokio::CommandWrap::from(command);
    #[cfg(unix)]
    wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    wrapped.wrap(process_wrap::tokio::JobObject);
    let mut child = wrapped
        .spawn()
        .context("Could not start isolated MCP transport worker")?;
    let pid = child.id().context("MCP worker has no process identity")?;
    let mut input = child
        .inner_mut()
        .stdin()
        .take()
        .context("MCP worker stdin unavailable")?;
    let mut output = child
        .inner_mut()
        .stdout()
        .take()
        .context("MCP worker stdout unavailable")?;
    let stop = CancellationToken::new();
    let finished = CancellationToken::new();
    let process_stop = stop.clone();
    let process_finished = finished.clone();
    tokio::spawn(async move {
        tokio::select! {_ = child.wait()=>{},_ = process_stop.cancelled()=>{let _=Box::into_pin(child.kill()).await;}}
        // Let the reader drain buffered error frames after a natural exit.
        // Cancelling it here can lose the final OAuth challenge on a short-lived worker.
        process_finished.cancel();
    });
    // Cancel the process if bootstrap is rejected, times out, or this future is dropped.
    let guard = stop.clone().drop_guard();
    let nonce = uuid::Uuid::new_v4().to_string();
    write_frame(&mut input,&json!({"kind":"bootstrap","nonce":nonce,"profile":profile,"server":server,"config":config,"testProfile":quixi_proto::paths::test_profile()})).await?;
    let (tx, rx) = mpsc::channel(32);
    let reader_stop = stop.clone();
    let authenticated = Arc::new(AtomicBool::new(false));
    let auth = authenticated.clone();
    tokio::spawn(async move {
        loop {
            let frame = tokio::select! {_ = reader_stop.cancelled()=>break,frame=read_frame(&mut output)=>match frame{Ok(Some(frame))=>frame,_=>break}};
            let message = match frame["kind"].as_str() {
                Some("ready") if frame["nonce"] == nonce => {
                    auth.store(frame["authenticated"] == true, Ordering::SeqCst);
                    continue;
                }
                Some("error") if frame["nonce"] == nonce => {
                    json!({"jsonrpc":"2.0","id":frame["id"],"error":{"code":WORKER_ERROR,"message":frame["error"]["message"],"data":{"quixiWorkerError":true,"kind":frame["error"]["kind"],"challenge":frame["error"]["challenge"]}}})
                }
                Some("message") => {
                    let mut message = frame["message"].clone();
                    // Remote peers cannot forge a private worker error's provenance.
                    if let Some(data) = message
                        .get_mut("error")
                        .and_then(|e| e.get_mut("data"))
                        .and_then(Value::as_object_mut)
                    {
                        data.remove("quixiWorkerError");
                    }
                    message
                }
                _ => break,
            };
            let Ok(message) = serde_json::from_value(message) else {
                break;
            };
            if tx.send(message).await.is_err() {
                break;
            }
        }
        reader_stop.cancel();
    });
    guard.disarm();
    Ok(WorkerTransport {
        writer: Arc::new(Mutex::new(input)),
        messages: rx,
        stop,
        finished,
        pid,
        authenticated,
    })
}

fn transport_error(error: &(dyn std::error::Error + 'static)) -> Value {
    let mut current = Some(error);
    let mut challenge = None;
    let mut kind = "transport";
    while let Some(e) = current {
        if let Some(e) =
            e.downcast_ref::<rmcp::transport::streamable_http_client::AuthRequiredError>()
        {
            challenge = Some(e.www_authenticate_header.clone());
            kind = "auth";
        }
        if let Some(e) =
            e.downcast_ref::<rmcp::transport::streamable_http_client::InsufficientScopeError>()
        {
            challenge = Some(e.www_authenticate_header.clone());
            kind = "scope";
        }
        current = e.source();
    }
    let message = match kind {
        "auth" => "MCP remote worker HTTP 401: authorization required".to_owned(),
        "scope" => "MCP remote worker HTTP 403: insufficient scope".to_owned(),
        _ => format!("MCP remote transport failed: {error}"),
    };
    json!({"kind":kind,"message":message,"challenge":challenge})
}
fn lossless_message(message: RxJsonRpcMessage<RoleClient>) -> Result<Value> {
    let schema = match &message {
        JsonRpcMessage::Request(request) => match &request.request {
            ServerRequest::ElicitRequest(request) => match &request.params {
                ElicitRequestParams::FormElicitationParams {
                    requested_schema, ..
                } => requested_schema.original_json_schema().cloned(),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    let mut value = serde_json::to_value(message)?;
    if let Some(schema) = schema {
        value["params"]["requestedSchema"] = schema;
    }
    Ok(value)
}

async fn relay<T: Transport<RoleClient> + 'static>(
    mut transport: T,
    first: Value,
    nonce: &str,
) -> Result<()> {
    let (tx, mut input) = mpsc::channel(32);
    let reader = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        while let Ok(Some(frame)) = read_frame(&mut stdin).await {
            if tx.send(frame).await.is_err() {
                break;
            }
        }
    });
    let mut output = tokio::io::stdout();
    let mut sending = FuturesUnordered::new();
    let mut next = Some(first);
    loop {
        if let Some(frame) = next.take() {
            if frame["kind"] == "close" {
                break;
            }
            if frame["kind"] != "message" {
                bail!("Unexpected MCP worker input frame");
            }
            let value = frame["message"].clone();
            let id = value.get("id").cloned();
            let message = serde_json::from_value(value)?;
            let send = transport.send(message);
            sending.push(async move { (id, send.await) });
        }
        tokio::select! {
            frame=input.recv(),if sending.len()<32=>{match frame{Some(frame)=>next=Some(frame),None=>break}}
            result=sending.next(),if !sending.is_empty()=>{if let Some((id,Err(error)))=result{if let Some(id)=id{write_frame(&mut output,&json!({"kind":"error","nonce":nonce,"id":id,"error":transport_error(&error)})).await?;}break;}}
            message=transport.receive()=>{match message{Some(message)=>write_frame(&mut output,&json!({"kind":"message","message":lossless_message(message)?})).await?,None=>{
                // Transport shutdown and the final send failure can become ready
                // together. Drain failure acknowledgements before closing stdout
                // so an initial 401/403 does not become a generic EOF instead.
                while let Ok(Some((id, result)))=tokio::time::timeout(Duration::from_secs(1),sending.next()).await {
                    if let (Some(id),Err(error))=(id,result) { write_frame(&mut output,&json!({"kind":"error","nonce":nonce,"id":id,"error":transport_error(&error)})).await?; }
                }
                break;
            }}}
        }
    }
    reader.abort();
    let _ = tokio::time::timeout(Duration::from_secs(2), transport.close()).await;
    Ok(())
}

async fn run() -> Result<()> {
    super::client::init_tls();
    let mut input = tokio::io::stdin();
    let bootstrap = read_frame(&mut input)
        .await?
        .context("Missing worker bootstrap")?;
    if bootstrap["kind"] != "bootstrap" {
        bail!("Missing worker bootstrap");
    }
    let nonce = bootstrap["nonce"]
        .as_str()
        .context("Missing worker identity")?;
    let profile = PathBuf::from(
        bootstrap["profile"]
            .as_str()
            .context("Missing worker profile")?,
    );
    let server = bootstrap["server"]
        .as_str()
        .context("Missing worker server")?;
    let config: McpServerConfigEntry = serde_json::from_value(bootstrap["config"].clone())?;
    if let Some(path) = bootstrap["testProfile"].as_str() {
        quixi_proto::paths::configure_test_profile(Path::new(path)).map_err(anyhow::Error::msg)?;
    }
    // Read the parent's first RPC before connecting, so initial HTTP/SSE failures
    // have the correct request ID and preserve the OAuth challenge over the pipe.
    let first = read_frame(&mut input)
        .await?
        .context("Parent closed before initialization")?;
    let id = first["message"]["id"].clone();
    drop(input);
    let mut output = tokio::io::stdout();
    let connected = async {
        if config.transport_kind() == "http" {
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in &config.headers {
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
            let transport_config = StreamableHttpClientTransportConfig::with_uri(
                config.url.clone().unwrap_or_default(),
            )
            .max_sse_event_size(MAX_FRAME)
            .reinit_on_expired_session(false);
            if let Some(authenticated) = super::oauth::authenticated_client(
                &profile,
                server,
                config.url.as_deref().unwrap_or(""),
                client.clone(),
            )
            .await?
            {
                write_frame(
                    &mut output,
                    &json!({"kind":"ready","nonce":nonce,"authenticated":true}),
                )
                .await?;
                relay(
                    StreamableHttpClientTransport::with_client(authenticated, transport_config),
                    first.clone(),
                    nonce,
                )
                .await
            } else {
                write_frame(
                    &mut output,
                    &json!({"kind":"ready","nonce":nonce,"authenticated":false}),
                )
                .await?;
                relay(
                    StreamableHttpClientTransport::with_client(client, transport_config),
                    first.clone(),
                    nonce,
                )
                .await
            }
        } else if config.transport_kind() == "sse" {
            let stop = CancellationToken::new();
            let transport = super::sse::connect(&profile, server, &config, &stop).await?;
            let authenticated = super::oauth::authorized_manager(
                &profile,
                server,
                config.url.as_deref().unwrap_or(""),
            )
            .await?
            .is_some();
            write_frame(
                &mut output,
                &json!({"kind":"ready","nonce":nonce,"authenticated":authenticated}),
            )
            .await?;
            relay(transport, first.clone(), nonce).await
        } else {
            bail!("Only HTTP and SSE use the remote transport worker")
        }
    }
    .await;
    if let Err(error) = connected {
        write_frame(
            &mut output,
            &json!({"kind":"error","nonce":nonce,"id":id,"error":transport_error(error.as_ref())}),
        )
        .await?;
    }
    Ok(())
}
/// Multi-call executable entry point. Run before GUI initialization or logging.
pub fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let result = runtime.block_on(run());
    // Tokio stdin uses a blocking reader; never wait indefinitely for the parent
    // to close its pipe after a transport failure has already ended this worker.
    runtime.shutdown_timeout(Duration::from_millis(100));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn frames_are_bounded_and_preserve_private_payloads() {
        let (mut writer, mut reader) = tokio::io::duplex(2048);
        let value = json!({"headers":{"Authorization":"fixture-private"}});
        write_frame(&mut writer, &value).await.unwrap();
        assert_eq!(read_frame(&mut reader).await.unwrap(), Some(value));
        writer
            .write_all(&((MAX_FRAME + 1) as u32).to_be_bytes())
            .await
            .unwrap();
        assert!(read_frame(&mut reader).await.is_err());
    }
    #[test]
    fn relay_preserves_original_elicitation_constraints() {
        let original = json!({"jsonrpc":"2.0","id":"ask","method":"elicitation/create","params":{"mode":"form","message":"Choose a code","requestedSchema":{"type":"object","properties":{"code":{"type":"string","pattern":"^[A-Z]{3}$"}},"required":["code"]}}});
        let decoded = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(lossless_message(decoded).unwrap(), original);
    }
}
