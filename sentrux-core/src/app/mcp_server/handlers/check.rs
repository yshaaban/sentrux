use super::agent_format::{
    actions_from_issues, compare_agent_issues, issue_blocks_gate,
    issues_from_findings_and_obligations, AgentCheckResponse, AgentGate, AgentIssue,
    CheckAvailability, CheckDiagnostics,
};
use super::semantic_batch::SemanticAnalysisBatch;
use super::*;
use crate::metrics::v2::{build_clone_drift_findings, FindingSeverity, SemanticFinding};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

const DEFAULT_AGENT_ACTION_LIMIT: usize = 3;

pub fn check_def() -> ToolDef {
    ToolDef {
        name: "check",
        description:
            "Return fast changed-scope issues for the current scan as a flat, agent-optimized list.",
        input_schema: json!({
            "type": "object",
            "properties": {},
        }),
        min_tier: Tier::Free,
        handler: handle_check,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_check(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let started_at = Instant::now();
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let (session_v2, _) = current_session_v2_baseline_with_status(state, &root)?;
    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let changed_files = context.changed_files.clone();
    let bundle = context.bundle;
    let persisted_baseline = load_persisted_baseline(&root).ok().flatten();
    let expected_patch_cache_identity = if context.reused_cached_scan {
        state.cached_scan_identity.clone()
    } else {
        context.scan_identity.clone()
    };

    let (issues, diagnostics) = if !context.changed_scope_available {
        build_unavailable_changed_scope_response()
    } else if changed_files.is_empty() {
        build_known_empty_changed_scope_response()
    } else {
        build_fast_check_issues(
            state,
            &root,
            &bundle.snapshot,
            &bundle.health,
            &changed_files,
            session_v2.as_ref(),
            expected_patch_cache_identity.as_ref(),
        )
    };
    let gate = compute_agent_gate(&issues);
    let actions = actions_from_issues(&issues, DEFAULT_AGENT_ACTION_LIMIT);
    let signal_summary = build_check_signal_summary(&issues, &actions);
    let summary = build_check_summary(
        &issues,
        context.changed_scope_available,
        &changed_files,
        gate,
    );
    let response = AgentCheckResponse {
        issues,
        actions,
        signal_summary: signal_summary.clone(),
        gate,
        summary,
        changed_files: changed_files.iter().cloned().collect(),
        diagnostics,
    };
    crate::app::mcp_server::session_telemetry::record_check_run(
        state,
        &root,
        crate::app::mcp_server::session_telemetry::CheckRunTelemetry {
            changed_files: &changed_files,
            gate: response.gate,
            actions: &response.actions,
            issues: &response.issues,
            diagnostics: &response.diagnostics,
            signal_summary: response.signal_summary.clone(),
            session_baseline_available: session_v2.is_some(),
            reused_cached_scan: context.reused_cached_scan,
            elapsed_ms: started_at.elapsed().as_millis() as u64,
        },
    );

    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root,
            bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }

    serde_json::to_value(&response)
        .map_err(|error| format!("Failed to serialize check response: {error}"))
}

fn build_fast_check_issues(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    changed_files: &BTreeSet<String>,
    session_v2: Option<&SessionV2Baseline>,
    _expected_patch_cache_identity: Option<&ScanCacheIdentity>,
) -> (Vec<AgentIssue>, CheckDiagnostics) {
    let (rules_config, rules_error) = load_v2_rules_config(state, root);
    let semantic_status = analyze_check_semantics(state, root);
    let mut warnings = Vec::new();
    let changed_analysis = semantic_status
        .semantic
        .as_ref()
        .map(|semantic| {
            build_semantic_analysis_batch(
                &rules_config,
                semantic,
                Some(snapshot),
                crate::metrics::v2::ObligationScope::Changed,
                changed_files,
            )
        })
        .unwrap_or_default();

    let obligation_values = build_changed_obligation_values(&changed_analysis);
    let mut findings = build_changed_semantic_finding_values(&changed_analysis);
    findings.extend(build_changed_structural_finding_values(
        root,
        snapshot,
        health,
        &rules_config,
        changed_files,
    ));

    if let Some(session_v2) = session_v2 {
        let baseline_files = session_v2
            .file_hashes
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        findings.extend(
            crate::metrics::v2::build_missing_test_findings(
                snapshot,
                changed_files,
                &baseline_files,
            )
            .iter()
            .map(semantic_finding_value),
        );
    } else {
        warnings.push(
            "missing test coverage checks skipped because no session baseline is available"
                .to_string(),
        );
    }

    if rules_config.module_contract.is_empty() {
        let shape = project_shape_report_cached(state, root, snapshot, &rules_config);
        findings.extend(
            crate::metrics::v2::build_zero_config_boundary_findings(
                &rules_config,
                snapshot,
                &shape,
                changed_files,
            )
            .iter()
            .map(semantic_finding_value),
        );
    }

    findings.extend(session_introduced_clone_finding_values(
        session_v2,
        health,
        changed_files,
    ));

    let mut suppressible_values = obligation_values;
    suppressible_values.extend(findings);
    let suppression_application = apply_suppressions(&rules_config, suppressible_values);
    let (visible_obligations, visible_findings) =
        partition_check_issue_values(suppression_application.visible_findings);
    let mut issues = issues_from_findings_and_obligations(&visible_findings, &visible_obligations);
    issues.sort_by(compare_agent_issues);

    let diagnostics = build_fast_check_diagnostics(
        &rules_error,
        &semantic_status.error,
        semantic_status.available,
        warnings,
    );

    (issues, diagnostics)
}

struct SemanticCheckStatus {
    semantic: Option<crate::analysis::semantic::SemanticSnapshot>,
    error: Option<String>,
    available: bool,
}

fn analyze_check_semantics(state: &mut McpState, root: &Path) -> SemanticCheckStatus {
    match analyze_semantic_snapshot(state, root) {
        Ok(semantic) => SemanticCheckStatus {
            available: semantic.is_some(),
            semantic,
            error: None,
        },
        Err(error) => SemanticCheckStatus {
            semantic: None,
            error: Some(error),
            available: false,
        },
    }
}

fn build_changed_obligation_values(changed_analysis: &SemanticAnalysisBatch) -> Vec<Value> {
    changed_analysis
        .obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .map(obligation_issue_value)
        .collect::<Vec<_>>()
}

fn build_changed_semantic_finding_values(changed_analysis: &SemanticAnalysisBatch) -> Vec<Value> {
    changed_analysis
        .findings
        .iter()
        .filter(|finding| finding.kind != "closed_domain_exhaustiveness")
        .map(semantic_finding_value)
        .map(mark_changed_scope)
        .collect::<Vec<_>>()
}

fn build_fast_check_diagnostics(
    rules_error: &Option<String>,
    semantic_error: &Option<String>,
    semantic_available: bool,
    warnings: Vec<String>,
) -> CheckDiagnostics {
    let availability = CheckAvailability {
        semantic: semantic_available,
        evolution: false,
        rules: rules_error.is_none(),
        changed_scope: true,
    };
    let errors = diagnostics_errors(rules_error.clone(), semantic_error.clone());
    let partial_results = !semantic_available || errors.values().any(Option::is_some);
    CheckDiagnostics {
        errors,
        warnings,
        partial_results,
        availability,
    }
}

fn build_unavailable_changed_scope_response() -> (Vec<AgentIssue>, CheckDiagnostics) {
    (
        Vec::new(),
        build_empty_check_diagnostics(
            false,
            true,
            Some("changed scope unavailable; fast-path checks were skipped"),
        ),
    )
}

fn build_known_empty_changed_scope_response() -> (Vec<AgentIssue>, CheckDiagnostics) {
    (Vec::new(), build_empty_check_diagnostics(true, false, None))
}

fn build_empty_check_diagnostics(
    changed_scope_available: bool,
    partial_results: bool,
    warning: Option<&str>,
) -> CheckDiagnostics {
    let warnings = warning
        .map(|message| vec![message.to_string()])
        .unwrap_or_default();

    CheckDiagnostics {
        errors: diagnostics_errors(None, None),
        warnings,
        partial_results,
        availability: CheckAvailability {
            semantic: false,
            evolution: false,
            rules: false,
            changed_scope: changed_scope_available,
        },
    }
}

fn diagnostics_errors(
    rules_error: Option<String>,
    semantic_error: Option<String>,
) -> BTreeMap<String, Option<String>> {
    BTreeMap::from([
        ("rules".to_string(), rules_error),
        ("semantic".to_string(), semantic_error),
    ])
}

fn compute_agent_gate(issues: &[AgentIssue]) -> AgentGate {
    if issues.iter().any(issue_blocks_gate) {
        return AgentGate::Fail;
    }
    if issues
        .iter()
        .any(|issue| issue.severity.priority() >= FindingSeverity::Medium.priority())
    {
        return AgentGate::Warn;
    }
    AgentGate::Pass
}

fn build_check_summary(
    issues: &[AgentIssue],
    changed_scope_available: bool,
    changed_files: &BTreeSet<String>,
    gate: AgentGate,
) -> String {
    if !changed_scope_available {
        return "Changed scope unavailable; returned partial fast-path results.".to_string();
    }
    if changed_files.is_empty() {
        return "No working-tree changes detected.".to_string();
    }
    match gate {
        AgentGate::Fail => format!(
            "{} blocking issue(s) detected in changed scope.",
            issues.len()
        ),
        AgentGate::Warn => format!("{} changed-scope issue(s) need attention.", issues.len()),
        AgentGate::Pass => {
            if issues.is_empty() {
                "No changed-scope issues detected.".to_string()
            } else {
                format!("{} non-blocking watchpoint(s) detected.", issues.len())
            }
        }
    }
}

fn semantic_finding_value(finding: &SemanticFinding) -> Value {
    serde_json::to_value(finding).unwrap_or_else(|_| json!({}))
}

fn structural_report_value(report: crate::metrics::v2::StructuralDebtReport) -> Value {
    serde_json::to_value(report).unwrap_or_else(|_| json!({}))
}

fn mark_changed_scope(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("changed_scope".to_string(), json!(true));
    }

    value
}

