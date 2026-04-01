//! MCP (Model Context Protocol) server — stdio transport.
//!
//! Implements the minimal MCP spec: initialize, tools/list, tools/call.
//! Runs as `sentrux --mcp` — reads JSON-RPC from stdin, writes to stdout.
//! All analysis runs locally. Zero network calls.
//!
//! Architecture:
//! - `registry.rs`: ToolDef + ToolRegistry (dispatch, license gating, tool listing)
//! - `handlers.rs`: Core tool handlers + definitions (scan, health, gate, etc.)
//! - `handlers_evo.rs`: Evolution/analysis tool handlers + definitions
//! - `tools.rs`: build_registry() — single registration point

mod agent_brief;
pub mod handlers;
pub mod handlers_evo;
pub mod registry;
mod response;
mod semantic_cache;

use crate::analysis::scanner::common::ScanMetadata;
use crate::analysis::semantic::SemanticSnapshot;
use crate::app::bridge::TypeScriptBridgeSupervisor;
use crate::core::snapshot::Snapshot;
use crate::license::{self, Tier};
use crate::metrics;
use crate::metrics::arch;
use crate::metrics::evolution;
use semantic_cache::{SemanticCacheIdentity, SemanticCacheSource};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub const SESSION_V2_SCHEMA_VERSION: u32 = 3;
const MIN_SUPPORTED_SESSION_V2_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionV2ConfidenceSnapshot {
    pub scan_confidence_0_10000: Option<u32>,
    pub rule_coverage_0_10000: Option<u32>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionV2Baseline {
    #[serde(default = "default_session_v2_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub project_fingerprint: Option<String>,
    #[serde(default)]
    pub sentrux_version: Option<String>,
    pub file_hashes: BTreeMap<String, u64>,
    pub finding_payloads: BTreeMap<String, Value>,
    pub git_head: Option<String>,
    #[serde(default)]
    pub working_tree_paths: BTreeSet<String>,
    #[serde(default)]
    pub confidence: SessionV2ConfidenceSnapshot,
}

const fn default_session_v2_schema_version() -> u32 {
    1
}

pub fn session_v2_schema_supported(schema_version: u32) -> bool {
    (MIN_SUPPORTED_SESSION_V2_SCHEMA_VERSION..=SESSION_V2_SCHEMA_VERSION).contains(&schema_version)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanCacheIdentity {
    pub git_head: Option<String>,
    pub working_tree_paths: BTreeSet<String>,
    pub working_tree_hashes: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulesCacheIdentity {
    pub rules_path: PathBuf,
    pub exists: bool,
    pub len: Option<u64>,
    pub modified_unix_nanos: Option<u128>,
}

#[derive(Debug, Clone, Default)]
pub struct PatchSafetyAnalysisCache {
    pub scan_identity: Option<ScanCacheIdentity>,
    pub session_signature: Option<u64>,
    pub visible_findings: Vec<Value>,
    pub suppression_hits: Vec<Value>,
    pub suppressed_finding_count: usize,
    pub expired_suppressions: Vec<Value>,
    pub expired_suppression_match_count: usize,
    pub changed_visible_findings: Vec<Value>,
    pub changed_obligations: Vec<crate::metrics::v2::ObligationReport>,
    pub changed_touched_concepts: BTreeSet<String>,
    pub clone_error: Option<String>,
    pub all_semantic_error: Option<String>,
    pub changed_semantic_error: Option<String>,
    pub rules_error: Option<String>,
}

/// Mutable state shared across MCP requests.
/// Handlers receive `&mut McpState` directly — no more exploded parameters.
/// Public so external crates (private-integration-crate) can access cached data.
pub struct McpState {
    pub tier: Tier,
    pub scan_root: Option<PathBuf>,
    pub cached_snapshot: Option<Arc<Snapshot>>,
    pub cached_scan_metadata: Option<ScanMetadata>,
    pub cached_semantic: Option<SemanticSnapshot>,
    pub cached_semantic_identity: Option<SemanticCacheIdentity>,
    pub cached_semantic_source: Option<SemanticCacheSource>,
    pub cached_health: Option<metrics::HealthReport>,
    pub cached_arch: Option<arch::ArchReport>,
    pub baseline: Option<arch::ArchBaseline>,
    pub session_v2: Option<SessionV2Baseline>,
    pub cached_evolution: Option<evolution::EvolutionReport>,
    pub cached_scan_identity: Option<ScanCacheIdentity>,
    pub cached_rules_identity: Option<RulesCacheIdentity>,
    pub cached_rules_config: Option<crate::metrics::rules::RulesConfig>,
    pub cached_rules_error: Option<String>,
    pub cached_patch_safety: Option<PatchSafetyAnalysisCache>,
    pub semantic_bridge: Option<TypeScriptBridgeSupervisor>,
}

/// Run the MCP server loop. Blocks until stdin is closed.
/// Accepts an optional callback to register additional tools (e.g., pro tools from private-integration-crate).
pub fn run_mcp_server(register_extra: Option<&dyn Fn(&mut registry::ToolRegistry)>) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Load license tier once at startup
    let tier = license::current_tier();

    // Build tool registry once (all schemas + handlers + tier requirements)
    let mut registry = build_registry();
    if let Some(register) = register_extra {
        register(&mut registry);
    }

    let mut state = McpState {
        tier,
        scan_root: None,
        cached_snapshot: None,
        cached_scan_metadata: None,
        cached_semantic: None,
        cached_semantic_identity: None,
        cached_semantic_source: None,
        cached_health: None,
        cached_arch: None,
        baseline: None,
        session_v2: None,
        cached_evolution: None,
        cached_scan_identity: None,
        cached_rules_identity: None,
        cached_rules_config: None,
        cached_rules_error: None,
        cached_patch_safety: None,
        semantic_bridge: None,
    };

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                let _ = writeln!(stdout, "{}", err_resp);
                let _ = stdout.flush();
                continue;
            }
        };

        match dispatch_request(&request, &registry, &mut state) {
            Some(response) => {
                if writeln!(stdout, "{}", response).is_err() || stdout.flush().is_err() {
                    eprintln!("[mcp] stdout write failed, client likely disconnected");
                    break;
                }
            }
            None => continue,
        }
    }
}

