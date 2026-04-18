use super::agent_format::{
    actions_from_issues, compare_agent_issues, obligation_value_to_agent_issue, to_agent_issue,
    AgentIssue, AgentIssueEvidence, IssueConfidence, IssueOrigin, IssueSource, RepairPacket,
};
use super::check::obligation_issue_value;
use super::test_support::{
    append_session_clone_watchpoint_note, commit_all, init_git_repo, temp_root,
    write_contract_propagation_fixture_files, write_file, write_session_clone_duplicate,
    write_session_clone_fixture_files, write_session_clone_followthrough_fixture_files,
    write_session_clone_followthrough_source_drift,
};
use super::{fresh_mcp_state, handle_check, handle_scan, handle_session_start};
use crate::analysis::project_shape::{ModuleContractSuggestion, ProjectShapeReport};
use crate::license::Tier;
use crate::metrics::v2::{
    FindingSeverity, ObligationConfidence, ObligationOrigin, ObligationReport, ObligationSite,
    ObligationTrustTier,
};
use serde_json::{json, Value};
use std::fs;

fn test_issue(
    scope: &str,
    file: &str,
    line: Option<u32>,
    kind: &str,
    message: &str,
    severity: FindingSeverity,
    source: IssueSource,
    origin: IssueOrigin,
    confidence: IssueConfidence,
) -> AgentIssue {
    AgentIssue {
        scope: scope.to_string(),
        concept_id: None,
        file: file.to_string(),
        line,
        kind: kind.to_string(),
        message: message.to_string(),
        severity,
        trust_tier: match confidence {
            IssueConfidence::High => "trusted",
            IssueConfidence::Medium => "watchpoint",
            IssueConfidence::Experimental => "experimental",
        }
        .to_string(),
        presentation_class: "structural_debt".to_string(),
        leverage_class: "secondary_cleanup".to_string(),
        score_0_10000: 6_000,
        fix_hint: None,
        evidence: Vec::new(),
        source,
        origin,
        confidence,
        evidence_metrics: AgentIssueEvidence::default(),
        repair_packet: RepairPacket {
            risk_statement: "test packet".to_string(),
            likely_fix_sites: vec![file.to_string()],
            inspection_context: vec![file.to_string()],
            smallest_safe_first_cut: Some("test first cut".to_string()),
            verify_after: vec!["re-run sentrux check".to_string()],
            do_not_touch_yet: Vec::new(),
            completeness_0_10000: 9_000,
            complete: true,
            required_fields: super::agent_guidance::RepairPacketRequiredFields {
                risk_statement: true,
                repair_surface: true,
                first_cut: true,
                verification: true,
            },
            missing_fields: Vec::new(),
        },
    }
}
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
        test_issue(
            "SidebarTaskRow",
            "src/components/SidebarTaskRow.tsx",
            Some(12),
            "dependency_sprawl",
            "SidebarTaskRow depends on too many modules.",
            FindingSeverity::Medium,
            IssueSource::Structural,
            IssueOrigin::Explicit,
            IssueConfidence::High,
        ),
        test_issue(
            "task_presentation_status",
            "src/components/SidebarTaskRow.tsx",
            Some(18),
            "forbidden_raw_read",
            "Task presentation status is read directly.",
            FindingSeverity::Medium,
            IssueSource::Rules,
            IssueOrigin::Explicit,
            IssueConfidence::High,
        ),
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
        test_issue(
            "src/app.ts",
            "src/app.ts",
            None,
            "large_file",
            "src/app.ts grew to 900 lines.",
            FindingSeverity::Medium,
            IssueSource::Structural,
            IssueOrigin::ZeroConfig,
            IssueConfidence::Medium,
        ),
        test_issue(
            "src/app.ts",
            "src/app.ts",
            None,
            "dependency_sprawl",
            "src/app.ts fans out across too many dependencies.",
            FindingSeverity::Medium,
            IssueSource::Structural,
            IssueOrigin::ZeroConfig,
            IssueConfidence::Medium,
        ),
    ];
    issues.sort_by(compare_agent_issues);

    assert_eq!(issues[0].kind, "dependency_sprawl");
}

