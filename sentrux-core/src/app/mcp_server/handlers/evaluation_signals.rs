use super::agent_format::{AgentAction, AgentIssue, IssueSource};
use super::finding_kind;
use crate::metrics::v2::ObligationReport;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub(crate) struct ActionQualitySummary {
    pub(crate) complete_action_count: usize,
    pub(crate) incomplete_action_count: usize,
    pub(crate) top_action_complete: bool,
    pub(crate) top_action_blocking: bool,
    pub(crate) top_action_completeness_0_10000: Option<u32>,
    pub(crate) top_action_source: Option<String>,
    pub(crate) top_action_presentation_class: Option<String>,
    pub(crate) top_action_leverage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub(crate) struct CheckSignalSummary {
    pub(crate) propagation_issue_count: usize,
    pub(crate) exhaustiveness_issue_count: usize,
    pub(crate) session_introduced_clone_issue_count: usize,
    pub(crate) clone_propagation_drift_issue_count: usize,
    pub(crate) touched_clone_family_issue_count: usize,
    pub(crate) action_quality: ActionQualitySummary,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub(crate) struct SessionSignalSummary {
    pub(crate) introduced_finding_count: usize,
    pub(crate) resolved_finding_count: usize,
    pub(crate) net_new_finding_count: i32,
    pub(crate) missing_propagation_obligation_count: usize,
    pub(crate) missing_exhaustiveness_obligation_count: usize,
    pub(crate) introduced_session_clone_count: usize,
    pub(crate) introduced_clone_propagation_drift_count: usize,
    pub(crate) introduced_touched_clone_family_count: usize,
    pub(crate) regression_detected: bool,
    pub(crate) clear_to_stop: bool,
    pub(crate) action_quality: ActionQualitySummary,
}

pub(crate) fn build_check_signal_summary(
    issues: &[AgentIssue],
    actions: &[AgentAction],
) -> CheckSignalSummary {
    CheckSignalSummary {
        propagation_issue_count: issues
            .iter()
            .filter(|issue| is_propagation_kind(issue.kind.as_str()))
            .count(),
        exhaustiveness_issue_count: issues
            .iter()
            .filter(|issue| is_exhaustiveness_kind(issue.kind.as_str()))
            .count(),
        session_introduced_clone_issue_count: issues
            .iter()
            .filter(|issue| issue.kind == "session_introduced_clone")
            .count(),
        clone_propagation_drift_issue_count: issues
            .iter()
            .filter(|issue| issue.kind == "clone_propagation_drift")
            .count(),
        touched_clone_family_issue_count: issues
            .iter()
            .filter(|issue| issue.kind == "touched_clone_family")
            .count(),
        action_quality: build_action_quality_summary(actions),
    }
}

pub(crate) fn build_session_signal_summary(
    introduced_findings: &[Value],
    resolved_findings: &[Value],
    missing_obligations: &[ObligationReport],
    actions: &[AgentAction],
    decision: &str,
) -> SessionSignalSummary {
    SessionSignalSummary {
        introduced_finding_count: introduced_findings.len(),
        resolved_finding_count: resolved_findings.len(),
        net_new_finding_count: introduced_findings.len() as i32 - resolved_findings.len() as i32,
        missing_propagation_obligation_count: missing_obligations
            .iter()
            .filter(|obligation| is_propagation_kind(obligation.kind.as_str()))
            .count(),
        missing_exhaustiveness_obligation_count: missing_obligations
            .iter()
            .filter(|obligation| is_exhaustiveness_kind(obligation.kind.as_str()))
            .count(),
        introduced_session_clone_count: introduced_findings
            .iter()
            .filter(|finding| finding_kind(finding) == "session_introduced_clone")
            .count(),
        introduced_clone_propagation_drift_count: introduced_findings
            .iter()
            .filter(|finding| finding_kind(finding) == "clone_propagation_drift")
            .count(),
        introduced_touched_clone_family_count: introduced_findings
            .iter()
            .filter(|finding| finding_kind(finding) == "touched_clone_family")
            .count(),
        regression_detected: decision != "pass",
        clear_to_stop: decision == "pass" && actions.is_empty(),
        action_quality: build_action_quality_summary(actions),
    }
}

fn build_action_quality_summary(actions: &[AgentAction]) -> ActionQualitySummary {
    let complete_action_count = actions
        .iter()
        .filter(|action| action.repair_packet.complete)
        .count();
    let incomplete_action_count = actions.len().saturating_sub(complete_action_count);
    let top_action = actions.first();

    ActionQualitySummary {
        complete_action_count,
        incomplete_action_count,
        top_action_complete: top_action.is_some_and(|action| action.repair_packet.complete),
        top_action_blocking: top_action.is_some_and(|action| action.blocking),
        top_action_completeness_0_10000: top_action
            .map(|action| action.repair_packet.completeness_0_10000),
        top_action_source: top_action.map(|action| issue_source_label(action.source).to_string()),
        top_action_presentation_class: top_action.map(|action| action.presentation_class.clone()),
        top_action_leverage_class: top_action.map(|action| action.leverage_class.clone()),
    }
}

fn is_propagation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "contract_surface_completeness" | "incomplete_propagation"
    )
}

