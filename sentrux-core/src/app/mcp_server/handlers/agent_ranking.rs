use super::agent_format::{
    obligation_value_to_agent_issue, to_agent_issue, AgentAction, AgentIssue, IssueOrigin,
    IssueSource,
};
use super::signal_policy::{
    action_kind_weight, action_leverage_weight, action_presentation_weight,
    default_lane_action_limit, default_lane_kind_rule, default_lane_source_allowed,
};
use serde_json::Value;
use std::cmp::Ordering;

pub(crate) fn issue_blocks_gate(issue: &AgentIssue) -> bool {
    match issue.source {
        IssueSource::Obligation => issue.severity == crate::metrics::v2::FindingSeverity::High,
        IssueSource::Rules => {
            issue.origin == IssueOrigin::Explicit && issue.severity.priority() >= 2
        }
        IssueSource::Structural | IssueSource::Clone => false,
    }
}

pub(crate) fn actions_from_issues(issues: &[AgentIssue], limit: usize) -> Vec<AgentAction> {
    let action_limit = limit.max(1).min(default_lane_action_limit());

    issues
        .iter()
        .filter(|issue| issue_is_default_lane_eligible(issue))
        .take(action_limit)
        .enumerate()
        .map(|(index, issue)| AgentAction {
            priority: index + 1,
            scope: issue.scope.clone(),
            concept_id: issue.concept_id.clone(),
            file: issue.file.clone(),
            line: issue.line,
            kind: issue.kind.clone(),
            message: issue.message.clone(),
            severity: issue.severity,
            trust_tier: issue.trust_tier.clone(),
            presentation_class: issue.presentation_class.clone(),
            leverage_class: issue.leverage_class.clone(),
            score_0_10000: issue.score_0_10000,
            fix_hint: issue.fix_hint.clone(),
            evidence: issue.evidence.clone(),
            blocking: issue_blocks_gate(issue),
            source: issue.source,
            origin: issue.origin,
            confidence: issue.confidence,
            why_now: why_now_for_issue(issue),
            evidence_metrics: issue.evidence_metrics.clone(),
            repair_packet: issue.repair_packet.clone(),
        })
        .collect()
}

pub(crate) fn issues_from_findings_and_obligations(
    findings: &[Value],
    missing_obligations: &[Value],
) -> Vec<AgentIssue> {
    let mut issues = missing_obligations
        .iter()
        .map(obligation_value_to_agent_issue)
        .collect::<Vec<_>>();
    issues.extend(findings.iter().map(to_agent_issue));
    issues.sort_by(compare_agent_issues);
    issues
}

pub(crate) fn actions_from_findings_and_obligations(
    findings: &[Value],
    missing_obligations: &[Value],
    limit: usize,
) -> Vec<AgentAction> {
    let issues = issues_from_findings_and_obligations(findings, missing_obligations);
    actions_from_issues(&issues, limit)
}

fn issue_gate_weight(issue: &AgentIssue) -> u8 {
    if issue_blocks_gate(issue) {
        1
    } else {
        0
    }
}

fn issue_source_weight(issue: &AgentIssue) -> u8 {
    if matches!(
        issue.kind.as_str(),
        "session_introduced_clone" | "clone_propagation_drift"
    ) {
        return 2;
    }
    if issue.kind == "touched_clone_family" {
        return 1;
    }

    match (issue.source, issue.origin) {
        (IssueSource::Obligation, _) => 5,
        (IssueSource::Rules, IssueOrigin::Explicit) => 4,
        (IssueSource::Rules, IssueOrigin::ZeroConfig) => 2,
        (IssueSource::Structural, _)
            if is_broad_structural_pressure(issue) && !issue_patch_directly_worsened(issue) =>
        {
            0
        }
        (IssueSource::Structural, _) => 1,
        (IssueSource::Clone, _) => 0,
    }
}

fn issue_confidence_weight(issue: &AgentIssue) -> u8 {
    match issue.confidence {
        super::agent_format::IssueConfidence::High => 2,
        super::agent_format::IssueConfidence::Medium => 1,
        super::agent_format::IssueConfidence::Experimental => 0,
    }
}

