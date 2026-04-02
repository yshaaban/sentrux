use super::*;
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;
use std::collections::BTreeMap;

const PREFERRED_ACCESSOR_PREFIX: &str = "preferred accessor: ";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IssueSource {
    Obligation,
    Structural,
    Clone,
    Rules,
}

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IssueOrigin {
    Explicit,
    ZeroConfig,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IssueConfidence {
    High,
    Medium,
    Experimental,
}

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentGate {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentIssue {
    pub(crate) scope: String,
    pub(crate) file: String,
    pub(crate) line: Option<u32>,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) fix_hint: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) source: IssueSource,
    pub(crate) origin: IssueOrigin,
    pub(crate) confidence: IssueConfidence,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentAction {
    pub(crate) priority: usize,
    pub(crate) scope: String,
    pub(crate) file: String,
    pub(crate) line: Option<u32>,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) fix_hint: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) blocking: bool,
    pub(crate) source: IssueSource,
    pub(crate) origin: IssueOrigin,
    pub(crate) confidence: IssueConfidence,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CheckAvailability {
    pub(crate) semantic: bool,
    pub(crate) evolution: bool,
    pub(crate) rules: bool,
    pub(crate) changed_scope: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CheckDiagnostics {
    pub(crate) errors: BTreeMap<String, Option<String>>,
    pub(crate) warnings: Vec<String>,
    pub(crate) partial_results: bool,
    pub(crate) availability: CheckAvailability,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentCheckResponse {
    pub(crate) issues: Vec<AgentIssue>,
    pub(crate) actions: Vec<AgentAction>,
    pub(crate) gate: AgentGate,
    pub(crate) summary: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) diagnostics: CheckDiagnostics,
}

pub(crate) fn to_agent_issue(finding: &Value) -> AgentIssue {
    let kind = finding_kind(finding).to_string();
    let files = finding_files(finding);
    let file = files.first().cloned().unwrap_or_default();
    AgentIssue {
        scope: issue_scope_for_value(finding, &files, &kind),
        file,
        line: finding
            .get("line")
            .and_then(|value| value.as_u64())
            .map(|line| line as u32),
        message: finding
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| finding_kind(finding))
            .to_string(),
        severity: severity_of_value(finding),
        fix_hint: fix_hint_for_value(finding, &kind),
        evidence: finding
            .get("evidence")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        source: issue_source_for_kind(&kind),
        origin: issue_origin_for_value(finding, &kind),
        confidence: issue_confidence_for_value(finding),
        kind,
    }
}

pub(crate) fn obligation_value_to_agent_issue(obligation: &Value) -> AgentIssue {
    let kind = obligation
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("missing_obligation")
        .to_string();
    let files = obligation_files(obligation);
    let file = files.first().cloned().unwrap_or_default();
    let scope = obligation
        .get("concept_id")
        .or_else(|| obligation.get("concept"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| files.first().cloned())
        .unwrap_or_else(|| kind.clone());

    AgentIssue {
        scope,
        file,
        line: obligation_line(obligation),
        kind: kind.clone(),
        message: obligation
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("Changed concept still has missing update sites")
            .to_string(),
        severity: obligation_severity(obligation),
        fix_hint: Some(
            "Update the missing sites tied to the changed concept before continuing.".to_string(),
        ),
        evidence: obligation_evidence(obligation),
        source: IssueSource::Obligation,
        origin: obligation_origin(obligation),
        confidence: obligation_confidence(obligation),
    }
}

fn fix_hint_for_value(finding: &Value, kind: &str) -> Option<String> {
    if kind == "forbidden_raw_read" {
        if let Some(accessor) = preferred_accessor_from_finding(finding) {
            return Some(format!(
                "Replace the raw read with {accessor} instead of recreating the projection in the caller."
            ));
        }
    }

    fix_hint_for_kind(kind)
}

fn preferred_accessor_from_finding(finding: &Value) -> Option<String> {
    finding
        .get("evidence")
        .and_then(Value::as_array)
        .and_then(|values| {
            values.iter().find_map(|value| {
                value
                    .as_str()
                    .and_then(|evidence| evidence.strip_prefix(PREFERRED_ACCESSOR_PREFIX))
                    .map(str::to_string)
            })
        })
}

pub(crate) fn issue_blocks_gate(issue: &AgentIssue) -> bool {
    match issue.source {
        IssueSource::Obligation => issue.severity == FindingSeverity::High,
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
            file: issue.file.clone(),
            line: issue.line,
            kind: issue.kind.clone(),
            message: issue.message.clone(),
            fix_hint: issue.fix_hint.clone(),
            evidence: issue.evidence.clone(),
            blocking: issue_blocks_gate(issue),
            source: issue.source,
            origin: issue.origin,
            confidence: issue.confidence,
        })
        .collect()
}

