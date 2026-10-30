//! OAuth is an explicit user flow; connecting never opens a browser or grants access.
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use rmcp::transport::auth::{
    AuthClient, AuthError, AuthorizationManager, AuthorizationRequest, CredentialRefreshGuard,
    CredentialStore, InMemoryCredentialStore, OAuthState, StoredCredentials, WWWAuthenticateParams,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone)]
struct Vault {
    path: PathBuf,
    identity: String,
    test_profile: bool,
    refresh: Arc<Mutex<()>>,
    challenge: Option<String>,
}
fn store_error(error: impl std::fmt::Display) -> AuthError {
    AuthError::CredentialStoreError(error.to_string())
}
impl Vault {
    fn new(profile: &Path, server: &str, base_url: &str) -> Self {
        // Binding to both config name and endpoint prevents credentials moving
        // to a different server after a configuration edit.
        let identity = format!(
            "{:x}",
            Sha256::digest(format!("{}\0{server}\0{base_url}", profile.display()).as_bytes())
        );
        let path = profile.join("mcp-oauth").join(format!("{identity}.json"));
        static LOCKS: OnceLock<parking_lot::Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
            OnceLock::new();
        let mut locks = LOCKS.get_or_init(Default::default).lock();
        let refresh = locks.get(&path).and_then(Weak::upgrade).unwrap_or_else(|| {
            let lock = Arc::new(Mutex::new(()));
            locks.insert(path.clone(), Arc::downgrade(&lock));
            lock
        });
        Self {
            path,
            identity,
            test_profile: cfg!(test) || quixi_proto::paths::test_profile().is_some(),
            refresh,
            challenge: None,
        }
    }
    fn key(&self, create: bool) -> Result<Vec<u8>> {
        if self.test_profile {
            let path = self.path.with_extension("test-key");
            match fs::read(&path) {
                Ok(key) => return Ok(key),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {}
                Err(e) => return Err(e.into()),
            }
            fs::create_dir_all(path.parent().unwrap())?;
            let mut key = vec![0; 32];
            OsRng.fill_bytes(&mut key);
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    file.write_all(&key)?;
                    file.sync_all()?;
                    Ok(key)
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(fs::read(path)?),
                Err(e) => Err(e.into()),
            }
        } else {
            let entry = keyring::Entry::new("QuixiDesk MCP OAuth", &self.identity)?;
            match entry.get_password() {
                Ok(encoded) => Ok(STANDARD.decode(encoded)?),
                Err(keyring::Error::NoEntry) if create => {
                    let mut key = vec![0; 32];
                    OsRng.fill_bytes(&mut key);
                    entry.set_password(&STANDARD.encode(&key))?;
                    Ok(key)
                }
                Err(error) => Err(anyhow!(
                    "Unlock the system credential store and retry: {error}"
                )),
            }
        }
    }
    fn read(&self) -> Result<Option<StoredCredentials>> {
        Ok(self.read_record()?.map(|(credentials, _)| credentials))
    }
    fn read_record(&self) -> Result<Option<(StoredCredentials, Option<String>)>> {
        let bytes = match fs::read(&self.path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let document: serde_json::Value = serde_json::from_slice(&bytes)
            .context("Invalid encrypted OAuth credentials; existing file preserved")?;
        if document["version"] != 1 {
            bail!("Unsupported OAuth credential format; existing file preserved");
        }
        let nonce = STANDARD.decode(document["nonce"].as_str().unwrap_or_default())?;
        if nonce.len() != 12 {
            bail!("Invalid OAuth credential nonce; existing file preserved");
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key(false)?)
            .map_err(|_| anyhow!("Invalid OAuth vault key"))?;
        let encrypted = STANDARD.decode(document["ciphertext"].as_str().unwrap_or_default())?;
        let plain = cipher
            .decrypt(Nonce::from_slice(&nonce), encrypted.as_slice())
            .map_err(|_| {
                anyhow!("OAuth credentials could not be decrypted; existing file preserved")
            })?;
        let payload: serde_json::Value = serde_json::from_slice(&plain)?;
        if let Some(credentials) = payload.get("credentials") {
            Ok(Some((
                serde_json::from_value(credentials.clone())?,
                payload["challenge"].as_str().map(str::to_owned),
            )))
        } else {
            Ok(Some((serde_json::from_value(payload)?, None)))
        }
    }
    fn write(&self, credentials: &StoredCredentials) -> Result<()> {
        let previous = self.read_record()?;
        let challenge = self
            .challenge
            .clone()
            .or_else(|| previous.and_then(|(_, challenge)| challenge));
        fs::create_dir_all(self.path.parent().unwrap())?;
        let cipher = Aes256Gcm::new_from_slice(&self.key(true)?)
            .map_err(|_| anyhow!("Invalid OAuth vault key"))?;
        let mut nonce = [0; 12];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                serde_json::to_vec(&json!({"credentials":credentials,"challenge":challenge}))?
                    .as_slice(),
            )
            .map_err(|_| anyhow!("OAuth encryption failed"))?;
        let mut file = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?;
        file.write_all(&serde_json::to_vec(&json!({"version":1,"nonce":STANDARD.encode(nonce),"ciphertext":STANDARD.encode(encrypted)}))?)?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(|e| e.error)?;
        Ok(())
    }
}
#[async_trait::async_trait]
impl CredentialStore for Vault {
    async fn load(&self) -> std::result::Result<Option<StoredCredentials>, AuthError> {
        self.read().map_err(store_error)
    }
    async fn save(&self, credentials: StoredCredentials) -> std::result::Result<(), AuthError> {
        self.write(&credentials).map_err(store_error)
    }
    async fn clear(&self) -> std::result::Result<(), AuthError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(store_error(e)),
        }
    }
    async fn acquire_refresh_guard(
        &self,
    ) -> std::result::Result<Option<CredentialRefreshGuard>, AuthError> {
        Ok(Some(CredentialRefreshGuard::new(
            self.refresh.clone().lock_owned().await,
        )))
    }
}

