use super::clone_support::CloneFindingPayload;
use super::semantic_batch::ChangedPatchScope;
use super::*;
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) struct PatchCheckContext {
    pub(crate) bundle: ScanBundle,
    pub(crate) changed_files: BTreeSet<String>,
    pub(crate) changed_scope_available: bool,
    pub(crate) reused_cached_scan: bool,
    pub(crate) scan_identity: Option<ScanCacheIdentity>,
}

pub(crate) fn prepare_patch_check_context(
    state: &McpState,
    root: &Path,
    session_v2: Option<&SessionV2Baseline>,
) -> Result<PatchCheckContext, String> {
    if let Some(bundle) = cached_scan_bundle(state, root) {
        let current_identity = current_scan_identity(root);
        let changed_scope = changed_scope_from_session_context(
            root,
            &bundle.snapshot,
            session_v2,
            current_identity.as_ref(),
        );
        let changed_files = changed_scope.known_files();
        if changed_files.is_empty() || scan_cache_matches_identity(state, current_identity.as_ref())
        {
            return Ok(PatchCheckContext {
                bundle,
                changed_files,
                changed_scope_available: changed_scope.is_available(),
                reused_cached_scan: true,
                scan_identity: None,
            });
        }
    }

    let (bundle, scan_identity) = do_scan_with_identity(root)?;
    let changed_scope =
        changed_scope_from_session_context(root, &bundle.snapshot, session_v2, None);

    Ok(PatchCheckContext {
        bundle,
        changed_files: changed_scope.known_files(),
        changed_scope_available: changed_scope.is_available(),
        reused_cached_scan: false,
        scan_identity,
    })
}

pub(crate) fn current_patch_safety_cache_identity(
    state: &McpState,
    context: &PatchCheckContext,
) -> Option<ScanCacheIdentity> {
    if context.reused_cached_scan {
        state.cached_scan_identity.clone()
    } else {
        context.scan_identity.clone()
    }
}

fn cached_patch_safety_analysis(
    state: &McpState,
    scan_identity: Option<&ScanCacheIdentity>,
    session_signature: Option<u64>,
    allow_cold_evolution: bool,
) -> Option<PatchSafetyAnalysisCache> {
    let scan_identity = scan_identity?;
    let cached = state.cached_patch_safety.as_ref()?;
    if cached.scan_identity.as_ref() == Some(scan_identity)
        && cached.session_signature == session_signature
        && cached.allow_cold_evolution == allow_cold_evolution
    {
        return Some(cached.clone());
    }

    None
}

fn cache_patch_safety_analysis(state: &mut McpState, analysis: &PatchSafetyAnalysisCache) {
    if analysis.scan_identity.is_some() {
        state.cached_patch_safety = Some(analysis.clone());
    }
}

pub(crate) fn has_known_empty_patch_scope(context: &PatchCheckContext) -> bool {
    context.changed_scope_available && context.changed_files.is_empty()
}

pub(crate) fn patch_safety_semantic_error(analysis: &PatchSafetyAnalysisCache) -> Option<String> {
    merge_optional_errors(
        analysis
            .changed_semantic_error
            .clone()
            .or(analysis.all_semantic_error.clone()),
        analysis.clone_error.clone(),
    )
}

pub(crate) fn build_patch_safety_analysis(
    state: &mut McpState,
    root: &Path,
    bundle: &ScanBundle,
    changed_files: &BTreeSet<String>,
    session_v2: Option<&SessionV2Baseline>,
    cache_identity: Option<ScanCacheIdentity>,
    allow_cold_evolution: bool,
) -> PatchSafetyAnalysisCache {
    let session_signature = session_v2_analysis_signature(session_v2);
    if let Some(cached) = cached_patch_safety_analysis(
        state,
        cache_identity.as_ref(),
        session_signature,
        allow_cold_evolution,
    ) {
        return cached;
    }

    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        root,
        &bundle.snapshot,
        &bundle.health,
        bundle.health.duplicate_groups.len(),
        allow_cold_evolution,
    );
    let (rules_config, rules_error) = load_v2_rules_config(state, root);
    let structural_reports = filter_structural_reports_by_rules(
        crate::metrics::v2::build_structural_debt_reports_with_root(
            root,
            &bundle.snapshot,
            &bundle.health,
        ),
        &rules_config,
    );
    let semantic = match analyze_patch_safety_semantic_snapshot(state, root) {
        Ok(semantic) => semantic,
        Err(error) => {
            let analysis = build_patch_safety_semantic_unavailable_analysis(
                &rules_config,
                &clone_payload,
                &structural_reports,
                clone_error,
                rules_error,
                error,
                cache_identity.clone(),
                session_signature,
                allow_cold_evolution,
            );
            cache_patch_safety_analysis(state, &analysis);
            return analysis;
        }
    };

    let changed_scope = analyze_changed_patch_scope(
        state,
        root,
        &rules_config,
        Some(&bundle.snapshot),
        semantic.as_ref(),
        changed_files,
    );
    let suppression_application = build_patch_safety_suppression_application(
        &rules_config,
        semantic.as_ref(),
        &bundle.snapshot,
        &clone_payload,
        &structural_reports,
    );
    let analysis = build_patch_safety_cache(
        suppression_application,
        changed_scope,
        clone_error,
        rules_error,
        cache_identity,
        session_signature,
        allow_cold_evolution,
    );

    cache_patch_safety_analysis(state, &analysis);

    analysis
}

