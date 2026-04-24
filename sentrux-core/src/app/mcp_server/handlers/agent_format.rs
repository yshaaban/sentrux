use super::agent_guidance::{
    fix_hint_for_value, obligation_confidence, obligation_evidence, obligation_files,
    obligation_fix_hint, obligation_line, obligation_message, obligation_origin,
    obligation_score_0_10000, obligation_severity, obligation_trust_tier,
    repair_packet_for_finding, repair_packet_for_obligation,
};
use super::evaluation_signals::CheckSignalSummary;
use super::*;
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;
use std::collections::BTreeMap;

pub(crate) use super::agent_guidance::RepairPacket;
pub(crate) use super::agent_ranking::{
    actions_from_findings_and_obligations, actions_from_issues, compare_agent_issues,
    issue_blocks_gate, issue_is_default_lane_eligible, issues_from_findings_and_obligations,
};

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
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

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) concept_id: Option<String>,
    pub(crate) file: String,
    pub(crate) line: Option<u32>,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) trust_tier: String,
    pub(crate) presentation_class: String,
    pub(crate) leverage_class: String,
    pub(crate) score_0_10000: u32,
    pub(crate) fix_hint: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) source: IssueSource,
    pub(crate) origin: IssueOrigin,
    pub(crate) confidence: IssueConfidence,
    #[serde(skip_serializing_if = "AgentIssueEvidence::is_empty", default)]
    pub(crate) evidence_metrics: AgentIssueEvidence,
    pub(crate) repair_packet: RepairPacket,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentAction {
    pub(crate) priority: usize,
    pub(crate) scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) concept_id: Option<String>,
    pub(crate) file: String,
    pub(crate) line: Option<u32>,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) trust_tier: String,
    pub(crate) presentation_class: String,
    pub(crate) leverage_class: String,
    pub(crate) score_0_10000: u32,
    pub(crate) fix_hint: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) blocking: bool,
    pub(crate) source: IssueSource,
    pub(crate) origin: IssueOrigin,
    pub(crate) confidence: IssueConfidence,
    pub(crate) why_now: Vec<String>,
    #[serde(skip_serializing_if = "AgentIssueEvidence::is_empty", default)]
    pub(crate) evidence_metrics: AgentIssueEvidence,
    pub(crate) repair_packet: RepairPacket,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct AgentIssueEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default_rollout_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signal_treatment_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) patch_directly_worsened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) changed_scope: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signal_treatment_intervention_net_value_score_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_action_help_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_action_follow_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reviewer_acceptance_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remediation_success_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) task_success_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) intervention_net_value_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reviewed_precision: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_1_actionable_precision: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_3_actionable_precision: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reviewer_disagreement_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) patch_expansion_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) intervention_cost_checks_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) review_noise_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repair_packet_complete_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repair_packet_fix_surface_clear_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repair_packet_verification_clear_rate: Option<f64>,
}

impl AgentIssueEvidence {
    fn is_empty(&self) -> bool {
        self.default_rollout_ready.is_none()
            && self.signal_treatment_ready.is_none()
            && self.patch_directly_worsened.is_none()
            && self.changed_scope.is_none()
            && self
                .signal_treatment_intervention_net_value_score_delta
                .is_none()
            && self.top_action_help_rate.is_none()
            && self.top_action_follow_rate.is_none()
            && self.reviewer_acceptance_rate.is_none()
            && self.remediation_success_rate.is_none()
            && self.task_success_rate.is_none()
            && self.intervention_net_value_score.is_none()
            && self.reviewed_precision.is_none()
            && self.top_1_actionable_precision.is_none()
            && self.top_3_actionable_precision.is_none()
            && self.reviewer_disagreement_rate.is_none()
            && self.patch_expansion_rate.is_none()
            && self.intervention_cost_checks_mean.is_none()
            && self.review_noise_rate.is_none()
            && self.repair_packet_complete_rate.is_none()
            && self.repair_packet_fix_surface_clear_rate.is_none()
            && self.repair_packet_verification_clear_rate.is_none()
    }
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
    pub(crate) signal_summary: CheckSignalSummary,
    pub(crate) gate: AgentGate,
    pub(crate) summary: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) diagnostics: CheckDiagnostics,
}