async fn manager(profile: &Path, server: &str, base_url: &str) -> Result<AuthorizationManager> {
    crate::client::init_tls();
    let url = Url::parse(base_url)?;
    let loopback = url
        .host_str()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]");
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        bail!("OAuth requires HTTPS (HTTP is allowed only for loopback development servers)");
    }
    let mut manager = if loopback {
        AuthorizationManager::new_allowing_private_addresses(base_url).await?
    } else {
        AuthorizationManager::new(base_url).await?
    };
    manager.set_credential_store(Vault::new(profile, server, base_url));
    Ok(manager)
}

/// Restore tokens and SDK refresh handling without launching an authorization UI.
pub async fn authenticated_client(
    profile: &Path,
    server: &str,
    base_url: &str,
    http_client: reqwest::Client,
) -> Result<Option<AuthClient<reqwest::Client>>> {
    Ok(restored_manager(profile, server, base_url)
        .await?
        .map(|manager| AuthClient::new(http_client, manager)))
}

async fn restored_manager(
    profile: &Path,
    server: &str,
    base_url: &str,
) -> Result<Option<AuthorizationManager>> {
    // Avoid metadata requests for servers which have never been authorized.
    let Some((_, challenge)) = Vault::new(profile, server, base_url).read_record()? else {
        return Ok(None);
    };
    let mut manager = manager(profile, server, base_url).await?;
    if let Some(challenge) = challenge {
        let metadata = manager
            .resolve_metadata_from_challenge(Some(&challenge))
            .await?
            .metadata;
        manager.set_metadata(metadata);
    }
    if !manager.initialize_from_store().await? {
        return Ok(None);
    }
    Ok(Some(manager))
}

