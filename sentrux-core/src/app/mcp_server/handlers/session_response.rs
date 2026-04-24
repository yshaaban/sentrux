use super::checkpoint::SessionBaselineStatus;
use super::debt::DebtReportOutputs;
use super::evaluation_signals::build_session_signal_summary;
use super::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

struct LegacyDiffSummary {
    signal_before: Option<i32>,
    signal_after: Option<i32>,
    signal_delta: Option<i32>,
    coupling_change: Option<Vec<f64>>,
    cycles_change: Option<Vec<usize>>,
    violations: Vec<String>,
}

fn build_legacy_diff_summary(legacy_diff: Option<&arch::ArchDiff>) -> LegacyDiffSummary {
    LegacyDiffSummary {
        signal_before: legacy_diff.map(|diff| (diff.signal_before * 10000.0).round() as i32),
        signal_after: legacy_diff.map(|diff| (diff.signal_after * 10000.0).round() as i32),
        signal_delta: legacy_diff
            .map(|diff| ((diff.signal_after - diff.signal_before) * 10000.0).round() as i32),
        coupling_change: legacy_diff.map(|diff| vec![diff.coupling_before, diff.coupling_after]),
        cycles_change: legacy_diff.map(|diff| vec![diff.cycles_before, diff.cycles_after]),
        violations: legacy_diff
            .map(|diff| diff.violations.clone())
            .unwrap_or_default(),
    }
}

type SessionEndResultMap = serde_json::Map<String, Value>;

fn build_session_result_map(
    gate_decision: &str,
    summary: &str,
    legacy_summary: &LegacyDiffSummary,
) -> SessionEndResultMap {
    let mut result = serde_json::Map::new();
    result.insert("pass".to_string(), json!(gate_decision != "fail"));
    insert_legacy_summary_fields(&mut result, legacy_summary);
    result.insert("summary".to_string(), json!(summary));
    result
}

fn insert_legacy_summary_fields(
    result: &mut SessionEndResultMap,
    legacy_summary: &LegacyDiffSummary,
) {
    result.insert(
        "signal_before".to_string(),
        json!(legacy_summary.signal_before),
    );
    result.insert(
        "signal_after".to_string(),
        json!(legacy_summary.signal_after),
    );
    result.insert(
        "signal_delta".to_string(),
        json!(legacy_summary.signal_delta),
    );
    result.insert(
        "coupling_change".to_string(),
        json!(legacy_summary.coupling_change),
    );
    result.insert(
        "cycles_change".to_string(),
        json!(legacy_summary.cycles_change),
    );
    result.insert("violations".to_string(), json!(legacy_summary.violations));
}

fn insert_empty_change_fields(result: &mut SessionEndResultMap) {
    result.insert("changed_files".to_string(), json!(Vec::<String>::new()));
    result.insert("changed_concepts".to_string(), json!(Vec::<String>::new()));
}

fn insert_change_fields(
    result: &mut SessionEndResultMap,
    changed_files: &BTreeSet<String>,
    changed_concepts: Vec<String>,
) {
    result.insert(
        "changed_files".to_string(),
        json!(changed_files.iter().cloned().collect::<Vec<_>>()),
    );
    result.insert("changed_concepts".to_string(), json!(changed_concepts));
}

fn insert_empty_finding_fields(result: &mut SessionEndResultMap) {
    result.insert(
        "introduced_findings".to_string(),
        json!(Vec::<Value>::new()),
    );
    result.insert("introduced_clone_finding_count".to_string(), json!(0));
    result.insert(
        "introduced_clone_findings".to_string(),
        json!(Vec::<Value>::new()),
    );
    result.insert("resolved_findings".to_string(), json!(Vec::<Value>::new()));
    result.insert("finding_detail_count".to_string(), json!(0));
    result.insert("finding_details".to_string(), json!(Vec::<Value>::new()));
    result.insert("experimental_finding_count".to_string(), json!(0));
    result.insert(
        "experimental_findings".to_string(),
        json!(Vec::<Value>::new()),
    );
}

fn insert_finding_fields(
    result: &mut SessionEndResultMap,
    introduced_findings: Vec<Value>,
    introduced_clone_findings: Vec<Value>,
    resolved_findings: Vec<Value>,
    finding_details: Vec<Value>,
    experimental_findings: Vec<Value>,
) {
    let introduced_findings = findings_with_agent_guidance(introduced_findings);
    let introduced_clone_findings = findings_with_agent_guidance(introduced_clone_findings);
    let finding_details = findings_with_agent_guidance(finding_details);
    result.insert(
        "introduced_findings".to_string(),
        json!(introduced_findings),
    );
    result.insert(
        "introduced_clone_finding_count".to_string(),
        json!(introduced_clone_findings.len()),
    );
    result.insert(
        "introduced_clone_findings".to_string(),
        json!(introduced_clone_findings),
    );
    result.insert("resolved_findings".to_string(), json!(resolved_findings));
    result.insert(
        "finding_detail_count".to_string(),
        json!(finding_details.len()),
    );
    result.insert("finding_details".to_string(), json!(finding_details));
    result.insert(
        "experimental_finding_count".to_string(),
        json!(experimental_findings.len()),
    );
    result.insert(
        "experimental_findings".to_string(),
        json!(experimental_findings),
    );
}