pub(crate) fn finding_with_agent_guidance(mut finding: Value) -> Value {
    if !finding.is_object() {
        return finding;
    }

    let issue = to_agent_issue(&finding);
    if let Some(object) = finding.as_object_mut() {
        insert_agent_guidance_fields(object, issue);
    }

    finding
}

pub(crate) fn findings_with_agent_guidance(findings: Vec<Value>) -> Vec<Value> {
    findings
        .into_iter()
        .map(finding_with_agent_guidance)
        .collect()
}

pub(crate) fn obligation_with_agent_guidance(mut obligation: Value) -> Value {
    if !obligation.is_object() {
        return obligation;
    }

    let issue = obligation_value_to_agent_issue(&obligation);
    if let Some(object) = obligation.as_object_mut() {
        insert_agent_guidance_fields(object, issue);
    }

    obligation
}

pub(crate) fn obligations_with_agent_guidance(obligations: Vec<Value>) -> Vec<Value> {
    obligations
        .into_iter()
        .map(obligation_with_agent_guidance)
        .collect()
}

fn insert_agent_guidance_fields(object: &mut serde_json::Map<String, Value>, issue: AgentIssue) {
    let repair_packet = issue.repair_packet;

    if object.get("repair_packet").is_none() {
        object.insert("repair_packet".to_string(), json!(&repair_packet));
    }
    if object.get("fix_hint").is_none() {
        if let Some(fix_hint) = issue.fix_hint {
            object.insert("fix_hint".to_string(), json!(fix_hint));
        }
    }
    if object.get("likely_fix_sites").is_none() && !repair_packet.likely_fix_sites.is_empty() {
        object.insert(
            "likely_fix_sites".to_string(),
            json!(&repair_packet.likely_fix_sites),
        );
    }
    if object.get("verification_steps").is_none() && !repair_packet.verification_steps.is_empty() {
        object.insert(
            "verification_steps".to_string(),
            json!(&repair_packet.verification_steps),
        );
    }
    if object.get("smallest_safe_first_cut").is_none() {
        if let Some(smallest_safe_first_cut) = repair_packet.smallest_safe_first_cut {
            object.insert(
                "smallest_safe_first_cut".to_string(),
                json!(smallest_safe_first_cut),
            );
        }
    }
}

const PATCH_WORSENED_FIELD_KEYS: &[&str] = &[
    "patch_directly_worsened",
    "patch_worsened",
    "current_patch_worsened",
    "introduced_by_patch",
    "session_introduced",
];
const TREATMENT_NET_VALUE_DELTA_FIELD_KEYS: &[&str] = &[
    "signal_treatment_intervention_net_value_score_delta",
    "intervention_net_value_score_delta",
];
const PATCH_EXPANSION_COST_FIELD_KEYS: &[&str] =
    &["patch_expansion_cost", "intervention_cost_checks_mean"];

pub(crate) fn to_agent_issue(finding: &Value) -> AgentIssue {
    let finding = decorate_finding_with_classification(finding);
    let kind = canonical_issue_kind(finding_kind(&finding)).to_string();
    let files = finding_files(&finding);
    let file = files.first().cloned().unwrap_or_default();
    let repair_packet = repair_packet_for_finding(&finding, &kind);
    let evidence_metrics = issue_evidence_for_value(&finding);
    AgentIssue {
        scope: issue_scope_for_value(&finding, &files, &kind),
        concept_id: finding
            .get("concept_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        file,
        line: finding
            .get("line")
            .and_then(|value| value.as_u64())
            .map(|line| line as u32),
        message: finding
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| finding_kind(&finding))
            .to_string(),
        severity: severity_of_value(&finding),
        trust_tier: finding
            .get("trust_tier")
            .and_then(Value::as_str)
            .unwrap_or("trusted")
            .to_string(),
        presentation_class: finding
            .get("presentation_class")
            .and_then(Value::as_str)
            .unwrap_or("structural_debt")
            .to_string(),
        leverage_class: finding
            .get("leverage_class")
            .and_then(Value::as_str)
            .unwrap_or("secondary_cleanup")
            .to_string(),
        score_0_10000: finding
            .get("score_0_10000")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .unwrap_or_default(),
        fix_hint: fix_hint_for_value(&finding, &kind),
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
        origin: issue_origin_for_value(&finding, &kind),
        confidence: issue_confidence_for_value(&finding, &kind),
        evidence_metrics,
        repair_packet,
        kind,
    }
}

