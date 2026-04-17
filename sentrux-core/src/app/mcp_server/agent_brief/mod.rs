use crate::app::mcp_server::handlers::{
    actions_from_findings_and_obligations, issues_from_findings_and_obligations, AgentAction,
    AgentIssue, RepairPacket,
};
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;
use serde_json::{json, Value};

mod policy;
mod render;
mod select;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBriefMode {
    RepoOnboarding,
    Patch,
    PreMerge,
}

impl AgentBriefMode {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("patch") {
            "repo_onboarding" => Ok(Self::RepoOnboarding),
            "patch" => Ok(Self::Patch),
            "pre_merge" => Ok(Self::PreMerge),
            other => Err(format!(
                "Unsupported mode '{other}'. Expected repo_onboarding, patch, or pre_merge."
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepoOnboarding => "repo_onboarding",
            Self::Patch => "patch",
            Self::PreMerge => "pre_merge",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum BriefSeverity {
    Low,
    Medium,
    High,
}

impl BriefSeverity {
    fn from_issue(issue: &AgentIssue) -> Self {
        match issue.severity {
            FindingSeverity::High => Self::High,
            FindingSeverity::Medium => Self::Medium,
            FindingSeverity::Low => Self::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BriefTrustTier {
    Trusted,
    Watchpoint,
    Experimental,
}

impl BriefTrustTier {
    fn from_issue(issue: &AgentIssue) -> Self {
        match issue.trust_tier.as_str() {
            "watchpoint" => Self::Watchpoint,
            "experimental" => Self::Experimental,
            _ => Self::Trusted,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BriefLeverageClass {
    SecondaryCleanup,
    LocalRefactorTarget,
    ArchitectureSignal,
    RegrowthWatchpoint,
    ToolingDebt,
    BoundaryDiscipline,
    HardeningNote,
    Experimental,
}

impl BriefLeverageClass {
    const fn is_high_leverage(self) -> bool {
        matches!(
            self,
            Self::ArchitectureSignal | Self::LocalRefactorTarget | Self::BoundaryDiscipline
        )
    }

    fn from_issue(issue: &AgentIssue) -> Self {
        match issue.leverage_class.as_str() {
            "local_refactor_target" => Self::LocalRefactorTarget,
            "architecture_signal" => Self::ArchitectureSignal,
            "regrowth_watchpoint" => Self::RegrowthWatchpoint,
            "tooling_debt" => Self::ToolingDebt,
            "boundary_discipline" => Self::BoundaryDiscipline,
            "hardening_note" => Self::HardeningNote,
            "experimental" => Self::Experimental,
            _ => Self::SecondaryCleanup,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentBriefInput {
    pub mode: AgentBriefMode,
    pub repo_shape: Value,
    pub findings: Vec<Value>,
    pub experimental_findings: Vec<Value>,
    pub missing_obligations: Vec<Value>,
    pub watchpoints: Vec<Value>,
    pub resolved_findings: Vec<Value>,
    pub changed_files: Vec<String>,
    pub changed_concepts: Vec<String>,
    pub decision: Option<String>,
    pub summary: Option<String>,
    pub confidence: Value,
    pub scan_trust: Value,
    pub freshness: Value,
    pub strict: Option<bool>,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
struct AgentBrief {
    kind: &'static str,
    mode: &'static str,
    decision: String,
    summary: String,
    repo_shape: Value,
    action_count: usize,
    actions: Vec<AgentAction>,
    primary_target_count: usize,
    primary_targets: Vec<AgentBriefTarget>,
    missing_obligation_count: usize,
    missing_obligations: Vec<Value>,
    watchpoint_count: usize,
    watchpoints: Vec<Value>,
    resolved_finding_count: usize,
    resolved_findings: Vec<Value>,
    do_not_chase_count: usize,
    do_not_chase: Vec<DeferredItem>,
    confidence: Value,
    scan_trust: Value,
    freshness: Value,
}

#[derive(Debug, Serialize)]
struct AgentBriefTarget {
    scope: String,
    kind: String,
    severity: BriefSeverity,
    trust_tier: BriefTrustTier,
    leverage_class: BriefLeverageClass,
    presentation_class: String,
    score_0_10000: u32,
    summary: String,
    blocking: bool,
    why_now: Vec<String>,
    likely_fix_sites: Vec<String>,
    inspection_focus: Vec<String>,
    repair_packet: RepairPacket,
    next_tools: Vec<NextToolCall>,
}

#[derive(Debug, Serialize)]
struct NextToolCall {
    tool: String,
    args: Value,
}

#[derive(Debug, Serialize)]
struct DeferredItem {
    scope: String,
    kind: String,
    reason: String,
    summary: String,
}

pub fn build_agent_brief(input: AgentBriefInput) -> Result<Value, String> {
    let ranked_issues =
        issues_from_findings_and_obligations(&input.findings, &input.missing_obligations);
    let primary_targets = match input.mode {
        AgentBriefMode::RepoOnboarding => select::select_onboarding_targets(&input, &ranked_issues),
        AgentBriefMode::Patch => select::select_patch_targets(&input, &ranked_issues),
        AgentBriefMode::PreMerge => select::select_pre_merge_targets(&input, &ranked_issues),
    };
    let primary_targets = select::visible_primary_targets(&input, primary_targets, &ranked_issues);

    let do_not_chase = policy::build_do_not_chase(&input);
    let decision = policy::decide(&input, &primary_targets);
    let summary = policy::summarize(&input, &decision, primary_targets.len());
    let actions = actions_from_findings_and_obligations(
        &input.findings,
        &input.missing_obligations,
        input.limit.max(1),
    );

    serde_json::to_value(AgentBrief {
        kind: "agent_brief",
        mode: input.mode.as_str(),
        decision,
        summary,
        repo_shape: summarize_repo_shape(&input.repo_shape),
        action_count: actions.len(),
        actions,
        primary_target_count: primary_targets.len(),
        primary_targets,
        missing_obligation_count: input.missing_obligations.len(),
        missing_obligations: input.missing_obligations,
        watchpoint_count: input.watchpoints.len(),
        watchpoints: truncate_values(input.watchpoints, input.limit),
        resolved_finding_count: input.resolved_findings.len(),
        resolved_findings: truncate_values(input.resolved_findings, input.limit),
        do_not_chase_count: do_not_chase.len(),
        do_not_chase,
        confidence: input.confidence,
        scan_trust: input.scan_trust,
        freshness: input.freshness,
    })
    .map_err(|error| format!("Failed to serialize agent brief: {error}"))
}

fn summarize_repo_shape(repo_shape: &Value) -> Value {
    let boundary_roots = repo_shape
        .get("boundary_roots")
        .and_then(Value::as_array)
        .map(|roots| {
            roots
                .iter()
                .filter_map(|root| root.get("root").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "primary_archetype": repo_shape.get("primary_archetype").cloned().unwrap_or(Value::Null),
        "effective_archetypes": repo_shape.get("effective_archetypes").cloned().unwrap_or(json!([])),
        "boundary_roots": boundary_roots,
        "starter_rules_toml": repo_shape.get("starter_rules_toml").cloned().unwrap_or(Value::Null),
        "working_rules_toml": repo_shape.get("working_rules_toml").cloned().unwrap_or(Value::Null),
    })
}

fn truncate_values(values: Vec<Value>, limit: usize) -> Vec<Value> {
    values.into_iter().take(limit.max(1)).collect()
}

fn string_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| default.to_string())
}

fn file_list(value: &Value) -> Vec<String> {
    let mut files = value
        .get("files")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if files.is_empty() {
        if let Some(path) = value.get("path").and_then(Value::as_str) {
            files.push(path.to_string());
        }
    }
    files
}

fn finding_scope(value: &Value) -> String {
    if let Some(scope) = value.get("scope").and_then(Value::as_str) {
        return scope.to_string();
    }
    if let Some(concept_id) = value.get("concept_id").and_then(Value::as_str) {
        return concept_id.to_string();
    }
    let files = file_list(value);
    if files.is_empty() {
        return string_field(value, "kind", "finding");
    }
    if files.len() == 1 {
        return files[0].clone();
    }
    files.join("|")
}
