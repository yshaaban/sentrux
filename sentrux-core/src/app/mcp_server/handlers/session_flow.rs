use super::super::checkpoint::SessionBaselineStatus;
use super::super::debt::DebtReportOutputs;
use super::session_analysis::{
    current_patch_safety_cache_identity, patch_safety_semantic_error, prepare_patch_check_context,
    PatchCheckContext,
};
use super::*;
use crate::metrics::v2::FindingSeverity;
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) struct PreparedPatchRun {
    pub(crate) context: PatchCheckContext,
    pub(crate) session_v2: Option<SessionV2Baseline>,
    pub(crate) session_v2_status: SessionBaselineStatus,
    pub(crate) patch_cache_identity: Option<ScanCacheIdentity>,
    pub(crate) rules_config: RulesConfig,
    pub(crate) rules_error: Option<String>,
}

pub(crate) struct SessionLegacyContext {
    pub(crate) baseline: Option<arch::ArchBaseline>,
    pub(crate) baseline_error: Option<String>,
    pub(crate) diff: Option<arch::ArchDiff>,
}

pub(crate) struct GateBaselineContext {
    pub(crate) baseline: Option<arch::ArchBaseline>,
    pub(crate) diff: Option<arch::ArchDiff>,
}

pub(crate) struct SessionEndAnalysisData {
    pub(crate) changed_concepts: Vec<String>,
    pub(crate) introduced_findings: Vec<Value>,
    pub(crate) introduced_clone_findings: Vec<Value>,
    pub(crate) resolved_findings: Vec<Value>,
    pub(crate) action_payloads: Vec<AgentAction>,
    pub(crate) finding_details: Vec<Value>,
    pub(crate) experimental_findings: Vec<Value>,
    pub(crate) missing_obligations: Vec<crate::metrics::v2::ObligationReport>,
    pub(crate) debt_outputs: DebtReportOutputs,
    pub(crate) gate_decision: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) blocking_findings: Vec<Value>,
    pub(crate) semantic_error: Option<String>,
    pub(crate) introduced_finding_kinds: Vec<String>,
    pub(crate) signal_summary: SessionSignalSummary,
}

pub(crate) fn prepare_patch_run(
    state: &mut McpState,
    root: &Path,
) -> Result<PreparedPatchRun, String> {
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let (rules_config, rules_error) = load_v2_rules_config(state, root);

    Ok(PreparedPatchRun {
        context,
        session_v2,
        session_v2_status,
        patch_cache_identity,
        rules_config,
        rules_error,
    })
}

pub(crate) fn load_session_legacy_context(
    state: &McpState,
    root: &Path,
    health: &metrics::HealthReport,
) -> SessionLegacyContext {
    let (baseline, baseline_error) = match state.baseline.clone() {
        Some(baseline) => (Some(baseline), None),
        None => match load_persisted_baseline(root) {
            Ok(baseline) => (baseline, None),
            Err(error) => (None, Some(error)),
        },
    };
    let diff = baseline.as_ref().map(|baseline| baseline.diff(health));

    SessionLegacyContext {
        baseline,
        baseline_error,
        diff,
    }
}

pub(crate) fn load_gate_baseline_context(
    state: &McpState,
    root: &Path,
    health: &metrics::HealthReport,
) -> GateBaselineContext {
    let baseline = load_persisted_baseline(root)
        .ok()
        .flatten()
        .or(state.baseline.clone());
    let diff = baseline.as_ref().map(|baseline| baseline.diff(health));

    GateBaselineContext { baseline, diff }
}

pub(crate) fn analyze_session_end_result(
    state: &mut McpState,
    root: &Path,
    run: &PreparedPatchRun,
    legacy_diff: Option<&arch::ArchDiff>,
    analysis: &PatchSafetyAnalysisCache,
) -> SessionEndAnalysisData {
    let missing_obligations = collect_missing_obligations(analysis);
    let (visible_introduced_findings, introduced_experimental_findings) =
        collect_session_end_introduced_findings(
            analysis,
            run.session_v2.as_ref(),
            &run.context.changed_files,
        );
    let blocking_findings = collect_session_end_blocking_findings(
        analysis,
        run.session_v2.as_ref(),
        &visible_introduced_findings,
    );
    let resolved_findings = collect_resolved_findings(analysis, run.session_v2.as_ref());
    let introduced_findings = decorate_findings(&visible_introduced_findings);
    let introduced_clone_findings = collect_introduced_clone_findings(&introduced_findings);
    let (opportunity_findings, experimental_findings) = collect_opportunity_findings(
        analysis,
        run.session_v2.as_ref(),
        &visible_introduced_findings,
        introduced_experimental_findings,
    );
    let finding_details = serialized_values(&build_finding_details(&opportunity_findings, 10));
    let action_payloads = actions_from_findings_and_obligations(
        &introduced_findings,
        &serialized_values(&missing_obligations),
        10,
    );
    let debt_outputs = build_debt_report_outputs(
        state,
        root,
        &run.context.bundle.snapshot,
        &run.context.bundle.health,
        &opportunity_findings,
        &analysis.changed_obligations,
        &[],
        &run.context.changed_files,
        5,
        true,
    );
    let gate_decision = session_end_gate_decision(
        &missing_obligations,
        &blocking_findings,
        legacy_diff,
        &visible_introduced_findings,
    );
    let summary = session_end_summary(gate_decision, legacy_diff);
    let introduced_finding_kinds = introduced_findings
        .iter()
        .map(|finding| finding_kind(finding).to_string())
        .collect::<Vec<_>>();
    let signal_summary = build_session_signal_summary(
        &introduced_findings,
        &resolved_findings,
        &missing_obligations,
        &action_payloads,
        gate_decision,
    );

    SessionEndAnalysisData {
        changed_concepts: analysis.changed_touched_concepts.iter().cloned().collect(),
        introduced_findings,
        introduced_clone_findings,
        resolved_findings,
        action_payloads,
        finding_details,
        experimental_findings,
        missing_obligations,
        debt_outputs,
        gate_decision,
        summary,
        blocking_findings,
        semantic_error: patch_safety_semantic_error(analysis),
        introduced_finding_kinds,
        signal_summary,
    }
}

