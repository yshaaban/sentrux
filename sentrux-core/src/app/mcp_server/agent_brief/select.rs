use super::{
    render::target_from_issue, AgentBriefInput, AgentBriefMode, AgentBriefTarget, AgentIssue,
};
use crate::app::mcp_server::handlers::{
    default_lane_action_limit, issue_blocks_gate, issue_is_default_lane_eligible, IssueConfidence,
    IssueSource,
};
use crate::metrics::v2::FindingSeverity;
use std::collections::BTreeSet;

fn primary_target_limit(input: &AgentBriefInput) -> usize {
    input.limit.max(1).min(default_lane_action_limit())
}

pub(super) fn visible_primary_targets(
    input: &AgentBriefInput,
    primary_targets: Vec<AgentBriefTarget>,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    let limit = primary_target_limit(input);
    let mut visible_targets = primary_targets.into_iter().take(limit).collect::<Vec<_>>();
    ensure_visible_blocking_target(input, &mut visible_targets, ranked_issues);
    visible_targets
}

fn ensure_visible_blocking_target(
    input: &AgentBriefInput,
    visible_targets: &mut Vec<AgentBriefTarget>,
    ranked_issues: &[AgentIssue],
) {
    if input.mode != AgentBriefMode::PreMerge || input.decision.as_deref() != Some("fail") {
        return;
    }
    if visible_targets.iter().any(|target| target.blocking) {
        return;
    }

    let Some(blocking_issue) = ranked_issues
        .iter()
        .find(|issue| issue_blocks_gate(issue) && issue_is_default_lane_eligible(issue))
    else {
        return;
    };
    let blocking_target = target_from_issue(blocking_issue, input);
    if visible_targets
        .iter()
        .any(|target| target.kind == blocking_target.kind && target.scope == blocking_target.scope)
    {
        return;
    }

    if !visible_targets.is_empty() {
        visible_targets.pop();
    }
    visible_targets.push(blocking_target);
}

pub(super) fn select_onboarding_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    select_primary_targets(ranked_issues, input, false)
}

pub(super) fn select_patch_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    select_primary_targets(ranked_issues, input, true)
}

pub(super) fn select_pre_merge_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    let mut selected = select_primary_targets(ranked_issues, input, true);
    if let Some(issue) = blocking_pre_merge_issue_to_surface(input, &selected, ranked_issues) {
        selected.push(target_from_issue(issue, input));
    }
    selected
}

fn blocking_pre_merge_issue_to_surface<'a>(
    input: &AgentBriefInput,
    selected: &[AgentBriefTarget],
    ranked_issues: &'a [AgentIssue],
) -> Option<&'a AgentIssue> {
    if input.decision.as_deref() != Some("fail") {
        return None;
    }
    if selected.iter().any(|target| target.blocking) {
        return None;
    }

    ranked_issues
        .iter()
        .find(|issue| issue_blocks_gate(issue) && issue_is_default_lane_eligible(issue))
}

fn select_primary_targets(
    ranked_issues: &[AgentIssue],
    input: &AgentBriefInput,
    patch_scope_only: bool,
) -> Vec<AgentBriefTarget> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    let limit = primary_target_limit(input);

    for issue in ranked_issues {
        if !issue_is_primary_target(issue, input, patch_scope_only) {
            continue;
        }
        let target = target_from_issue(issue, input);
        let scope_key = format!("{}:{}", target.scope, target.kind);
        if seen.insert(scope_key) {
            selected.push(target);
        }
        if selected.len() >= limit {
            break;
        }
    }
    selected
}

fn issue_is_primary_target(
    issue: &AgentIssue,
    input: &AgentBriefInput,
    patch_scope_only: bool,
) -> bool {
    if issue.trust_tier == "experimental" || issue.presentation_class == "experimental" {
        return false;
    }
    if matches!(
        issue.kind.as_str(),
        "exact_clone_group" | "clone_group" | "clone_family"
    ) {
        return false;
    }
    if !issue_is_default_lane_eligible(issue) {
        return false;
    }
    if patch_scope_only && !issue_is_patch_relevant(issue, input) {
        return false;
    }
    if issue_blocks_gate(issue) {
        return true;
    }

    match input.mode {
        AgentBriefMode::RepoOnboarding => issue_meets_repo_onboarding_primary_contract(issue),
        AgentBriefMode::Patch => {
            issue.source == IssueSource::Obligation
                || (issue_is_trusted_primary_candidate(issue)
                    && issue_has_complete_repair_packet(issue)
                    && issue_has_actionable_anchor(issue)
                    && (issue_is_high_leverage(issue)
                        || issue.severity == FindingSeverity::High
                        || issue.score_0_10000 >= 7_500))
        }
        AgentBriefMode::PreMerge => {
            issue_is_trusted_primary_candidate(issue)
                && issue_has_complete_repair_packet(issue)
                && issue_has_actionable_anchor(issue)
                && (issue.severity == FindingSeverity::High
                    || (input.strict.unwrap_or(false)
                        && issue.severity == FindingSeverity::Medium
                        && issue_is_high_leverage(issue)))
        }
    }
}

fn issue_meets_repo_onboarding_primary_contract(issue: &AgentIssue) -> bool {
    issue_is_trusted_primary_candidate(issue)
        && issue_has_complete_repair_packet(issue)
        && issue_has_actionable_anchor(issue)
        && (issue_is_high_leverage(issue)
            || issue.severity == FindingSeverity::High
            || issue.score_0_10000 >= 7_500)
}

fn issue_is_trusted_primary_candidate(issue: &AgentIssue) -> bool {
    issue.trust_tier == "trusted"
        && issue.confidence != IssueConfidence::Experimental
        && issue.presentation_class != "watchpoint"
}

fn issue_has_complete_repair_packet(issue: &AgentIssue) -> bool {
    issue.repair_packet.complete
}

fn issue_has_actionable_anchor(issue: &AgentIssue) -> bool {
    issue.concept_id.is_some()
        || !issue.repair_packet.likely_fix_sites.is_empty()
        || !issue.repair_packet.inspection_context.is_empty()
        || !issue.evidence.is_empty()
        || issue.fix_hint.is_some()
}

fn issue_is_high_leverage(issue: &AgentIssue) -> bool {
    matches!(
        issue.leverage_class.as_str(),
        "boundary_discipline" | "architecture_signal" | "local_refactor_target"
    )
}

fn issue_is_patch_relevant(issue: &AgentIssue, input: &AgentBriefInput) -> bool {
    if issue.source == IssueSource::Obligation {
        return true;
    }
    if input.changed_concepts.is_empty() && input.changed_files.is_empty() {
        return true;
    }
    if issue
        .concept_id
        .as_ref()
        .is_some_and(|concept| input.changed_concepts.iter().any(|value| value == concept))
    {
        return true;
    }
    input.changed_files.iter().any(|path| path == &issue.file)
}
