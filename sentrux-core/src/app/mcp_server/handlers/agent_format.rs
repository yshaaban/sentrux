use super::*;
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;
use std::collections::BTreeMap;

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
        fix_hint: fix_hint_for_kind(&kind),
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

pub(crate) fn issue_blocks_gate(issue: &AgentIssue) -> bool {
    match issue.source {
        IssueSource::Obligation => issue.severity == FindingSeverity::High,
        IssueSource::Rules => {
            issue.origin == IssueOrigin::Explicit && issue.severity.priority() >= 2
        }
        IssueSource::Structural | IssueSource::Clone => false,
    }
}

fn issue_source_for_kind(kind: &str) -> IssueSource {
    if matches!(
        kind,
        "large_file"
            | "dependency_sprawl"
            | "unstable_hotspot"
            | "cycle_cluster"
            | "missing_test_coverage"
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
        "missing_test_coverage" => "Add a sibling test covering the new production surface.",
        "zero_config_boundary_violation" => {
            "Replace the deep import with the module's public API."
        }
        _ => return None,
    };
    Some(hint.to_string())
}