pub(crate) fn build_empty_gate_result(
    bundle: &ScanBundle,
    rules_config: &RulesConfig,
    session_v2_status: SessionBaselineStatus,
    legacy_baseline_delta: Option<&arch::ArchDiff>,
    rules_error: Option<String>,
    strict: bool,
) -> Value {
    let mut response = json!({
        "decision": "pass",
        "strict": strict,
        "summary": "No working-tree changes detected",
        "changed_files": Vec::<String>::new(),
        "introduced_findings": Vec::<Value>::new(),
        "experimental_finding_count": 0,
        "experimental_findings": Vec::<Value>::new(),
        "blocking_findings": Vec::<Value>::new(),
        "missing_obligations": Vec::<Value>::new(),
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&[]),
        "suppression_hits": Vec::<Value>::new(),
        "suppressed_finding_count": 0,
        "expired_suppressions": Vec::<Value>::new(),
        "expired_suppression_match_count": 0,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": build_v2_confidence_report(&bundle.metadata, rules_config, session_v2_status),
        "baseline_delta": legacy_baseline_delta_json(legacy_baseline_delta),
    });
    if let Some(object) = response.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![
                DiagnosticEntry::new("rules", rules_error),
                DiagnosticEntry::new("semantic", None),
            ],
            Vec::new(),
        );
    }

    response
}

pub(crate) fn analyze_gate_result(
    bundle: &ScanBundle,
    rules_config: &RulesConfig,
    session_v2_status: SessionBaselineStatus,
    legacy_baseline_delta: Option<&arch::ArchDiff>,
    changed_files: &BTreeSet<String>,
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
    strict: bool,
) -> Value {
    let missing_obligations = collect_missing_obligations(analysis);
    let introduced_findings = collect_gate_introduced_findings(analysis, session_v2, changed_files);
    let (visible_introduced_findings, experimental_findings) =
        partition_experimental_findings(&introduced_findings, 10);
    let blocking_findings = collect_gate_blocking_findings(&visible_introduced_findings, strict);
    let decision = gate_decision(&missing_obligations, &blocking_findings);
    let summary = gate_summary(decision, changed_files.is_empty());
    let semantic_error = patch_safety_semantic_error(analysis);
    let guided_introduced_findings = findings_with_agent_guidance(visible_introduced_findings);
    let guided_experimental_findings = findings_with_agent_guidance(experimental_findings);
    let guided_blocking_findings = findings_with_agent_guidance(blocking_findings);
    let guided_missing_obligations =
        obligations_with_agent_guidance(serialized_values(&missing_obligations));

    let mut response = json!({
        "decision": decision,
        "strict": strict,
        "summary": summary,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "introduced_findings": guided_introduced_findings,
        "experimental_finding_count": guided_experimental_findings.len(),
        "experimental_findings": guided_experimental_findings,
        "blocking_findings": guided_blocking_findings,
        "missing_obligations": guided_missing_obligations,
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&analysis.changed_obligations),
        "suppression_hits": analysis.suppression_hits,
        "suppressed_finding_count": analysis.suppressed_finding_count,
        "expired_suppressions": analysis.expired_suppressions,
        "expired_suppression_match_count": analysis.expired_suppression_match_count,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": build_v2_confidence_report(&bundle.metadata, rules_config, session_v2_status),
        "baseline_delta": legacy_baseline_delta_json(legacy_baseline_delta),
    });
    if let Some(object) = response.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![
                DiagnosticEntry::new("rules", analysis.rules_error.clone()),
                DiagnosticEntry::new("semantic", semantic_error),
            ],
            Vec::new(),
        );
    }

    response
}

fn collect_missing_obligations(
    analysis: &PatchSafetyAnalysisCache,
) -> Vec<crate::metrics::v2::ObligationReport> {
    analysis
        .changed_obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect()
}

