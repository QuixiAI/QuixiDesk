//! Legacy HTTP+SSE transport. Session state lives for the lifetime of the transport.
use std::{future::Future, io, path::Path, sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::Transport,
    RoleClient,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::McpServerConfigEntry;

const MAX_EVENT: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum SseError {
    Io(io::Error),
    Auth(rmcp::transport::streamable_http_client::AuthRequiredError),
    Scope(rmcp::transport::streamable_http_client::InsufficientScopeError),
}
impl std::fmt::Display for SseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Auth(e) => write!(f, "MCP SSE HTTP 401: {e}"),
            Self::Scope(e) => write!(f, "MCP SSE HTTP 403: {e}"),
        }
    }
}
impl std::error::Error for SseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Io(e) => e,
            Self::Auth(e) => e,
            Self::Scope(e) => e,
        })
    }
}
impl From<io::Error> for SseError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
fn response_status(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) -> std::result::Result<(), SseError> {
    let challenge = headers
        .get(reqwest::header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    match status.as_u16() {
        401 => Err(SseError::Auth(
            rmcp::transport::streamable_http_client::AuthRequiredError::new(challenge),
        )),
        403 => Err(SseError::Scope(
            rmcp::transport::streamable_http_client::InsufficientScopeError::new(challenge, None),
        )),
        _ if status.is_success() => Ok(()),
        _ => Err(io::Error::other(format!("MCP SSE HTTP {status}")).into()),
    }
}

/// The guard also covers cancellation while waiting for the initial endpoint event.
struct Reader(tokio::task::JoinHandle<()>);
impl Drop for Reader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) struct SseTransport {
    client: reqwest::Client,
    authorization: Option<Arc<rmcp::transport::auth::AuthorizationManager>>,
    endpoint: url::Url,
    messages: mpsc::Receiver<RxJsonRpcMessage<RoleClient>>,
    stop: CancellationToken,
    _reader: Reader,
}

impl Drop for SseTransport {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

pub(crate) async fn connect(
    profile: &Path,
    server: &str,
    config: &McpServerConfigEntry,
    parent: &CancellationToken,
) -> Result<SseTransport> {
    let url = url::Url::parse(config.url.as_deref().context("Missing SSE URL")?)?;
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in &config.headers {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())?,
            reqwest::header::HeaderValue::from_str(value)?,
        );
    }
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .build()?;
    let stop = parent.child_token();
    let authorization = super::oauth::authorized_manager(profile, server, url.as_str()).await?;
    let request = authorize(
        client
            .get(url.clone())
            .header("Accept", "text/event-stream"),
        authorization.as_ref(),
        &stop,
    )
    .await?;
    let response = tokio::select! {
        _ = stop.cancelled() => bail!("MCP SSE connection cancelled"),
        result = request.send() => result?,
    };
    response_status(response.status(), response.headers())?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if !content_type.eq_ignore_ascii_case("text/event-stream") {
        bail!("MCP SSE endpoint did not return text/event-stream");
    }
    let (endpoint_tx, endpoint_rx) = oneshot::channel();
    let (messages_tx, messages) = mpsc::channel(32);
    let reader_stop = stop.clone();
    let reader = Reader(tokio::spawn(async move {
        let mut endpoint_tx = Some(endpoint_tx);
        let result = read_stream(response, &url, &mut endpoint_tx, messages_tx, &reader_stop).await;
        if let Some(tx) = endpoint_tx.take() {
            let error = result.err().unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "MCP SSE closed before endpoint event",
                )
            });
            let _ = tx.send(Err(error));
        }
    }));
    let endpoint = tokio::select! {
        _ = stop.cancelled() => bail!("MCP SSE connection cancelled"),
        result = endpoint_rx => result.context("MCP SSE reader stopped")??,
    };
    Ok(SseTransport {
        client,
        authorization,
        endpoint,
        messages,
        stop,
        _reader: reader,
    })
}

impl Transport<RoleClient> for SseTransport {
    type Error = SseError;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = std::result::Result<(), SseError>> + Send + 'static {
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let stop = self.stop.clone();
        let authorization = self.authorization.clone();
        async move {
            let request = authorize(
                client.post(endpoint).json(&item),
                authorization.as_ref(),
                &stop,
            )
            .await?;
            let response = tokio::select! {
                _ = stop.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted, "MCP SSE request cancelled").into()),
                response = request.send() => response.map_err(io::Error::other)?,
            };
            response_status(response.status(), response.headers())?;
            Ok(())
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        tokio::select! { _ = self.stop.cancelled() => None, message = self.messages.recv() => message }
    }

    async fn close(&mut self) -> std::result::Result<(), SseError> {
        self.stop.cancel();
        self._reader.0.abort();
        self.messages.close();
        Ok(())
    }
}

/// Refresh credentials before each request; never replay a tool POST after a 401.
async fn authorize(
    request: reqwest::RequestBuilder,
    authorization: Option<&Arc<rmcp::transport::auth::AuthorizationManager>>,
    stop: &CancellationToken,
) -> io::Result<reqwest::RequestBuilder> {
    let Some(manager) = authorization else {
        return Ok(request);
    };
    let token = tokio::select! {
        _ = stop.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted, "MCP SSE authorization cancelled")),
        token = manager.get_access_token() => token.map_err(|e| io::Error::other(format!("MCP SSE OAuth authorization: {e}")))?,
    };
    Ok(request.bearer_auth(token))
}

/// Accept relative endpoints, but never send configured authorization to another origin.
fn message_endpoint(base: &url::Url, value: &str) -> io::Result<url::Url> {
    let endpoint = base.join(value).map_err(io::Error::other)?;
    if endpoint.origin() != base.origin()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err(io::Error::other(
            "MCP SSE endpoint must have the same origin and no credentials",
        ));
    }
    Ok(endpoint)
}

