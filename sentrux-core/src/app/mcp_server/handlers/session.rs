use super::*;

#[path = "session_analysis.rs"]
mod session_analysis;
#[path = "session_flow.rs"]
mod session_flow;
#[path = "session_response.rs"]
mod session_response;

pub(crate) use self::session_analysis::{
    build_patch_safety_analysis, current_patch_safety_cache_identity, has_known_empty_patch_scope,
    patch_safety_semantic_error, prepare_patch_check_context,
};
use self::session_flow::{
    analyze_gate_result, analyze_session_end_result, build_empty_gate_result,
    load_gate_baseline_context, load_session_legacy_context, prepare_patch_run,
};

use self::session_response::{
    build_empty_session_end_result, build_session_end_result, finish_patch_check_scan_state,
};

pub fn session_start_def() -> ToolDef {
    ToolDef {
        name: "session_start",
        description: "Save current health metrics as baseline for later comparison via 'gate' or 'session_end'.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_session_start,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_session_start(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let health = state
        .cached_health
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let metadata = state
        .cached_scan_metadata
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let baseline = arch::ArchBaseline::from_health(&health);
    let signal = baseline.quality_signal;
    let baseline_path = save_baseline(&root, &baseline)?;
    let (session_v2, suppression_application, semantic_error) =
        build_session_v2_baseline(state, &root, &snapshot, &health, &metadata);
    let (rules_config, rules_error) = load_v2_rules_config(state, &root);

    state.baseline = Some(baseline);
    let session_v2_baseline_path = save_session_v2_baseline(&root, &session_v2)?;
    state.session_v2 = Some(session_v2);
    state.cached_patch_safety = None;

    let mut response = json!({
        "status": "Baseline saved",
        "quality_signal": (signal * 10000.0).round() as u32,
        "baseline_path": baseline_path,
        "session_v2_baseline_path": session_v2_baseline_path,
        "session_finding_count": state.session_v2.as_ref().map(|baseline| baseline.finding_payloads.len()).unwrap_or(0),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "confidence": build_v2_confidence_report(
            &metadata,
            &rules_config,
            compatible_session_baseline_status(SESSION_V2_SCHEMA_VERSION),
        ),
        "message": "Call 'session_end' after making changes to see the diff"
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(object, rules_error, semantic_error, Vec::new());
    }
    crate::app::mcp_server::session_telemetry::record_session_started(
        state,
        &root,
        (signal * 10000.0).round() as u32,
        state
            .session_v2
            .as_ref()
            .map(|baseline| baseline.finding_payloads.len())
            .unwrap_or(0),
        &session_v2_baseline_path,
    );
    Ok(response)
}

pub fn session_end_def() -> ToolDef {
    ToolDef {
        name: "session_end",
        description: "Re-scan and compare current state against session baseline. Returns diff showing what degraded.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_session_end,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_session_end(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let mut run = prepare_patch_run(state, &root)?;
    let mut legacy = load_session_legacy_context(state, &root, &run.context.bundle.health);
    if legacy.baseline.is_none() && legacy.baseline_error.is_none() {
        legacy.baseline_error =
            Some("Legacy baseline unavailable; structural delta fields were omitted".to_string());
    }

    if has_known_empty_patch_scope(&run.context) {
        let result = build_empty_session_end_result(
            state,
            &root,
            &run.context.bundle,
            &run.rules_config,
            run.rules_error.clone(),
            run.session_v2_status,
            legacy.diff.as_ref(),
            legacy.baseline_error.clone(),
            &run.context.changed_files,
            run.context.reused_cached_scan,
        );
        finish_patch_check_scan_state(
            state,
            root,
            run.context.bundle,
            legacy.baseline,
            run.context.scan_identity.take(),
            run.context.reused_cached_scan,
        );
        return Ok(result);
    }

    let analysis = build_patch_safety_analysis(
        state,
        &root,
        &run.context.bundle,
        &run.context.changed_files,
        run.session_v2.as_ref(),
        run.patch_cache_identity.clone(),
        true,
    );
    let result_data =
        analyze_session_end_result(state, &root, &run, legacy.diff.as_ref(), &analysis);
    crate::app::mcp_server::session_telemetry::record_session_ended(
        state,
        &root,
        crate::app::mcp_server::session_telemetry::SessionEndTelemetry {
            changed_files: &run.context.changed_files,
            decision: result_data.gate_decision,
            action_payloads: &result_data.action_payloads,
            introduced_finding_kinds: result_data.introduced_finding_kinds,
            missing_obligation_count: result_data.missing_obligations.len(),
            introduced_clone_finding_count: result_data.introduced_clone_findings.len(),
            signal_summary: result_data.signal_summary.clone(),
            reused_cached_scan: run.context.reused_cached_scan,
        },
    );
    let result = build_session_end_result(
        &run.context.bundle,
        &run.rules_config,
        run.session_v2_status,
        legacy.diff.as_ref(),
        &run.context.changed_files,
        result_data.changed_concepts,
        result_data.introduced_findings,
        result_data.introduced_clone_findings,
        result_data.resolved_findings,
        result_data.action_payloads,
        result_data.finding_details,
        result_data.experimental_findings,
        result_data.missing_obligations,
        &analysis,
        result_data.debt_outputs,
        result_data.gate_decision,
        result_data.summary,
        result_data.blocking_findings,
        result_data.signal_summary,
        legacy.baseline_error.clone(),
        result_data.semantic_error,
    );
    finish_patch_check_scan_state(
        state,
        root,
        run.context.bundle,
        legacy.baseline,
        run.context.scan_identity.take(),
        run.context.reused_cached_scan,
    );
    Ok(result)
}

pub fn gate_def() -> ToolDef {
    ToolDef {
        name: "gate",
        description: "Evaluate whether the current patch introduces high-confidence touched-concept regressions.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "strict": {
                    "type": "boolean",
                    "description": "If true, medium-severity introduced findings also fail the gate."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_gate,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_gate(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let strict = args
        .get("strict")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    compute_touched_concept_gate(state, &root, strict)
}

fn compute_touched_concept_gate(
    state: &mut McpState,
    root: &Path,
    strict: bool,
) -> Result<Value, String> {
    let mut run = prepare_patch_run(state, root)?;
    let legacy = load_gate_baseline_context(state, root, &run.context.bundle.health);

    if has_known_empty_patch_scope(&run.context) {
        let response = build_empty_gate_result(
            &run.context.bundle,
            &run.rules_config,
            run.session_v2_status,
            legacy.diff.as_ref(),
            run.rules_error.clone(),
            strict,
        );
        finish_patch_check_scan_state(
            state,
            root.to_path_buf(),
            run.context.bundle,
            legacy.baseline,
            run.context.scan_identity.take(),
            run.context.reused_cached_scan,
        );
        return Ok(response);
    }

    let analysis = build_patch_safety_analysis(
        state,
        root,
        &run.context.bundle,
        &run.context.changed_files,
        run.session_v2.as_ref(),
        run.patch_cache_identity.clone(),
        true,
    );
    let response = analyze_gate_result(
        &run.context.bundle,
        &run.rules_config,
        run.session_v2_status,
        legacy.diff.as_ref(),
        &run.context.changed_files,
        &analysis,
        run.session_v2.as_ref(),
        strict,
    );
    finish_patch_check_scan_state(
        state,
        root.to_path_buf(),
        run.context.bundle,
        legacy.baseline,
        run.context.scan_identity.take(),
        run.context.reused_cached_scan,
    );

    Ok(response)
}

pub fn cli_save_v2_session(root: &Path) -> Result<Value, String> {
    let mut state = fresh_mcp_state();
    let bundle = do_scan(root)?;
    let baseline = arch::ArchBaseline::from_health(&bundle.health);
    let signal = baseline.quality_signal;
    let baseline_path = save_baseline(root, &baseline)?;
    let (session_v2, suppression_application, _semantic_error) = build_session_v2_baseline(
        &mut state,
        root,
        &bundle.snapshot,
        &bundle.health,
        &bundle.metadata,
    );
    let session_v2_baseline_path = save_session_v2_baseline(root, &session_v2)?;
    let session_finding_count = session_v2.finding_payloads.len();
    let (rules_config, _) = load_v2_rules_config(&mut state, root);

    Ok(json!({
        "status": "Baseline saved",
        "quality_signal": (signal * 10000.0).round() as u32,
        "baseline_path": baseline_path,
        "session_v2_baseline_path": session_v2_baseline_path,
        "session_finding_count": session_finding_count,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "confidence": build_v2_confidence_report(
            &bundle.metadata,
            &rules_config,
            compatible_session_baseline_status(SESSION_V2_SCHEMA_VERSION),
        ),
        "message": "Run 'sentrux gate' after making changes to evaluate touched-concept regressions"
    }))
}

pub fn cli_evaluate_v2_gate(root: &Path, strict: bool) -> Result<Value, String> {
    let mut state = fresh_mcp_state();
    compute_touched_concept_gate(&mut state, root, strict)
}