/// The legacy SSE transport refreshes through this manager before each request.
pub async fn authorized_manager(
    profile: &Path,
    server: &str,
    base_url: &str,
) -> Result<Option<Arc<AuthorizationManager>>> {
    Ok(restored_manager(profile, server, base_url)
        .await?
        .map(Arc::new))
}

pub async fn clear(profile: &Path, server: &str, base_url: &str) -> Result<()> {
    let vault = Vault::new(profile, server, base_url);
    let _guard = vault.refresh.lock().await;
    vault.clear().await?;
    Ok(())
}

pub struct PendingAuthorization {
    pub authorization_url: String,
    pub redirect_uri: String,
    state: OAuthState,
    listener: TcpListener,
    cancelled: CancellationToken,
    expected_state: String,
    staged: InMemoryCredentialStore,
    vault: Vault,
}

/// Start SDK discovery, registration and PKCE; callers display the returned URL.
pub async fn begin(
    profile: &Path,
    server: &str,
    base_url: &str,
    client_id: Option<&str>,
    challenge: Option<&str>,
    cancelled: CancellationToken,
) -> Result<PendingAuthorization> {
    let prepare = async {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let redirect_uri = format!(
            "http://127.0.0.1:{}/oauth/callback/{}",
            listener.local_addr()?.port(),
            uuid::Uuid::new_v4()
        );
        // Superseded/cancelled browser flows must not overwrite live credentials.
        let staged = InMemoryCredentialStore::new();
        let mut vault = Vault::new(profile, server, base_url);
        let previous = vault.read_record()?;
        vault.challenge = challenge.map(str::to_owned).or_else(|| {
            previous
                .as_ref()
                .and_then(|(_, challenge)| challenge.clone())
        });
        let mut manager = manager(profile, server, base_url).await?;
        manager.set_credential_store(staged.clone());
        let mut state = OAuthState::Unauthorized(manager);
        let mut request = AuthorizationRequest::new(&redirect_uri)
            .with_client_name("QuixiDesk")
            .with_application_type("native");
        if let Some(id) = client_id.filter(|id| !id.trim().is_empty()) {
            request = request.with_preregistered_client(id);
        }
        if let Some(challenge) = &vault.challenge {
            request = request.with_challenge(challenge);
        }
        // Retain previously granted capabilities when the server asks for more
        // scopes, while preserving the old grant until the user completes login.
        if let Some((previous, _)) = previous {
            let mut scopes = previous.granted_scopes;
            if let Some(challenge) = challenge {
                if let Some(extra) =
                    WWWAuthenticateParams::parse(challenge, &Url::parse(base_url)?).scope
                {
                    for scope in extra.split_whitespace() {
                        if !scopes.iter().any(|old| old == scope) {
                            scopes.push(scope.into());
                        }
                    }
                }
            }
            if !scopes.is_empty() {
                request = request.with_scopes(scopes);
            }
        }
        state.start_authorization(request).await?;
        let authorization_url = state.get_authorization_url().await?;
        let expected_state = Url::parse(&authorization_url)?
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(|| anyhow!("OAuth response did not include a state challenge"))?;
        Ok(PendingAuthorization {
            authorization_url,
            redirect_uri,
            state,
            listener,
            cancelled: cancelled.clone(),
            expected_state,
            staged,
            vault,
        })
    };
    tokio::select! {
        biased;
        _=cancelled.cancelled()=>bail!("OAuth authorization cancelled"),
        result=tokio::time::timeout(Duration::from_secs(60),prepare)=>result.context("OAuth discovery or registration timed out")?,
    }
}