pub(crate) fn obligation_value_to_agent_issue(obligation: &Value) -> AgentIssue {
    let kind = obligation
        .get("kind")
        .and_then(Value::as_str)
        .map(derived_obligation_issue_kind)
        .unwrap_or("missing_obligation")
        .to_string();
    let files = obligation_files(obligation);
    let file = files.first().cloned().unwrap_or_default();
    let concept_id = obligation_concept_id(obligation);
    let scope = obligation_scope(obligation, &files, &kind, concept_id.as_deref());

    AgentIssue {
        scope,
        concept_id,
        file,
        line: obligation_line(obligation),
        kind: kind.clone(),
        message: obligation_message(obligation, &kind),
        severity: obligation_severity(obligation),
        trust_tier: obligation_trust_tier(obligation).to_string(),
        presentation_class: "hardening_note".to_string(),
        leverage_class: "hardening_note".to_string(),
        score_0_10000: obligation_score_0_10000(obligation),
        fix_hint: obligation_fix_hint(obligation, &kind),
        evidence: obligation_evidence(obligation),
        source: IssueSource::Obligation,
        origin: obligation_origin(obligation),
        confidence: obligation_confidence(obligation),
        evidence_metrics: AgentIssueEvidence::default(),
        repair_packet: repair_packet_for_obligation(obligation, &kind),
    }
}

fn issue_evidence_for_value(finding: &Value) -> AgentIssueEvidence {
    AgentIssueEvidence {
        default_rollout_ready: default_rollout_ready_field(finding),
        signal_treatment_ready: boolean_field(finding, &["signal_treatment_ready"]),
        patch_directly_worsened: boolean_field(finding, PATCH_WORSENED_FIELD_KEYS),
        changed_scope: boolean_field(finding, &["changed_scope"]),
        signal_treatment_intervention_net_value_score_delta: number_field(
            finding,
            TREATMENT_NET_VALUE_DELTA_FIELD_KEYS,
        ),
        top_action_help_rate: number_field(finding, &["top_action_help_rate"]),
        top_action_follow_rate: number_field(finding, &["top_action_follow_rate"]),
        reviewer_acceptance_rate: number_field(finding, &["reviewer_acceptance_rate"]),
        remediation_success_rate: number_field(finding, &["remediation_success_rate"]),
        task_success_rate: number_field(finding, &["task_success_rate"]),
        intervention_net_value_score: number_field(finding, &["intervention_net_value_score"]),
        reviewed_precision: number_field(finding, &["reviewed_precision"]),
        top_1_actionable_precision: number_field(finding, &["top_1_actionable_precision"]),
        top_3_actionable_precision: number_field(finding, &["top_3_actionable_precision"]),
        reviewer_disagreement_rate: number_field(finding, &["reviewer_disagreement_rate"]),
        patch_expansion_rate: number_field(finding, &["patch_expansion_rate"]),
        intervention_cost_checks_mean: number_field(finding, PATCH_EXPANSION_COST_FIELD_KEYS),
        review_noise_rate: number_field(finding, &["review_noise_rate"]),
        repair_packet_complete_rate: number_field(finding, &["repair_packet_complete_rate"]),
        repair_packet_fix_surface_clear_rate: number_field(
            finding,
            &["repair_packet_fix_surface_clear_rate"],
        ),
        repair_packet_verification_clear_rate: number_field(
            finding,
            &["repair_packet_verification_clear_rate"],
        ),
    }
}

fn default_rollout_ready_field(finding: &Value) -> Option<bool> {
    first_field_value(finding, &["default_rollout_recommendation"])
        .and_then(Value::as_str)
        .map(|value| value == "ready_for_default_on")
}

fn number_field(value: &Value, keys: &[&str]) -> Option<f64> {
    first_field_value(value, keys).and_then(value_to_number)
}

fn value_to_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
        .or_else(|| value.as_u64().map(|number| number as f64))
        .or_else(|| value.as_str().and_then(|number| number.parse::<f64>().ok()))
}

fn boolean_field(value: &Value, keys: &[&str]) -> Option<bool> {
    first_field_value(value, keys).and_then(value_to_bool)
}

fn first_field_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(key))
}

fn value_to_bool(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value.as_u64().map(|number| number > 0))
        .or_else(|| value.as_i64().map(|number| number > 0))
        .or_else(|| {
            value
                .as_str()
                .and_then(|flag| match flag.trim().to_ascii_lowercase().as_str() {
                    "true" | "yes" | "ready" => Some(true),
                    "false" | "no" | "pending" => Some(false),
                    _ => None,
                })
        })
}

