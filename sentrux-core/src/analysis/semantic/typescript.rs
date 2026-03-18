//! TypeScript bridge configuration and request/response protocol.

use super::types::ProjectModel;
use crate::analysis::scanner::common::normalize_path;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const TS_BRIDGE_PROTOCOL_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeScriptBridgeConfig {
    pub node_binary: String,
    pub package_dir: String,
    pub entrypoint: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeScriptBridgeRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeScriptBridgeResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<TypeScriptBridgeError>,
}

#[derive(Debug, Clone)]
pub struct TypeScriptBridgeCommand {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: String,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeScriptBridgeError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInitializeParams {
    pub protocol_version: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCapabilities {
    pub semantic_analysis: bool,
    pub incremental_updates: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BridgeServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInitializeResult {
    pub protocol_version: String,
    pub capabilities: BridgeCapabilities,
    pub server_info: BridgeServerInfo,
}

pub fn default_bridge_config() -> TypeScriptBridgeConfig {
    let package_dir = repository_root().join("ts-bridge");
    let entrypoint = package_dir.join("dist").join("index.js");

    TypeScriptBridgeConfig {
        node_binary: "node".to_string(),
        package_dir: normalize_path(package_dir.to_string_lossy()),
        entrypoint: normalize_path(entrypoint.to_string_lossy()),
    }
}

pub fn bridge_command(config: &TypeScriptBridgeConfig) -> TypeScriptBridgeCommand {
    let mut env = BTreeMap::new();
    env.insert(
        "SENTRUX_TS_BRIDGE_PACKAGE_DIR".to_string(),
        config.package_dir.clone(),
    );

    TypeScriptBridgeCommand {
        program: config.node_binary.clone(),
        args: vec![config.entrypoint.clone()],
        current_dir: config.package_dir.clone(),
        env,
    }
}

pub fn initialize_request(id: u64) -> TypeScriptBridgeRequest {
    TypeScriptBridgeRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "initialize".to_string(),
        params: serde_json::to_value(BridgeInitializeParams {
            protocol_version: TS_BRIDGE_PROTOCOL_VERSION.to_string(),
        })
        .unwrap_or_else(|_| serde_json::json!({})),
    }
}

pub fn ping_request(id: u64) -> TypeScriptBridgeRequest {
    TypeScriptBridgeRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "ping".to_string(),
        params: serde_json::json!({}),
    }
}

pub fn shutdown_request(id: u64) -> TypeScriptBridgeRequest {
    TypeScriptBridgeRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "shutdown".to_string(),
        params: serde_json::json!({}),
    }
}

pub fn analyze_project_request(id: u64, project: &ProjectModel) -> TypeScriptBridgeRequest {
    TypeScriptBridgeRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: "analyze_projects".to_string(),
        params: serde_json::to_value(project).unwrap_or_else(|_| serde_json::json!({})),
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{
        bridge_command, default_bridge_config, initialize_request, BridgeInitializeParams,
        TS_BRIDGE_PROTOCOL_VERSION,
    };

    #[test]
    fn bridge_command_points_at_repo_root_package() {
        let config = default_bridge_config();
        let command = bridge_command(&config);

        assert_eq!(command.program, "node");
        assert!(command.current_dir.ends_with("/ts-bridge"));
        assert!(command.args[0].ends_with("/ts-bridge/dist/index.js"));
    }

    #[test]
    fn initialize_request_uses_current_protocol_version() {
        let request = initialize_request(7);
        let params: BridgeInitializeParams =
            serde_json::from_value(request.params).expect("initialize params");

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "initialize");
        assert_eq!(params.protocol_version, TS_BRIDGE_PROTOCOL_VERSION);
    }
}