fn issue_kind_weight(issue: &AgentIssue) -> u8 {
    action_kind_weight(issue.kind.as_str())
}

fn issue_trust_tier_weight(issue: &AgentIssue) -> u8 {
    match issue.trust_tier.as_str() {
        "trusted" => 3,
        "watchpoint" => 2,
        "experimental" => 1,
        _ => 0,
    }
}

fn issue_leverage_weight(issue: &AgentIssue) -> u8 {
    action_leverage_weight(issue.leverage_class.as_str())
}

fn issue_presentation_weight(issue: &AgentIssue) -> u8 {
    action_presentation_weight(issue.presentation_class.as_str())
}

fn issue_repairability_weight(issue: &AgentIssue) -> u8 {
    (issue.repair_packet.completeness_0_10000 / 2000).min(5) as u8
}

fn is_broad_structural_pressure(issue: &AgentIssue) -> bool {
    matches!(
        issue.kind.as_str(),
        "large_file" | "dependency_sprawl" | "unstable_hotspot" | "missing_test_coverage"
    )
}

fn issue_patch_directly_worsened(issue: &AgentIssue) -> bool {
    issue
        .evidence_metrics
        .patch_directly_worsened
        .unwrap_or(false)
}

fn issue_in_changed_scope(issue: &AgentIssue) -> bool {
    issue.evidence_metrics.changed_scope.unwrap_or(false)
}

fn issue_source_name(source: IssueSource) -> &'static str {
    match source {
        IssueSource::Obligation => "obligation",
        IssueSource::Structural => "structural",
        IssueSource::Clone => "clone",
        IssueSource::Rules => "rules",
    }
}

fn issue_is_default_lane_eligible(issue: &AgentIssue) -> bool {
    if !default_lane_source_allowed(issue_source_name(issue.source)) {
        return false;
    }

    let kind_rule = default_lane_kind_rule(issue.kind.as_str());
    if !kind_rule.eligible {
        return false;
    }
    if kind_rule.require_patch_directly_worsened && !issue_patch_directly_worsened(issue) {
        return false;
    }
    if kind_rule.require_repair_surface && !issue.repair_packet.required_fields.repair_surface {
        return false;
    }
    if kind_rule.require_changed_scope && !issue_in_changed_scope(issue) {
        return false;
    }

    true
}

fn compare_boolean_true_first(left: bool, right: bool) -> Ordering {
    right.cmp(&left)
}

fn compare_optional_boolean_true_first(left: Option<bool>, right: Option<bool>) -> Ordering {
    compare_boolean_true_first(left.unwrap_or(false), right.unwrap_or(false))
}