async fn response(stream: &mut TcpStream, status: &str, text: &str) {
    let reply=format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{text}",text.len());
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.shutdown().await;
}
fn validate_callback(target: &str, redirect: &str, expected: &str) -> Result<Url> {
    if !target.starts_with('/') || target.starts_with("//") {
        bail!("Invalid callback target");
    }
    let expected_url = Url::parse(redirect)?;
    let callback = expected_url.join(target)?;
    if callback.origin() != expected_url.origin() || callback.path() != expected_url.path() {
        bail!("Unexpected callback path");
    }
    let pairs = callback.query_pairs().collect::<Vec<_>>();
    for key in ["state", "code", "error", "iss"] {
        if pairs.iter().filter(|(k, _)| k == key).count() > 1 {
            bail!("Duplicate callback parameter");
        }
    }
    if pairs
        .iter()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.as_ref())
        != Some(expected)
    {
        bail!("OAuth state did not match");
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        bail!("OAuth authorization was declined");
    }
    if !pairs.iter().any(|(k, v)| k == "code" && !v.is_empty()) {
        bail!("OAuth callback did not include a code");
    }
    Ok(callback)
}
impl PendingAuthorization {
    /// Wait for a valid local callback. Dropping/cancelling closes the listener.
    pub async fn wait(mut self) -> Result<()> {
        let cancelled = self.cancelled.clone();
        let wait = async {
            loop {
                let (mut stream, peer) = self.listener.accept().await?;
                if !peer.ip().is_loopback() {
                    continue;
                }
                let request = tokio::time::timeout(Duration::from_secs(5), async {
                    let mut bytes = Vec::new();
                    let mut buf = [0; 1024];
                    while bytes.len() < 16_384 {
                        let n = stream.read(&mut buf).await?;
                        if n == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Ok::<_, std::io::Error>(bytes)
                })
                .await;
                let Ok(Ok(bytes)) = request else {
                    continue;
                };
                let line = String::from_utf8_lossy(&bytes);
                let mut first = line.lines().next().unwrap_or_default().split_whitespace();
                let method = first.next();
                let target = first.next().unwrap_or_default();
                let callback = if method == Some("GET") {
                    validate_callback(target, &self.redirect_uri, &self.expected_state)
                } else {
                    Err(anyhow!("Expected GET callback"))
                };
                match callback {
                    Ok(callback) => {
                        // SDK validates the stored, one-use state and issuer and
                        // exchanges the code with the matching PKCE verifier.
                        let outcome: Result<()> = async {
                            self.state.handle_callback_url(callback.as_str()).await?;
                            let credentials =
                                self.staged.load().await?.ok_or_else(|| {
                                    anyhow!("OAuth callback returned no credentials")
                                })?;
                            let _guard = self.vault.refresh.lock().await;
                            if self.cancelled.is_cancelled() {
                                bail!("OAuth authorization cancelled");
                            }
                            self.vault.save(credentials).await?;
                            Ok(())
                        }
                        .await;
                        if outcome.is_ok() {
                            response(
                                &mut stream,
                                "200 OK",
                                "QuixiDesk is connected. You may close this window.",
                            )
                            .await;
                        } else {
                            response(
                                &mut stream,
                                "400 Bad Request",
                                "Authorization failed. Return to QuixiDesk and try again.",
                            )
                            .await;
                        }
                        outcome?;
                        return Ok(());
                    }
                    Err(error) => {
                        response(
                            &mut stream,
                            "400 Bad Request",
                            "Invalid or declined authorization. Return to QuixiDesk.",
                        )
                        .await;
                        if error.to_string() == "OAuth authorization was declined" {
                            return Err(error);
                        }
                    }
                }
            }
        };
        tokio::select! {
            biased;
            _=cancelled.cancelled()=>bail!("OAuth authorization cancelled"),
            result=tokio::time::timeout(Duration::from_secs(5*60),wait)=>result.context("OAuth authorization timed out")?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_rejects_wrong_state_routes_and_duplicate_fields() {
        let redirect = "http://127.0.0.1:4321/oauth/callback/random";
        assert!(validate_callback(
            "/oauth/callback/random?state=right&code=ok",
            redirect,
            "right"
        )
        .is_ok());
        for target in [
            "/oauth/callback/random?state=wrong&code=ok",
            "/other?state=right&code=ok",
            "//evil.test/oauth/callback/random?state=right&code=ok",
            "/oauth/callback/random?state=right&state=wrong&code=ok",
            "/oauth/callback/random?state=right&code=one&code=two",
            "/oauth/callback/random?state=right&error=access_denied",
        ] {
            assert!(
                validate_callback(target, redirect, "right").is_err(),
                "{target}"
            );
        }
    }
    #[tokio::test]
    async fn vault_is_encrypted_isolated_and_preserves_corrupt_data() {
        let profile = tempfile::tempdir().unwrap();
        let vault = Vault::new(profile.path(), "test", "https://mcp.example.test");
        let credentials:StoredCredentials=serde_json::from_value(json!({"client_id":"client","token_response":{"access_token":"secret-access-token","token_type":"bearer","refresh_token":"secret-refresh-token"}})).unwrap();
        vault.save(credentials).await.unwrap();
        let raw = fs::read_to_string(&vault.path).unwrap();
        assert!(!raw.contains("secret-"));
        assert_eq!(vault.load().await.unwrap().unwrap().client_id, "client");
        assert!(
            Vault::new(profile.path(), "test", "https://different.example.test")
                .load()
                .await
                .unwrap()
                .is_none()
        );
        fs::write(&vault.path, b"corrupt").unwrap();
        assert!(vault.load().await.is_err());
        assert_eq!(fs::read(&vault.path).unwrap(), b"corrupt");
        vault.clear().await.unwrap();
        assert!(vault.load().await.unwrap().is_none());
    }

    async fn oauth_server() -> (
        String,
        Arc<parking_lot::Mutex<Vec<(String, String)>>>,
        tokio::task::JoinHandle<()>,
    ) {
        oauth_server_mode(false).await
    }
    async fn oauth_server_mode(
        challenge_only: bool,
    ) -> (
        String,
        Arc<parking_lot::Mutex<Vec<(String, String)>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let seen = requests.clone();
        let endpoint = base.clone();
        let worker = tokio::spawn(async move {
            let mut token_count = 0;
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = vec![];
                let mut buf = [0; 4096];
                let header_end = loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
                let path = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|n| n.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let n = stream.read(&mut buf).await.unwrap();
                    bytes.extend_from_slice(&buf[..n]);
                }
                let body =
                    String::from_utf8_lossy(&bytes[header_end..header_end + length]).to_string();
                seen.lock().push((path.clone(), body.clone()));
                let issuer = if challenge_only {
                    format!("{endpoint}/issuer")
                } else {
                    endpoint.clone()
                };
                let missing = challenge_only
                    && path.starts_with("/.well-known/")
                    && !path.ends_with("/issuer");
                let document = if missing {
                    json!({})
                } else if path == "/private-metadata"
                    || path.starts_with("/.well-known/oauth-protected-resource")
                {
                    json!({"resource":format!("{endpoint}/mcp"),"authorization_servers":[issuer],"scopes_supported":["tools:read"]})
                } else if path.starts_with("/.well-known/") {
                    json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),"token_endpoint":format!("{issuer}/token"),"registration_endpoint":format!("{issuer}/register"),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"],"authorization_response_iss_parameter_supported":true})
                } else if path.ends_with("/register") {
                    let registration: serde_json::Value = serde_json::from_str(&body).unwrap();
                    json!({"client_id":"quixi-fixture","redirect_uris":registration["redirect_uris"],"token_endpoint_auth_method":"none"})
                } else if path.ends_with("/token") {
                    token_count += 1;
                    json!({"access_token":format!("access-{token_count}"),"refresh_token":format!("refresh-{token_count}"),"token_type":"Bearer","expires_in":3600,"scope":"tools:read"})
                } else {
                    json!({})
                };
                let body = document.to_string();
                let status = if missing { "404 Not Found" } else { "200 OK" };
                let reply=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        (base, requests, worker)
    }

    #[tokio::test]
    async fn real_oauth_discovery_pkce_callback_vault_restore_and_refresh() {
        let (base, requests, server) = oauth_server().await;
        let profile = tempfile::tempdir().unwrap();
        let resource = format!("{base}/mcp");
        let pending = begin(
            profile.path(),
            "fixture",
            &resource,
            None,
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let authorization = Url::parse(&pending.authorization_url).unwrap();
        let params = authorization
            .query_pairs()
            .into_owned()
            .collect::<HashMap<_, _>>();
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["resource"], resource);
        let mut callback = Url::parse(&pending.redirect_uri).unwrap();
        let callback_base = callback.clone();
        let waiting = tokio::spawn(pending.wait());
        callback
            .query_pairs_mut()
            .append_pair("state", "wrong")
            .append_pair("code", "bad");
        assert_eq!(reqwest::get(callback).await.unwrap().status(), 400);
        assert!(!requests.lock().iter().any(|(p, _)| p == "/token"));
        let mut callback = callback_base;
        callback
            .query_pairs_mut()
            .append_pair("state", &params["state"])
            .append_pair("code", "approved-code")
            .append_pair("iss", &base);
        assert_eq!(reqwest::get(callback).await.unwrap().status(), 200);
        waiting.await.unwrap().unwrap();
        let first_exchange = requests
            .lock()
            .iter()
            .find(|(p, _)| p == "/token")
            .unwrap()
            .1
            .clone();
        let exchange = url::form_urlencoded::parse(first_exchange.as_bytes())
            .into_owned()
            .collect::<HashMap<_, _>>();
        assert_eq!(exchange["code"], "approved-code");
        assert_eq!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(exchange["code_verifier"].as_bytes())),
            params["code_challenge"]
        );
        let vault = Vault::new(profile.path(), "fixture", &resource);
        let mut stored = vault.load().await.unwrap().unwrap();
        stored.token_received_at = Some(1);
        vault.save(stored).await.unwrap();
        let client =
            authenticated_client(profile.path(), "fixture", &resource, reqwest::Client::new())
                .await
                .unwrap()
                .unwrap();
        assert_eq!(client.get_access_token().await.unwrap(), "access-2");
        let last = requests
            .lock()
            .iter()
            .filter(|(p, _)| p == "/token")
            .last()
            .unwrap()
            .1
            .clone();
        assert!(last.contains("grant_type=refresh_token"));
        assert!(last.contains("refresh_token=refresh-1"));
        let mut stored = vault.load().await.unwrap().unwrap();
        stored.token_received_at = Some(1);
        vault.save(stored).await.unwrap();
        let sse_manager = authorized_manager(profile.path(), "fixture", &resource)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sse_manager.get_access_token().await.unwrap(), "access-3");
        assert!(!fs::read_to_string(&vault.path)
            .unwrap()
            .contains("refresh-2"));
        clear(profile.path(), "fixture", &resource).await.unwrap();
        assert!(
            authenticated_client(profile.path(), "fixture", &resource, reqwest::Client::new())
                .await
                .unwrap()
                .is_none()
        );
        server.abort();
    }

