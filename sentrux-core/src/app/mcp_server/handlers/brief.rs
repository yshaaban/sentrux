use super::checkpoint::SessionBaselineStatus;
use super::debt::DebtReportOutputs;
use super::session::{
    build_patch_safety_analysis, current_patch_safety_cache_identity, has_known_empty_patch_scope,
    patch_safety_semantic_error, prepare_patch_check_context,
};
use super::*;
use crate::analysis::inferred_rules::{merge_inferred_rules, InferredRulesSummary};
use crate::metrics::v2::FindingSeverity;
use std::collections::BTreeMap;

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
    let context = load_onboarding_brief_context(state)?;
    let surface = build_onboarding_brief_surface(state, &context, limit);
    let mut brief = build_onboarding_agent_brief_value(state, &context, &surface, strict, limit)?;
    if let Some(object) = brief.as_object_mut() {
        insert_onboarding_brief_support_fields(
            object,
            state,
            &surface.suppression_application,
            &context.inferred_rules,
        );
        insert_rules_semantic_context_diagnostics(
            object,
            context.rules_error.clone(),
            merge_optional_errors(
                surface.semantic_or_rules_error.clone(),
                surface.clone_error.clone(),
            ),
            surface.debt_context_error.clone(),
        );
        extend_diagnostics_availability(
            object,
            vec![("evolution", state.cached_evolution.is_some())],
        );
    }

    Ok(brief)
}

struct OnboardingBriefContext {
    health: metrics::HealthReport,
    snapshot: std::sync::Arc<Snapshot>,
    root: PathBuf,
    metadata: crate::analysis::scanner::common::ScanMetadata,
    rules_config: crate::metrics::rules::RulesConfig,
    rules_error: Option<String>,
    inferred_rules: InferredRulesSummary,
    session_v2_status: SessionBaselineStatus,
}

fn load_onboarding_brief_context(state: &mut McpState) -> Result<OnboardingBriefContext, String> {
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
    let (configured_rules, rules_error) = load_v2_rules_config(state, &root);
    let project_shape_report =
        project_shape_report_cached(state, &root, &snapshot, &configured_rules);
    let semantic = match analyze_semantic_snapshot(state, &root) {
        Ok(snapshot) => snapshot,
        Err(_) => None,
    };
    let (rules_config, inferred_rules) =
        merge_inferred_rules(&configured_rules, &project_shape_report, semantic.as_ref());
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    Ok(OnboardingBriefContext {
        health,
        snapshot,
        root,
        metadata,
        rules_config,
        rules_error,
        inferred_rules,
        session_v2_status,
    })
}

struct OnboardingBriefSurface {
    suppression_application: SuppressionApplication,
    clone_error: Option<String>,
    semantic_or_rules_error: Option<String>,
    debt_context_error: Option<String>,
    visible_findings: Vec<Value>,
    experimental_findings: Vec<Value>,
    debt_outputs: DebtReportOutputs,
}

fn build_onboarding_brief_surface(
    state: &mut McpState,
    context: &OnboardingBriefContext,
    limit: usize,
) -> OnboardingBriefSurface {
    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        &context.root,
        &context.snapshot,
        &context.health,
        context.health.duplicate_groups.len(),
        true,
    );
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &context.root,
        Some(&context.snapshot),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let structural_reports = crate::metrics::v2::build_structural_debt_reports_with_root(
        &context.root,
        &context.snapshot,
        &context.health,
    );
    let other_findings = combined_other_finding_values(&semantic_findings, &structural_reports);
    let merged_findings = merge_findings(
        clone_payload.prioritized_findings.clone(),
        other_findings,
        usize::MAX,
    );
    let (suppression_application, suppression_error) =
        apply_root_suppressions(state, &context.root, merged_findings);
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
        &context.root,
        &context.snapshot,
        &context.health,
        &visible_findings,
        &obligations,
        &clone_families,
        &BTreeSet::new(),
        limit,
        false,
    );
    let semantic_or_rules_error = merge_optional_errors(
        merge_optional_errors(context.rules_error.clone(), suppression_error),
        semantic_error,
    );

    OnboardingBriefSurface {
        suppression_application,
        clone_error,
        semantic_or_rules_error,
        debt_context_error: debt_outputs.context_error(),
        visible_findings,
        experimental_findings,
        debt_outputs,
    }
}

