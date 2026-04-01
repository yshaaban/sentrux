use super::*;

pub(crate) fn refresh_changed_scope(
    state: &mut McpState,
    root: &Path,
) -> Result<BTreeSet<String>, String> {
    let session_v2 = current_session_v2_baseline(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let changed_files = context.changed_files.clone();
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root.to_path_buf(),
            context.bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }
    Ok(changed_files)
}

pub fn findings_def() -> ToolDef {
    ToolDef {
        name: "findings",
        description: "Return primary v2 patch-safety and technical-debt findings for the current scan, with clone drift, concept debt summaries, debt signals, watchpoints, and confidence metadata.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of findings to return (default 10, max 50)"
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_findings,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_findings(
    args: &Value,
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
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(10)
        .min(FINDINGS_LIMIT_MAX as u64) as usize;
    let (rules_config, config_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        &root,
        &snapshot,
        &health,
        health.duplicate_groups.len(),
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
    let (visible_findings, experimental_findings) =
        partition_experimental_findings(&suppression_application.visible_findings, limit);
    let visible_findings = visible_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let experimental_findings = experimental_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let visible_clone_ids = visible_clone_ids(&visible_findings);
    let semantic_finding_count = visible_findings
        .iter()
        .filter(|finding| finding.get("concept_id").is_some())
        .count();
    let findings = visible_findings
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let finding_details = build_finding_details(&visible_findings, limit);
    let clone_families = filter_clone_values_by_visible_clone_ids(
        clone_payload.families,
        &visible_clone_ids,
        "clone_ids",
        limit.min(FINDINGS_CLONE_SUPPORT_LIMIT),
    );
    let clone_remediations = filter_clone_values_by_visible_clone_ids(
        clone_payload.remediation_hints,
        &visible_clone_ids,
        "clone_ids",
        limit.min(FINDINGS_CLONE_SUPPORT_LIMIT),
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
        limit.min(FINDINGS_DEBT_SUPPORT_LIMIT),
    );
    let mut result = serde_json::Map::new();
    result.insert("kind".to_string(), json!("mixed_findings"));
    result.insert(
        "confidence".to_string(),
        json!(build_v2_confidence_report(
            &metadata,
            &rules_config,
            session_v2_status
        )),
    );
    result.insert(
        "project_shape".to_string(),
        project_shape_json(&root, &snapshot, &rules_config),
    );
    result.insert(
        "clone_group_count".to_string(),
        json!(clone_payload.clone_group_count),
    );
    result.insert(
        "clone_family_count".to_string(),
        json!(clone_payload.clone_family_count),
    );
    result.insert(
        "visible_clone_group_count".to_string(),
        json!(visible_clone_ids.len()),
    );
    result.insert(
        "visible_clone_family_count".to_string(),
        json!(clone_families.len()),
    );
    result.insert("clone_families".to_string(), json!(clone_families));
    result.insert("clone_remediations".to_string(), json!(clone_remediations));
    let debt_context_error = insert_debt_report_fields(&mut result, debt_outputs);
    result.insert(
        "semantic_finding_count".to_string(),
        json!(semantic_finding_count),
    );
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
    let rules_error = merge_optional_errors(config_error, suppression_error);
    let semantic_error = merge_optional_errors(semantic_error, clone_error);
    result.insert(
        "suppression_hits".to_string(),
        json!(suppression_application.active_matches),
    );
    result.insert(
        "suppressed_finding_count".to_string(),
        json!(suppression_match_count(
            &suppression_application.active_matches
        )),
    );
    result.insert(
        "expired_suppressions".to_string(),
        json!(suppression_application.expired_matches),
    );
    result.insert(
        "expired_suppression_match_count".to_string(),
        json!(suppression_match_count(
            &suppression_application.expired_matches
        )),
    );
    result.insert("findings".to_string(), json!(findings));
    insert_rules_semantic_context_diagnostics(
        &mut result,
        rules_error,
        semantic_error,
        debt_context_error,
    );
    Ok(Value::Object(result))
}

pub fn obligations_def() -> ToolDef {
    ToolDef {
        name: "obligations",
        description: "Return required update sites for configured v2 concepts and conservative closed-domain exhaustiveness gaps.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all obligations or only obligations touched by working-tree changes (default all)."
                },
                "concept": {
                    "type": "string",
                    "description": "Optional concept id filter."
                },
                "file": {
                    "type": "string",
                    "description": "Optional file filter."
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional closed-domain symbol filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_obligations,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_obligations(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::ObligationScope::Changed,
        _ => crate::metrics::v2::ObligationScope::All,
    };
    let changed_files = if scope == crate::metrics::v2::ObligationScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };
    let concept_filter = args.get("concept").and_then(|value| value.as_str());
    let file_filter = args.get("file").and_then(|value| value.as_str());
    let symbol_filter = args.get("symbol").and_then(|value| value.as_str());
    let cached_snapshot = state.cached_snapshot.clone();

    let (_, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        scope,
        &changed_files,
    );
    let obligations = obligations
        .into_iter()
        .filter(|obligation| {
            concept_filter
                .map(|concept| obligation.concept_id.as_deref() == Some(concept))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            file_filter
                .map(|file| obligation.files.iter().any(|candidate| candidate == file))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            symbol_filter
                .map(|symbol| obligation.domain_symbol_name.as_deref() == Some(symbol))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let changed_concepts = crate::metrics::v2::changed_concepts_from_obligations(&obligations);
    let obligation_count = obligations.len();
    let missing_site_count: usize = obligations
        .iter()
        .map(|obligation| obligation.missing_sites.len())
        .sum();
    let context_burden: usize = obligations
        .iter()
        .map(|obligation| obligation.context_burden)
        .sum();
    let obligation_completeness_0_10000 =
        crate::metrics::v2::obligation_score_0_10000(&obligations);

    let mut response = json!({
        "kind": "obligations",
        "scope": if scope == crate::metrics::v2::ObligationScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "changed_concepts": changed_concepts,
        "obligation_count": obligation_count,
        "missing_site_count": missing_site_count,
        "context_burden": context_burden,
        "obligation_completeness_0_10000": obligation_completeness_0_10000,
        "obligations": obligations
    });
    if let Some(object) = response.as_object_mut() {
        insert_semantic_diagnostics(object, semantic_error);
    }
    Ok(response)
}

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

pub fn concentration_def() -> ToolDef {
    ToolDef {
        name: "concentration",
        description: "Rank coordination hotspots using static file features, concept writes, complexity, and optional git churn context.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all files or only currently changed files (default all)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of hotspot findings to return (default 10)."
                },
                "days": {
                    "type": "integer",
                    "description": "Optional git lookback window in days for churn context."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_concentration,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_concentration(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => "changed",
        _ => "all",
    };
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(10);
    let lookback_days = args
        .get("days")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32);

    let changed_files = if scope == "changed" {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };
    let snapshot = state
        .cached_snapshot
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let mut file_paths = crate::app::mcp_server::handlers_evo::build_known_files(snapshot)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if scope == "changed" {
        file_paths.retain(|path| changed_files.contains(path));
    }
    let complexity_map = crate::app::mcp_server::handlers_evo::build_complexity_map(snapshot);

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (semantic, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(semantic) => (semantic, None),
        Err(error) => (None, Some(error)),
    };
    let (history, evolution_error) = concentration_history(state, &root, lookback_days);
    let concentration_result = crate::metrics::v2::build_concentration_reports(
        &root,
        &file_paths,
        &complexity_map,
        &config,
        semantic.as_ref(),
        history.as_ref(),
    );
    let reports = concentration_result.reports;
    let findings = crate::metrics::v2::build_concentration_findings(&reports, limit);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let top_reports = reports.iter().take(limit).cloned().collect::<Vec<_>>();
    let rules_error = merge_optional_errors(rules_error, suppression_rules_error);

    let mut response = json!({
        "kind": "concentration",
        "scope": scope,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "report_count": reports.len(),
        "finding_count": findings.len(),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": top_reports,
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_evolution_diagnostics(
            object,
            rules_error,
            semantic_error,
            evolution_error,
            concentration_result.read_warnings,
        );
    }
    Ok(response)
}

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
    let (reports, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => {
            let obligation_scope = if scope == crate::metrics::v2::StateScope::Changed {
                crate::metrics::v2::ObligationScope::Changed
            } else {
                crate::metrics::v2::ObligationScope::All
            };
            let obligations =
                crate::metrics::v2::build_obligations(&config, &semantic, obligation_scope, &changed_files);
            (
                crate::metrics::v2::build_state_integrity_reports(
                    &config,
                    &semantic,
                    &obligations,
                    scope,
                    &changed_files,
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
    };
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

    let rules_error = merge_optional_errors(rules_error, suppression_rules_error);
    let mut response = json!({
        "kind": "state",
        "scope": if scope == crate::metrics::v2::StateScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "state_model_count": reports.len(),
        "finding_count": findings.len(),
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
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(object, rules_error, semantic_error, Vec::new());
    }
    Ok(response)
}