/// Dispatch a parsed JSON-RPC request. Returns None for notifications.
fn dispatch_request(
    request: &Value,
    registry: &registry::ToolRegistry,
    state: &mut McpState,
) -> Option<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => Some(handle_initialize(&id)),
        "initialized" => None,
        "tools/list" => Some(handle_tools_list(&id, registry)),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            Some(handle_tools_call(&id, &params, registry, state))
        }
        "ping" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        })),
        _ => {
            if request.get("id").is_none() {
                None
            } else {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("Unknown method: {method}") }
                }))
            }
        }
    }
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "sentrux",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list(id: &Value, registry: &registry::ToolRegistry) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": registry.definitions()
        }
    })
}

fn handle_tools_call(
    id: &Value,
    params: &Value,
    registry: &registry::ToolRegistry,
    state: &mut McpState,
) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    // Copy tier to avoid borrow conflict (&state.tier vs &mut state)
    let tier = state.tier;
    let result = registry.dispatch(tool_name, &args, &tier, state);
    format_tool_result(id, result)
}

/// Format a tool result (Ok or Err) into a JSON-RPC response.
fn format_tool_result(id: &Value, result: Result<Value, String>) -> Value {
    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": content.to_string()
                }]
            }
        }),
        Err(msg) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": msg
                }],
                "isError": true
            }
        }),
    }
}

/// Build the core tool registry with free tools registered.
/// Called once at MCP server startup. Returns a mutable registry
/// so callers (e.g., sentrux-bin with pro feature) can register additional tools.
pub fn build_registry() -> registry::ToolRegistry {
    let mut reg = registry::ToolRegistry::new();

    // Core scan/session tools
    reg.register(handlers::scan_def());
    reg.register(handlers::rescan_def());
    reg.register(handlers::session_start_def());
    reg.register(handlers::session_end_def());
    reg.register(handlers::gate_def());
    reg.register(handlers::findings_def());
    reg.register(handlers::obligations_def());
    reg.register(handlers::parity_def());
    reg.register(handlers::state_def());
    reg.register(handlers::concentration_def());
    reg.register(handlers::agent_brief_def());

    // Health — one true score + root-cause-organized diagnostics
    reg.register(handlers::health_def());

    // Rules
    reg.register(handlers::concepts_def());
    reg.register(handlers::project_shape_def());
    reg.register(handlers::explain_concept_def());
    reg.register(handlers::trace_symbol_def());
    reg.register(handlers::check_rules_def());

    // Evolution (git history analysis)
    reg.register(handlers_evo::evolution_def());

    // DSM & Test Gaps
    reg.register(handlers_evo::dsm_def());
    reg.register(handlers_evo::test_gaps_def());

    reg
}