pub(crate) fn actions_from_findings_and_obligations(
    findings: &[Value],
    missing_obligations: &[Value],
    limit: usize,
) -> Vec<AgentAction> {
    let mut issues = missing_obligations
        .iter()
        .map(obligation_value_to_agent_issue)
        .collect::<Vec<_>>();
    issues.extend(findings.iter().map(to_agent_issue));
    issues.sort_by(compare_agent_issues);
    actions_from_issues(&issues, limit)
}

fn issue_source_for_kind(kind: &str) -> IssueSource {
    if matches!(
        kind,
        "large_file"
            | "dependency_sprawl"
            | "unstable_hotspot"
            | "cycle_cluster"
            | "missing_test_coverage"
            | "session_introduced_clone"
    ) {
        return IssueSource::Structural;
    }
    if matches!(kind, "exact_clone_group" | "clone_group" | "clone_family") {
        return IssueSource::Clone;
    }
    if kind == "closed_domain_exhaustiveness" {
        return IssueSource::Obligation;
    }
    IssueSource::Rules
}

fn obligation_origin(obligation: &Value) -> IssueOrigin {
    if matches!(
        obligation.get("origin").and_then(Value::as_str),
        Some("zero_config")
    ) || obligation.get("concept_id").is_none()
    {
        IssueOrigin::ZeroConfig
    } else {
        IssueOrigin::Explicit
    }
}

fn obligation_confidence(obligation: &Value) -> IssueConfidence {
    match obligation_origin(obligation) {
        IssueOrigin::Explicit => IssueConfidence::High,
        IssueOrigin::ZeroConfig => IssueConfidence::Medium,
    }
}

fn obligation_severity(obligation: &Value) -> FindingSeverity {
    if obligation
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "closed_domain_exhaustiveness")
        || obligation
            .get("missing_variants")
            .and_then(Value::as_array)
            .is_some_and(|variants| !variants.is_empty())
    {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    }
}

