use super::handlers::{
    AgentAction, AgentGate, AgentIssue, CheckDiagnostics, CheckSignalSummary, SessionSignalSummary,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AGENT_SESSION_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct AgentSessionState {
    pub(crate) server_run_id: String,
    pub(crate) active_session_run_id: Option<String>,
    next_event_index: u64,
}

impl AgentSessionState {
    pub(crate) fn new() -> Self {
        Self {
            server_run_id: run_id("mcp"),
            active_session_run_id: None,
            next_event_index: 0,
        }
    }

    fn next_event_index(&mut self) -> u64 {
        self.next_event_index += 1;
        self.next_event_index
    }

    pub(crate) fn start_session(&mut self) -> String {
        let session_run_id = run_id("session");
        self.active_session_run_id = Some(session_run_id.clone());
        session_run_id
    }

    fn end_session(&mut self) {
        self.active_session_run_id = None;
    }

    fn session_context(&self) -> (String, &'static str) {
        match &self.active_session_run_id {
            Some(session_run_id) => (session_run_id.clone(), "explicit"),
            None => (self.server_run_id.clone(), "implicit"),
        }
    }
}

pub(crate) struct CheckRunTelemetry<'a> {
    pub(crate) changed_files: &'a BTreeSet<String>,
    pub(crate) gate: AgentGate,
    pub(crate) actions: &'a [AgentAction],
    pub(crate) issues: &'a [AgentIssue],
    pub(crate) diagnostics: &'a CheckDiagnostics,
    pub(crate) signal_summary: CheckSignalSummary,
    pub(crate) session_baseline_available: bool,
    pub(crate) reused_cached_scan: bool,
    pub(crate) elapsed_ms: u64,
}

pub(crate) struct SessionEndTelemetry<'a> {
    pub(crate) changed_files: &'a BTreeSet<String>,
    pub(crate) decision: &'a str,
    pub(crate) action_payloads: &'a [AgentAction],
    pub(crate) introduced_finding_kinds: Vec<String>,
    pub(crate) missing_obligation_count: usize,
    pub(crate) introduced_clone_finding_count: usize,
    pub(crate) signal_summary: SessionSignalSummary,
    pub(crate) reused_cached_scan: bool,
}

pub(crate) fn record_session_started(
    state: &mut super::McpState,
    root: &Path,
    quality_signal_0_10000: u32,
    session_finding_count: usize,
    baseline_path: &Path,
) {
    state.agent_session.start_session();
    let mut event = base_event(state, root, "session_started");
    event.insert(
        "quality_signal_0_10000".to_string(),
        json!(quality_signal_0_10000),
    );
    event.insert(
        "session_finding_count".to_string(),
        json!(session_finding_count),
    );
    event.insert(
        "baseline_path".to_string(),
        json!(baseline_path.to_string_lossy().to_string()),
    );
    write_event(root, Value::Object(event));
}

pub(crate) fn record_check_run(
    state: &mut super::McpState,
    root: &Path,
    telemetry: CheckRunTelemetry<'_>,
) {
    let mut event = base_event(state, root, "check_run");
    insert_changed_file_fields(&mut event, telemetry.changed_files);
    insert_action_fields(&mut event, telemetry.actions);
    event.insert("gate".to_string(), json!(telemetry.gate));
    event.insert("issue_count".to_string(), json!(telemetry.issues.len()));
    event.insert(
        "signal_summary".to_string(),
        json!(telemetry.signal_summary),
    );
    event.insert(
        "partial_results".to_string(),
        json!(telemetry.diagnostics.partial_results),
    );
    event.insert(
        "availability".to_string(),
        serde_json::to_value(&telemetry.diagnostics.availability).unwrap_or_else(|_| json!({})),
    );
    event.insert(
        "warning_count".to_string(),
        json!(telemetry.diagnostics.warnings.len()),
    );
    event.insert(
        "session_baseline_available".to_string(),
        json!(telemetry.session_baseline_available),
    );
    event.insert(
        "reused_cached_scan".to_string(),
        json!(telemetry.reused_cached_scan),
    );
    event.insert("elapsed_ms".to_string(), json!(telemetry.elapsed_ms));
    write_event(root, Value::Object(event));
}