fn build_onboarding_agent_brief_value(
    state: &mut McpState,
    context: &OnboardingBriefContext,
    surface: &OnboardingBriefSurface,
    strict: Option<bool>,
    limit: usize,
) -> Result<Value, String> {
    build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: project_shape_json_cached(
            state,
            &context.root,
            &context.snapshot,
            &context.rules_config,
        ),
        findings: surface.visible_findings.clone(),
        experimental_findings: surface.experimental_findings.clone(),
        missing_obligations: Vec::new(),
        watchpoints: surface.debt_outputs.serialized_watchpoints(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!(build_v2_confidence_report(
            &context.metadata,
            &context.rules_config,
            context.session_v2_status.clone()
        )),
        scan_trust: scan_trust_json(&context.metadata),
        freshness: onboarding_brief_freshness(context),
        strict,
        limit,
    })
}

fn onboarding_brief_freshness(context: &OnboardingBriefContext) -> Value {
    json!({
        "baseline_loaded": context.session_v2_status.loaded,
        "session_baseline_compatible": context.session_v2_status.compatible,
        "git_head": current_git_head(&context.root),
        "working_tree_path_count": working_tree_changed_files(&context.root).unwrap_or_default().len(),
    })
}

fn insert_onboarding_brief_support_fields(
    object: &mut serde_json::Map<String, Value>,
    state: &McpState,
    suppression_application: &SuppressionApplication,
    inferred_rules: &InferredRulesSummary,
) {
    object.insert(
        "semantic_cache".to_string(),
        semantic_cache_status_json(state),
    );
    object.insert("inferred_rules".to_string(), json!(inferred_rules));
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
    let context = load_patch_brief_context(state)?;
    reset_patch_brief_runtime_caches(state, context.reused_cached_scan);
    let brief = if context.known_empty_patch_scope {
        build_empty_patch_mode_agent_brief(state, &context, mode, limit, strict)?
    } else {
        build_active_patch_mode_brief(state, &context, mode, limit, strict)?
    };
    finalize_patch_brief_scan_cache(
        state,
        context.root,
        context.bundle,
        context.reused_cached_scan,
        context.scan_identity,
    );
    Ok(brief)
}

struct PatchBriefContext {
    root: PathBuf,
    session_v2: Option<SessionV2Baseline>,
    session_v2_status: SessionBaselineStatus,
    bundle: ScanBundle,
    changed_files: BTreeSet<String>,
    patch_cache_identity: Option<ScanCacheIdentity>,
    reused_cached_scan: bool,
    scan_identity: Option<ScanCacheIdentity>,
    rules_config: crate::metrics::rules::RulesConfig,
    rules_error: Option<String>,
    known_empty_patch_scope: bool,
}

fn load_patch_brief_context(state: &mut McpState) -> Result<PatchBriefContext, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, &root)?;
    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let reused_cached_scan = context.reused_cached_scan;
    let scan_identity = context.scan_identity.clone();
    let known_empty_patch_scope = has_known_empty_patch_scope(&context);
    let bundle = context.bundle;
    let changed_files = context.changed_files;
    let (rules_config, rules_error) = load_v2_rules_config(state, &root);
    Ok(PatchBriefContext {
        root,
        session_v2,
        session_v2_status,
        bundle,
        changed_files,
        patch_cache_identity,
        reused_cached_scan,
        scan_identity,
        rules_config,
        rules_error,
        known_empty_patch_scope,
    })
}

fn reset_patch_brief_runtime_caches(state: &mut McpState, reused_cached_scan: bool) {
    if !reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
}

fn finalize_patch_brief_scan_cache(
    state: &mut McpState,
    root: PathBuf,
    bundle: ScanBundle,
    reused_cached_scan: bool,
    scan_identity: Option<ScanCacheIdentity>,
) {
    let persisted_baseline = load_persisted_baseline(&root).ok().flatten();
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
}

fn build_empty_patch_mode_agent_brief(
    state: &mut McpState,
    context: &PatchBriefContext,
    mode: AgentBriefMode,
    limit: usize,
    strict: bool,
) -> Result<Value, String> {
    let debt_outputs = build_debt_report_outputs(
        state,
        &context.root,
        &context.bundle.snapshot,
        &context.bundle.health,
        &[],
        &[],
        &[],
        &context.changed_files,
        limit.max(5),
        false,
    );
    let mut brief = build_agent_brief(AgentBriefInput {
        mode,
        repo_shape: project_shape_json_cached(
            state,
            &context.root,
            &context.bundle.snapshot,
            &context.rules_config,
        ),
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
            &context.bundle.metadata,
            &context.rules_config,
            context.session_v2_status.clone()
        )),
        scan_trust: scan_trust_json(&context.bundle.metadata),
        freshness: patch_brief_freshness(context, 0),
        strict: Some(strict),
        limit,
    })?;
    if let Some(object) = brief.as_object_mut() {
        insert_empty_patch_brief_support_fields(object, state, strict);
        insert_rules_semantic_context_diagnostics(
            object,
            context.rules_error.clone(),
            None,
            debt_outputs.context_error(),
        );
        extend_diagnostics_availability(
            object,
            vec![("evolution", state.cached_evolution.is_some())],
        );
    }
    Ok(brief)
}