fn derived_obligation_kind(obligation: &crate::metrics::v2::ObligationReport) -> &str {
    if obligation.kind == "contract_surface_completeness" {
        "incomplete_propagation"
    } else {
        obligation.kind.as_str()
    }
}

pub(crate) fn obligation_issue_value(obligation: &crate::metrics::v2::ObligationReport) -> Value {
    let kind = derived_obligation_kind(obligation);
    let summary = if kind == "incomplete_propagation" {
        let concept = obligation
            .concept_id
            .as_deref()
            .or(obligation.domain_symbol_name.as_deref())
            .unwrap_or("changed contract");
        format!(
            "Propagation is incomplete for '{}': update the remaining sibling surfaces listed in the evidence.",
            concept
        )
    } else {
        obligation.summary.clone()
    };
    json!({
        "source": "obligation",
        "kind": kind,
        "severity": obligation.severity,
        "concept_id": obligation.concept_id.clone(),
        "domain_symbol_name": obligation.domain_symbol_name.clone(),
        "summary": summary,
        "files": obligation.files.clone(),
        "missing_sites": obligation.missing_sites.clone(),
        "missing_variants": obligation.missing_variants.clone(),
        "evidence": obligation.missing_sites.iter().map(|site| {
            let line_suffix = site.line.map(|line| format!(":{line}")).unwrap_or_default();
            format!("{}{} [{}]", site.path, line_suffix, site.detail)
        }).collect::<Vec<_>>(),
        "origin": obligation.origin,
        "trust_tier": obligation.trust_tier,
        "confidence": obligation.confidence,
        "score_0_10000": obligation.score_0_10000,
        "line": obligation.missing_sites.iter().find_map(|site| site.line),
    })
}