fn is_exhaustiveness_kind(kind: &str) -> bool {
    kind == "closed_domain_exhaustiveness"
}

fn issue_source_label(source: IssueSource) -> &'static str {
    match source {
        IssueSource::Obligation => "obligation",
        IssueSource::Structural => "structural",
        IssueSource::Clone => "clone",
        IssueSource::Rules => "rules",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_check_signal_summary, build_session_signal_summary, ActionQualitySummary};
    use crate::app::mcp_server::handlers::agent_format::{
        AgentIssueEvidence, IssueOrigin, IssueSource,
    };
    use crate::app::mcp_server::handlers::{
        AgentAction, AgentIssue, IssueConfidence, RepairPacket,
    };
    use crate::metrics::v2::{
        FindingSeverity, ObligationConfidence, ObligationOrigin, ObligationReport, ObligationSite,
        ObligationTrustTier,
    };
    use serde_json::json;

    fn action(kind: &str, source: IssueSource, complete: bool, blocking: bool) -> AgentAction {
        AgentAction {
            priority: 1,
            scope: "scope".to_string(),
            concept_id: None,
            file: "src/app.ts".to_string(),
            line: Some(1),
            kind: kind.to_string(),
            message: "message".to_string(),
            severity: FindingSeverity::High,
            trust_tier: "trusted".to_string(),
            presentation_class: "structural_debt".to_string(),
            leverage_class: "architecture_signal".to_string(),
            score_0_10000: 9000,
            fix_hint: Some("fix".to_string()),
            evidence: vec!["evidence".to_string()],
            blocking,
            source,
            origin: IssueOrigin::Explicit,
            confidence: IssueConfidence::High,
            why_now: vec!["why".to_string()],
            evidence_metrics: AgentIssueEvidence::default(),
            repair_packet: RepairPacket {
                risk_statement: "risk".to_string(),
                likely_fix_sites: vec!["src/app.ts".to_string()],
                inspection_context: Vec::new(),
                smallest_safe_first_cut: Some("cut".to_string()),
                verify_after: vec!["verify".to_string()],
                do_not_touch_yet: Vec::new(),
                completeness_0_10000: if complete { 10_000 } else { 7_500 },
                complete,
                required_fields:
                    crate::app::mcp_server::handlers::agent_guidance::RepairPacketRequiredFields {
                        risk_statement: true,
                        repair_surface: true,
                        first_cut: complete,
                        verification: true,
                    },
                missing_fields: if complete {
                    Vec::new()
                } else {
                    vec!["first_cut".to_string()]
                },
            },
        }
    }

    fn issue(kind: &str) -> AgentIssue {
        AgentIssue {
            scope: "scope".to_string(),
            concept_id: None,
            file: "src/app.ts".to_string(),
            line: Some(1),
            kind: kind.to_string(),
            message: "message".to_string(),
            severity: FindingSeverity::High,
            trust_tier: "trusted".to_string(),
            presentation_class: "structural_debt".to_string(),
            leverage_class: "architecture_signal".to_string(),
            score_0_10000: 9000,
            fix_hint: Some("fix".to_string()),
            evidence: vec!["evidence".to_string()],
            source: IssueSource::Rules,
            origin: IssueOrigin::Explicit,
            confidence: IssueConfidence::High,
            evidence_metrics: AgentIssueEvidence::default(),
            repair_packet: RepairPacket {
                risk_statement: "risk".to_string(),
                likely_fix_sites: vec!["src/app.ts".to_string()],
                inspection_context: Vec::new(),
                smallest_safe_first_cut: Some("cut".to_string()),
                verify_after: vec!["verify".to_string()],
                do_not_touch_yet: Vec::new(),
                completeness_0_10000: 10_000,
                complete: true,
                required_fields:
                    crate::app::mcp_server::handlers::agent_guidance::RepairPacketRequiredFields {
                        risk_statement: true,
                        repair_surface: true,
                        first_cut: true,
                        verification: true,
                    },
                missing_fields: Vec::new(),
            },
        }
    }

    fn obligation(kind: &str) -> ObligationReport {
        ObligationReport {
            id: kind.to_string(),
            kind: kind.to_string(),
            concept_id: Some("task_status".to_string()),
            domain_symbol_name: None,
            origin: ObligationOrigin::Explicit,
            trust_tier: ObligationTrustTier::Trusted,
            confidence: ObligationConfidence::High,
            severity: FindingSeverity::High,
            score_0_10000: 9000,
            summary: "summary".to_string(),
            files: vec!["src/app.ts".to_string()],
            required_sites: vec![ObligationSite {
                path: "src/app.ts".to_string(),
                kind: "kind".to_string(),
                line: Some(1),
                detail: "detail".to_string(),
            }],
            satisfied_sites: Vec::new(),
            missing_sites: vec![ObligationSite {
                path: "src/app.ts".to_string(),
                kind: "kind".to_string(),
                line: Some(1),
                detail: "detail".to_string(),
            }],
            missing_variants: Vec::new(),
            context_burden: 1,
        }
    }

    #[test]
    fn check_signal_summary_counts_propagation_and_clone_followthrough() {
        let issues = vec![
            issue("incomplete_propagation"),
            issue("closed_domain_exhaustiveness"),
            issue("clone_propagation_drift"),
            issue("touched_clone_family"),
        ];
        let actions = vec![
            action(
                "incomplete_propagation",
                IssueSource::Obligation,
                true,
                true,
            ),
            action("touched_clone_family", IssueSource::Clone, false, false),
        ];

        let summary = build_check_signal_summary(&issues, &actions);

        assert_eq!(summary.propagation_issue_count, 1);
        assert_eq!(summary.exhaustiveness_issue_count, 1);
        assert_eq!(summary.clone_propagation_drift_issue_count, 1);
        assert_eq!(summary.touched_clone_family_issue_count, 1);
        assert_eq!(
            summary.action_quality,
            ActionQualitySummary {
                complete_action_count: 1,
                incomplete_action_count: 1,
                top_action_complete: true,
                top_action_blocking: true,
                top_action_completeness_0_10000: Some(10_000),
                top_action_source: Some("obligation".to_string()),
                top_action_presentation_class: Some("structural_debt".to_string()),
                top_action_leverage_class: Some("architecture_signal".to_string()),
            }
        );
    }

    #[test]
    fn session_signal_summary_tracks_clone_and_propagation_regressions() {
        let introduced_findings = vec![
            json!({ "kind": "session_introduced_clone" }),
            json!({ "kind": "clone_propagation_drift" }),
            json!({ "kind": "touched_clone_family" }),
        ];
        let resolved_findings = vec![json!({ "kind": "large_file" })];
        let missing_obligations = vec![
            obligation("incomplete_propagation"),
            obligation("closed_domain_exhaustiveness"),
        ];
        let actions = vec![action(
            "clone_propagation_drift",
            IssueSource::Clone,
            true,
            false,
        )];

        let summary = build_session_signal_summary(
            &introduced_findings,
            &resolved_findings,
            &missing_obligations,
            &actions,
            "warn",
        );

        assert_eq!(summary.introduced_finding_count, 3);
        assert_eq!(summary.resolved_finding_count, 1);
        assert_eq!(summary.net_new_finding_count, 2);
        assert_eq!(summary.missing_propagation_obligation_count, 1);
        assert_eq!(summary.missing_exhaustiveness_obligation_count, 1);
        assert_eq!(summary.introduced_session_clone_count, 1);
        assert_eq!(summary.introduced_clone_propagation_drift_count, 1);
        assert_eq!(summary.introduced_touched_clone_family_count, 1);
        assert!(summary.regression_detected);
        assert!(!summary.clear_to_stop);
        assert_eq!(summary.action_quality.complete_action_count, 1);
        assert_eq!(
            summary.action_quality.top_action_source.as_deref(),
            Some("clone")
        );
    }
}