fn collect_session_end_introduced_findings(
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
    changed_files: &BTreeSet<String>,
) -> (Vec<Value>, Vec<Value>) {
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let introduced_findings = session_v2
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let introduced_findings = merge_session_introduced_clone_findings(
        introduced_findings,
        &analysis.visible_findings,
        session_v2,
        changed_files,
        10,
    );

    partition_review_surface_experimental_findings(&introduced_findings, 10)
}

fn collect_session_end_blocking_findings(
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
    visible_introduced_findings: &[Value],
) -> Vec<Value> {
    let mut blocking_findings = visible_introduced_findings
        .iter()
        .filter(|finding| severity_of_value(finding) == FindingSeverity::High)
        .cloned()
        .collect::<Vec<_>>();
    if session_v2.is_none() {
        blocking_findings.extend(
            analysis
                .changed_visible_findings
                .iter()
                .filter(|finding| {
                    !is_experimental_finding(finding)
                        && severity_of_value(finding) == FindingSeverity::High
                })
                .cloned(),
        );
    }

    blocking_findings
}

fn collect_resolved_findings(
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
) -> Vec<Value> {
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    session_v2
        .map(|session_v2| {
            session_v2
                .finding_payloads
                .iter()
                .filter(|(key, _)| !current_finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|finding| !is_experimental_finding(finding))
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect()
}

fn collect_introduced_clone_findings(introduced_findings: &[Value]) -> Vec<Value> {
    introduced_findings
        .iter()
        .filter(|finding| is_agent_clone_signal_kind(finding_kind(finding)))
        .cloned()
        .collect()
}

fn collect_opportunity_findings(
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
    visible_introduced_findings: &[Value],
    introduced_experimental_findings: Vec<Value>,
) -> (Vec<Value>, Vec<Value>) {
    let (opportunity_findings, opportunity_experimental_findings) = if session_v2.is_some() {
        (
            visible_introduced_findings.to_vec(),
            introduced_experimental_findings,
        )
    } else {
        partition_review_surface_experimental_findings(&analysis.changed_visible_findings, 10)
    };

    (
        decorate_findings(&opportunity_findings),
        decorate_findings(&opportunity_experimental_findings),
    )
}

fn decorate_findings(findings: &[Value]) -> Vec<Value> {
    findings
        .iter()
        .map(decorate_finding_with_classification)
        .collect()
}

fn session_end_gate_decision(
    missing_obligations: &[crate::metrics::v2::ObligationReport],
    blocking_findings: &[Value],
    legacy_diff: Option<&arch::ArchDiff>,
    visible_introduced_findings: &[Value],
) -> &'static str {
    if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else if legacy_diff.is_some_and(|diff| diff.degraded)
        || !visible_introduced_findings.is_empty()
    {
        "warn"
    } else {
        "pass"
    }
}

fn session_end_summary(gate_decision: &str, legacy_diff: Option<&arch::ArchDiff>) -> &'static str {
    if gate_decision == "fail" {
        "Touched-concept regressions detected"
    } else if legacy_diff.is_some_and(|diff| diff.degraded) {
        "Quality degraded"
    } else if legacy_diff.is_none() {
        "Patch safety check complete; legacy structural delta unavailable"
    } else {
        "Quality stable or improved"
    }
}

fn collect_gate_introduced_findings(
    analysis: &PatchSafetyAnalysisCache,
    session_v2: Option<&SessionV2Baseline>,
    changed_files: &BTreeSet<String>,
) -> Vec<Value> {
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let introduced_findings = session_v2
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            analysis
                .changed_visible_findings
                .iter()
                .filter(|finding| {
                    let concept_id = finding_concept_id(finding).unwrap_or_default();
                    analysis.changed_touched_concepts.is_empty()
                        || analysis.changed_touched_concepts.contains(concept_id)
                })
                .cloned()
                .collect::<Vec<_>>()
        });

    merge_session_introduced_clone_findings(
        introduced_findings,
        &analysis.visible_findings,
        session_v2,
        changed_files,
        10,
    )
}

fn collect_gate_blocking_findings(
    visible_introduced_findings: &[Value],
    strict: bool,
) -> Vec<Value> {
    visible_introduced_findings
        .iter()
        .filter(|finding| {
            let severity = severity_of_value(finding);
            severity == FindingSeverity::High || (strict && severity == FindingSeverity::Medium)
        })
        .cloned()
        .collect()
}

fn gate_decision(
    missing_obligations: &[crate::metrics::v2::ObligationReport],
    blocking_findings: &[Value],
) -> &'static str {
    if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else {
        "pass"
    }
}

fn gate_summary(decision: &str, changed_files_empty: bool) -> &'static str {
    if decision == "fail" {
        "Touched-concept regressions detected"
    } else if changed_files_empty {
        "No working-tree changes detected"
    } else {
        "No blocking touched-concept regressions detected"
    }
}