fn obligation_files(obligation: &Value) -> Vec<String> {
    let files = obligation
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !files.is_empty() {
        return files;
    }

    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| site.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn obligation_line(obligation: &Value) -> Option<u32> {
    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .and_then(|sites| {
            sites
                .iter()
                .find_map(|site| site.get("line").and_then(Value::as_u64))
        })
        .map(|line| line as u32)
}

fn obligation_evidence(obligation: &Value) -> Vec<String> {
    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| {
                    let path = site.get("path").and_then(Value::as_str)?;
                    let detail = site
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("missing site");
                    let line_suffix = site
                        .get("line")
                        .and_then(Value::as_u64)
                        .map(|line| format!(":{line}"))
                        .unwrap_or_default();
                    Some(format!("{path}{line_suffix} [{detail}]"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn issue_scope_for_value(finding: &Value, files: &[String], kind: &str) -> String {
    finding
        .get("scope")
        .or_else(|| finding.get("concept_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| files.first().cloned())
        .unwrap_or_else(|| kind.to_string())
}

fn right_gate_weight(issue: &AgentIssue) -> u8 {
    if issue_blocks_gate(issue) {
        1
    } else {
        0
    }
}

fn issue_source_weight(issue: &AgentIssue) -> u8 {
    match (issue.source, issue.origin) {
        (IssueSource::Rules, IssueOrigin::Explicit) => 4,
        (IssueSource::Obligation, _) => 3,
        (IssueSource::Rules, IssueOrigin::ZeroConfig) => 2,
        (IssueSource::Structural, _) => 1,
        (IssueSource::Clone, _) => 0,
    }
}

fn issue_confidence_weight(issue: &AgentIssue) -> u8 {
    match issue.confidence {
        IssueConfidence::High => 2,
        IssueConfidence::Medium => 1,
        IssueConfidence::Experimental => 0,
    }
}

pub(crate) fn compare_agent_issues(left: &AgentIssue, right: &AgentIssue) -> std::cmp::Ordering {
    right_gate_weight(right)
        .cmp(&right_gate_weight(left))
        .then_with(|| issue_source_weight(right).cmp(&issue_source_weight(left)))
        .then_with(|| right.severity.priority().cmp(&left.severity.priority()))
        .then_with(|| issue_confidence_weight(right).cmp(&issue_confidence_weight(left)))
        .then_with(|| left.file.cmp(&right.file))
        .then_with(|| left.kind.cmp(&right.kind))
}

fn issue_origin_for_value(finding: &Value, kind: &str) -> IssueOrigin {
    if kind == "zero_config_boundary_violation" || kind == "missing_test_coverage" {
        return IssueOrigin::ZeroConfig;
    }
    if matches!(
        finding.get("origin").and_then(|value| value.as_str()),
        Some("zero_config")
    ) {
        return IssueOrigin::ZeroConfig;
    }
    IssueOrigin::Explicit
}

fn issue_confidence_for_value(finding: &Value) -> IssueConfidence {
    match finding.get("trust_tier").and_then(|value| value.as_str()) {
        Some("experimental") => IssueConfidence::Experimental,
        Some("watchpoint") => IssueConfidence::Medium,
        _ => match finding.get("confidence").and_then(|value| value.as_str()) {
            Some("experimental") => IssueConfidence::Experimental,
            Some("medium") => IssueConfidence::Medium,
            _ => IssueConfidence::High,
        },
    }
}

fn fix_hint_for_kind(kind: &str) -> Option<String> {
    let hint = match kind {
        "forbidden_raw_read" => {
            "Route the read through the concept's canonical accessor instead of reading raw state."
        }
        "forbidden_writer" | "writer_outside_allowlist" => {
            "Move the write behind an allowed writer or update the rule if the new writer is intentional."
        }
        "multi_writer_concept" => {
            "Reduce the concept to one authoritative writer or document the additional writer explicitly."
        }
        "closed_domain_exhaustiveness" => {
            "Update the switch or mapping so every domain variant is handled."
        }
        "state_model_missing_exhaustive_switch" | "state_model_missing_assert_never" => {
            "Restore the exhaustive switch and assert-never guard for the state model."
        }
        "large_file" => "Split the file along the boundary suggested by the evidence and keep the public surface thin.",
        "dependency_sprawl" => {
            "Extract a narrower facade or move behavior behind an existing module boundary."
        }
        "unstable_hotspot" => "Stabilize the hotspot before adding more change pressure.",
        "cycle_cluster" => "Cut the highest-leverage cycle seam first and re-run check.",
        "exact_clone_group" | "clone_group" | "clone_family" => {
            "Extract shared behavior or collapse the duplicated flow."
        }
        "session_introduced_clone" => {
            "Collapse the new duplicate now: extract the shared behavior or route both call sites through the same owner before they drift."
        }
        "missing_test_coverage" => "Add a sibling test covering the new production surface.",
        "zero_config_boundary_violation" => {
            "Replace the deep import with the module's public API."
        }
        _ => return None,
    };
    Some(hint.to_string())
}
