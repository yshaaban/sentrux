use super::*;

#[path = "debt_aggregation.rs"]
mod debt_aggregation;
#[path = "debt_concepts.rs"]
mod debt_concepts;

use debt_aggregation::{
    build_debt_clusters, build_inspection_watchpoints, collect_debt_signals,
    debt_signal_watchpoints, merge_watchpoints, truncate_debt_signals,
};
use debt_concepts::{build_concept_debt_summaries, signal_severity};

pub(crate) fn debt_signal_candidate_files(
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
    clone_families: &[Value],
    extra_files: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for finding in findings {
        files.extend(finding_files(finding));
    }
    for obligation in obligations {
        files.extend(obligation.files.iter().cloned());
    }
    for family in clone_families {
        files.extend(finding_files(family));
    }
    files.extend(extra_files.iter().cloned());
    files
}

pub(crate) fn build_debt_report_outputs(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
    clone_families: &[Value],
    extra_files: &BTreeSet<String>,
    limit: usize,
    allow_cold_evolution: bool,
) -> DebtReportOutputs {
    let candidate_files =
        debt_signal_candidate_files(findings, obligations, clone_families, extra_files);
    let (concentration_reports, context_error) = debt_signal_concentration_reports(
        state,
        root,
        snapshot,
        &candidate_files,
        allow_cold_evolution,
    );
    let concept_summaries = build_concept_debt_summaries(findings, obligations);
    let (rules_config, _) = load_v2_rules_config(state, root);
    let structural_reports =
        structural_reports_for_scope(root, snapshot, health, &rules_config, extra_files);
    let all_debt_signals = collect_debt_signals(
        &concept_summaries,
        &structural_reports,
        findings,
        clone_families,
        &concentration_reports,
    );
    let trusted_debt_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == DebtTrustTier::Trusted)
        .cloned()
        .collect::<Vec<_>>();
    let watchpoint_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == DebtTrustTier::Watchpoint)
        .cloned()
        .collect::<Vec<_>>();
    let experimental_debt_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == DebtTrustTier::Experimental)
        .cloned()
        .collect::<Vec<_>>();
    let debt_signals = truncate_debt_signals(trusted_debt_signals.clone(), limit);
    let cluster_sources = trusted_debt_signals
        .iter()
        .chain(watchpoint_signals.iter())
        .cloned()
        .collect::<Vec<_>>();
    let debt_clusters = build_debt_clusters(&cluster_sources, limit);
    let concept_watchpoints = build_inspection_watchpoints(
        &concept_summaries,
        clone_families,
        &concentration_reports,
        limit,
    );
    let watchpoints = merge_watchpoints(
        concept_watchpoints,
        debt_signal_watchpoints(&watchpoint_signals, limit),
        limit,
    );

    DebtReportOutputs {
        concept_summaries: concept_summaries.into_iter().take(limit).collect(),
        debt_signals,
        experimental_debt_signals: truncate_debt_signals(experimental_debt_signals, limit),
        debt_clusters,
        watchpoints,
        context_error,
    }
}

pub(crate) fn insert_debt_report_fields(
    result: &mut serde_json::Map<String, Value>,
    debt_outputs: DebtReportOutputs,
) -> Option<String> {
    let DebtReportOutputs {
        concept_summaries,
        debt_signals,
        experimental_debt_signals,
        debt_clusters,
        watchpoints,
        context_error,
    } = debt_outputs;
    let concept_summary_count = concept_summaries.len();
    let debt_signal_count = debt_signals.len();
    let experimental_debt_signal_count = experimental_debt_signals.len();
    let debt_cluster_count = debt_clusters.len();
    let watchpoint_count = watchpoints.len();

    result.insert(
        "concept_summary_count".to_string(),
        json!(concept_summary_count),
    );
    result.insert("concept_summaries".to_string(), json!(concept_summaries));
    result.insert("debt_signal_count".to_string(), json!(debt_signal_count));
    result.insert("debt_signals".to_string(), json!(debt_signals));
    result.insert(
        "experimental_debt_signal_count".to_string(),
        json!(experimental_debt_signal_count),
    );
    result.insert(
        "experimental_debt_signals".to_string(),
        json!(experimental_debt_signals),
    );
    result.insert("debt_cluster_count".to_string(), json!(debt_cluster_count));
    result.insert("debt_clusters".to_string(), json!(debt_clusters));
    result.insert("watchpoint_count".to_string(), json!(watchpoint_count));
    result.insert("watchpoints".to_string(), json!(watchpoints));

    context_error
}

pub(crate) fn structural_reports_for_scope(
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    rules_config: &crate::metrics::rules::RulesConfig,
    scope_files: &BTreeSet<String>,
) -> Vec<crate::metrics::v2::StructuralDebtReport> {
    let reports = filter_structural_reports_by_rules(
        crate::metrics::v2::build_structural_debt_reports_with_root(root, snapshot, health),
        rules_config,
    );
    if scope_files.is_empty() {
        return reports;
    }

    reports
        .into_iter()
        .filter(|report| report.files.iter().any(|path| scope_files.contains(path)))
        .collect()
}