fn partition_check_issue_values(values: Vec<Value>) -> (Vec<Value>, Vec<Value>) {
    let mut obligations = Vec::new();
    let mut findings = Vec::new();

    for value in values {
        if value.get("source").and_then(Value::as_str) == Some("obligation") {
            obligations.push(value);
        } else {
            findings.push(value);
        }
    }

    (obligations, findings)
}

fn build_changed_structural_finding_values(
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    rules_config: &crate::metrics::rules::RulesConfig,
    changed_files: &BTreeSet<String>,
) -> Vec<Value> {
    filter_structural_reports_by_rules(
        crate::metrics::v2::build_structural_debt_reports_with_root(root, snapshot, health),
        rules_config,
    )
    .into_iter()
    .filter(|report| {
        report.files.iter().any(|path| changed_files.contains(path))
            && matches!(
                report.kind.as_str(),
                "large_file" | "dependency_sprawl" | "unstable_hotspot" | "cycle_cluster"
            )
    })
    .map(structural_report_value)
    .map(mark_changed_scope)
    .collect()
}

fn session_introduced_clone_finding_values(
    session_v2: Option<&SessionV2Baseline>,
    health: &metrics::HealthReport,
    changed_files: &BTreeSet<String>,
) -> Vec<Value> {
    let current_findings = serialized_values(&build_clone_drift_findings(
        &health.duplicate_groups,
        None,
        health.duplicate_groups.len(),
    ));
    let introduced_findings =
        build_session_introduced_clone_findings(&current_findings, session_v2, changed_files, 10);
    let followthrough_findings =
        build_clone_followthrough_findings(&current_findings, session_v2, changed_files, 10);
    merge_findings(introduced_findings, followthrough_findings, 10)
}
