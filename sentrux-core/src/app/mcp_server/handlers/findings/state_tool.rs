use super::*;

pub fn state_def() -> ToolDef {
    ToolDef {
        name: "state",
        description: "Return conservative state-integrity analysis for configured state models using closed-domain coverage and obligation completeness.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all configured state models or only state models touched by current changes (default all)."
                },
                "id": {
                    "type": "string",
                    "description": "Optional state model id filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_state,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_state(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::StateScope::Changed,
        _ => crate::metrics::v2::StateScope::All,
    };
    let state_filter = args.get("id").and_then(|value| value.as_str());
    let changed_files = if scope == crate::metrics::v2::StateScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (reports, semantic_error) =
        load_state_reports(state, &root, &config, scope, &changed_files);
    let reports = reports
        .into_iter()
        .filter(|report| {
            state_filter
                .map(|state_model_id| report.id == state_model_id)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let findings = crate::metrics::v2::build_state_integrity_findings(&reports);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let state_integrity_score_0_10000 = if reports.is_empty() {
        None
    } else {
        Some(crate::metrics::v2::state_integrity_score_0_10000(&reports))
    };
    let missing_variant_count = reports
        .iter()
        .map(|report| report.missing_variants.len())
        .sum::<usize>();
    let missing_site_count = reports
        .iter()
        .map(|report| report.missing_sites.len())
        .sum::<usize>();
    let transition_site_count = reports
        .iter()
        .map(|report| report.transition_sites.len())
        .sum::<usize>();
    let transition_gap_count = reports
        .iter()
        .map(|report| report.transition_gap_sites.len())
        .sum::<usize>();

    let mut response = build_state_response(
        scope,
        &changed_files,
        &reports,
        &suppression_application,
        missing_variant_count,
        missing_site_count,
        transition_site_count,
        transition_gap_count,
        state_integrity_score_0_10000,
    );
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(
            object,
            merge_optional_errors(rules_error, suppression_rules_error),
            semantic_error,
            Vec::new(),
        );
    }
    Ok(response)
}

fn load_state_reports(
    state: &mut McpState,
    root: &Path,
    config: &crate::metrics::rules::RulesConfig,
    scope: crate::metrics::v2::StateScope,
    changed_files: &BTreeSet<String>,
) -> (
    Vec<crate::metrics::v2::StateIntegrityReport>,
    Option<String>,
) {
    match analyze_semantic_snapshot(state, root) {
        Ok(Some(semantic)) => {
            let obligation_scope = if scope == crate::metrics::v2::StateScope::Changed {
                crate::metrics::v2::ObligationScope::Changed
            } else {
                crate::metrics::v2::ObligationScope::All
            };
            let obligations = crate::metrics::v2::build_obligations(
                config,
                &semantic,
                obligation_scope,
                changed_files,
            );
            (
                crate::metrics::v2::build_state_integrity_reports(
                    config,
                    &semantic,
                    &obligations,
                    scope,
                    changed_files,
                ),
                None,
            )
        }
        Ok(None) => (
            Vec::new(),
            (!config.state_model.is_empty()).then(|| {
                "State integrity analysis requires TypeScript semantic analysis for configured state models"
                    .to_string()
            }),
        ),
        Err(error) => (Vec::new(), Some(error)),
    }
}

fn build_state_response(
    scope: crate::metrics::v2::StateScope,
    changed_files: &BTreeSet<String>,
    reports: &[crate::metrics::v2::StateIntegrityReport],
    suppression_application: &SuppressionApplication,
    missing_variant_count: usize,
    missing_site_count: usize,
    transition_site_count: usize,
    transition_gap_count: usize,
    state_integrity_score_0_10000: Option<u32>,
) -> Value {
    json!({
        "kind": "state",
        "scope": if scope == crate::metrics::v2::StateScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "state_model_count": reports.len(),
        "finding_count": suppression_application.visible_findings.len(),
        "missing_variant_count": missing_variant_count,
        "missing_site_count": missing_site_count,
        "transition_site_count": transition_site_count,
        "transition_gap_count": transition_gap_count,
        "state_integrity_score_0_10000": state_integrity_score_0_10000,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": reports,
    })
}