    #[tokio::test]
    async fn challenge_only_metadata_survives_restart_and_cancelled_scope_upgrade() {
        let (base, requests, server) = oauth_server_mode(true).await;
        let profile = tempfile::tempdir().unwrap();
        let resource = format!("{base}/mcp");
        let challenge =
            format!("Bearer resource_metadata=\"{base}/private-metadata\", scope=\"tools:read\"");
        let pending = begin(
            profile.path(),
            "fixture",
            &resource,
            None,
            Some(&challenge),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let authorization = Url::parse(&pending.authorization_url).unwrap();
        assert_eq!(authorization.path(), "/issuer/authorize");
        let state = authorization
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let mut callback = Url::parse(&pending.redirect_uri).unwrap();
        callback
            .query_pairs_mut()
            .append_pair("state", &state)
            .append_pair("code", "approved")
            .append_pair("iss", &format!("{base}/issuer"));
        let waiting = tokio::spawn(pending.wait());
        assert_eq!(reqwest::get(callback).await.unwrap().status(), 200);
        waiting.await.unwrap().unwrap();
        let vault = Vault::new(profile.path(), "fixture", &resource);
        assert_eq!(
            vault.read_record().unwrap().unwrap().1.as_deref(),
            Some(challenge.as_str())
        );
        let mut stored = vault.load().await.unwrap().unwrap();
        stored.token_received_at = Some(1);
        vault.save(stored).await.unwrap();
        let restored = authorized_manager(profile.path(), "fixture", &resource)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.get_access_token().await.unwrap(), "access-2");
        assert_eq!(
            requests
                .lock()
                .iter()
                .filter(|(p, _)| p == "/private-metadata")
                .count(),
            2
        );
        let old_bytes = fs::read(&vault.path).unwrap();
        let upgrade =
            format!("Bearer resource_metadata=\"{base}/private-metadata\", scope=\"tools:write\"");
        let cancelled = CancellationToken::new();
        let pending = begin(
            profile.path(),
            "fixture",
            &resource,
            None,
            Some(&upgrade),
            cancelled.clone(),
        )
        .await
        .unwrap();
        let url = Url::parse(&pending.authorization_url).unwrap();
        let scopes = url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .unwrap()
            .1
            .into_owned();
        assert!(scopes.split_whitespace().any(|s| s == "tools:read"));
        assert!(scopes.split_whitespace().any(|s| s == "tools:write"));
        cancelled.cancel();
        assert!(pending.wait().await.is_err());
        assert_eq!(
            fs::read(&vault.path).unwrap(),
            old_bytes,
            "Cancelling upgrade must preserve the existing grant"
        );
        server.abort();
    }