#[test]
fn architecture_cycles_rank_above_large_file_cleanup() {
    let mut large_file_issue = test_issue(
        "src/lib/ipc.ts",
        "src/lib/ipc.ts",
        None,
        "large_file",
        "src/lib/ipc.ts is oversized.",
        FindingSeverity::Medium,
        IssueSource::Structural,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    large_file_issue.trust_tier = "trusted".to_string();
    large_file_issue.presentation_class = "guarded_facade".to_string();
    large_file_issue.leverage_class = "boundary_discipline".to_string();

    let mut cycle_issue = test_issue(
        "cycle:src/mcp/index.ts|src/mcp/server.ts",
        "src/mcp/index.ts",
        None,
        "cycle_cluster",
        "src/mcp/index.ts and src/mcp/server.ts form a cycle.",
        FindingSeverity::High,
        IssueSource::Structural,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    cycle_issue.trust_tier = "watchpoint".to_string();
    cycle_issue.presentation_class = "watchpoint".to_string();
    cycle_issue.leverage_class = "architecture_signal".to_string();
    cycle_issue.score_0_10000 = 10_000;

    let mut issues = vec![large_file_issue, cycle_issue];
    issues.sort_by(compare_agent_issues);

    assert_eq!(issues[0].kind, "cycle_cluster");
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
        test_issue(
            "src/app.ts",
            "src/app.ts",
            None,
            "large_file",
            "src/app.ts grew to 900 lines.",
            FindingSeverity::Medium,
            IssueSource::Structural,
            IssueOrigin::ZeroConfig,
            IssueConfidence::Medium,
        ),
        test_issue(
            "src/source.ts and src/copy.ts",
            "src/copy.ts",
            None,
            "session_introduced_clone",
            "Patch introduced a duplicate helper.",
            FindingSeverity::Medium,
            IssueSource::Clone,
            IssueOrigin::Explicit,
            IssueConfidence::High,
        ),
    ];
    issues.sort_by(compare_agent_issues);

    assert_eq!(issues[0].kind, "session_introduced_clone");
}

#[test]
fn evidence_backed_actions_break_ties_inside_the_same_signal_family() {
    let mut better_issue = test_issue(
        "task_status_projection",
        "src/app/status.ts",
        Some(10),
        "forbidden_raw_read",
        "Task status is read directly.",
        FindingSeverity::Medium,
        IssueSource::Rules,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    better_issue.evidence_metrics = AgentIssueEvidence {
        signal_treatment_ready: Some(true),
        top_action_help_rate: Some(0.9),
        top_action_follow_rate: Some(0.8),
        reviewer_acceptance_rate: Some(0.85),
        remediation_success_rate: Some(0.75),
        task_success_rate: Some(0.8),
        intervention_net_value_score: Some(0.7),
        reviewer_disagreement_rate: Some(0.05),
        patch_expansion_rate: Some(0.1),
        intervention_cost_checks_mean: Some(1.0),
        ..AgentIssueEvidence::default()
    };

    let mut weaker_issue = test_issue(
        "task_status_projection",
        "src/app/status.ts",
        Some(10),
        "forbidden_raw_read",
        "Task status is read directly.",
        FindingSeverity::Medium,
        IssueSource::Rules,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    weaker_issue.evidence_metrics = AgentIssueEvidence {
        top_action_help_rate: Some(0.2),
        top_action_follow_rate: Some(0.2),
        reviewer_acceptance_rate: Some(0.4),
        remediation_success_rate: Some(0.2),
        task_success_rate: Some(0.3),
        intervention_net_value_score: Some(0.1),
        reviewer_disagreement_rate: Some(0.4),
        patch_expansion_rate: Some(0.7),
        intervention_cost_checks_mean: Some(4.0),
        ..AgentIssueEvidence::default()
    };

    let mut issues = vec![weaker_issue, better_issue];
    issues.sort_by(compare_agent_issues);

    assert_eq!(
        issues[0].evidence_metrics.signal_treatment_ready,
        Some(true)
    );
    assert!(actions_from_issues(&issues, 1)[0]
        .why_now
        .iter()
        .any(|reason| reason == "helped_prior_sessions"));
}

#[test]
fn patch_worsened_structural_pressure_ranks_above_equivalent_non_worsened_pressure() {
    let mut patch_worsened_issue = test_issue(
        "src/app.ts",
        "src/app.ts",
        None,
        "dependency_sprawl",
        "src/app.ts fans out across too many dependencies.",
        FindingSeverity::Medium,
        IssueSource::Structural,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    patch_worsened_issue.trust_tier = "trusted".to_string();
    patch_worsened_issue.leverage_class = "architecture_signal".to_string();
    patch_worsened_issue.presentation_class = "structural_debt".to_string();
    patch_worsened_issue
        .evidence_metrics
        .patch_directly_worsened = Some(true);

    let mut inherited_issue = test_issue(
        "src/app.ts",
        "src/app.ts",
        None,
        "dependency_sprawl",
        "src/app.ts fans out across too many dependencies.",
        FindingSeverity::Medium,
        IssueSource::Structural,
        IssueOrigin::Explicit,
        IssueConfidence::High,
    );
    inherited_issue.trust_tier = "trusted".to_string();
    inherited_issue.leverage_class = "architecture_signal".to_string();
    inherited_issue.presentation_class = "structural_debt".to_string();

    let mut issues = vec![inherited_issue, patch_worsened_issue];
    issues.sort_by(compare_agent_issues);

    assert_eq!(
        issues[0].evidence_metrics.patch_directly_worsened,
        Some(true)
    );
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
    assert_eq!(
        response["signal_summary"]["clone_propagation_drift_issue_count"],
        json!(1)
    );
    assert_eq!(
        response["signal_summary"]["action_quality"]["top_action_source"],
        json!("clone")
    );
    assert_eq!(
        response["signal_summary"]["action_quality"]["top_action_complete"],
        json!(true)
    );
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
    assert!(
        response["signal_summary"]["propagation_issue_count"]
            .as_u64()
            .is_some_and(|count| count >= 1),
        "unexpected signal summary: {}",
        response["signal_summary"]
    );
    assert_eq!(
        response["signal_summary"]["action_quality"]["top_action_source"],
        json!("obligation")
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
fn check_preserves_closed_domain_obligation_metadata_without_fabricating_concept_ids() {
    let value = obligation_issue_value(&ObligationReport {
        id: "closed-domain-task-presentation-status".to_string(),
        kind: "closed_domain_exhaustiveness".to_string(),
        concept_id: None,
        domain_symbol_name: Some("TaskPresentationStatus".to_string()),
        origin: ObligationOrigin::ZeroConfig,
        trust_tier: ObligationTrustTier::Watchpoint,
        confidence: ObligationConfidence::Medium,
        severity: FindingSeverity::High,
        score_0_10000: 8_700,
        summary: "Domain 'TaskPresentationStatus' still needs exhaustive handling.".to_string(),
        files: vec!["src/app/task-presentation-status.ts".to_string()],
        required_sites: Vec::new(),
        satisfied_sites: Vec::new(),
        missing_sites: vec![ObligationSite {
            path: "src/app/task-presentation-status.ts".to_string(),
            kind: "closed_domain".to_string(),
            line: Some(27),
            detail: "missing exhaustive branch".to_string(),
        }],
        missing_variants: vec!["loading".to_string(), "ready".to_string()],
        context_burden: 1,
    });
    let issue = obligation_value_to_agent_issue(&value);

    assert_eq!(value["concept_id"], Value::Null);
    assert_eq!(value["domain_symbol_name"], json!("TaskPresentationStatus"));
    assert_eq!(value["missing_variants"], json!(["loading", "ready"]));
    assert_eq!(issue.scope, "TaskPresentationStatus");
    assert_eq!(issue.concept_id, None);
    assert!(issue.message.contains("loading"));
    assert!(issue.message.contains("ready"));
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

#[test]
fn check_records_signal_summary_in_repo_local_session_events() {
    let root = temp_root("check-session-signal-summary");
    write_contract_propagation_fixture_files(&root);
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
    write_file(
        &root,
        "src/domain/server-state-bootstrap.ts",
        "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task', 'project'];\nexport type ServerStateBootstrapPayloadMap = { task: { id: string }, project: { id: string } };\n",
    );
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("rescan fixture");
    handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    let event_log = root.join(".sentrux").join("agent-session-events.jsonl");
    let last_event = fs::read_to_string(event_log)
        .expect("read event log")
        .lines()
        .last()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse event"))
        .expect("check run event");

    assert_eq!(last_event["event_type"], json!("check_run"));
    assert!(last_event["signal_summary"]["propagation_issue_count"]
        .as_u64()
        .is_some_and(|count| count >= 1));
    assert_eq!(
        last_event["signal_summary"]["action_quality"]["top_action_source"],
        json!("obligation")
    );
}
