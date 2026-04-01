use super::*;

pub(crate) fn semantic_findings_and_obligations(
    state: &mut McpState,
    root: &Path,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> (
    Vec<crate::metrics::v2::SemanticFinding>,
    Vec<crate::metrics::v2::ObligationReport>,
    Option<String>,
) {
    let (analysis, error) = semantic_analysis_batch(state, root, snapshot, scope, changed_files);
    (analysis.findings, analysis.obligations, error)
}

#[derive(Default)]
pub(crate) struct SemanticAnalysisBatch {
    pub(crate) findings: Vec<crate::metrics::v2::SemanticFinding>,
    pub(crate) obligations: Vec<crate::metrics::v2::ObligationReport>,
    pub(crate) state_reports: Vec<crate::metrics::v2::StateIntegrityReport>,
}

fn semantic_analysis_batch(
    state: &mut McpState,
    root: &Path,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> (SemanticAnalysisBatch, Option<String>) {
    let (config, config_error) = load_v2_rules_config(state, root);
    match analyze_semantic_snapshot(state, root) {
        Ok(Some(semantic)) => (
            build_semantic_analysis_batch(&config, &semantic, snapshot, scope, changed_files),
            config_error,
        ),
        Ok(None) => (SemanticAnalysisBatch::default(), config_error),
        Err(error) => (
            SemanticAnalysisBatch::default(),
            merge_optional_errors(config_error, Some(error)),
        ),
    }
}

pub(crate) fn build_semantic_analysis_batch(
    config: &crate::metrics::rules::RulesConfig,
    semantic: &SemanticSnapshot,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> SemanticAnalysisBatch {
    let mut findings = crate::metrics::v2::build_authority_and_access_findings_with_snapshot(
        config, semantic, snapshot,
    );
    let obligations = crate::metrics::v2::build_obligations(config, semantic, scope, changed_files);
    findings.extend(crate::metrics::v2::build_obligation_findings(&obligations));
    let state_scope = if scope == crate::metrics::v2::ObligationScope::Changed {
        crate::metrics::v2::StateScope::Changed
    } else {
        crate::metrics::v2::StateScope::All
    };
    let state_reports = crate::metrics::v2::build_state_integrity_reports(
        config,
        semantic,
        &obligations,
        state_scope,
        changed_files,
    );
    findings.extend(crate::metrics::v2::build_state_integrity_findings(
        &state_reports,
    ));

    SemanticAnalysisBatch {
        findings,
        obligations,
        state_reports,
    }
}

pub(crate) fn state_model_ids_from_findings(
    findings: &[crate::metrics::v2::SemanticFinding],
) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding.kind.starts_with("state_model_"))
        .map(|finding| finding.concept_id.clone())
        .collect()
}

pub(crate) fn state_model_ids_from_reports(
    reports: &[crate::metrics::v2::StateIntegrityReport],
) -> BTreeSet<String> {
    reports.iter().map(|report| report.id.clone()).collect()
}

#[derive(Default)]
pub(crate) struct ChangedPatchScope {
    pub(crate) obligations: Vec<crate::metrics::v2::ObligationReport>,
    pub(crate) semantic_error: Option<String>,
    pub(crate) suppression_application: SuppressionApplication,
    pub(crate) touched_concepts: BTreeSet<String>,
}

pub(crate) fn analyze_changed_patch_scope(
    state: &mut McpState,
    root: &Path,
    config: &crate::metrics::rules::RulesConfig,
    snapshot: Option<&Snapshot>,
    semantic: Option<&SemanticSnapshot>,
    changed_files: &BTreeSet<String>,
) -> ChangedPatchScope {
    if changed_files.is_empty() {
        return ChangedPatchScope::default();
    }

    let (analysis, semantic_error) = match semantic {
        Some(semantic) => (
            build_semantic_analysis_batch(
                config,
                semantic,
                snapshot,
                crate::metrics::v2::ObligationScope::Changed,
                changed_files,
            ),
            None,
        ),
        None => semantic_analysis_batch(
            state,
            root,
            snapshot,
            crate::metrics::v2::ObligationScope::Changed,
            changed_files,
        ),
    };
    let mut touched_concepts =
        crate::metrics::v2::changed_concept_ids_from_files(config, changed_files)
            .into_iter()
            .collect::<BTreeSet<_>>();
    touched_concepts.extend(crate::metrics::v2::changed_state_model_ids_from_files(
        config,
        changed_files,
    ));
    touched_concepts.extend(crate::metrics::v2::changed_concepts_from_obligations(
        &analysis.obligations,
    ));
    touched_concepts.extend(state_model_ids_from_reports(&analysis.state_reports));
    touched_concepts.extend(state_model_ids_from_findings(&analysis.findings));
    let changed_findings = serialized_values(&analysis.findings);
    let suppression_application = apply_suppressions(config, changed_findings);

    ChangedPatchScope {
        obligations: analysis.obligations,
        semantic_error,
        suppression_application,
        touched_concepts,
    }
}