fn insert_empty_patch_brief_support_fields(
    object: &mut serde_json::Map<String, Value>,
    state: &McpState,
    strict: bool,
) {
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

struct ActivePatchBriefSurface {
    analysis: PatchSafetyAnalysisCache,
    changed_concepts: Vec<String>,
    missing_obligations: Vec<crate::metrics::v2::ObligationReport>,
    candidate_findings: Vec<Value>,
    experimental_findings: Vec<Value>,
    introduced_clone_findings: Vec<Value>,
    blocking_findings: Vec<Value>,
    resolved_findings: Vec<Value>,
    debt_outputs: DebtReportOutputs,
    gate_decision: &'static str,
    semantic_error: Option<String>,
}

fn build_active_patch_mode_brief(
    state: &mut McpState,
    context: &PatchBriefContext,
    mode: AgentBriefMode,
    limit: usize,
    strict: bool,
) -> Result<Value, String> {
    let surface = build_active_patch_brief_surface(state, context, mode, limit, strict);
    let mut brief = build_agent_brief(AgentBriefInput {
        mode,
        repo_shape: project_shape_json_cached(
            state,
            &context.root,
            &context.bundle.snapshot,
            &context.rules_config,
        ),
        findings: surface.candidate_findings.clone(),
        experimental_findings: surface.experimental_findings.clone(),
        missing_obligations: serialized_values(&surface.missing_obligations),
        watchpoints: surface.debt_outputs.serialized_watchpoints(),
        resolved_findings: surface.resolved_findings.clone(),
        changed_files: context.changed_files.iter().cloned().collect(),
        changed_concepts: surface.changed_concepts.clone(),
        decision: Some(surface.gate_decision.to_string()),
        summary: None,
        confidence: json!(build_v2_confidence_report(
            &context.bundle.metadata,
            &context.rules_config,
            context.session_v2_status.clone()
        )),
        scan_trust: scan_trust_json(&context.bundle.metadata),
        freshness: patch_brief_freshness(context, context.changed_files.len()),
        strict: Some(strict),
        limit,
    })?;
    if let Some(object) = brief.as_object_mut() {
        insert_active_patch_brief_support_fields(object, state, context, &surface, strict);
        insert_rules_semantic_context_diagnostics(
            object,
            surface.analysis.rules_error.clone(),
            surface.semantic_error.clone(),
            merge_optional_errors(
                context.rules_error.clone(),
                surface.debt_outputs.context_error(),
            ),
        );
        extend_diagnostics_availability(
            object,
            vec![("evolution", state.cached_evolution.is_some())],
        );
    }
    Ok(brief)
}

fn build_active_patch_brief_surface(
    state: &mut McpState,
    context: &PatchBriefContext,
    mode: AgentBriefMode,
    limit: usize,
    strict: bool,
) -> ActivePatchBriefSurface {
    let analysis = build_patch_safety_analysis(
        state,
        &context.root,
        &context.bundle,
        &context.changed_files,
        context.session_v2.as_ref(),
        context.patch_cache_identity.clone(),
        false,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let changed_concepts = analysis.changed_touched_concepts.iter().cloned().collect();
    let missing_obligations = analysis
        .changed_obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let introduced_findings =
        patch_brief_introduced_findings(&analysis, &current_finding_payloads, context, limit);
    let (candidate_findings, experimental_findings, blocking_findings, introduced_clone_findings) =
        classify_patch_brief_findings(introduced_findings, mode, strict, limit);
    let resolved_findings =
        patch_brief_resolved_findings(context.session_v2.as_ref(), &current_finding_payloads);
    let debt_outputs = build_debt_report_outputs(
        state,
        &context.root,
        &context.bundle.snapshot,
        &context.bundle.health,
        &candidate_findings,
        &analysis.changed_obligations,
        &[],
        &context.changed_files,
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
    ActivePatchBriefSurface {
        semantic_error: patch_safety_semantic_error(&analysis),
        analysis,
        changed_concepts,
        missing_obligations,
        candidate_findings,
        experimental_findings,
        introduced_clone_findings,
        blocking_findings,
        resolved_findings,
        debt_outputs,
        gate_decision,
    }
}

fn patch_brief_introduced_findings(
    analysis: &PatchSafetyAnalysisCache,
    current_finding_payloads: &BTreeMap<String, Value>,
    context: &PatchBriefContext,
    limit: usize,
) -> Vec<Value> {
    let introduced_findings = context
        .session_v2
        .as_ref()
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| analysis.changed_visible_findings.clone());
    merge_session_introduced_clone_findings(
        introduced_findings,
        &analysis.visible_findings,
        context.session_v2.as_ref(),
        &context.changed_files,
        limit.max(10),
    )
}

fn classify_patch_brief_findings(
    introduced_findings: Vec<Value>,
    mode: AgentBriefMode,
    strict: bool,
    limit: usize,
) -> (Vec<Value>, Vec<Value>, Vec<Value>, Vec<Value>) {
    let (visible_findings, experimental_findings) =
        partition_review_surface_experimental_findings(&introduced_findings, limit.max(10));
    let blocking_findings = visible_findings
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
    let candidate_findings = visible_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let introduced_clone_findings = candidate_findings
        .iter()
        .filter(|finding| is_agent_clone_signal_kind(finding_kind(finding)))
        .cloned()
        .collect::<Vec<_>>();
    (
        candidate_findings,
        experimental_findings
            .into_iter()
            .map(|finding| decorate_finding_with_classification(&finding))
            .collect::<Vec<_>>(),
        blocking_findings
            .into_iter()
            .map(|finding| decorate_finding_with_classification(&finding))
            .collect::<Vec<_>>(),
        introduced_clone_findings,
    )
}

fn patch_brief_resolved_findings(
    session_v2: Option<&SessionV2Baseline>,
    current_finding_payloads: &BTreeMap<String, Value>,
) -> Vec<Value> {
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
        .collect::<Vec<_>>()
}

fn insert_active_patch_brief_support_fields(
    object: &mut serde_json::Map<String, Value>,
    state: &McpState,
    context: &PatchBriefContext,
    surface: &ActivePatchBriefSurface,
    strict: bool,
) {
    object.insert(
        "semantic_cache".to_string(),
        semantic_cache_status_json(state),
    );
    object.insert(
        "changed_files".to_string(),
        json!(context.changed_files.iter().cloned().collect::<Vec<_>>()),
    );
    object.insert(
        "changed_concepts".to_string(),
        json!(surface.changed_concepts),
    );
    object.insert(
        "introduced_finding_count".to_string(),
        json!(surface.candidate_findings.len()),
    );
    object.insert(
        "introduced_findings".to_string(),
        json!(surface.candidate_findings),
    );
    object.insert(
        "introduced_clone_finding_count".to_string(),
        json!(surface.introduced_clone_findings.len()),
    );
    object.insert(
        "introduced_clone_findings".to_string(),
        json!(surface.introduced_clone_findings),
    );
    object.insert(
        "blocking_finding_count".to_string(),
        json!(surface.blocking_findings.len()),
    );
    object.insert(
        "blocking_findings".to_string(),
        json!(surface.blocking_findings),
    );
    object.insert(
        "touched_concept_gate".to_string(),
        json!({
            "decision": surface.gate_decision,
            "strict": strict,
        }),
    );
    object.insert(
        "suppression_hits".to_string(),
        json!(surface.analysis.suppression_hits),
    );
    object.insert(
        "suppressed_finding_count".to_string(),
        json!(surface.analysis.suppressed_finding_count),
    );
    object.insert(
        "expired_suppressions".to_string(),
        json!(surface.analysis.expired_suppressions),
    );
    object.insert(
        "expired_suppression_match_count".to_string(),
        json!(surface.analysis.expired_suppression_match_count),
    );
}

fn patch_brief_freshness(context: &PatchBriefContext, working_tree_path_count: usize) -> Value {
    json!({
        "baseline_loaded": context.session_v2_status.loaded,
        "session_baseline_compatible": context.session_v2_status.compatible,
        "git_head": current_git_head(&context.root),
        "working_tree_path_count": working_tree_path_count,
    })
}
