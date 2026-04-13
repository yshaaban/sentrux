use super::agent_format::{
    actions_from_issues, compare_agent_issues, to_agent_issue, AgentIssue, IssueConfidence,
    IssueOrigin, IssueSource,
};
use super::test_support::{
    append_session_clone_watchpoint_note, commit_all, init_git_repo, temp_root,
    write_contract_propagation_fixture_files, write_file, write_session_clone_duplicate,
    write_session_clone_fixture_files, write_session_clone_followthrough_fixture_files,
    write_session_clone_followthrough_source_drift,
};
use super::{fresh_mcp_state, handle_check, handle_scan, handle_session_start};
use crate::analysis::project_shape::{ModuleContractSuggestion, ProjectShapeReport};
use crate::license::Tier;
use crate::metrics::v2::FindingSeverity;
use serde_json::json;
use std::fs;
#[test]
fn check_returns_partial_when_changed_scope_is_unavailable() {
    let root = temp_root("check-unavailable-scope");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert_eq!(response["issues"], json!([]));
    assert_eq!(response["actions"], json!([]));
    assert_eq!(
        response["diagnostics"]["availability"]["changed_scope"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["semantic"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["rules"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["evolution"],
        json!(false)
    );
    assert_eq!(response["diagnostics"]["partial_results"], json!(true));
}

#[test]
fn check_returns_clean_result_when_changed_scope_is_known_empty() {
    let root = temp_root("check-empty-scope");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert_eq!(response["issues"], json!([]));
    assert_eq!(response["actions"], json!([]));
    assert_eq!(response["changed_files"], json!([]));
    assert_eq!(response["gate"], json!("pass"));
    assert_eq!(
        response["diagnostics"]["availability"]["changed_scope"],
        json!(true)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["semantic"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["rules"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["evolution"],
        json!(false)
    );
    assert_eq!(response["diagnostics"]["partial_results"], json!(false));
}

#[test]
fn check_warns_when_missing_test_signal_is_skipped_without_session_baseline() {
    let root = temp_root("check-missing-test-baseline-warning");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/task-health-monitor.ts",
        "export function monitorTaskHealth(): number { return 1; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let warnings = response["diagnostics"]["warnings"]
        .as_array()
        .expect("warnings array");

    assert!(warnings.iter().any(|warning| {
        warning
            .as_str()
            .is_some_and(|warning| warning.contains("missing test coverage checks skipped"))
    }));
    assert!(response["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .all(|issue| issue["kind"] != "missing_test_coverage"));
}

#[test]
fn check_applies_suppressions_to_inferred_boundary_findings() {
    let root = temp_root("check-boundary-suppression");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
            [[suppress]]
            kind = "zero_config_boundary_violation"
            file = "src/app/feature.ts"
            reason = "approved exception"
        "#,
    );
    write_file(
        &root,
        "src/module/index.ts",
        "export { value } from './internal';\n",
    );
    write_file(&root, "src/module/internal.ts", "export const value = 1;\n");
    write_file(
        &root,
        "src/app/feature.ts",
        "import { value } from '../module/internal';\nvoid value;\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/app/feature.ts",
        "import { value } from '../module/internal';\nexport const feature = value + 1;\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    state.cached_project_shape = Some(ProjectShapeReport {
        module_contracts: vec![ModuleContractSuggestion {
            id: "module_api".to_string(),
            root: "src/module".to_string(),
            public_api: vec!["src/module/index.ts".to_string()],
            nested_public_api: Vec::new(),
            confidence: "high".to_string(),
            evidence: vec!["detected boundary root".to_string()],
        }],
        ..ProjectShapeReport::default()
    });
    state.cached_project_shape_identity = state.cached_scan_identity.clone();

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert!(response["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .all(|issue| issue["kind"] != "zero_config_boundary_violation"));
}

#[test]
fn actions_prioritize_explicit_rule_breaks_over_structural_watchpoints() {
    let mut issues = vec![
        AgentIssue {
            scope: "SidebarTaskRow".to_string(),
            file: "src/components/SidebarTaskRow.tsx".to_string(),
            line: Some(12),
            kind: "dependency_sprawl".to_string(),
            message: "SidebarTaskRow depends on too many modules.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Structural,
            origin: IssueOrigin::Explicit,
            confidence: IssueConfidence::High,
        },
        AgentIssue {
            scope: "task_presentation_status".to_string(),
            file: "src/components/SidebarTaskRow.tsx".to_string(),
            line: Some(18),
            kind: "forbidden_raw_read".to_string(),
            message: "Task presentation status is read directly.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Rules,
            origin: IssueOrigin::Explicit,
            confidence: IssueConfidence::High,
        },
    ];
    issues.sort_by(compare_agent_issues);
    let actions = actions_from_issues(&issues, 4);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].kind, "forbidden_raw_read");
    assert_eq!(actions[1].kind, "dependency_sprawl");
}

#[test]
fn structural_actions_prioritize_sprawl_over_large_file() {
    let mut issues = vec![
        AgentIssue {
            scope: "src/app.ts".to_string(),
            file: "src/app.ts".to_string(),
            line: None,
            kind: "large_file".to_string(),
            message: "src/app.ts grew to 900 lines.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Structural,
            origin: IssueOrigin::ZeroConfig,
            confidence: IssueConfidence::Medium,
        },
        AgentIssue {
            scope: "src/app.ts".to_string(),
            file: "src/app.ts".to_string(),
            line: None,
            kind: "dependency_sprawl".to_string(),
            message: "src/app.ts fans out across too many dependencies.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Structural,
            origin: IssueOrigin::ZeroConfig,
            confidence: IssueConfidence::Medium,
        },
    ];
    issues.sort_by(compare_agent_issues);

    assert_eq!(issues[0].kind, "dependency_sprawl");
}

#[test]
fn forbidden_raw_read_actions_name_the_preferred_accessor_when_available() {
    let primary_accessor = "src/app/task-presentation-status.ts::getTaskDotStatus";
    let secondary_accessor = "src/app/task-presentation-status.ts::getTaskDotStatusLabel";
    let canonical_owner = "src/store/core.ts::store.taskGitStatus";
    let issue = to_agent_issue(&json!({
        "kind": "forbidden_raw_read",
        "concept_id": "task_presentation_status",
        "summary": "Concept 'task_presentation_status' is read from a forbidden raw access path at src/components/SidebarTaskRow.tsx",
        "files": ["src/components/SidebarTaskRow.tsx"],
        "evidence": [
            "src/components/SidebarTaskRow.tsx::store.taskGitStatus",
            format!("canonical owner: {canonical_owner}"),
            format!("preferred accessor: {primary_accessor}"),
            format!("preferred accessor: {secondary_accessor}")
        ]
    }));

    assert_eq!(
        issue.fix_hint.as_deref(),
        Some(
            "Replace the raw read with src/app/task-presentation-status.ts::getTaskDotStatus from src/store/core.ts::store.taskGitStatus instead of recreating the projection in the caller."
        )
    );
}

#[test]
fn check_surfaces_session_introduced_clone_actions() {
    let root = temp_root("check-session-introduced-clone");
    write_session_clone_fixture_files(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
    write_session_clone_duplicate(&root);
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("rescan fixture");
    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let issues = response["issues"].as_array().expect("issues array");
    let actions = response["actions"].as_array().expect("actions array");

    assert!(issues
        .iter()
        .any(|issue| issue["kind"] == "session_introduced_clone"));
    assert_eq!(actions[0]["kind"], json!("session_introduced_clone"));
    assert!(
        actions[0]["fix_hint"].as_str().is_some_and(|hint| {
            hint.contains("src/copy.ts::") && hint.contains("src/source.ts::")
        }),
        "unexpected check response: {response}"
    );
}

#[test]
fn session_introduced_clone_actions_rank_above_structural_watchpoints() {
    let mut issues = vec![
        AgentIssue {
            scope: "src/app.ts".to_string(),
            file: "src/app.ts".to_string(),
            line: None,
            kind: "large_file".to_string(),
            message: "src/app.ts grew to 900 lines.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Structural,
            origin: IssueOrigin::ZeroConfig,
            confidence: IssueConfidence::Medium,
        },
        AgentIssue {
            scope: "src/source.ts and src/copy.ts".to_string(),
            file: "src/copy.ts".to_string(),
            line: None,
            kind: "session_introduced_clone".to_string(),
            message: "Patch introduced a duplicate helper.".to_string(),
            severity: FindingSeverity::Medium,
            fix_hint: None,
            evidence: Vec::new(),
            source: IssueSource::Clone,
            origin: IssueOrigin::Explicit,
            confidence: IssueConfidence::High,
        },
    ];
    issues.sort_by(compare_agent_issues);

    assert_eq!(issues[0].kind, "session_introduced_clone");
}

#[test]
fn check_surfaces_clone_propagation_drift_actions() {
    let root = temp_root("check-clone-propagation-drift");
    write_session_clone_followthrough_fixture_files(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
    write_session_clone_followthrough_source_drift(&root);
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("rescan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let actions = response["actions"].as_array().expect("actions array");
    let drift_action = actions
        .iter()
        .find(|action| action["kind"] == "clone_propagation_drift")
        .expect("clone propagation drift action");

    assert_eq!(actions[0]["kind"], json!("clone_propagation_drift"));
    assert!(drift_action["fix_hint"]
        .as_str()
        .is_some_and(|hint| hint.contains("src/copy.ts::buildStatusBadge")
            && hint.contains("src/source.ts::buildStatusBadge")));
}

#[test]
fn check_surfaces_touched_clone_family_watchpoint() {
    let root = temp_root("check-touched-clone-family");
    write_session_clone_fixture_files(&root);
    write_session_clone_duplicate(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
    append_session_clone_watchpoint_note(&root);
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("rescan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let actions = response["actions"].as_array().expect("actions array");
    let family_action = actions
        .iter()
        .find(|action| action["kind"] == "touched_clone_family")
        .expect("touched clone family action");

    assert!(family_action["fix_hint"]
        .as_str()
        .is_some_and(|hint| hint.contains("sibling clone")));
}

#[test]
fn check_surfaces_incomplete_propagation_for_contract_updates() {
    let root = temp_root("check-incomplete-propagation");
    write_contract_propagation_fixture_files(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/domain/server-state-bootstrap.ts",
        "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task', 'project'];\nexport type ServerStateBootstrapPayloadMap = { task: { id: string }, project: { id: string } };\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let actions = response["actions"].as_array().expect("actions array");

    assert!(
        actions
            .iter()
            .any(|action| action["kind"] == "incomplete_propagation"),
        "unexpected check response: {response}"
    );
    assert!(actions
        .iter()
        .find(|action| action["kind"] == "incomplete_propagation")
        .and_then(|action| action["fix_hint"].as_str())
        .is_some_and(|hint| hint.contains("remaining sibling surfaces")));
}

#[test]
fn raw_contract_surface_findings_map_to_incomplete_propagation_actions() {
    let issue = to_agent_issue(&json!({
        "kind": "contract_surface_completeness",
        "summary": "Related contract surfaces are no longer aligned.",
        "files": ["src/domain/server-state-bootstrap.ts"],
        "evidence": ["missing required site: src/app/server-state-bootstrap-registry.ts"]
    }));

    assert_eq!(issue.kind, "incomplete_propagation");
    assert_eq!(issue.source, IssueSource::Obligation);
    assert!(issue
        .fix_hint
        .as_deref()
        .is_some_and(|hint| hint.contains("remaining sibling surfaces")));
}

#[test]
fn zero_config_boundary_findings_default_to_medium_confidence() {
    let issue = to_agent_issue(&json!({
        "kind": "zero_config_boundary_violation",
        "summary": "Feature module bypasses inferred public API.",
        "files": ["src/app/feature.ts"],
        "evidence": ["inferred boundary confidence: high"]
    }));

    assert_eq!(issue.origin, IssueOrigin::ZeroConfig);
    assert_eq!(issue.confidence, IssueConfidence::Medium);
}

#[test]
fn check_records_repo_local_session_events() {
    let root = temp_root("check-session-telemetry");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 2; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
    handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    let event_log = root.join(".sentrux").join("agent-session-events.jsonl");
    let source = fs::read_to_string(event_log).expect("read event log");

    assert!(source.contains("\"event_type\":\"session_started\""));
    assert!(source.contains("\"event_type\":\"check_run\""));
    assert!(source.contains("\"session_mode\":\"explicit\""));
}
