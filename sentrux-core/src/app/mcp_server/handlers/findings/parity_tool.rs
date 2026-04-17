use super::*;

pub fn parity_def() -> ToolDef {
    ToolDef {
        name: "parity",
        description: "Return explicit contract parity analysis as supporting context for configured v2 contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all configured contracts or only contracts touched by current changes (default all)."
                },
                "contract": {
                    "type": "string",
                    "description": "Optional contract id filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_parity,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_parity(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::ParityScope::Changed,
        _ => crate::metrics::v2::ParityScope::All,
    };
    let contract_filter = args.get("contract").and_then(|value| value.as_str());

    let changed_files = if scope == crate::metrics::v2::ParityScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (parity_result, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => (
            crate::metrics::v2::build_parity_reports(
                &config,
                &semantic,
                &root,
                scope,
                &changed_files,
            ),
            None,
        ),
        Ok(None) => (
            crate::metrics::v2::ParityBuildResult::default(),
            (!config.contract.is_empty()).then(|| {
                "Contract parity requires TypeScript semantic analysis for configured contracts"
                    .to_string()
            }),
        ),
        Err(error) => (
            crate::metrics::v2::ParityBuildResult::default(),
            Some(error),
        ),
    };
    let reports = parity_result
        .reports
        .into_iter()
        .filter(|report| {
            contract_filter
                .map(|contract| report.id == contract)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let findings = crate::metrics::v2::build_parity_findings(&reports);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let missing_cell_count = reports
        .iter()
        .map(|report| report.missing_cells.len())
        .sum::<usize>();
    let assessable_cell_count = reports
        .iter()
        .map(|report| report.satisfied_cells.len() + report.missing_cells.len())
        .sum::<usize>();
    let parity_score_0_10000 = if assessable_cell_count == 0 {
        None
    } else {
        Some(crate::metrics::v2::parity_score_0_10000(&reports))
    };
    let rules_error = merge_optional_errors(rules_error, suppression_rules_error);

    let mut response = json!({
        "kind": "parity",
        "scope": if scope == crate::metrics::v2::ParityScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "contract_count": reports.len(),
        "assessable_cell_count": assessable_cell_count,
        "missing_cell_count": missing_cell_count,
        "parity_score_0_10000": parity_score_0_10000,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": reports,
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(
            object,
            rules_error,
            semantic_error,
            parity_result.read_warnings,
        );
    }
    Ok(response)
}
