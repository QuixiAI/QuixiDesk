//! Server requests for user input. Nothing here opens a URL or submits a default.
//! All records are scoped to a server connection and are removed on cancellation.
use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

const MAX_PENDING: usize = 32;
const MAX_REQUEST_BYTES: usize = 128 * 1024;
const TTL: Duration = Duration::from_secs(300);

struct Pending {
    view: Value,
    deadline: Instant,
    request_cancel: CancellationToken,
    connection_cancel: CancellationToken,
    parent_cancel: Option<CancellationToken>,
    response: Option<oneshot::Sender<Value>>,
}
impl Pending {
    fn unavailable(&self) -> bool {
        self.deadline <= Instant::now()
            || self.connection_cancel.is_cancelled()
            || (self.view["status"] == "pending"
                && (self.request_cancel.is_cancelled()
                    || self
                        .parent_cancel
                        .as_ref()
                        .is_some_and(CancellationToken::is_cancelled)))
    }
}

#[derive(Default)]
pub struct InteractionBroker {
    pending: Mutex<HashMap<String, Pending>>,
}
impl InteractionBroker {
    /// A wire-compatible request entry point for transport relays.
    #[cfg(test)]
    pub async fn request(
        &self,
        server: &str,
        connection: u64,
        request_id: Value,
        params: Value,
        request_cancel: &CancellationToken,
        connection_cancel: &CancellationToken,
    ) -> Result<Value> {
        self.request_scoped(
            server,
            connection,
            request_id,
            params,
            request_cancel,
            connection_cancel,
            None,
        )
        .await
    }
    /// `parent_cancel` belongs to the outgoing tool/resource request, when known.
    pub async fn request_scoped(
        &self,
        server: &str,
        connection: u64,
        request_id: Value,
        params: Value,
        request_cancel: &CancellationToken,
        connection_cancel: &CancellationToken,
        parent_cancel: Option<&CancellationToken>,
    ) -> Result<Value> {
        self.request_with_ttl(
            server,
            connection,
            request_id,
            params,
            request_cancel,
            connection_cancel,
            parent_cancel,
            TTL,
        )
        .await
    }
    async fn request_with_ttl(
        &self,
        server: &str,
        connection: u64,
        request_id: Value,
        params: Value,
        request_cancel: &CancellationToken,
        connection_cancel: &CancellationToken,
        parent_cancel: Option<&CancellationToken>,
        ttl: Duration,
    ) -> Result<Value> {
        if request_cancel.is_cancelled()
            || connection_cancel.is_cancelled()
            || parent_cancel.is_some_and(CancellationToken::is_cancelled)
        {
            return Ok(json!({"action":"cancel"}));
        }
        let (sender, receiver) = oneshot::channel();
        let record = make_record(
            server,
            connection,
            request_id,
            params,
            request_cancel.clone(),
            connection_cancel.clone(),
            parent_cancel.cloned(),
            Some(sender),
            ttl,
        )?;
        let id = record.view["id"].as_str().unwrap().to_owned();
        {
            let mut pending = self.pending.lock();
            prune(&mut pending);
            check_capacity_and_identity(&pending, &record)?;
            pending.insert(id.clone(), record);
        }
        let _lease = RequestLease { broker: self, id };
        let never = CancellationToken::new();
        let parent = parent_cancel.unwrap_or(&never);
        tokio::select! {
            biased;
            _=request_cancel.cancelled()=>Ok(json!({"action":"cancel"})),
            _=connection_cancel.cancelled()=>Ok(json!({"action":"cancel"})),
            _=parent.cancelled()=>Ok(json!({"action":"cancel"})),
            _=tokio::time::sleep(ttl)=>Ok(json!({"action":"cancel"})),
            result=receiver=>Ok(result.unwrap_or_else(|_|json!({"action":"cancel"}))),
        }
    }
    /// Safe to poll from the UI. Responses and entered values are never retained.
    pub fn pending(&self) -> Vec<Value> {
        let mut records = self.pending.lock();
        prune(&mut records);
        let mut rows = records.values().map(|p| p.view.clone()).collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a["createdAt"]
                .as_u64()
                .cmp(&b["createdAt"].as_u64())
                .then(a["id"].as_str().cmp(&b["id"].as_str()))
        });
        rows
    }
    pub fn respond(&self, id: &str, action: &str, content: Option<Value>) -> Result<Value> {
        if !["accept", "decline", "cancel"].contains(&action) {
            bail!("Choose accept, decline, or cancel");
        }
        let mut records = self.pending.lock();
        prune(&mut records);
        let record = records
            .get_mut(id)
            .context("This request expired, was cancelled, or was already answered")?;
        if record.view["status"] != "pending" {
            if action == "cancel" {
                records.remove(id);
                return Ok(json!({"ok":true,"status":"cancelled","requestAlreadyAccepted":true}));
            }
            bail!("This request has already been answered");
        }
        let mut result = json!({"action":action});
        if action == "accept" && record.view["mode"] == "form" {
            let content = content.context("Form responses require submitted values")?;
            let object = content
                .as_object()
                .context("Form responses must be an object")?;
            let schema = &record.view["requestedSchema"];
            let properties = schema["properties"].as_object().unwrap();
            if object.keys().any(|k| !properties.contains_key(k)) {
                bail!("The response contains a field not requested by the server");
            }
            if serde_json::to_vec(&content)?.len() > MAX_REQUEST_BYTES {
                bail!("Form response is too large");
            }
            jsonschema::options()
                .should_validate_formats(true)
                .build(schema)
                .map_err(|e| anyhow!("Invalid form schema: {e}"))?
                .validate(&content)
                .map_err(|e| anyhow!("Invalid form response: {e}"))?;
            result["content"] = content;
        }
        // URL acceptance records consent only. It never claims the external flow completed.
        let open_url = if action == "accept" && record.view["mode"] == "url" {
            Some(record.view["url"].clone())
        } else {
            None
        };
        if let Some(sender) = record.response.take() {
            if sender.send(result).is_err() {
                records.remove(id);
                bail!("The server stopped waiting for this request");
            }
        }
        if let Some(url) = open_url {
            record.view["status"] = json!("waiting_external");
            Ok(json!({"ok":true,"status":"waiting_external","url":url}))
        } else {
            records.remove(id);
            Ok(json!({"ok":true,"status":action}))
        }
    }
    /// A completion can only settle the matching, accepted URL request on this connection.
    pub fn complete_url(&self, server: &str, connection: u64, elicitation_id: &str) -> bool {
        let mut records = self.pending.lock();
        prune(&mut records);
        let Some(record) = records.values_mut().find(|p| {
            p.view["server"] == server
                && p.view["connection"] == connection
                && p.view["elicitationId"] == elicitation_id
                && p.view["status"] == "waiting_external"
        }) else {
            return false;
        };
        record.view["status"] = json!("completed");
        record.view["message"]=json!("The server reported that the browser interaction completed. Retry the original request when ready.");
        record.view.as_object_mut().unwrap().remove("url");
        record.deadline = Instant::now() + Duration::from_secs(15);
        true
    }
    #[cfg(test)]
    pub fn cancel_connection(&self, server: &str, connection: u64) {
        self.pending
            .lock()
            .retain(|_, p| p.view["server"] != server || p.view["connection"] != connection);
    }
    /// Handle JSON-RPC -32042 without re-executing the failed request. The caller
    /// returns the failure and these IDs; browser completion never retries a mutation.
    pub fn enqueue_url_required(
        &self,
        server: &str,
        connection: u64,
        request_id: Value,
        data: &Value,
        connection_cancel: &CancellationToken,
    ) -> Result<Vec<String>> {
        let elicitations = data["elicitations"]
            .as_array()
            .context("URL elicitation error omitted elicitations")?;
        if elicitations.is_empty() || elicitations.len() > 8 {
            bail!("URL elicitation error must contain between one and eight requests");
        }
        let mut batch = Vec::new();
        for params in elicitations {
            if params["mode"] != "url" {
                bail!("URL elicitation errors may contain only URL requests");
            }
            let mut record = make_record(
                server,
                connection,
                request_id.clone(),
                params.clone(),
                CancellationToken::new(),
                connection_cancel.clone(),
                None,
                None,
                TTL,
            )?;
            record.view["requiresRetry"] = json!(true);
            batch.push(record);
        }
        let mut records = self.pending.lock();
        prune(&mut records);
        if records.len() + batch.len() > MAX_PENDING {
            bail!("Too many pending server requests");
        }
        let mut ids = Vec::new();
        // Validate duplicates before publishing any part of the batch.
        let mut unique = std::collections::HashSet::new();
        for record in &batch {
            check_capacity_and_identity(&records, record)?;
            if !unique.insert(record.view["elicitationId"].as_str().unwrap().to_owned()) {
                bail!("Duplicate elicitation ID in error response");
            }
        }
        for record in batch {
            let id = record.view["id"].as_str().unwrap().to_owned();
            records.insert(id.clone(), record);
            ids.push(id);
        }
        Ok(ids)
    }
}
struct RequestLease<'a> {
    broker: &'a InteractionBroker,
    id: String,
}
impl Drop for RequestLease<'_> {
    fn drop(&mut self) {
        let mut records = self.broker.pending.lock();
        if records
            .get(&self.id)
            .is_some_and(|r| r.view["status"] == "pending")
        {
            records.remove(&self.id);
        }
    }
}
fn prune(records: &mut HashMap<String, Pending>) {
    records.retain(|_, p| !p.unavailable());
}
fn check_capacity_and_identity(records: &HashMap<String, Pending>, record: &Pending) -> Result<()> {
    if records.len() >= MAX_PENDING {
        bail!("Too many pending server requests");
    }
    if record.view["mode"] == "url"
        && records.values().any(|p| {
            p.view["server"] == record.view["server"]
                && p.view["connection"] == record.view["connection"]
                && p.view["elicitationId"] == record.view["elicitationId"]
        })
    {
        bail!("Duplicate active elicitation ID");
    }
    Ok(())
}
fn make_record(
    server: &str,
    connection: u64,
    request_id: Value,
    params: Value,
    request_cancel: CancellationToken,
    connection_cancel: CancellationToken,
    parent_cancel: Option<CancellationToken>,
    response: Option<oneshot::Sender<Value>>,
    ttl: Duration,
) -> Result<Pending> {
    if connection_cancel.is_cancelled() {
        bail!("MCP connection is no longer available");
    }
    if serde_json::to_vec(&params)?.len() > MAX_REQUEST_BYTES {
        bail!("Elicitation request is too large");
    }
    let message = params["message"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .context("Elicitation request needs a message")?;
    let mode = params["mode"].as_str().unwrap_or("form");
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut view = json!({"id":uuid::Uuid::new_v4().to_string(),"server":server,"connection":connection,"requestId":request_id,"mode":mode,"message":message,"createdAt":created,"expiresAt":created+ttl.as_millis() as u64,"status":"pending"});
    match mode {
        "form" => {
            let schema = params["requestedSchema"].clone();
            validate_form_schema(&schema, message)?;
            view["requestedSchema"] = schema;
        }
        "url" => {
            let raw = params["url"]
                .as_str()
                .context("URL elicitation omitted its URL")?;
            if raw.len() > 8192 {
                bail!("Elicitation URL is too long");
            }
            let url = url::Url::parse(raw).context("Invalid elicitation URL")?;
            if !matches!(url.scheme(), "https" | "http")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                bail!("Elicitation URLs must use HTTP(S) without embedded credentials");
            }
            let id = params["elicitationId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 1024)
                .context("URL elicitation omitted its identifier")?;
            view["url"] = json!(url.as_str());
            view["host"] = json!(url.authority());
            view["elicitationId"] = json!(id);
        }
        _ => bail!("Unsupported elicitation mode '{mode}'"),
    }
    Ok(Pending {
        view,
        deadline: Instant::now() + ttl,
        request_cancel,
        connection_cancel,
        parent_cancel,
        response,
    })
}
fn secret_label(label: &str) -> bool {
    let lower = label.to_lowercase();
    let compact = lower
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();
    [
        "password",
        "passphrase",
        "apikey",
        "apitoken",
        "accesstoken",
        "refreshtoken",
        "clientsecret",
        "privatekey",
        "creditcard",
        "cardnumber",
        "paymentcredentials",
        "cvv",
        "cvc",
    ]
    .iter()
    .any(|word| compact.contains(word))
}
fn validate_form_schema(schema: &Value, message: &str) -> Result<()> {
    if schema["type"] != "object" {
        bail!("Elicitation form schema must be an object");
    }
    let props = schema["properties"]
        .as_object()
        .context("Elicitation form needs properties")?;
    if props.len() > 64 {
        bail!("An elicitation form may request at most 64 fields");
    }
    if secret_label(message) {
        bail!("Sensitive credentials must be requested through URL mode, not a form");
    }
    for (name, field) in props {
        if secret_label(name)
            || field
                .get("title")
                .and_then(Value::as_str)
                .is_some_and(secret_label)
            || field
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(secret_label)
            || field["writeOnly"] == true
            || field["format"] == "password"
        {
            bail!("Sensitive credentials must be requested through URL mode, not a form");
        }
        let kind = field["type"]
            .as_str()
            .context("Every form field needs a primitive type")?;
        if !matches!(kind, "string" | "number" | "integer" | "boolean" | "array") {
            bail!("Nested form fields are not supported");
        }
        if kind == "array" {
            let items = &field["items"];
            let strings = items["type"] == "string"
                && items["enum"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty() && v.iter().all(Value::is_string));
            let titled = items["anyOf"]
                .as_array()
                .is_some_and(|v| !v.is_empty() && v.iter().all(|v| v["const"].is_string()));
            if !strings && !titled {
                bail!("Array form fields must be string selection lists");
            }
        }
        if let Some(choices) = field.get("oneOf") {
            if kind != "string"
                || !choices
                    .as_array()
                    .is_some_and(|v| !v.is_empty() && v.iter().all(|v| v["const"].is_string()))
            {
                bail!("Form choices must be string selections");
            }
        }
        for key in [
            "$ref",
            "$dynamicRef",
            "properties",
            "additionalProperties",
            "allOf",
            "if",
            "then",
            "else",
            "not",
        ] {
            if field.get(key).is_some() {
                bail!("Unsupported form schema keyword '{key}'");
            }
        }
    }
    for key in [
        "$ref",
        "$dynamicRef",
        "$defs",
        "definitions",
        "allOf",
        "anyOf",
        "oneOf",
        "if",
        "then",
        "else",
        "patternProperties",
    ] {
        if schema.get(key).is_some() {
            bail!("Unsupported form schema keyword '{key}'");
        }
    }
    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .context("Required fields must be an array")?;
        for name in required {
            if !name.as_str().is_some_and(|s| props.contains_key(s)) {
                bail!("Required field is missing from properties");
            }
        }
    }
    jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|e| anyhow!("Invalid elicitation schema: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn form() -> Value {
        json!({"message":"Choose a club name","requestedSchema":{"type":"object","properties":{"name":{"type":"string","minLength":3},"seats":{"type":"integer","minimum":1},"email":{"type":"string","format":"email"}},"required":["name"]}})
    }
    fn url() -> Value {
        json!({"mode":"url","message":"Connect your club account","url":"https://example.com/club/connect","elicitationId":"club-connection"})
    }
    async fn wait_row(broker: &InteractionBroker) -> Value {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(p) = broker.pending().first() {
                    return p.clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    fn spawn(
        broker: Arc<InteractionBroker>,
        params: Value,
        connection: CancellationToken,
        parent: CancellationToken,
    ) -> tokio::task::JoinHandle<Result<Value>> {
        tokio::spawn(async move {
            broker
                .request_scoped(
                    "fixture",
                    7,
                    json!(42),
                    params,
                    &CancellationToken::new(),
                    &connection,
                    Some(&parent),
                )
                .await
        })
    }
    #[tokio::test]
    async fn form_waits_for_explicit_valid_submission_and_never_retains_entered_values() {
        let b = Arc::new(InteractionBroker::default());
        let task = spawn(
            b.clone(),
            form(),
            CancellationToken::new(),
            CancellationToken::new(),
        );
        let row = wait_row(&b).await;
        let id = row["id"].as_str().unwrap();
        assert!(!task.is_finished());
        assert!(b.respond(id, "accept", Some(json!({"name":"ab"}))).is_err());
        assert!(b
            .respond(id, "accept", Some(json!({"name":"Club","email":"bad"})))
            .is_err());
        assert!(b
            .respond(
                id,
                "accept",
                Some(json!({"name":"Club","unrequested":"secret"}))
            )
            .is_err());
        assert_eq!(b.pending().len(), 1);
        b.respond(id, "accept", Some(json!({"name":"Chess club","seats":12})))
            .unwrap();
        assert_eq!(
            task.await.unwrap().unwrap(),
            json!({"action":"accept","content":{"name":"Chess club","seats":12}})
        );
        assert!(b.pending().is_empty());
        assert!(b
            .respond(id, "accept", Some(json!({"name":"Again"})))
            .is_err());
    }
    #[tokio::test]
    async fn decline_and_cancel_do_not_send_form_values() {
        for action in ["decline", "cancel"] {
            let b = Arc::new(InteractionBroker::default());
            let t = spawn(
                b.clone(),
                form(),
                CancellationToken::new(),
                CancellationToken::new(),
            );
            let p = wait_row(&b).await;
            b.respond(
                p["id"].as_str().unwrap(),
                action,
                Some(json!({"name":"Not sent"})),
            )
            .unwrap();
            assert_eq!(t.await.unwrap().unwrap(), json!({"action":action}));
        }
    }
    #[tokio::test]
    async fn parent_stop_connection_close_and_timeout_cancel_pending_requests() {
        for connection_stopped in [false, true] {
            let b = Arc::new(InteractionBroker::default());
            let connection = CancellationToken::new();
            let parent = CancellationToken::new();
            let t = spawn(b.clone(), form(), connection.clone(), parent.clone());
            wait_row(&b).await;
            if connection_stopped {
                connection.cancel();
            } else {
                parent.cancel();
            }
            assert_eq!(t.await.unwrap().unwrap(), json!({"action":"cancel"}));
            assert!(b.pending().is_empty());
        }
        let b = InteractionBroker::default();
        let result = b
            .request_with_ttl(
                "fixture",
                1,
                json!(1),
                form(),
                &CancellationToken::new(),
                &CancellationToken::new(),
                None,
                Duration::from_millis(20),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({"action":"cancel"}));
        assert!(b.pending().is_empty());
    }
    #[tokio::test]
    async fn dropping_the_handler_does_not_leave_a_live_approval() {
        let b = Arc::new(InteractionBroker::default());
        let t = spawn(
            b.clone(),
            form(),
            CancellationToken::new(),
            CancellationToken::new(),
        );
        wait_row(&b).await;
        t.abort();
        let _ = t.await;
        assert!(b.pending().is_empty());
    }
    #[tokio::test]
    async fn url_consent_and_completion_are_separate_and_connection_bound() {
        let b = Arc::new(InteractionBroker::default());
        let t = spawn(
            b.clone(),
            url(),
            CancellationToken::new(),
            CancellationToken::new(),
        );
        let p = wait_row(&b).await;
        assert_eq!(p["host"], "example.com");
        assert!(!b.complete_url("fixture", 7, "club-connection"));
        let result = b
            .respond(p["id"].as_str().unwrap(), "accept", None)
            .unwrap();
        assert_eq!(result["url"], "https://example.com/club/connect");
        assert_eq!(t.await.unwrap().unwrap(), json!({"action":"accept"}));
        assert_eq!(b.pending()[0]["status"], "waiting_external");
        assert!(!b.complete_url("another", 7, "club-connection"));
        assert!(!b.complete_url("fixture", 8, "club-connection"));
        assert!(b.complete_url("fixture", 7, "club-connection"));
        assert!(!b.complete_url("fixture", 7, "club-connection"));
        assert_eq!(b.pending()[0]["status"], "completed");
        assert!(b.pending()[0].get("url").is_none());
    }
    #[tokio::test]
    async fn unsafe_urls_and_secret_forms_never_become_pending() {
        let b = InteractionBroker::default();
        for unsafe_url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com",
            "data:text/html,hello",
        ] {
            let mut p = url();
            p["url"] = json!(unsafe_url);
            assert!(b
                .request(
                    "fixture",
                    1,
                    json!(1),
                    p,
                    &CancellationToken::new(),
                    &CancellationToken::new()
                )
                .await
                .is_err());
        }
        for name in ["password", "api_key", "accessToken", "creditCardNumber"] {
            let mut p = form();
            p["requestedSchema"]["properties"] = json!({name:{"type":"string"}});
            p["requestedSchema"]["required"] = json!([name]);
            assert!(b
                .request(
                    "fixture",
                    1,
                    json!(1),
                    p,
                    &CancellationToken::new(),
                    &CancellationToken::new()
                )
                .await
                .is_err());
        }
        assert!(b.pending().is_empty());
    }
    #[test]
    fn form_schema_supports_titled_and_multiple_choices_but_not_remote_refs() {
        let schema = json!({"type":"object","properties":{"kind":{"type":"string","oneOf":[{"const":"chess","title":"Chess"}]},"days":{"type":"array","items":{"anyOf":[{"const":"mon","title":"Monday"}]}}}});
        assert!(validate_form_schema(&schema, "Choose club settings").is_ok());
        let mut schema = schema;
        schema["properties"]["kind"]["$ref"] = json!("https://example.com/secret-schema");
        assert!(validate_form_schema(&schema, "Choose club settings").is_err());
    }
    #[test]
    fn required_url_errors_wait_for_consent_and_never_retry() {
        let b = InteractionBroker::default();
        let ids = b
            .enqueue_url_required(
                "fixture",
                3,
                json!(8),
                &json!({"elicitations":[url()]}),
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(b.pending()[0]["requiresRetry"], true);
        assert!(b
            .enqueue_url_required(
                "fixture",
                3,
                json!(9),
                &json!({"elicitations":[url()]}),
                &CancellationToken::new()
            )
            .is_err());
        assert_eq!(b.pending().len(), 1);
        b.respond(&ids[0], "decline", None).unwrap();
        assert!(b.pending().is_empty());
    }
    #[tokio::test]
    async fn cancelling_one_connection_does_not_cancel_another() {
        let b = Arc::new(InteractionBroker::default());
        let a = spawn(
            b.clone(),
            form(),
            CancellationToken::new(),
            CancellationToken::new(),
        );
        wait_row(&b).await;
        let ids = b
            .enqueue_url_required(
                "other",
                7,
                json!(8),
                &json!({"elicitations":[url()]}),
                &CancellationToken::new(),
            )
            .unwrap();
        b.cancel_connection("fixture", 7);
        assert_eq!(a.await.unwrap().unwrap(), json!({"action":"cancel"}));
        assert_eq!(b.pending().len(), 1);
        b.respond(&ids[0], "cancel", None).unwrap();
    }
}