    #[tokio::test]
    async fn wrong_issuer_is_rejected_before_token_exchange_and_persistence() {
        let (base, requests, server) = oauth_server().await;
        let profile = tempfile::tempdir().unwrap();
        let resource = format!("{base}/mcp");
        let pending = begin(
            profile.path(),
            "fixture",
            &resource,
            None,
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let state = Url::parse(&pending.authorization_url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let mut callback = Url::parse(&pending.redirect_uri).unwrap();
        callback
            .query_pairs_mut()
            .append_pair("state", &state)
            .append_pair("code", "wrong-issuer-code")
            .append_pair("iss", "https://another-issuer.example");
        let waiting = tokio::spawn(pending.wait());
        assert_eq!(reqwest::get(callback).await.unwrap().status(), 400);
        assert!(waiting.await.unwrap().is_err());
        assert!(!requests.lock().iter().any(|(p, _)| p == "/token"));
        assert!(Vault::new(profile.path(), "fixture", &resource)
            .load()
            .await
            .unwrap()
            .is_none());
        server.abort();
    }

    #[tokio::test]
    async fn cancellation_closes_callback_listener_without_storing_tokens() {
        let (base, _, server) = oauth_server().await;
        let profile = tempfile::tempdir().unwrap();
        let resource = format!("{base}/mcp");
        let cancelled = CancellationToken::new();
        let pending = begin(
            profile.path(),
            "fixture",
            &resource,
            Some("registered-client"),
            None,
            cancelled.clone(),
        )
        .await
        .unwrap();
        let callback = pending.redirect_uri.clone();
        cancelled.cancel();
        assert!(pending
            .wait()
            .await
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        assert!(reqwest::get(callback).await.is_err());
        assert!(Vault::new(profile.path(), "fixture", &resource)
            .load()
            .await
            .unwrap()
            .is_none());
        server.abort();
    }
}
