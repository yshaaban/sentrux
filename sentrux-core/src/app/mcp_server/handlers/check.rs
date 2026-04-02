use super::agent_format::{
    actions_from_issues, compare_agent_issues, issue_blocks_gate, to_agent_issue,
    AgentCheckResponse, AgentGate, AgentIssue, CheckAvailability, CheckDiagnostics,
};
use super::*;
use crate::metrics::v2::{FindingSeverity, SemanticFinding};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

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
    let actions = actions_from_issues(&issues, issues.len());
    let summary = build_check_summary(
        &issues,
        context.changed_scope_available,
        &changed_files,
        gate,
    );
    let response = AgentCheckResponse {
        issues,
        actions,
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
            issue_count: response.issues.len(),
            diagnostics: &response.diagnostics,
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
    expected_patch_cache_identity: Option<&ScanCacheIdentity>,
) -> (Vec<AgentIssue>, CheckDiagnostics) {
    let (rules_config, rules_error) = load_v2_rules_config(state, root);
    let (semantic, semantic_error, semantic_available) =
        match analyze_semantic_snapshot(state, root) {
            Ok(semantic) => {
                let available = semantic.is_some();
                (semantic, None, available)
            }
            Err(error) => (None, Some(error), false),
        };

    let mut warnings = Vec::new();
    let changed_analysis = semantic
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

    let mut findings = changed_analysis
        .obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .map(obligation_finding_value)
        .collect::<Vec<_>>();
    findings.extend(
        changed_analysis
            .findings
            .iter()
            .filter(|finding| finding.kind != "closed_domain_exhaustiveness")
            .map(semantic_finding_value),
    );
    findings.extend(build_changed_structural_finding_values(
        health,
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

    findings.extend(cached_clone_finding_values(
        state,
        session_v2,
        changed_files,
        expected_patch_cache_identity,
    ));

    let suppression_application = apply_suppressions(&rules_config, findings);
    let mut issues = suppression_application
        .visible_findings
        .iter()
        .map(to_agent_issue)
        .collect::<Vec<_>>();
    issues.sort_by(compare_agent_issues);

    let availability = CheckAvailability {
        semantic: semantic_available,
        evolution: false,
        rules: rules_error.is_none(),
        changed_scope: true,
    };
    let errors = diagnostics_errors(rules_error.clone(), semantic_error.clone());
    let partial_results = !semantic_available || errors.values().any(Option::is_some);

    (
        issues,
        CheckDiagnostics {
            errors,
            warnings,
            partial_results,
            availability,
        },
    )
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

fn obligation_finding_severity(
    obligation: &crate::metrics::v2::ObligationReport,
) -> FindingSeverity {
    if obligation.kind == "closed_domain_exhaustiveness" || !obligation.missing_variants.is_empty()
    {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    }
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

fn obligation_finding_value(obligation: &crate::metrics::v2::ObligationReport) -> Value {
    let severity = obligation_finding_severity(obligation);
    json!({
        "kind": obligation.kind,
        "severity": severity,
        "concept_id": obligation.concept_id.clone().unwrap_or_else(|| obligation.domain_symbol_name.clone().unwrap_or_default()),
        "summary": obligation.summary,
        "files": obligation.files,
        "evidence": obligation.missing_sites.iter().map(|site| {
            let line_suffix = site.line.map(|line| format!(":{line}")).unwrap_or_default();
            format!("{}{} [{}]", site.path, line_suffix, site.detail)
        }).collect::<Vec<_>>(),
        "origin": if obligation.concept_id.is_some() { "explicit" } else { "zero_config" },
        "confidence": if obligation.concept_id.is_some() { "high" } else { "medium" },
        "line": obligation.missing_sites.iter().find_map(|site| site.line),
    })
}

fn build_changed_structural_finding_values(
    health: &metrics::HealthReport,
    changed_files: &BTreeSet<String>,
) -> Vec<Value> {
    let mut findings = Vec::new();
    for file in &health.long_files {
        if !changed_files.contains(&file.path) {
            continue;
        }
        findings.push(json!({
            "kind": "large_file",
            "severity": FindingSeverity::Medium,
            "concept_id": file.path,
            "summary": format!("{} grew to {} lines and should likely be split.", file.path, file.value),
            "files": [file.path.clone()],
            "evidence": [format!("line_count: {}", file.value)],
            "origin": "zero_config",
            "confidence": "medium",
        }));
    }
    for file in &health.god_files {
        if !changed_files.contains(&file.path) {
            continue;
        }
        findings.push(json!({
            "kind": "dependency_sprawl",
            "severity": FindingSeverity::Medium,
            "concept_id": file.path,
            "summary": format!("{} fans out across {} edges and is trending toward sprawl.", file.path, file.value),
            "files": [file.path.clone()],
            "evidence": [format!("fan_out: {}", file.value)],
            "origin": "zero_config",
            "confidence": "medium",
        }));
    }
    for file in &health.hotspot_files {
        if !changed_files.contains(&file.path) {
            continue;
        }
        findings.push(json!({
            "kind": "unstable_hotspot",
            "severity": FindingSeverity::Medium,
            "concept_id": file.path,
            "summary": format!("{} has high inbound pressure and should be stabilized before more changes.", file.path),
            "files": [file.path.clone()],
            "evidence": [format!("fan_in: {}", file.value)],
            "origin": "zero_config",
            "confidence": "medium",
        }));
    }
    for cycle in &health.circular_dep_files {
        if !cycle.iter().any(|path| changed_files.contains(path)) {
            continue;
        }
        findings.push(json!({
            "kind": "cycle_cluster",
            "severity": FindingSeverity::Medium,
            "concept_id": cycle.first().cloned().unwrap_or_default(),
            "summary": format!("Changed files participate in a dependency cycle spanning {} files.", cycle.len()),
            "files": cycle,
            "evidence": cycle.iter().map(|path| format!("cycle member: {path}")).collect::<Vec<_>>(),
            "origin": "zero_config",
            "confidence": "medium",
        }));
    }
    findings
}

fn cached_clone_finding_values(
    state: &McpState,
    session_v2: Option<&SessionV2Baseline>,
    changed_files: &BTreeSet<String>,
    expected_scan_identity: Option<&ScanCacheIdentity>,
) -> Vec<Value> {
    let Some(cached) = &state.cached_patch_safety else {
        return Vec::new();
    };
    let expected_signature = super::session_v2_analysis_signature(session_v2);
    if cached.session_signature != expected_signature
        || cached.scan_identity.as_ref() != expected_scan_identity
    {
        return Vec::new();
    }

    cached
        .changed_visible_findings
        .iter()
        .filter(|finding| {
            matches!(
                finding_kind(finding),
                "exact_clone_group" | "clone_group" | "clone_family"
            )
        })
        .filter(|finding| {
            finding_files(finding)
                .iter()
                .any(|path| changed_files.contains(path))
        })
        .cloned()
        .collect()
}