fn insert_action_fields(result: &mut SessionEndResultMap, action_payloads: Vec<AgentAction>) {
    result.insert("action_count".to_string(), json!(action_payloads.len()));
    result.insert("actions".to_string(), json!(action_payloads));
}

fn insert_signal_summary_field(
    result: &mut SessionEndResultMap,
    signal_summary: SessionSignalSummary,
) {
    result.insert("signal_summary".to_string(), json!(signal_summary));
}

fn insert_empty_obligation_fields(result: &mut SessionEndResultMap, gate_decision: &str) {
    result.insert(
        "missing_obligations".to_string(),
        json!(Vec::<Value>::new()),
    );
    result.insert(
        "obligation_completeness_0_10000".to_string(),
        json!(crate::metrics::v2::obligation_score_0_10000(&[])),
    );
    result.insert(
        "touched_concept_gate".to_string(),
        json!({
            "decision": gate_decision,
            "blocking_findings": Vec::<Value>::new(),
        }),
    );
}

fn insert_obligation_fields(
    result: &mut SessionEndResultMap,
    missing_obligations: Vec<crate::metrics::v2::ObligationReport>,
    changed_obligations: &[crate::metrics::v2::ObligationReport],
    gate_decision: &str,
    blocking_findings: Vec<Value>,
) {
    let blocking_findings = findings_with_agent_guidance(blocking_findings);
    result.insert(
        "missing_obligations".to_string(),
        json!(missing_obligations),
    );
    result.insert(
        "obligation_completeness_0_10000".to_string(),
        json!(crate::metrics::v2::obligation_score_0_10000(
            changed_obligations
        )),
    );
    result.insert(
        "touched_concept_gate".to_string(),
        json!({
            "decision": gate_decision,
            "blocking_findings": blocking_findings,
        }),
    );
}

fn insert_empty_suppression_fields(result: &mut SessionEndResultMap) {
    result.insert("suppression_hits".to_string(), json!(Vec::<Value>::new()));
    result.insert("suppressed_finding_count".to_string(), json!(0));
    result.insert(
        "expired_suppressions".to_string(),
        json!(Vec::<Value>::new()),
    );
    result.insert("expired_suppression_match_count".to_string(), json!(0));
}

fn insert_suppression_fields(
    result: &mut SessionEndResultMap,
    analysis: &PatchSafetyAnalysisCache,
) {
    result.insert(
        "suppression_hits".to_string(),
        json!(analysis.suppression_hits),
    );
    result.insert(
        "suppressed_finding_count".to_string(),
        json!(analysis.suppressed_finding_count),
    );
    result.insert(
        "expired_suppressions".to_string(),
        json!(analysis.expired_suppressions),
    );
    result.insert(
        "expired_suppression_match_count".to_string(),
        json!(analysis.expired_suppression_match_count),
    );
}

fn insert_session_context_fields(
    result: &mut SessionEndResultMap,
    bundle: &ScanBundle,
    rules_config: &RulesConfig,
    session_v2_status: SessionBaselineStatus,
    legacy_diff: Option<&arch::ArchDiff>,
) {
    result.insert("scan_trust".to_string(), scan_trust_json(&bundle.metadata));
    result.insert(
        "confidence".to_string(),
        json!(build_v2_confidence_report(
            &bundle.metadata,
            rules_config,
            session_v2_status
        )),
    );
    result.insert(
        "baseline_delta".to_string(),
        legacy_baseline_delta_json(legacy_diff),
    );
}

fn insert_session_end_diagnostics(
    result: &mut SessionEndResultMap,
    rules_error: Option<String>,
    semantic_error: Option<String>,
    debt_context_error: Option<String>,
    baseline_error: Option<String>,
) {
    insert_error_diagnostics(
        result,
        vec![
            DiagnosticEntry::new("rules", rules_error),
            DiagnosticEntry::new("semantic", semantic_error),
            DiagnosticEntry::new("context", debt_context_error),
            DiagnosticEntry::new("baseline", baseline_error),
        ],
        Vec::new(),
    );
}