pub(crate) fn structural_signal(report: &crate::metrics::v2::StructuralDebtReport) -> DebtSignal {
    annotate_debt_signal(DebtSignal {
        kind: report.kind.clone(),
        trust_tier: DebtTrustTier::from_str(report.trust_tier.as_str()),
        presentation_class: PresentationClass::from_str(report.presentation_class.as_str()),
        leverage_class: Some(FindingLeverageClass::from_str(
            report.leverage_class.as_str(),
        )),
        scope: report.scope.clone(),
        signal_class: SignalClass::from_str(report.signal_class.as_str()),
        signal_families: report.signal_families.clone(),
        severity: report.severity,
        score_0_10000: report.score_0_10000,
        summary: report.summary.clone(),
        impact: report.impact.clone(),
        files: report.files.clone(),
        role_tags: report.role_tags.clone(),
        leverage_reasons: report.leverage_reasons.clone(),
        evidence: report.evidence.clone(),
        inspection_focus: report.inspection_focus.clone(),
        candidate_split_axes: report.candidate_split_axes.clone(),
        related_surfaces: report.related_surfaces.clone(),
        metrics: DebtSignalMetrics {
            file_count: report.metrics.file_count,
            line_count: report.metrics.line_count,
            function_count: report.metrics.function_count,
            fan_in: report.metrics.fan_in,
            fan_out: report.metrics.fan_out,
            instability_0_10000: report.metrics.instability_0_10000,
            dead_symbol_count: report.metrics.dead_symbol_count,
            dead_line_count: report.metrics.dead_line_count,
            cycle_size: report.metrics.cycle_size,
            cut_candidate_count: report.metrics.cut_candidate_count,
            largest_cycle_after_best_cut: report.metrics.largest_cycle_after_best_cut,
            inbound_reference_count: report.metrics.inbound_reference_count,
            public_surface_count: report.metrics.public_surface_count,
            reachable_from_tests: report.metrics.reachable_from_tests,
            guardrail_test_count: report.metrics.guardrail_test_count,
            role_count: report.metrics.role_count,
            max_complexity: report.metrics.max_complexity,
            ..DebtSignalMetrics::default()
        },
    })
}

