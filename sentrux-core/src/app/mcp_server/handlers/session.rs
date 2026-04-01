use super::*;
use crate::metrics::v2::FindingSeverity;

pub(crate) struct PatchCheckContext {
    pub(crate) bundle: ScanBundle,
    pub(crate) changed_files: BTreeSet<String>,
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
        let changed_files = changed_files_from_session_context(
            root,
            &bundle.snapshot,
            session_v2,
            current_identity.as_ref(),
        );
        if changed_files.is_empty() || scan_cache_matches_identity(state, current_identity.as_ref())
        {
            return Ok(PatchCheckContext {
                bundle,
                changed_files,
                reused_cached_scan: true,
                scan_identity: None,
            });
        }
    }

    let (bundle, scan_identity) = do_scan_with_identity(root)?;
    let changed_files =
        changed_files_from_session_context(root, &bundle.snapshot, session_v2, None);

    Ok(PatchCheckContext {
        bundle,
        changed_files,
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
) -> Option<PatchSafetyAnalysisCache> {
    let scan_identity = scan_identity?;
    let cached = state.cached_patch_safety.as_ref()?;
    if cached.scan_identity.as_ref() == Some(scan_identity)
        && cached.session_signature == session_signature
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
) -> PatchSafetyAnalysisCache {
    let session_signature = session_v2_analysis_signature(session_v2);
    if let Some(cached) =
        cached_patch_safety_analysis(state, cache_identity.as_ref(), session_signature)
    {
        return cached;
    }

    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        root,
        &bundle.snapshot,
        &bundle.health,
        bundle.health.duplicate_groups.len(),
    );
    let structural_reports = crate::metrics::v2::build_structural_debt_reports_with_root(
        root,
        &bundle.snapshot,
        &bundle.health,
    );
    let (rules_config, rules_error) = load_v2_rules_config(state, root);
    let semantic = match analyze_semantic_snapshot(state, root) {
        Ok(semantic) => semantic,
        Err(error) => {
            let suppression_application = apply_suppressions(
                &rules_config,
                finding_values(
                    &clone_payload.exact_findings,
                    &serialized_values(&structural_reports),
                ),
            );
            let analysis = PatchSafetyAnalysisCache {
                scan_identity: cache_identity.clone(),
                session_signature,
                visible_findings: suppression_application.visible_findings,
                suppression_hits: serialized_values(&suppression_application.active_matches),
                suppressed_finding_count: suppression_match_count(
                    &suppression_application.active_matches,
                ),
                expired_suppressions: serialized_values(&suppression_application.expired_matches),
                expired_suppression_match_count: suppression_match_count(
                    &suppression_application.expired_matches,
                ),
                clone_error,
                all_semantic_error: merge_optional_errors(rules_error.clone(), Some(error.clone())),
                changed_semantic_error: merge_optional_errors(rules_error.clone(), Some(error)),
                rules_error,
                ..PatchSafetyAnalysisCache::default()
            };
            cache_patch_safety_analysis(state, &analysis);
            return analysis;
        }
    };

    let all_analysis = semantic
        .as_ref()
        .map(|semantic| {
            build_semantic_analysis_batch(
                &rules_config,
                semantic,
                Some(&bundle.snapshot),
                crate::metrics::v2::ObligationScope::All,
                &BTreeSet::new(),
            )
        })
        .unwrap_or_default();
    let all_finding_values =
        combined_other_finding_values(&all_analysis.findings, &structural_reports);
    let suppression_application = apply_suppressions(
        &rules_config,
        finding_values(&clone_payload.exact_findings, &all_finding_values),
    );
    let changed_scope = analyze_changed_patch_scope(
        state,
        root,
        &rules_config,
        Some(&bundle.snapshot),
        semantic.as_ref(),
        changed_files,
    );

    let analysis = PatchSafetyAnalysisCache {
        scan_identity: cache_identity,
        session_signature,
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
    };

    cache_patch_safety_analysis(state, &analysis);

    analysis
}

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
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, &root)?;
    let (baseline, mut baseline_error) = match state.baseline.clone() {
        Some(baseline) => (Some(baseline), None),
        None => match load_persisted_baseline(&root) {
            Ok(baseline) => (baseline, None),
            Err(error) => (None, Some(error)),
        },
    };

    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let bundle = context.bundle;
    let legacy_diff = baseline
        .as_ref()
        .map(|baseline| baseline.diff(&bundle.health));
    let changed_files = context.changed_files;
    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let analysis = build_patch_safety_analysis(
        state,
        &root,
        &bundle,
        &changed_files,
        session_v2.as_ref(),
        patch_cache_identity,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let (rules_config, _) = load_v2_rules_config(state, &root);
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
        .unwrap_or_default();
    let (visible_introduced_findings, experimental_findings) =
        partition_experimental_findings(&introduced_findings, 10);
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
    let gate_decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded)
        || !visible_introduced_findings.is_empty()
    {
        "warn"
    } else {
        "pass"
    };
    let semantic_error = patch_safety_semantic_error(&analysis);
    if baseline.is_none() && baseline_error.is_none() {
        baseline_error =
            Some("Legacy baseline unavailable; structural delta fields were omitted".to_string());
    }
    let introduced_findings = visible_introduced_findings
        .iter()
        .map(decorate_finding_with_classification)
        .collect::<Vec<_>>();
    let (opportunity_findings, experimental_findings) = if session_v2.is_some() {
        (visible_introduced_findings.clone(), experimental_findings)
    } else {
        partition_experimental_findings(&analysis.changed_visible_findings, 10)
    };
    let opportunity_findings = opportunity_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let experimental_findings = experimental_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let finding_details = build_finding_details(&opportunity_findings, 10);
    let debt_outputs = build_debt_report_outputs(
        state,
        &root,
        &bundle.snapshot,
        &bundle.health,
        &opportunity_findings,
        &analysis.changed_obligations,
        &[],
        &changed_files,
        5,
    );
    let signal_before = legacy_diff
        .as_ref()
        .map(|diff| (diff.signal_before * 10000.0).round() as i32);
    let signal_after = legacy_diff
        .as_ref()
        .map(|diff| (diff.signal_after * 10000.0).round() as i32);
    let signal_delta = legacy_diff
        .as_ref()
        .map(|diff| ((diff.signal_after - diff.signal_before) * 10000.0).round() as i32);
    let coupling_change = legacy_diff
        .as_ref()
        .map(|diff| vec![diff.coupling_before, diff.coupling_after]);
    let cycles_change = legacy_diff
        .as_ref()
        .map(|diff| vec![diff.cycles_before, diff.cycles_after]);
    let legacy_violations = legacy_diff
        .as_ref()
        .map(|diff| diff.violations.clone())
        .unwrap_or_default();

    let summary = if gate_decision == "fail" {
        "Touched-concept regressions detected"
    } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded) {
        "Quality degraded"
    } else if legacy_diff.is_none() {
        "Patch safety check complete; legacy structural delta unavailable"
    } else {
        "Quality stable or improved"
    };
    let mut result = serde_json::Map::new();
    result.insert("pass".to_string(), json!(gate_decision != "fail"));
    result.insert("signal_before".to_string(), json!(signal_before));
    result.insert("signal_after".to_string(), json!(signal_after));
    result.insert("signal_delta".to_string(), json!(signal_delta));
    result.insert("coupling_change".to_string(), json!(coupling_change));
    result.insert("cycles_change".to_string(), json!(cycles_change));
    result.insert("violations".to_string(), json!(legacy_violations));
    result.insert("summary".to_string(), json!(summary));
    result.insert(
        "changed_files".to_string(),
        json!(changed_files.iter().cloned().collect::<Vec<_>>()),
    );
    result.insert("changed_concepts".to_string(), json!(changed_concepts));
    result.insert(
        "introduced_findings".to_string(),
        json!(introduced_findings),
    );
    result.insert("resolved_findings".to_string(), json!(resolved_findings));
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
    result.insert(
        "missing_obligations".to_string(),
        json!(missing_obligations),
    );
    let debt_context_error = insert_debt_report_fields(&mut result, debt_outputs);
    result.insert(
        "obligation_completeness_0_10000".to_string(),
        json!(crate::metrics::v2::obligation_score_0_10000(
            &analysis.changed_obligations
        )),
    );
    result.insert(
        "touched_concept_gate".to_string(),
        json!({
            "decision": gate_decision,
            "blocking_findings": blocking_findings,
        }),
    );
    result.insert(
        "suppression_hits".to_string(),
        json!(analysis.suppression_hits),
    );
    result.insert(
        "suppressed_finding_count".to_string(),
        json!(analysis.suppressed_finding_count),
    );
    result.insert(
        "expired_suppressions".to_string(),
        json!(analysis.expired_suppressions),
    );
    result.insert(
        "expired_suppression_match_count".to_string(),
        json!(analysis.expired_suppression_match_count),
    );
    result.insert("scan_trust".to_string(), scan_trust_json(&bundle.metadata));
    result.insert(
        "confidence".to_string(),
        json!(build_v2_confidence_report(
            &bundle.metadata,
            &rules_config,
            session_v2_status
        )),
    );
    result.insert(
        "baseline_delta".to_string(),
        legacy_baseline_delta_json(legacy_diff.as_ref()),
    );
    insert_error_diagnostics(
        &mut result,
        vec![
            DiagnosticEntry::new("rules", analysis.rules_error.clone()),
            DiagnosticEntry::new("semantic", semantic_error.clone()),
            DiagnosticEntry::new("context", debt_context_error.clone()),
            DiagnosticEntry::new("baseline", baseline_error.clone()),
        ],
        Vec::new(),
    );
    let result = Value::Object(result);

    if !context.reused_cached_scan {
        let preserved_semantic = state.cached_semantic.take();
        let preserved_evolution = state.cached_evolution.take();
        let preserved_patch_safety = state.cached_patch_safety.take();
        update_scan_cache(state, root, bundle, baseline, context.scan_identity);
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
    } else {
        state.baseline = baseline;
    }

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
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let bundle = context.bundle;
    let changed_files = context.changed_files;

    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let analysis = build_patch_safety_analysis(
        state,
        root,
        &bundle,
        &changed_files,
        session_v2.as_ref(),
        patch_cache_identity,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let (rules_config, _) = load_v2_rules_config(state, root);
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
    let (visible_introduced_findings, experimental_findings) =
        partition_experimental_findings(&introduced_findings, 10);
    let blocking_findings = visible_introduced_findings
        .iter()
        .filter(|finding| {
            let severity = severity_of_value(finding);
            severity == FindingSeverity::High || (strict && severity == FindingSeverity::Medium)
        })
        .cloned()
        .collect::<Vec<_>>();
    let decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else {
        "pass"
    };
    let semantic_error = patch_safety_semantic_error(&analysis);
    let summary = if decision == "fail" {
        "Touched-concept regressions detected"
    } else if changed_files.is_empty() {
        "No working-tree changes detected"
    } else {
        "No blocking touched-concept regressions detected"
    };
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    let legacy_baseline_delta = persisted_baseline
        .as_ref()
        .or(state.baseline.as_ref())
        .map(|baseline| baseline.diff(&bundle.health));
    let mut response = json!({
        "decision": decision,
        "strict": strict,
        "summary": summary,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "introduced_findings": visible_introduced_findings,
        "experimental_finding_count": experimental_findings.len(),
        "experimental_findings": experimental_findings,
        "blocking_findings": blocking_findings,
        "missing_obligations": missing_obligations,
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&analysis.changed_obligations),
        "suppression_hits": analysis.suppression_hits,
        "suppressed_finding_count": analysis.suppressed_finding_count,
        "expired_suppressions": analysis.expired_suppressions,
        "expired_suppression_match_count": analysis.expired_suppression_match_count,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": build_v2_confidence_report(&bundle.metadata, &rules_config, session_v2_status),
        "baseline_delta": legacy_baseline_delta_json(legacy_baseline_delta.as_ref()),
    });
    if let Some(object) = response.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![
                DiagnosticEntry::new("rules", analysis.rules_error.clone()),
                DiagnosticEntry::new("semantic", semantic_error.clone()),
            ],
            Vec::new(),
        );
    }

    if !context.reused_cached_scan {
        let preserved_semantic = state.cached_semantic.take();
        let preserved_evolution = state.cached_evolution.take();
        let preserved_patch_safety = state.cached_patch_safety.take();
        update_scan_cache(
            state,
            root.to_path_buf(),
            bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }

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