fn build_patch_safety_suppression_application(
    rules_config: &crate::metrics::rules::RulesConfig,
    semantic: Option<&crate::analysis::semantic::SemanticSnapshot>,
    snapshot: &Snapshot,
    clone_payload: &CloneFindingPayload,
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
) -> SuppressionApplication {
    let all_analysis = semantic
        .map(|semantic| {
            build_semantic_analysis_batch(
                rules_config,
                semantic,
                Some(snapshot),
                crate::metrics::v2::ObligationScope::All,
                &BTreeSet::new(),
            )
        })
        .unwrap_or_default();
    let all_finding_values =
        combined_other_finding_values(&all_analysis.findings, structural_reports);
    apply_suppressions(
        rules_config,
        finding_values(&clone_payload.exact_findings, &all_finding_values),
    )
}

fn build_patch_safety_cache(
    suppression_application: SuppressionApplication,
    changed_scope: ChangedPatchScope,
    clone_error: Option<String>,
    rules_error: Option<String>,
    scan_identity: Option<ScanCacheIdentity>,
    session_signature: Option<u64>,
    allow_cold_evolution: bool,
) -> PatchSafetyAnalysisCache {
    PatchSafetyAnalysisCache {
        scan_identity,
        session_signature,
        allow_cold_evolution,
        visible_findings: suppression_application.visible_findings,
        suppression_hits: serialized_values(&suppression_application.active_matches),
        suppressed_finding_count: suppression_match_count(&suppression_application.active_matches),
        expired_suppressions: serialized_values(&suppression_application.expired_matches),
        expired_suppression_match_count: suppression_match_count(
            &suppression_application.expired_matches,
        ),
        changed_visible_findings: changed_scope
            .suppression_application
            .visible_findings
            .clone(),
        changed_obligations: changed_scope.obligations.clone(),
        changed_touched_concepts: changed_scope.touched_concepts.clone(),
        clone_error,
        all_semantic_error: rules_error.clone(),
        changed_semantic_error: merge_optional_errors(
            rules_error.clone(),
            changed_scope.semantic_error.clone(),
        ),
        rules_error,
    }
}

fn analyze_patch_safety_semantic_snapshot(
    state: &mut McpState,
    root: &Path,
) -> Result<Option<crate::analysis::semantic::SemanticSnapshot>, String> {
    analyze_semantic_snapshot(state, root)
}

fn build_patch_safety_semantic_unavailable_analysis(
    rules_config: &crate::metrics::rules::RulesConfig,
    clone_payload: &CloneFindingPayload,
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
    clone_error: Option<String>,
    rules_error: Option<String>,
    semantic_error: String,
    scan_identity: Option<ScanCacheIdentity>,
    session_signature: Option<u64>,
    allow_cold_evolution: bool,
) -> PatchSafetyAnalysisCache {
    let suppression_application = apply_suppressions(
        rules_config,
        finding_values(
            &clone_payload.exact_findings,
            &serialized_values(structural_reports),
        ),
    );
    PatchSafetyAnalysisCache {
        scan_identity,
        session_signature,
        allow_cold_evolution,
        visible_findings: suppression_application.visible_findings,
        suppression_hits: serialized_values(&suppression_application.active_matches),
        suppressed_finding_count: suppression_match_count(&suppression_application.active_matches),
        expired_suppressions: serialized_values(&suppression_application.expired_matches),
        expired_suppression_match_count: suppression_match_count(
            &suppression_application.expired_matches,
        ),
        clone_error,
        all_semantic_error: merge_optional_errors(
            rules_error.clone(),
            Some(semantic_error.clone()),
        ),
        changed_semantic_error: merge_optional_errors(rules_error.clone(), Some(semantic_error)),
        rules_error,
        ..PatchSafetyAnalysisCache::default()
    }
}