pub(crate) fn record_session_ended(
    state: &mut super::McpState,
    root: &Path,
    telemetry: SessionEndTelemetry<'_>,
) {
    let mut event = base_event(state, root, "session_ended");
    insert_changed_file_fields(&mut event, telemetry.changed_files);
    insert_action_fields(&mut event, telemetry.action_payloads);
    event.insert("decision".to_string(), json!(telemetry.decision));
    event.insert(
        "introduced_finding_kinds".to_string(),
        json!(telemetry.introduced_finding_kinds),
    );
    event.insert(
        "missing_obligation_count".to_string(),
        json!(telemetry.missing_obligation_count),
    );
    event.insert(
        "introduced_clone_finding_count".to_string(),
        json!(telemetry.introduced_clone_finding_count),
    );
    event.insert(
        "signal_summary".to_string(),
        json!(telemetry.signal_summary),
    );
    event.insert(
        "reused_cached_scan".to_string(),
        json!(telemetry.reused_cached_scan),
    );
    write_event(root, Value::Object(event));
    state.agent_session.end_session();
}

fn base_event(
    state: &mut super::McpState,
    root: &Path,
    event_type: &'static str,
) -> Map<String, Value> {
    let event_index = state.agent_session.next_event_index();
    let (session_run_id, session_mode) = state.agent_session.session_context();
    Map::from_iter([
        (
            "schema_version".to_string(),
            json!(AGENT_SESSION_EVENT_SCHEMA_VERSION),
        ),
        ("event_type".to_string(), json!(event_type)),
        ("event_index".to_string(), json!(event_index)),
        (
            "recorded_at_unix_ms".to_string(),
            json!(unix_timestamp_millis()),
        ),
        (
            "repo_root".to_string(),
            json!(root.to_string_lossy().to_string()),
        ),
        (
            "server_run_id".to_string(),
            json!(state.agent_session.server_run_id.clone()),
        ),
        ("session_run_id".to_string(), json!(session_run_id)),
        ("session_mode".to_string(), json!(session_mode)),
    ])
}

fn insert_changed_file_fields(event: &mut Map<String, Value>, changed_files: &BTreeSet<String>) {
    let changed_files = changed_files.iter().cloned().collect::<Vec<_>>();
    event.insert("changed_files".to_string(), json!(changed_files));
    event.insert("changed_file_count".to_string(), json!(changed_files.len()));
}

fn insert_action_fields(event: &mut Map<String, Value>, actions: &[AgentAction]) {
    let action_kinds = actions
        .iter()
        .map(|action| action.kind.clone())
        .collect::<Vec<_>>();
    let blocking_action_kinds = actions
        .iter()
        .filter(|action| action.blocking)
        .map(|action| action.kind.clone())
        .collect::<Vec<_>>();

    event.insert("action_count".to_string(), json!(actions.len()));
    event.insert("action_kinds".to_string(), json!(action_kinds));
    event.insert(
        "blocking_action_kinds".to_string(),
        json!(blocking_action_kinds),
    );
    event.insert(
        "top_action_kind".to_string(),
        json!(actions.first().map(|action| action.kind.clone())),
    );
    event.insert(
        "top_action_file".to_string(),
        json!(actions.first().map(|action| action.file.clone())),
    );
}

fn write_event(root: &Path, event: Value) {
    let Ok(serialized) = serde_json::to_string(&event) else {
        return;
    };
    let path = agent_session_events_path(root);
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{serialized}");
}

fn agent_session_events_path(root: &Path) -> PathBuf {
    root.join(".sentrux").join("agent-session-events.jsonl")
}

fn run_id(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        unix_timestamp_millis()
    )
}

fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
