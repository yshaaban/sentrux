use super::{
    AgentBriefInput, AgentBriefMode, AgentBriefTarget, AgentIssue, BriefLeverageClass,
    BriefSeverity, BriefTrustTier, NextToolCall,
};
use crate::app::mcp_server::handlers::{issue_blocks_gate, score_band_label, IssueSource};
use serde_json::json;

pub(super) fn target_from_issue(issue: &AgentIssue, input: &AgentBriefInput) -> AgentBriefTarget {
    let likely_fix_sites = issue.repair_packet.likely_fix_sites.clone();
    let summary = format!(
        "{} ({})",
        issue.message,
        score_band_label(issue.score_0_10000)
    );
    AgentBriefTarget {
        scope: issue.scope.clone(),
        kind: issue.kind.clone(),
        severity: BriefSeverity::from_issue(issue),
        trust_tier: BriefTrustTier::from_issue(issue),
        leverage_class: BriefLeverageClass::from_issue(issue),
        presentation_class: issue.presentation_class.clone(),
        score_0_10000: issue.score_0_10000,
        summary,
        blocking: issue_blocks_gate(issue),
        why_now: why_now(issue, input),
        likely_fix_sites,
        inspection_focus: inspection_focus(issue),
        repair_packet: issue.repair_packet.clone(),
        next_tools: issue
            .concept_id
            .as_deref()
            .map(|concept| concept_tools(concept, input.mode))
            .unwrap_or_default(),
    }
}

fn why_now(issue: &AgentIssue, input: &AgentBriefInput) -> Vec<String> {
    let mut reasons = Vec::new();
    if issue
        .concept_id
        .as_ref()
        .is_some_and(|concept| input.changed_concepts.iter().any(|value| value == concept))
    {
        reasons.push("touched_concept".to_string());
    }
    if issue.trust_tier == "trusted" {
        reasons.push("high_trust".to_string());
    }
    if BriefLeverageClass::from_issue(issue).is_high_leverage() {
        reasons.push("high_leverage".to_string());
    }
    if issue.repair_packet.complete {
        reasons.push("clear_fix_surface".to_string());
    }
    if issue.source == IssueSource::Obligation {
        reasons.push("blocking_obligation".to_string());
    }
    if matches!(input.mode, AgentBriefMode::PreMerge) && issue_blocks_gate(issue) {
        reasons.push("merge_blocker_candidate".to_string());
    }
    if reasons.is_empty() {
        reasons.push("useful_follow_up_signal".to_string());
    }
    reasons
}

fn concept_tools(concept_id: &str, mode: AgentBriefMode) -> Vec<NextToolCall> {
    let obligation_scope = match mode {
        AgentBriefMode::RepoOnboarding => "all",
        AgentBriefMode::Patch | AgentBriefMode::PreMerge => "changed",
    };
    vec![
        NextToolCall {
            tool: "explain_concept".to_string(),
            args: json!({ "id": concept_id }),
        },
        NextToolCall {
            tool: "obligations".to_string(),
            args: json!({ "concept": concept_id, "scope": obligation_scope }),
        },
    ]
}

fn inspection_focus(issue: &AgentIssue) -> Vec<String> {
    if issue.source == IssueSource::Obligation {
        return vec![
            "Inspect every missing sibling surface before treating the changed concept as complete."
                .to_string(),
        ];
    }
    if let Some(inspection_targets) = inspection_targets(issue) {
        return vec![format!(
            "Inspect {} before widening the patch.",
            inspection_targets.join(", ")
        )];
    }
    if !issue.evidence.is_empty() {
        return vec![
            "Inspect the cited evidence path before changing adjacent surfaces.".to_string(),
        ];
    }
    vec![
        "Inspect the narrowest owner that can absorb the fix before widening the change."
            .to_string(),
    ]
}

fn inspection_targets(issue: &AgentIssue) -> Option<&[String]> {
    if !issue.repair_packet.likely_fix_sites.is_empty() {
        return Some(issue.repair_packet.likely_fix_sites.as_slice());
    }
    if !issue.repair_packet.inspection_context.is_empty() {
        return Some(issue.repair_packet.inspection_context.as_slice());
    }
    None
}