pub(crate) fn clone_family_inspection_focus(family: &Value) -> Vec<String> {
    let mut focus = family
        .get("remediation_hints")
        .and_then(|value| value.as_array())
        .map(|hints| {
            hints
                .iter()
                .filter_map(|hint| hint.get("kind").and_then(|value| value.as_str()))
                .filter_map(|kind| match kind {
                    "sync_recent_divergence" => Some(
                        "inspect whether recent sibling edits should stay synchronized or intentionally diverge"
                            .to_string(),
                    ),
                    "extract_shared_helper" => Some(
                        "inspect whether the repeated logic is substantial enough to share behind one helper"
                            .to_string(),
                    ),
                    "collapse_clone_family" => Some(
                        "inspect whether the clone family is carrying avoidable duplicate maintenance"
                            .to_string(),
                    ),
                    "add_shared_behavior_tests" => Some(
                        "inspect whether shared behavior tests would make the family safer to change"
                            .to_string(),
                    ),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

pub(crate) fn clone_family_candidate_axes(family: &Value) -> Vec<String> {
    let mut axes = family
        .get("remediation_hints")
        .and_then(|value| value.as_array())
        .map(|hints| {
            hints
                .iter()
                .filter_map(|hint| hint.get("kind").and_then(|value| value.as_str()))
                .map(|kind| match kind {
                    "sync_recent_divergence" => "shared behavior boundary".to_string(),
                    "extract_shared_helper" => "shared helper boundary".to_string(),
                    "collapse_clone_family" => "duplicate maintenance boundary".to_string(),
                    "add_shared_behavior_tests" => "shared behavior test boundary".to_string(),
                    _ => kind.replace('_', " "),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    axes = dedupe_strings_preserve_order(axes);
    axes.truncate(3);
    axes
}

fn clone_family_signal(family: &Value) -> Option<DebtSignal> {
    let summary = family.get("summary")?.as_str()?.to_string();
    let scope = family.get("family_id")?.as_str()?.to_string();
    let severity = severity_of_value(family);
    let score_0_10000 = family
        .get("family_score")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let files = finding_files(family);
    let evidence = json_string_list(family.get("reasons"))
        .into_iter()
        .take(3)
        .collect();
    let inspection_focus = clone_family_inspection_focus(family);

    Some(annotate_debt_signal(DebtSignal {
        kind: "clone_family".to_string(),
        trust_tier: if score_0_10000 >= 6500 {
            DebtTrustTier::Trusted
        } else {
            DebtTrustTier::Watchpoint
        },
        presentation_class: PresentationClass::Watchpoint,
        leverage_class: None,
        scope,
        signal_class: if score_0_10000 >= 6500 {
            SignalClass::Debt
        } else {
            SignalClass::Watchpoint
        },
        signal_families: vec!["duplication".to_string(), "drift".to_string()],
        severity,
        score_0_10000,
        summary,
        impact: "Duplicate logic across related files increases the chance that a fix lands in only one sibling and the family drifts over time.".to_string(),
        files,
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus,
        candidate_split_axes: clone_family_candidate_axes(family),
        related_surfaces: finding_files(family),
        metrics: DebtSignalMetrics {
            file_count: family
                .get("file_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            member_count: family
                .get("member_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            recently_touched_file_count: family
                .get("recently_touched_file_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            divergence_score: family
                .get("divergence_score")
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            family_score_0_10000: Some(score_0_10000),
            ..DebtSignalMetrics::default()
        },
    }))
}

fn clone_group_signal(finding: &Value) -> Option<DebtSignal> {
    let scope = finding.get("clone_id")?.as_str()?.to_string();
    let severity = severity_of_value(finding);
    let score_0_10000 = finding
        .get("risk_score")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let summary = finding
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("Clone group needs consolidation")
        .to_string();
    let files = finding_files(finding);
    let evidence = json_string_list(finding.get("reasons"))
        .into_iter()
        .take(3)
        .collect::<Vec<_>>();

    Some(annotate_debt_signal(DebtSignal {
        kind: "clone_group".to_string(),
        trust_tier: if score_0_10000 >= 6500 {
            DebtTrustTier::Trusted
        } else {
            DebtTrustTier::Watchpoint
        },
        presentation_class: PresentationClass::Watchpoint,
        leverage_class: None,
        scope,
        signal_class: if score_0_10000 >= 6500 {
            SignalClass::Debt
        } else {
            SignalClass::Watchpoint
        },
        signal_families: vec!["duplication".to_string()],
        severity,
        score_0_10000,
        summary,
        impact: "Copy-paste maintenance means the same behavior must be kept in sync across multiple files.".to_string(),
        files,
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus: vec![
            "inspect whether the repeated logic should stay aligned or collapse behind one abstraction"
                .to_string(),
            "inspect whether shared behavior tests would make the copies safer to change"
                .to_string(),
        ],
        candidate_split_axes: vec![
            "shared helper boundary".to_string(),
            "shared behavior test boundary".to_string(),
        ],
        related_surfaces: finding_files(finding),
        metrics: DebtSignalMetrics {
            file_count: Some(finding_files(finding).len()),
            ..DebtSignalMetrics::default()
        },
    }))
}

fn hotspot_signal(report: &crate::metrics::v2::ConcentrationReport) -> Option<DebtSignal> {
    if report.score_0_10000 < 4000 {
        return None;
    }

    Some(annotate_debt_signal(DebtSignal {
        kind: "hotspot".to_string(),
        trust_tier: if report.score_0_10000 >= 6500 {
            DebtTrustTier::Trusted
        } else {
            DebtTrustTier::Watchpoint
        },
        presentation_class: classify_debt_presentation_class(
            "hotspot",
            if report.score_0_10000 >= 6500 {
                DebtTrustTier::Trusted
            } else {
                DebtTrustTier::Watchpoint
            },
            &report.path,
            std::slice::from_ref(&report.path),
            &[],
            report.reasons.len(),
            1,
            0,
            0,
        ),
        leverage_class: None,
        scope: report.path.clone(),
        signal_class: if report.score_0_10000 >= 6500 {
            SignalClass::Debt
        } else {
            SignalClass::Watchpoint
        },
        signal_families: vec!["coordination".to_string()],
        severity: signal_severity(report.score_0_10000),
        score_0_10000: report.score_0_10000,
        summary: format!(
            "File '{}' is carrying coordination hotspot pressure",
            report.path
        ),
        impact: "Side effects, async branches, and retry logic concentrated in one file increase coordination debt and regression risk.".to_string(),
        files: vec![report.path.clone()],
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence: report.reasons.iter().cloned().take(3).collect(),
        inspection_focus: vec![
            "inspect whether orchestration, side effects, and adapters are accumulating in one file"
                .to_string(),
            "inspect whether complexity is local to one seam or repeated across nearby files"
                .to_string(),
        ],
        candidate_split_axes: vec![
            "orchestration boundary".to_string(),
            "side-effect boundary".to_string(),
            "adapter boundary".to_string(),
        ],
        related_surfaces: vec![report.path.clone()],
        metrics: DebtSignalMetrics {
            file_count: Some(1),
            authority_breadth: Some(report.authority_breadth),
            side_effect_breadth: Some(report.side_effect_breadth),
            timer_retry_weight: Some(report.timer_retry_weight),
            async_branch_weight: Some(report.async_branch_weight),
            max_complexity: Some(report.max_complexity),
            churn_commits: Some(report.churn_commits),
            ..DebtSignalMetrics::default()
        },
    }))
}

pub(crate) fn json_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