#[derive(Default)]
struct Events {
    line: Vec<u8>,
    event: String,
    data: String,
    bytes: usize,
    previous_cr: bool,
    started: bool,
}
impl Events {
    fn feed(&mut self, byte: u8) -> io::Result<Option<(String, String)>> {
        if self.previous_cr && byte == b'\n' {
            self.previous_cr = false;
            return Ok(None);
        }
        self.previous_cr = byte == b'\r';
        self.bytes += 1;
        if self.bytes > MAX_EVENT {
            return Err(io::Error::other("MCP SSE event exceeded 16 MiB"));
        }
        if byte != b'\r' && byte != b'\n' {
            self.line.push(byte);
            return Ok(None);
        }
        let line = String::from_utf8(std::mem::take(&mut self.line)).map_err(io::Error::other)?;
        let line = if !self.started {
            self.started = true;
            line.strip_prefix('\u{feff}').unwrap_or(&line)
        } else {
            &line
        };
        if line.is_empty() {
            self.bytes = 0;
            if self.data.is_empty() {
                self.event.clear();
                return Ok(None);
            }
            self.data.pop(); // trailing newline added for the last data field
            return Ok(Some((
                std::mem::take(&mut self.event),
                std::mem::take(&mut self.data),
            )));
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = value.to_string(),
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            _ => {}
        }
        Ok(None)
    }
}

async fn read_stream(
    mut response: reqwest::Response,
    base: &url::Url,
    endpoint_tx: &mut Option<oneshot::Sender<io::Result<url::Url>>>,
    messages: mpsc::Sender<RxJsonRpcMessage<RoleClient>>,
    stop: &CancellationToken,
) -> io::Result<()> {
    let mut events = Events::default();
    loop {
        let chunk = tokio::select! {
            _ = stop.cancelled() => return Ok(()),
            chunk = response.chunk() => chunk.map_err(io::Error::other)?,
        };
        let Some(chunk) = chunk else {
            return Ok(());
        };
        for byte in chunk {
            if let Some((event, data)) = events.feed(byte)? {
                match event.as_str() {
                    "endpoint" => {
                        let endpoint = message_endpoint(base, &data)?;
                        if let Some(tx) = endpoint_tx.take() {
                            if tx.send(Ok(endpoint)).is_err() {
                                return Ok(());
                            }
                        } else {
                            return Err(io::Error::other("MCP SSE sent a duplicate endpoint"));
                        }
                    }
                    "message" | "" => {
                        if endpoint_tx.is_some() {
                            return Err(io::Error::other(
                                "MCP SSE message arrived before endpoint",
                            ));
                        }
                        let message = serde_json::from_str(&data).map_err(io::Error::other)?;
                        tokio::select! {
                            _ = stop.cancelled() => return Ok(()),
                            result = messages.send(message) => if result.is_err() { return Ok(()); },
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auth_errors_keep_typed_challenges_in_the_error_chain() {
        let challenge = "Bearer resource_metadata=\"https://example.test/custom-metadata\", scope=\"tools:read\"";
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::WWW_AUTHENTICATE,
            challenge.parse().unwrap(),
        );
        for status in [
            reqwest::StatusCode::UNAUTHORIZED,
            reqwest::StatusCode::FORBIDDEN,
        ] {
            let error = anyhow::Error::new(response_status(status, &headers).unwrap_err());
            let retained = error.chain().find_map(|e| {
                if let Some(auth) =
                    e.downcast_ref::<rmcp::transport::streamable_http_client::AuthRequiredError>()
                {
                    return Some(auth.www_authenticate_header.as_str());
                }
                e.downcast_ref::<rmcp::transport::streamable_http_client::InsufficientScopeError>()
                    .map(|auth| auth.www_authenticate_header.as_str())
            });
            assert_eq!(retained, Some(challenge));
        }
    }
    #[test]
    fn events_support_all_sse_newlines_and_multiline_data() {
        for ending in ["\n", "\r\n", "\r"] {
            let input = [
                ":comment",
                "event: message",
                "data: one",
                "data: two",
                "",
                "",
            ]
            .join(ending);
            let mut parser = Events::default();
            let events: Vec<_> = input
                .bytes()
                .filter_map(|b| parser.feed(b).unwrap())
                .collect();
            assert_eq!(events, vec![("message".into(), "one\ntwo".into())]);
        }
    }
    #[test]
    fn events_bound_ignored_fields_as_well_as_data() {
        let mut parser = Events {
            bytes: MAX_EVENT,
            ..Default::default()
        };
        assert!(parser.feed(b'x').is_err());
    }
    #[test]
    fn events_allow_utf8_bom_only_at_stream_start() {
        let mut parser = Events::default();
        let events: Vec<_> = "\u{feff}data: first\n\ndata: second\n\n"
            .bytes()
            .filter_map(|b| parser.feed(b).unwrap())
            .collect();
        assert_eq!(
            events,
            vec![("".into(), "first".into()), ("".into(), "second".into())]
        );
    }
    #[test]
    fn endpoint_never_leaks_credentials_cross_origin() {
        let base = url::Url::parse("https://example.test/sse").unwrap();
        assert_eq!(
            message_endpoint(&base, "/messages?id=1").unwrap().path(),
            "/messages"
        );
        for value in [
            "https://evil.test/messages",
            "http://example.test/messages",
            "https://example.test:444/messages",
            "https://user:secret@example.test/messages",
        ] {
            assert!(message_endpoint(&base, value).is_err(), "{value}");
        }
    }
}
