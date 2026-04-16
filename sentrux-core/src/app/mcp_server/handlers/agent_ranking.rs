use super::agent_format::{
    obligation_value_to_agent_issue, to_agent_issue, AgentAction, AgentIssue, IssueOrigin,
    IssueSource,
};
use serde_json::Value;

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
    issues
        .iter()
        .take(limit.max(1))
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

fn right_gate_weight(issue: &AgentIssue) -> u8 {
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
    match issue.kind.as_str() {
        "incomplete_propagation" | "closed_domain_exhaustiveness" => 8,
        "forbidden_raw_read" | "forbidden_writer" | "writer_outside_allowlist" => 7,
        "dependency_sprawl" | "cycle_cluster" => 6,
        "unstable_hotspot" | "session_introduced_clone" | "clone_propagation_drift" => 5,
        "touched_clone_family" => 3,
        "exact_clone_group" | "clone_group" | "clone_family" => 1,
        "large_file" => 0,
        _ => 4,
    }
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
    match issue.leverage_class.as_str() {
        "boundary_discipline" => 6,
        "architecture_signal" => 5,
        "local_refactor_target" => 4,
        "hardening_note" => 3,
        "regrowth_watchpoint" => 2,
        "tooling_debt" => 1,
        "secondary_cleanup" | "experimental" => 0,
        _ => 0,
    }
}

fn issue_presentation_weight(issue: &AgentIssue) -> u8 {
    match issue.presentation_class.as_str() {
        "guarded_facade" => 4,
        "hardening_note" => 3,
        "structural_debt" => 2,
        "tooling_debt" | "watchpoint" => 1,
        "experimental" => 0,
        _ => 0,
    }
}

fn issue_repairability_weight(issue: &AgentIssue) -> u8 {
    (issue.repair_packet.completeness_0_10000 / 2000).min(5) as u8
}

fn why_now_for_issue(issue: &AgentIssue) -> Vec<String> {
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
    if issue.source == IssueSource::Obligation {
        reasons.push("changed_concept".to_string());
    }
    if reasons.is_empty() {
        reasons.push("useful_follow_up_signal".to_string());
    }
    reasons
}

pub(crate) fn compare_agent_issues(left: &AgentIssue, right: &AgentIssue) -> std::cmp::Ordering {
    right_gate_weight(right)
        .cmp(&right_gate_weight(left))
        .then_with(|| issue_source_weight(right).cmp(&issue_source_weight(left)))
        .then_with(|| issue_kind_weight(right).cmp(&issue_kind_weight(left)))
        .then_with(|| right.severity.priority().cmp(&left.severity.priority()))
        .then_with(|| issue_leverage_weight(right).cmp(&issue_leverage_weight(left)))
        .then_with(|| issue_presentation_weight(right).cmp(&issue_presentation_weight(left)))
        .then_with(|| issue_trust_tier_weight(right).cmp(&issue_trust_tier_weight(left)))
        .then_with(|| issue_confidence_weight(right).cmp(&issue_confidence_weight(left)))
        .then_with(|| issue_repairability_weight(right).cmp(&issue_repairability_weight(left)))
        .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
        .then_with(|| left.file.cmp(&right.file))
        .then_with(|| left.kind.cmp(&right.kind))
}