fn compare_optional_number_desc(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left_value), Some(right_value)) => right_value
            .partial_cmp(&left_value)
            .unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_optional_number_asc(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left_value), Some(right_value)) => left_value
            .partial_cmp(&right_value)
            .unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_issue_evidence_metrics(left: &AgentIssue, right: &AgentIssue) -> Ordering {
    let left_metrics = &left.evidence_metrics;
    let right_metrics = &right.evidence_metrics;

    compare_optional_boolean_true_first(
        left_metrics.default_rollout_ready,
        right_metrics.default_rollout_ready,
    )
    .then_with(|| {
        compare_optional_boolean_true_first(
            left_metrics.signal_treatment_ready,
            right_metrics.signal_treatment_ready,
        )
    })
    .then_with(|| {
        compare_optional_boolean_true_first(
            left_metrics.patch_directly_worsened,
            right_metrics.patch_directly_worsened,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.signal_treatment_intervention_net_value_score_delta,
            right_metrics.signal_treatment_intervention_net_value_score_delta,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.top_action_help_rate,
            right_metrics.top_action_help_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.top_action_follow_rate,
            right_metrics.top_action_follow_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.reviewer_acceptance_rate,
            right_metrics.reviewer_acceptance_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.remediation_success_rate,
            right_metrics.remediation_success_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.task_success_rate,
            right_metrics.task_success_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.intervention_net_value_score,
            right_metrics.intervention_net_value_score,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.reviewed_precision,
            right_metrics.reviewed_precision,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.top_1_actionable_precision,
            right_metrics.top_1_actionable_precision,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.top_3_actionable_precision,
            right_metrics.top_3_actionable_precision,
        )
    })
    .then_with(|| {
        compare_optional_number_asc(
            left_metrics.reviewer_disagreement_rate,
            right_metrics.reviewer_disagreement_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_asc(
            left_metrics.patch_expansion_rate,
            right_metrics.patch_expansion_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_asc(
            left_metrics.intervention_cost_checks_mean,
            right_metrics.intervention_cost_checks_mean,
        )
    })
    .then_with(|| {
        compare_optional_number_asc(
            left_metrics.review_noise_rate,
            right_metrics.review_noise_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.repair_packet_complete_rate,
            right_metrics.repair_packet_complete_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.repair_packet_fix_surface_clear_rate,
            right_metrics.repair_packet_fix_surface_clear_rate,
        )
    })
    .then_with(|| {
        compare_optional_number_desc(
            left_metrics.repair_packet_verification_clear_rate,
            right_metrics.repair_packet_verification_clear_rate,
        )
    })
}

fn why_now_for_issue(issue: &AgentIssue) -> Vec<String> {
    let metrics = &issue.evidence_metrics;
    let mut reasons = Vec::new();
    if issue_blocks_gate(issue) {
        reasons.push("gate_blocker".to_string());
    }
    if issue_trust_tier_weight(issue) >= 3 {
        reasons.push("high_trust".to_string());
    }
    if issue_leverage_weight(issue) >= 4 {
        reasons.push("high_leverage".to_string());
    }
    if issue_repairability_weight(issue) >= 4 {
        reasons.push("clear_fix_surface".to_string());
    }
    if metrics.default_rollout_ready.unwrap_or(false) {
        reasons.push("default_rollout_ready".to_string());
    } else if metrics.signal_treatment_ready.unwrap_or(false) {
        reasons.push("treatment_proven".to_string());
    }
    if metrics.top_action_help_rate.is_some_and(|rate| rate >= 0.5) {
        reasons.push("helped_prior_sessions".to_string());
    }
    if metrics
        .top_action_follow_rate
        .is_some_and(|rate| rate >= 0.5)
    {
        reasons.push("followed_in_prior_sessions".to_string());
    }
    if metrics
        .reviewer_disagreement_rate
        .is_some_and(|rate| rate <= 0.15)
    {
        reasons.push("low_reviewer_disagreement".to_string());
    }
    if metrics
        .patch_expansion_rate
        .is_some_and(|rate| rate <= 0.25)
    {
        reasons.push("bounded_patch_surface".to_string());
    }
    if issue_patch_directly_worsened(issue) {
        reasons.push("patch_directly_worsened".to_string());
    }
    if issue.source == IssueSource::Obligation {
        reasons.push("changed_concept".to_string());
    }
    if reasons.is_empty() {
        reasons.push("useful_follow_up_signal".to_string());
    }
    reasons
}

pub(crate) fn compare_agent_issues(left: &AgentIssue, right: &AgentIssue) -> Ordering {
    issue_gate_weight(right)
        .cmp(&issue_gate_weight(left))
        .then_with(|| issue_source_weight(right).cmp(&issue_source_weight(left)))
        .then_with(|| issue_kind_weight(right).cmp(&issue_kind_weight(left)))
        .then_with(|| right.severity.priority().cmp(&left.severity.priority()))
        .then_with(|| issue_leverage_weight(right).cmp(&issue_leverage_weight(left)))
        .then_with(|| issue_presentation_weight(right).cmp(&issue_presentation_weight(left)))
        .then_with(|| issue_trust_tier_weight(right).cmp(&issue_trust_tier_weight(left)))
        .then_with(|| issue_confidence_weight(right).cmp(&issue_confidence_weight(left)))
        .then_with(|| issue_repairability_weight(right).cmp(&issue_repairability_weight(left)))
        .then_with(|| compare_issue_evidence_metrics(left, right))
        .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
        .then_with(|| left.file.cmp(&right.file))
        .then_with(|| left.kind.cmp(&right.kind))
}
