use super::session::{
    build_patch_safety_analysis, current_patch_safety_cache_identity, patch_safety_semantic_error,
    prepare_patch_check_context,
};
use super::*;
use crate::metrics::v2::FindingSeverity;

pub fn cli_agent_brief(
    root: &Path,
    mode: &str,
    strict: bool,
    limit: usize,
) -> Result<Value, String> {
    let mut state = fresh_mcp_state();
    let path = root
        .to_str()
        .ok_or("Invalid path encoding for agent brief root")?;
    handle_scan(&json!({ "path": path }), &Tier::Free, &mut state)?;
    handle_agent_brief(
        &json!({
            "mode": mode,
            "strict": strict,
            "limit": limit,
        }),
        &Tier::Free,
        &mut state,
    )
}

pub fn agent_brief_def() -> ToolDef {
    ToolDef {
        name: "agent_brief",
        description: "Compose a structured guidance brief for coding agents from v2 findings, obligations, project shape, and patch-safety context.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["repo_onboarding", "patch", "pre_merge"],
                    "description": "Guidance mode. Defaults to patch."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of primary targets to include (default 3, max 10)."
                },
                "strict": {
                    "type": "boolean",
                    "description": "Pre-merge strictness. Reserved for pre_merge mode."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_agent_brief,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_agent_brief(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let mode = AgentBriefMode::parse(args.get("mode").and_then(|value| value.as_str()))?;
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(3)
        .clamp(1, 10) as usize;
    let strict = args.get("strict").and_then(|value| value.as_bool());

    match mode {
        AgentBriefMode::RepoOnboarding => handle_onboarding_agent_brief(state, limit, strict),
        AgentBriefMode::Patch => handle_patch_agent_brief(state, limit),
        AgentBriefMode::PreMerge => {
            handle_pre_merge_agent_brief(state, limit, strict.unwrap_or(false))
        }
    }
}

fn handle_onboarding_agent_brief(
    state: &mut McpState,
    limit: usize,
    strict: Option<bool>,
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
    let (rules_config, rules_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        &root,
        &snapshot,
        &health,
        health.duplicate_groups.len(),
        true,
    );
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        Some(&snapshot),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let structural_reports =
        crate::metrics::v2::build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let other_findings = combined_other_finding_values(&semantic_findings, &structural_reports);
    let merged_findings = merge_findings(
        clone_payload.prioritized_findings.clone(),
        other_findings,
        usize::MAX,
    );
    let (suppression_application, suppression_error) =
        apply_root_suppressions(state, &root, merged_findings);
    let (visible_findings, experimental_findings) = partition_review_surface_experimental_findings(
        &suppression_application.visible_findings,
        limit,
    );
    let visible_findings = visible_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let experimental_findings = experimental_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let visible_clone_ids = visible_clone_ids(&visible_findings);
    let clone_families = filter_clone_values_by_visible_clone_ids(
        clone_payload.families,
        &visible_clone_ids,
        "clone_ids",
        limit,
    );
    let debt_outputs = build_debt_report_outputs(
        state,
        &root,
        &snapshot,
        &health,
        &visible_findings,
        &obligations,
        &clone_families,
        &BTreeSet::new(),
        limit,
        false,
    );
    let debt_context_error = debt_outputs.context_error();
    let semantic_or_rules_error = merge_optional_errors(
        merge_optional_errors(rules_error.clone(), suppression_error),
        semantic_error,
    );
    let freshness = json!({
        "baseline_loaded": session_v2_status.loaded,
        "session_baseline_compatible": session_v2_status.compatible,
        "git_head": current_git_head(&root),
        "working_tree_path_count": working_tree_changed_files(&root).unwrap_or_default().len(),
    });

    let mut brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: project_shape_json_cached(state, &root, &snapshot, &rules_config),
        findings: visible_findings,
        experimental_findings,
        missing_obligations: Vec::new(),
        watchpoints: debt_outputs.serialized_watchpoints(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!(build_v2_confidence_report(
            &metadata,
            &rules_config,
            session_v2_status
        )),
        scan_trust: scan_trust_json(&metadata),
        freshness,
        strict,
        limit,
    })?;

    if let Some(object) = brief.as_object_mut() {
        object.insert(
            "semantic_cache".to_string(),
            semantic_cache_status_json(state),
        );
        object.insert(
            "suppression_hits".to_string(),
            json!(suppression_application.active_matches),
        );
        object.insert(
            "suppressed_finding_count".to_string(),
            json!(suppression_match_count(
                &suppression_application.active_matches
            )),
        );
        object.insert(
            "expired_suppressions".to_string(),
            json!(suppression_application.expired_matches),
        );
        object.insert(
            "expired_suppression_match_count".to_string(),
            json!(suppression_match_count(
                &suppression_application.expired_matches
            )),
        );
    }

    if let Some(object) = brief.as_object_mut() {
        insert_rules_semantic_context_diagnostics(
            object,
            rules_error,
            merge_optional_errors(semantic_or_rules_error, clone_error),
            debt_context_error.clone(),
        );
        extend_diagnostics_availability(
            object,
            vec![("evolution", state.cached_evolution.is_some())],
        );
    }

    Ok(brief)
}

fn handle_patch_agent_brief(state: &mut McpState, limit: usize) -> Result<Value, String> {
    build_patch_mode_agent_brief(state, AgentBriefMode::Patch, limit, false)
}

fn handle_pre_merge_agent_brief(
    state: &mut McpState,
    limit: usize,
    strict: bool,
) -> Result<Value, String> {
    build_patch_mode_agent_brief(state, AgentBriefMode::PreMerge, limit, strict)
}

fn build_patch_mode_agent_brief(
    state: &mut McpState,
    mode: AgentBriefMode,
    limit: usize,
    strict: bool,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, &root)?;
    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let known_empty_patch_scope =
        context.changed_scope_available && context.changed_files.is_empty();
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let reused_cached_scan = context.reused_cached_scan;
    let scan_identity = context.scan_identity.clone();
    let bundle = context.bundle;
    let changed_files = context.changed_files;
    let persisted_baseline = load_persisted_baseline(&root).ok().flatten();
    let (rules_config, rules_error) = load_v2_rules_config(state, &root);

    if !reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    if known_empty_patch_scope {
        let debt_outputs = build_debt_report_outputs(
            state,
            &root,
            &bundle.snapshot,
            &bundle.health,
            &[],
            &[],
            &[],
            &changed_files,
            limit.max(5),
            false,
        );
        let freshness = json!({
            "baseline_loaded": session_v2_status.loaded,
            "session_baseline_compatible": session_v2_status.compatible,
            "git_head": current_git_head(&root),
            "working_tree_path_count": 0,
        });
        let mut brief = build_agent_brief(AgentBriefInput {
            mode,
            repo_shape: project_shape_json_cached(state, &root, &bundle.snapshot, &rules_config),
            findings: Vec::new(),
            experimental_findings: Vec::new(),
            missing_obligations: Vec::new(),
            watchpoints: debt_outputs.serialized_watchpoints(),
            resolved_findings: Vec::new(),
            changed_files: Vec::new(),
            changed_concepts: Vec::new(),
            decision: Some("pass".to_string()),
            summary: None,
            confidence: json!(build_v2_confidence_report(
                &bundle.metadata,
                &rules_config,
                session_v2_status
            )),
            scan_trust: scan_trust_json(&bundle.metadata),
            freshness,
            strict: Some(strict),
            limit,
        })?;
        let debt_context_error = debt_outputs.context_error();

        if let Some(object) = brief.as_object_mut() {
            object.insert(
                "semantic_cache".to_string(),
                semantic_cache_status_json(state),
            );
            object.insert("changed_files".to_string(), json!(Vec::<String>::new()));
            object.insert("changed_concepts".to_string(), json!(Vec::<String>::new()));
            object.insert("introduced_finding_count".to_string(), json!(0));
            object.insert(
                "introduced_findings".to_string(),
                json!(Vec::<Value>::new()),
            );
            object.insert("introduced_clone_finding_count".to_string(), json!(0));
            object.insert(
                "introduced_clone_findings".to_string(),
                json!(Vec::<Value>::new()),
            );
            object.insert("blocking_finding_count".to_string(), json!(0));
            object.insert("blocking_findings".to_string(), json!(Vec::<Value>::new()));
            object.insert(
                "touched_concept_gate".to_string(),
                json!({
                    "decision": "pass",
                    "strict": strict,
                }),
            );
            object.insert("suppression_hits".to_string(), json!(Vec::<Value>::new()));
            object.insert("suppressed_finding_count".to_string(), json!(0));
            object.insert(
                "expired_suppressions".to_string(),
                json!(Vec::<Value>::new()),
            );
            object.insert("expired_suppression_match_count".to_string(), json!(0));
        }

        if let Some(object) = brief.as_object_mut() {
            insert_rules_semantic_context_diagnostics(
                object,
                rules_error,
                None,
                debt_context_error,
            );
            extend_diagnostics_availability(
                object,
                vec![("evolution", state.cached_evolution.is_some())],
            );
        }

        if !reused_cached_scan {
            let preserved_semantic = state.cached_semantic.take();
            let preserved_evolution = state.cached_evolution.take();
            let preserved_patch_safety = state.cached_patch_safety.take();
            update_scan_cache(
                state,
                root,
                bundle,
                persisted_baseline.or(state.baseline.clone()),
                scan_identity,
            );
            state.cached_semantic = preserved_semantic;
            state.cached_evolution = preserved_evolution;
            state.cached_patch_safety = preserved_patch_safety;
        } else if persisted_baseline.is_some() {
            state.baseline = persisted_baseline;
        }

        return Ok(brief);
    }

    let analysis = build_patch_safety_analysis(
        state,
        &root,
        &bundle,
        &changed_files,
        session_v2.as_ref(),
        patch_cache_identity,
        false,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let changed_concepts = analysis
        .changed_touched_concepts
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let missing_obligations = analysis
        .changed_obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    let introduced_findings = session_v2
        .as_ref()
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| analysis.changed_visible_findings.clone());
    let introduced_findings = merge_session_introduced_clone_findings(
        introduced_findings,
        &analysis.visible_findings,
        session_v2.as_ref(),
        &changed_files,
        limit.max(10),
    );
    let (visible_introduced_findings, experimental_introduced_findings) =
        partition_review_surface_experimental_findings(&introduced_findings, limit.max(10));

    let blocking_findings = visible_introduced_findings
        .iter()
        .filter(|finding| {
            let severity = severity_of_value(finding);
            severity == FindingSeverity::High
                || (mode == AgentBriefMode::PreMerge
                    && strict
                    && severity == FindingSeverity::Medium)
        })
        .cloned()
        .collect::<Vec<_>>();
    let resolved_findings = session_v2
        .as_ref()
        .map(|session_v2| {
            session_v2
                .finding_payloads
                .iter()
                .filter(|(key, _)| !current_finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let resolved_findings = resolved_findings
        .into_iter()
        .filter(|finding| !is_experimental_finding(finding))
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();

    let candidate_findings = visible_introduced_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let introduced_clone_findings = candidate_findings
        .iter()
        .filter(|finding| is_agent_clone_signal_kind(finding_kind(finding)))
        .cloned()
        .collect::<Vec<_>>();
    let experimental_findings = experimental_introduced_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let blocking_findings = blocking_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let debt_outputs = build_debt_report_outputs(
        state,
        &root,
        &bundle.snapshot,
        &bundle.health,
        &candidate_findings,
        &analysis.changed_obligations,
        &[],
        &changed_files,
        limit.max(5),
        false,
    );
    let gate_decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else if candidate_findings.is_empty() {
        "pass"
    } else {
        "warn"
    };
    let semantic_error = patch_safety_semantic_error(&analysis);
    let freshness = json!({
        "baseline_loaded": session_v2_status.loaded,
        "session_baseline_compatible": session_v2_status.compatible,
        "git_head": current_git_head(&root),
        "working_tree_path_count": changed_files.len(),
    });
    let mut brief = build_agent_brief(AgentBriefInput {
        mode,
        repo_shape: project_shape_json_cached(state, &root, &bundle.snapshot, &rules_config),
        findings: candidate_findings.clone(),
        experimental_findings,
        missing_obligations: serialized_values(&missing_obligations),
        watchpoints: debt_outputs.serialized_watchpoints(),
        resolved_findings: resolved_findings.clone(),
        changed_files: changed_files.iter().cloned().collect(),
        changed_concepts: changed_concepts.clone(),
        decision: Some(gate_decision.to_string()),
        summary: None,
        confidence: json!(build_v2_confidence_report(
            &bundle.metadata,
            &rules_config,
            session_v2_status
        )),
        scan_trust: scan_trust_json(&bundle.metadata),
        freshness,
        strict: Some(strict),
        limit,
    })?;
    let debt_context_error = debt_outputs.context_error();

    if let Some(object) = brief.as_object_mut() {
        object.insert(
            "semantic_cache".to_string(),
            semantic_cache_status_json(state),
        );
        object.insert(
            "changed_files".to_string(),
            json!(changed_files.iter().cloned().collect::<Vec<_>>()),
        );
        object.insert("changed_concepts".to_string(), json!(changed_concepts));
        object.insert(
            "introduced_finding_count".to_string(),
            json!(candidate_findings.len()),
        );
        object.insert("introduced_findings".to_string(), json!(candidate_findings));
        object.insert(
            "introduced_clone_finding_count".to_string(),
            json!(introduced_clone_findings.len()),
        );
        object.insert(
            "introduced_clone_findings".to_string(),
            json!(introduced_clone_findings),
        );
        object.insert(
            "blocking_finding_count".to_string(),
            json!(blocking_findings.len()),
        );
        object.insert("blocking_findings".to_string(), json!(blocking_findings));
        object.insert(
            "touched_concept_gate".to_string(),
            json!({
                "decision": gate_decision,
                "strict": strict,
            }),
        );
        object.insert(
            "suppression_hits".to_string(),
            json!(analysis.suppression_hits),
        );
        object.insert(
            "suppressed_finding_count".to_string(),
            json!(analysis.suppressed_finding_count),
        );
        object.insert(
            "expired_suppressions".to_string(),
            json!(analysis.expired_suppressions),
        );
        object.insert(
            "expired_suppression_match_count".to_string(),
            json!(analysis.expired_suppression_match_count),
        );
    }

    if let Some(object) = brief.as_object_mut() {
        insert_rules_semantic_context_diagnostics(
            object,
            analysis.rules_error,
            semantic_error,
            merge_optional_errors(rules_error, debt_context_error.clone()),
        );
        extend_diagnostics_availability(
            object,
            vec![("evolution", state.cached_evolution.is_some())],
        );
    }

    if !reused_cached_scan {
        let preserved_semantic = state.cached_semantic.take();
        let preserved_evolution = state.cached_evolution.take();
        let preserved_patch_safety = state.cached_patch_safety.take();
        update_scan_cache(
            state,
            root,
            bundle,
            persisted_baseline.or(state.baseline.clone()),
            scan_identity,
        );
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }

    Ok(brief)
}