fn obligation_concept_id(obligation: &Value) -> Option<String> {
    obligation
        .get("concept_id")
        .and_then(Value::as_str)
        .or_else(|| obligation.get("concept").and_then(Value::as_str))
        .map(str::to_string)
}

fn obligation_scope(
    obligation: &Value,
    files: &[String],
    kind: &str,
    concept_id: Option<&str>,
) -> String {
    concept_id
        .map(str::to_string)
        .or_else(|| {
            obligation
                .get("domain_symbol_name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| files.first().cloned())
        .unwrap_or_else(|| kind.to_string())
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
    if matches!(
        kind,
        "session_introduced_clone" | "clone_propagation_drift" | "touched_clone_family"
    ) {
        return IssueSource::Clone;
    }
    if matches!(kind, "exact_clone_group" | "clone_group" | "clone_family") {
        return IssueSource::Clone;
    }
    if matches!(
        kind,
        "closed_domain_exhaustiveness" | "incomplete_propagation"
    ) {
        return IssueSource::Obligation;
    }
    IssueSource::Rules
}

fn derived_obligation_issue_kind(kind: &str) -> &str {
    canonical_issue_kind(kind)
}

fn canonical_issue_kind(kind: &str) -> &str {
    match kind {
        "contract_surface_completeness" => "incomplete_propagation",
        _ => kind,
    }
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

fn issue_confidence_for_value(finding: &Value, kind: &str) -> IssueConfidence {
    match finding.get("trust_tier").and_then(|value| value.as_str()) {
        Some("experimental") => IssueConfidence::Experimental,
        Some("watchpoint") => IssueConfidence::Medium,
        _ => match finding.get("confidence").and_then(|value| value.as_str()) {
            Some("experimental") => IssueConfidence::Experimental,
            Some("medium") => IssueConfidence::Medium,
            _ => match issue_origin_for_value(finding, kind) {
                IssueOrigin::Explicit => IssueConfidence::High,
                IssueOrigin::ZeroConfig => IssueConfidence::Medium,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{obligation_value_to_agent_issue, obligation_with_agent_guidance, to_agent_issue};
    use crate::metrics::v2::FindingSeverity;
    use serde_json::json;

    #[test]
    fn closed_domain_exhaustiveness_guidance_names_the_missing_site_and_variants() {
        let issue = obligation_value_to_agent_issue(&json!({
            "kind": "closed_domain_exhaustiveness",
            "concept_id": "task_presentation_status",
            "domain_symbol_name": "TaskPresentationStatus",
            "summary": "Domain 'TaskPresentationStatus' is missing exhaustive handling.",
            "files": ["src/app/task-presentation-status.ts"],
            "missing_variants": ["loading", "ready"],
            "missing_sites": [
                {
                    "path": "src/app/task-presentation-status.ts",
                    "kind": "closed_domain",
                    "line": 27,
                    "detail": "missing exhaustive branch"
                }
            ]
        }));

        assert_eq!(issue.kind, "closed_domain_exhaustiveness");
        assert!(
            issue.message.contains("TaskPresentationStatus"),
            "unexpected message: {}",
            issue.message
        );
        assert!(issue.message.contains("loading"));
        assert!(issue.message.contains("ready"));
        assert!(issue
            .message
            .contains("src/app/task-presentation-status.ts:27"));
        assert!(issue.fix_hint.as_deref().is_some_and(|hint| {
            hint.contains("explicit exhaustive switch or mapping")
                && hint.contains("src/app/task-presentation-status.ts:27")
                && hint.contains("fallback/default path")
        }));
    }

    #[test]
    fn closed_domain_exhaustiveness_finding_guidance_disallows_fallback_branches() {
        let issue = to_agent_issue(&json!({
            "kind": "closed_domain_exhaustiveness",
            "summary": "Domain 'TaskPresentationStatus' still needs exhaustive handling.",
            "files": ["src/app/task-presentation-status.ts"]
        }));

        assert!(issue.fix_hint.as_deref().is_some_and(|hint| {
            hint.contains("explicit exhaustive switch or mapping")
                && hint.contains("fallback/default branch")
        }));
    }

    #[test]
    fn large_file_guidance_uses_split_axis_and_related_surface_when_available() {
        let issue = to_agent_issue(&json!({
            "kind": "large_file",
            "summary": "File 'scripts/evals/run-repo-calibration-loop.mjs' is 1068 lines, above the javascript threshold of 500",
            "files": ["scripts/evals/run-repo-calibration-loop.mjs"],
            "candidate_split_axes": ["shared helper boundary"],
            "related_surfaces": ["scripts/lib/eval-batch.mjs"]
        }));

        assert_eq!(
            issue.fix_hint.as_deref(),
            Some(
                "Split the file along the shared helper boundary and move the behavior that couples to scripts/lib/eval-batch.mjs behind a smaller owner before adding more code here."
            )
        );
    }

    #[test]
    fn large_file_guidance_uses_multiple_axes_and_surfaces_when_present() {
        let issue = to_agent_issue(&json!({
            "kind": "large_file",
            "summary": "File 'src/App.tsx' is 620 lines, above the typescript threshold of 500",
            "files": ["src/App.tsx"],
            "candidate_split_axes": [
                "components dependency boundary",
                "providers dependency boundary"
            ],
            "related_surfaces": [
                "src/components/app-shell/Chrome.tsx",
                "src/providers/runtime.ts"
            ]
        }));

        assert_eq!(
            issue.fix_hint.as_deref(),
            Some(
                "Split the file along the components dependency boundary and the providers dependency boundary and move the behavior that couples to src/components/app-shell/Chrome.tsx and src/providers/runtime.ts behind smaller owners before adding more code here."
            )
        );
    }

    #[test]
    fn large_file_guidance_ignores_test_only_related_surfaces() {
        let issue = to_agent_issue(&json!({
            "kind": "large_file",
            "summary": "Guarded facade file 'src/components/terminal-session.ts' is 720 lines, above the typescript threshold of 500",
            "files": ["src/components/terminal-session.ts"],
            "candidate_split_axes": ["facade owner boundary"],
            "related_surfaces": [
                "src/components/terminal-session.architecture.test.ts"
            ]
        }));

        assert_eq!(
            issue.fix_hint.as_deref(),
            Some(
                "Split the file along the facade owner boundary and keep the public surface thin."
            )
        );
    }

    #[test]
    fn concept_only_obligation_stays_explicit_and_trusted() {
        let issue = obligation_value_to_agent_issue(&json!({
            "kind": "incomplete_propagation",
            "concept": "task_status",
            "summary": "Task status updates are still incomplete.",
            "files": ["src/task-status.ts"],
            "missing_sites": [
                {
                    "path": "src/task-status.ts",
                    "kind": "concept_followthrough",
                    "line": 42,
                    "detail": "missing propagation"
                }
            ]
        }));

        assert_eq!(issue.concept_id.as_deref(), Some("task_status"));
        assert_eq!(issue.origin, super::IssueOrigin::Explicit);
        assert_eq!(issue.confidence, super::IssueConfidence::High);
        assert_eq!(issue.trust_tier, "trusted");
        assert_eq!(issue.severity, FindingSeverity::High);
    }

    #[test]
    fn obligation_guidance_is_attached_to_raw_public_surfaces() {
        let obligation = obligation_with_agent_guidance(json!({
            "kind": "incomplete_propagation",
            "concept_id": "agent_guidance",
            "files": ["src/app/mcp_server/handlers/session_response.rs"],
            "missing_sites": [
                {
                    "path": "src/app/mcp_server/handlers/session_response.rs",
                    "kind": "required_file",
                    "detail": "update required DTO surface"
                }
            ]
        }));

        assert_eq!(
            obligation["likely_fix_sites"],
            json!(["src/app/mcp_server/handlers/session_response.rs"])
        );
        assert_eq!(
            obligation["repair_packet"]["required_fields"]["repair_surface"],
            json!(true)
        );
        assert!(obligation["fix_hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("DTO surface")));
    }

    #[test]
    fn generic_file_context_does_not_count_as_a_repair_surface() {
        let issue = to_agent_issue(&json!({
            "kind": "dead_private_code_cluster",
            "summary": "Private code is no longer referenced from live callers.",
            "files": ["src/app.ts"],
            "trust_tier": "trusted",
            "leverage_class": "secondary_cleanup",
            "score_0_10000": 9200
        }));

        assert_eq!(issue.repair_packet.likely_fix_sites, Vec::<String>::new());
        assert_eq!(issue.repair_packet.inspection_context, vec!["src/app.ts"]);
        assert_eq!(issue.repair_packet.complete, false);
        assert_eq!(issue.repair_packet.required_fields.repair_surface, false);
        assert!(issue
            .repair_packet
            .missing_fields
            .iter()
            .any(|field| field == "repair_surface"));
    }

    #[test]
    fn dependency_sprawl_uses_the_owner_file_as_a_concrete_repair_surface() {
        let issue = to_agent_issue(&json!({
            "kind": "dependency_sprawl",
            "summary": "Entry surface fans out across too many owners.",
            "files": ["src/app.ts"],
            "trust_tier": "trusted",
            "leverage_class": "architecture_signal",
            "score_0_10000": 9200
        }));

        assert_eq!(issue.repair_packet.likely_fix_sites, vec!["src/app.ts"]);
        assert_eq!(issue.repair_packet.complete, true);
        assert_eq!(issue.repair_packet.required_fields.repair_surface, true);
    }

    #[test]
    fn cycle_cluster_extracts_repair_surface_from_best_cut_evidence() {
        let issue = to_agent_issue(&json!({
            "kind": "cycle_cluster",
            "summary": "Files src/a.ts and src/b.ts form a dependency cycle.",
            "files": ["src/a.ts", "src/b.ts"],
            "evidence": [
                "best cut candidate: src/a.ts -> src/b.ts (removes 2 cyclic files)"
            ]
        }));

        assert_eq!(
            issue.repair_packet.likely_fix_sites,
            vec!["src/a.ts", "src/b.ts"]
        );
        assert_eq!(issue.repair_packet.complete, true);
    }

    #[test]
    fn clone_propagation_drift_uses_changed_and_unchanged_clone_sites_as_repair_surface() {
        let issue = to_agent_issue(&json!({
            "kind": "clone_propagation_drift",
            "summary": "The changed clone path no longer matches its unchanged sibling.",
            "files": ["src/source.ts", "src/copy.ts"],
            "trust_tier": "trusted",
            "leverage_class": "architecture_signal",
            "score_0_10000": 9100,
            "evidence": [
                "changed clone member: src/source.ts::renderStatus",
                "unchanged clone sibling: src/copy.ts::renderStatus"
            ]
        }));

        assert_eq!(
            issue.repair_packet.likely_fix_sites,
            vec!["src/source.ts::renderStatus", "src/copy.ts::renderStatus"]
        );
        assert_eq!(issue.repair_packet.complete, true);
    }

    #[test]
    fn propagation_obligation_prefers_production_sites_and_preserves_surface_specific_guidance() {
        let issue = obligation_value_to_agent_issue(&json!({
            "kind": "incomplete_propagation",
            "concept_id": "agent_guidance",
            "files": [
                "docs/agent-brief.md",
                "src/app/mcp_server/handlers/session_response.rs",
                "src/app/mcp_server/handlers/bootstrap-registry.rs"
            ],
            "missing_sites": [
                {
                    "path": "docs/agent-brief.md",
                    "kind": "required_file",
                    "detail": "update required test/doc surface"
                },
                {
                    "path": "src/app/mcp_server/handlers/session_response.rs",
                    "kind": "required_file",
                    "detail": "update required DTO surface"
                },
                {
                    "path": "src/app/mcp_server/handlers/bootstrap-registry.rs",
                    "kind": "required_symbol",
                    "line": 12,
                    "detail": "update required registry surface"
                }
            ]
        }));

        assert_eq!(issue.line, Some(12));
        assert_eq!(
            issue.evidence,
            vec![
                "src/app/mcp_server/handlers/bootstrap-registry.rs:12 [update required registry surface]"
                    .to_string(),
                "src/app/mcp_server/handlers/session_response.rs [update required DTO surface]"
                    .to_string(),
                "docs/agent-brief.md [update required test/doc surface]".to_string(),
            ]
        );
        assert_eq!(
            issue.repair_packet.likely_fix_sites,
            vec![
                "src/app/mcp_server/handlers/bootstrap-registry.rs:12".to_string(),
                "src/app/mcp_server/handlers/session_response.rs".to_string(),
                "docs/agent-brief.md".to_string(),
            ]
        );
        assert!(issue
            .fix_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("registry, DTO, and test/doc surfaces")));
    }
}
