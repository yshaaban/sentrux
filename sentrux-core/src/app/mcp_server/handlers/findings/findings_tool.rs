use super::*;

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
    let context = load_findings_context(args, state)?;
    let surface = build_findings_review_surface(state, &context);
    let mut result = serde_json::Map::new();
    let confidence = build_v2_confidence_report(
        &context.metadata,
        &surface.rules_config,
        surface.session_v2_status.clone(),
    );
    result.insert("kind".to_string(), json!("mixed_findings"));
    result.insert("confidence".to_string(), json!(confidence));
    result.insert(
        "project_shape".to_string(),
        project_shape_json_cached(
            state,
            &context.root,
            &context.snapshot,
            &surface.rules_config,
        ),
    );
    insert_findings_clone_fields(&mut result, &surface);
    insert_findings_result_fields(&mut result, &surface);
    insert_findings_suppression_fields(&mut result, &surface.suppression_application);
    let debt_context_error = insert_debt_report_fields(&mut result, surface.debt_outputs);
    insert_rules_semantic_context_diagnostics(
        &mut result,
        merge_optional_errors(surface.config_error, surface.suppression_error),
        merge_optional_errors(surface.semantic_error, surface.clone_error),
        debt_context_error,
    );
    Ok(Value::Object(result))
}

struct FindingsContext {
    health: metrics::HealthReport,
    snapshot: Snapshot,
    root: PathBuf,
    metadata: crate::app::mcp_server::ScanMetadata,
    limit: usize,
}

struct FindingsReviewSurface {
    rules_config: crate::metrics::rules::RulesConfig,
    session_v2_status: SessionBaselineStatus,
    clone_payload: CloneFindingPayload,
    suppression_application: SuppressionApplication,
    suppression_error: Option<String>,
    config_error: Option<String>,
    clone_error: Option<String>,
    semantic_error: Option<String>,
    visible_clone_group_count: usize,
    semantic_finding_count: usize,
    findings: Vec<Value>,
    finding_details: Vec<Value>,
    experimental_findings: Vec<Value>,
    clone_families: Vec<Value>,
    clone_remediations: Vec<Value>,
    debt_outputs: DebtReportOutputs,
}

fn load_findings_context(args: &Value, state: &McpState) -> Result<FindingsContext, String> {
    let health = state
        .cached_health
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .as_deref()
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
    Ok(FindingsContext {
        health,
        snapshot,
        root,
        metadata,
        limit,
    })
}

fn build_findings_review_surface(
    state: &mut McpState,
    context: &FindingsContext,
) -> FindingsReviewSurface {
    let (rules_config, config_error) = load_v2_rules_config(state, &context.root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&context.root);
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
    let merged_findings = merge_findings(
        clone_payload.prioritized_findings.clone(),
        combined_other_finding_values(&semantic_findings, &structural_reports),
        usize::MAX,
    );
    let (suppression_application, suppression_error) =
        apply_root_suppressions(state, &context.root, merged_findings);
    let (visible_findings, experimental_findings) =
        decorate_findings_surface(&suppression_application.visible_findings, context.limit);
    let visible_clone_ids = visible_clone_ids(&visible_findings);
    let findings = visible_findings
        .iter()
        .take(context.limit)
        .cloned()
        .collect::<Vec<_>>();
    let finding_details =
        serialized_values(&build_finding_details(&visible_findings, context.limit));
    let clone_families = filter_clone_values_by_visible_clone_ids(
        clone_payload.families.clone(),
        &visible_clone_ids,
        "clone_ids",
        context.limit.min(FINDINGS_CLONE_SUPPORT_LIMIT),
    );
    let clone_remediations = filter_clone_values_by_visible_clone_ids(
        clone_payload.remediation_hints.clone(),
        &visible_clone_ids,
        "clone_ids",
        context.limit.min(FINDINGS_CLONE_SUPPORT_LIMIT),
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
        context.limit.min(FINDINGS_DEBT_SUPPORT_LIMIT),
        true,
    );

    FindingsReviewSurface {
        rules_config,
        session_v2_status,
        clone_payload,
        suppression_application,
        suppression_error,
        config_error,
        clone_error,
        semantic_error,
        visible_clone_group_count: visible_clone_ids.len(),
        semantic_finding_count: visible_findings
            .iter()
            .filter(|finding| finding.get("concept_id").is_some())
            .count(),
        findings,
        finding_details,
        experimental_findings,
        clone_families,
        clone_remediations,
        debt_outputs,
    }
}

fn decorate_findings_surface(visible_findings: &[Value], limit: usize) -> (Vec<Value>, Vec<Value>) {
    let (visible_findings, experimental_findings) =
        partition_review_surface_experimental_findings(visible_findings, limit);
    (
        visible_findings
            .into_iter()
            .map(|finding| decorate_finding_with_classification(&finding))
            .collect::<Vec<_>>(),
        experimental_findings
            .into_iter()
            .map(|finding| decorate_finding_with_classification(&finding))
            .collect::<Vec<_>>(),
    )
}

fn insert_findings_clone_fields(
    result: &mut serde_json::Map<String, Value>,
    surface: &FindingsReviewSurface,
) {
    result.insert(
        "clone_group_count".to_string(),
        json!(surface.clone_payload.clone_group_count),
    );
    result.insert(
        "clone_family_count".to_string(),
        json!(surface.clone_payload.clone_family_count),
    );
    result.insert(
        "visible_clone_group_count".to_string(),
        json!(surface.visible_clone_group_count),
    );
    result.insert(
        "visible_clone_family_count".to_string(),
        json!(surface.clone_families.len()),
    );
    result.insert("clone_families".to_string(), json!(surface.clone_families));
    result.insert(
        "clone_remediations".to_string(),
        json!(surface.clone_remediations),
    );
}

fn insert_findings_result_fields(
    result: &mut serde_json::Map<String, Value>,
    surface: &FindingsReviewSurface,
) {
    result.insert(
        "semantic_finding_count".to_string(),
        json!(surface.semantic_finding_count),
    );
    result.insert(
        "finding_detail_count".to_string(),
        json!(surface.finding_details.len()),
    );
    result.insert(
        "finding_details".to_string(),
        json!(surface.finding_details),
    );
    result.insert(
        "experimental_finding_count".to_string(),
        json!(surface.experimental_findings.len()),
    );
    result.insert(
        "experimental_findings".to_string(),
        json!(surface.experimental_findings),
    );
    result.insert("findings".to_string(), json!(surface.findings));
}

fn insert_findings_suppression_fields(
    result: &mut serde_json::Map<String, Value>,
    suppression_application: &SuppressionApplication,
) {
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
}
