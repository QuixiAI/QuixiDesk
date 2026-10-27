pub mod paths;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EIPC_UUID_MAIN: &str = "5de9b164-2257-4603-a1e1-e00cdb72056c";
pub const EIPC_UUID_WINDOW: &str = "fcb91d11-972d-4fd6-b90b-72739699bff5";
pub const VSOCK_PORT_GUEST_DAEMON: u32 = 51234; // 0xc822

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EipcMethodKind {
    Call,
    StoreGetState,
    StoreGetStateSync,
    StoreUpdate,
}

#[derive(Debug, Clone)]
pub struct ParsedEipcChannel {
    pub raw: String,
    pub uuid: String,
    pub namespace: String,
    pub service: String,
    pub member: String,
    pub kind: EipcMethodKind,
}

impl ParsedEipcChannel {
    /// Parses `$eipc_message$_<uuid>_$_<namespace>_$_<Service>_$_<member>`
    pub fn parse(channel: &str) -> Option<Self> {
        let rest = channel.strip_prefix("$eipc_message$_")?;
        let parts: Vec<&str> = rest.split("_$_").collect();
        if parts.len() < 4 {
            return None;
        }
        let uuid = parts[0].to_string();
        let namespace = parts[1].to_string();
        let service = parts[2].to_string();
        let tail = parts[3..].join("_$_");

        let (member, kind) = if let Some(prop) = tail.strip_suffix("_$store$_getStateSync") {
            (prop.to_string(), EipcMethodKind::StoreGetStateSync)
        } else if let Some(prop) = tail.strip_suffix("_$store$_getState") {
            (prop.to_string(), EipcMethodKind::StoreGetState)
        } else if let Some(prop) = tail.strip_suffix("_$store$_update") {
            (prop.to_string(), EipcMethodKind::StoreUpdate)
        } else {
            (tail, EipcMethodKind::Call)
        };

        Some(Self {
            raw: channel.to_string(),
            uuid,
            namespace,
            service,
            member,
            kind,
        })
    }

    pub fn store_key(&self) -> String {
        format!("{}_$_{}", self.service, self.member)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EipcInvokePayload {
    pub channel: String,
    #[serde(default)]
    pub args: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EipcReplyPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl EipcReplyPayload {
    pub fn ok(val: Value) -> Self {
        Self {
            result: Some(val),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            result: None,
            error: Some(msg.into()),
        }
    }
}

/// LocalAgentModeSessions session record matching `Xv(e)` validator in `index.chunk-jPKPzV-P.js`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAgentSession {
    pub session_id: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_session_id: Option<String>,
    pub user_selected_folders: Vec<String>,
    #[serde(default)]
    pub user_selected_project_uuids: Vec<String>,
    pub is_running: bool,
    pub model: String,
    pub effort_level: String,
    pub created_at: u64,
    pub last_activity_at: u64,
    pub is_archived: bool,
    pub title: String,
    pub home_path: String,
    pub folder_exists: bool,
    #[serde(default)]
    pub pending_tool_permissions: Vec<Value>,
    pub permission_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    pub is_starred: bool,
    #[serde(default)]
    pub buffered_messages: Vec<Value>,
    pub session_type: String,
    pub host_loop_mode: bool,
}

fn default_perm_mode() -> String {
    "default".to_string()
}

/// LocalSessions (Claude Code Desktop) session record matching `oy(e)` validator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCodeSession {
    pub session_id: String,
    pub cwd: String,
    pub origin_cwd: String,
    pub is_running: bool,
    pub turn_running: bool,
    pub model: String,
    pub effort: String,
    pub created_at: u64,
    pub last_activity_at: u64,
    pub is_archived: bool,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default = "default_perm_mode")]
    pub permission_mode: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// CoworkSpace record matching `CoworkSpaces` in `index.chunk-CZuIlEEC.js`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoworkSpace {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub instructions: String,
    pub icon: String,
    pub color: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub folders: Vec<CoworkSpaceFolder>,
    pub links: Vec<CoworkSpaceLink>,
    pub projects: Vec<CoworkSpaceProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoworkSpaceFolder {
    pub path: String,
    pub name: String,
    pub added_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoworkSpaceLink {
    pub id: String,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoworkSpaceProject {
    pub uuid: String,
    pub name: String,
}

/// VSOCK JSON-RPC protocol for `quixi-guest-daemon` (`sdk-daemon` on port 51234)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmJsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmJsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eipc_channels() {
        let ch1 = format!(
            "$eipc_message$_{}_$_claude.web_$_LocalSessions_$_getAll",
            EIPC_UUID_MAIN
        );
        let p1 = ParsedEipcChannel::parse(&ch1).unwrap();
        assert_eq!(p1.namespace, "claude.web");
        assert_eq!(p1.service, "LocalSessions");
        assert_eq!(p1.member, "getAll");
        assert_eq!(p1.kind, EipcMethodKind::Call);

        let ch2 = format!(
            "$eipc_message$_{}_$_claude.web_$_ManagedConfig_$_managedRendererConfig_$store$_getStateSync",
            EIPC_UUID_MAIN
        );
        let p2 = ParsedEipcChannel::parse(&ch2).unwrap();
        assert_eq!(p2.kind, EipcMethodKind::StoreGetStateSync);
        assert_eq!(p2.store_key(), "ManagedConfig_$_managedRendererConfig");
    }
}