pub(crate) fn finish_patch_check_scan_state(
    state: &mut McpState,
    root: PathBuf,
    bundle: ScanBundle,
    baseline: Option<arch::ArchBaseline>,
    scan_identity: Option<ScanCacheIdentity>,
    reused_cached_scan: bool,
) {
    if !reused_cached_scan {
        let preserved_semantic = state.cached_semantic.take();
        let preserved_evolution = state.cached_evolution.take();
        let preserved_patch_safety = state.cached_patch_safety.take();
        update_scan_cache(state, root, bundle, baseline, scan_identity);
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
        return;
    }

    state.baseline = baseline;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_empty_session_end_result(
    state: &mut McpState,
    root: &Path,
    bundle: &ScanBundle,
    rules_config: &RulesConfig,
    rules_error: Option<String>,
    session_v2_status: SessionBaselineStatus,
    legacy_diff: Option<&arch::ArchDiff>,
    baseline_error: Option<String>,
    changed_files: &BTreeSet<String>,
    reused_cached_scan: bool,
) -> Value {
    let debt_outputs = build_debt_report_outputs(
        state,
        root,
        &bundle.snapshot,
        &bundle.health,
        &[],
        &[],
        &[],
        changed_files,
        5,
        true,
    );
    let legacy_summary = build_legacy_diff_summary(legacy_diff);
    let gate_decision = if legacy_diff.is_some_and(|diff| diff.degraded) {
        "warn"
    } else {
        "pass"
    };
    let summary = if legacy_diff.is_some_and(|diff| diff.degraded) {
        "Quality degraded"
    } else if legacy_diff.is_none() {
        "Patch safety check complete; legacy structural delta unavailable"
    } else {
        "Quality stable or improved"
    };
    let action_payloads: Vec<AgentAction> = Vec::new();
    let signal_summary =
        build_session_signal_summary(&[], &[], &[], &action_payloads, gate_decision);
    let mut result = build_session_result_map(gate_decision, summary, &legacy_summary);
    insert_empty_change_fields(&mut result);
    insert_empty_finding_fields(&mut result);
    insert_action_fields(&mut result, action_payloads);
    insert_empty_obligation_fields(&mut result, gate_decision);
    insert_signal_summary_field(&mut result, signal_summary.clone());
    insert_empty_suppression_fields(&mut result);
    insert_session_context_fields(
        &mut result,
        bundle,
        rules_config,
        session_v2_status,
        legacy_diff,
    );
    let debt_context_error = insert_debt_report_fields(&mut result, debt_outputs);
    insert_session_end_diagnostics(
        &mut result,
        rules_error,
        None,
        debt_context_error,
        baseline_error,
    );
    crate::app::mcp_server::session_telemetry::record_session_ended(
        state,
        root,
        crate::app::mcp_server::session_telemetry::SessionEndTelemetry {
            changed_files,
            decision: gate_decision,
            action_payloads: &[],
            introduced_finding_kinds: Vec::new(),
            missing_obligation_count: 0,
            introduced_clone_finding_count: 0,
            signal_summary,
            reused_cached_scan,
        },
    );

    Value::Object(result)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_session_end_result(
    bundle: &ScanBundle,
    rules_config: &RulesConfig,
    session_v2_status: SessionBaselineStatus,
    legacy_diff: Option<&arch::ArchDiff>,
    changed_files: &BTreeSet<String>,
    changed_concepts: Vec<String>,
    introduced_findings: Vec<Value>,
    introduced_clone_findings: Vec<Value>,
    resolved_findings: Vec<Value>,
    action_payloads: Vec<AgentAction>,
    finding_details: Vec<Value>,
    experimental_findings: Vec<Value>,
    missing_obligations: Vec<crate::metrics::v2::ObligationReport>,
    analysis: &PatchSafetyAnalysisCache,
    debt_outputs: DebtReportOutputs,
    gate_decision: &str,
    summary: &str,
    blocking_findings: Vec<Value>,
    signal_summary: SessionSignalSummary,
    baseline_error: Option<String>,
    semantic_error: Option<String>,
) -> Value {
    let legacy_summary = build_legacy_diff_summary(legacy_diff);
    let mut result = build_session_result_map(gate_decision, summary, &legacy_summary);
    insert_change_fields(&mut result, changed_files, changed_concepts);
    insert_finding_fields(
        &mut result,
        introduced_findings,
        introduced_clone_findings,
        resolved_findings,
        finding_details,
        experimental_findings,
    );
    insert_action_fields(&mut result, action_payloads);
    insert_obligation_fields(
        &mut result,
        missing_obligations,
        &analysis.changed_obligations,
        gate_decision,
        blocking_findings,
    );
    insert_signal_summary_field(&mut result, signal_summary);
    let debt_context_error = insert_debt_report_fields(&mut result, debt_outputs);
    insert_suppression_fields(&mut result, analysis);
    insert_session_context_fields(
        &mut result,
        bundle,
        rules_config,
        session_v2_status,
        legacy_diff,
    );
    insert_session_end_diagnostics(
        &mut result,
        analysis.rules_error.clone(),
        semantic_error,
        debt_context_error,
        baseline_error,
    );

    Value::Object(result)
}
