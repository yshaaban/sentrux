use crate::app::mcp_server::handlers::{
    actions_from_findings_and_obligations, issue_blocks_gate, issues_from_findings_and_obligations,
    AgentAction, AgentIssue, IssueSource, RepairPacket,
};
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

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
    let mut primary_targets = match input.mode {
        AgentBriefMode::RepoOnboarding => select_onboarding_targets(&input, &ranked_issues),
        AgentBriefMode::Patch => select_patch_targets(&input, &ranked_issues),
        AgentBriefMode::PreMerge => select_pre_merge_targets(&input, &ranked_issues),
    };
    primary_targets.truncate(input.limit.max(1));

    let do_not_chase = build_do_not_chase(&input);
    let decision = decide(&input, &primary_targets);
    let summary = summarize(&input, &decision, primary_targets.len());
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

fn select_onboarding_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    select_primary_targets(ranked_issues, input, false)
}

fn select_patch_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    select_primary_targets(ranked_issues, input, true)
}

fn select_pre_merge_targets(
    input: &AgentBriefInput,
    ranked_issues: &[AgentIssue],
) -> Vec<AgentBriefTarget> {
    select_primary_targets(ranked_issues, input, true)
}

fn select_primary_targets(
    ranked_issues: &[AgentIssue],
    input: &AgentBriefInput,
    patch_scope_only: bool,
) -> Vec<AgentBriefTarget> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();

    for issue in ranked_issues {
        if !issue_is_primary_target(issue, input, patch_scope_only) {
            continue;
        }
        let target = target_from_issue(issue, input);
        let scope_key = format!("{}:{}", target.scope, target.kind);
        if seen.insert(scope_key) {
            selected.push(target);
        }
        if selected.len() >= input.limit.max(1) {
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
    if patch_scope_only && !issue_is_patch_relevant(issue, input) {
        return false;
    }

    match input.mode {
        AgentBriefMode::RepoOnboarding => {
            issue_blocks_gate(issue)
                || issue.repair_packet.completeness_0_10000 >= 8_000
                || issue.score_0_10000 >= 7_000
                || matches!(
                    issue.leverage_class.as_str(),
                    "boundary_discipline" | "architecture_signal" | "local_refactor_target"
                )
        }
        AgentBriefMode::Patch => {
            issue_blocks_gate(issue)
                || issue.source == IssueSource::Obligation
                || issue.repair_packet.completeness_0_10000 >= 8_000
        }
        AgentBriefMode::PreMerge => {
            issue_blocks_gate(issue)
                || issue.severity == FindingSeverity::High
                || (input.strict.unwrap_or(false) && issue.severity == FindingSeverity::Medium)
        }
    }
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

fn target_from_issue(issue: &AgentIssue, input: &AgentBriefInput) -> AgentBriefTarget {
    let likely_fix_sites = issue.repair_packet.likely_fix_sites.clone();
    let summary = format!("{} ({})", issue.message, signal_band(issue.score_0_10000));
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
    if issue.repair_packet.completeness_0_10000 >= 8_000 {
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

fn build_do_not_chase(input: &AgentBriefInput) -> Vec<DeferredItem> {
    let mut deferred = input
        .experimental_findings
        .iter()
        .take(input.limit.max(1))
        .map(|finding| DeferredItem {
            scope: finding_scope(finding),
            kind: string_field(finding, "kind", "finding"),
            reason: "experimental_detector".to_string(),
            summary: string_field(finding, "summary", "Finding is still experimental"),
        })
        .collect::<Vec<_>>();

    if matches!(input.mode, AgentBriefMode::Patch | AgentBriefMode::PreMerge) {
        deferred.extend(
            input
                .watchpoints
                .iter()
                .filter(|watchpoint| {
                    !watchpoint_is_patch_relevant(watchpoint, &input.changed_concepts)
                })
                .take(input.limit.max(1))
                .map(|watchpoint| DeferredItem {
                    scope: finding_scope(watchpoint),
                    kind: string_field(watchpoint, "kind", "watchpoint"),
                    reason: "watchpoint_not_blocking".to_string(),
                    summary: string_field(
                        watchpoint,
                        "summary",
                        "Non-blocking watchpoint outside the current patch focus",
                    ),
                }),
        );
    }

    deferred.truncate(input.limit.max(1));
    deferred
}

fn decide(input: &AgentBriefInput, primary_targets: &[AgentBriefTarget]) -> String {
    match input.mode {
        AgentBriefMode::RepoOnboarding => {
            if primary_targets
                .iter()
                .any(|target| target.trust_tier == BriefTrustTier::Trusted)
            {
                "fix_now".to_string()
            } else {
                "continue".to_string()
            }
        }
        AgentBriefMode::Patch => {
            if !input.missing_obligations.is_empty()
                || primary_targets.iter().any(|target| {
                    target.severity == BriefSeverity::High
                        && target.trust_tier == BriefTrustTier::Trusted
                })
            {
                "fix_now".to_string()
            } else {
                "continue".to_string()
            }
        }
        AgentBriefMode::PreMerge => match input.decision.as_deref() {
            Some("fail") => "block".to_string(),
            Some("warn") => "fix_now".to_string(),
            _ => "continue".to_string(),
        },
    }
}

fn summarize(input: &AgentBriefInput, decision: &str, primary_target_count: usize) -> String {
    if let Some(summary) = &input.summary {
        return summary.clone();
    }

    match input.mode {
        AgentBriefMode::RepoOnboarding => {
            let primary_archetype = input
                .repo_shape
                .get("primary_archetype")
                .and_then(Value::as_str)
                .unwrap_or("repo");
            format!(
                "{primary_archetype} onboarding brief with {primary_target_count} primary target(s) and {} watchpoint(s)",
                input.watchpoints.len()
            )
        }
        AgentBriefMode::Patch => format!(
            "Patch brief: decision={decision}, {} primary target(s), {} missing obligation(s), {} changed file(s), {} changed concept(s)",
            primary_target_count,
            input.missing_obligations.len(),
            input.changed_files.len(),
            input.changed_concepts.len()
        ),
        AgentBriefMode::PreMerge => {
            let strict = input.strict.unwrap_or(false);
            format!(
                "Pre-merge brief: decision={decision}, strict={strict}, {} primary target(s), {} missing obligation(s), {} changed file(s)",
                primary_target_count,
                input.missing_obligations.len(),
                input.changed_files.len()
            )
        }
    }
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
    if !issue.repair_packet.likely_fix_sites.is_empty() {
        return vec![format!(
            "Inspect {} before widening the patch.",
            issue.repair_packet.likely_fix_sites.join(", ")
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

fn watchpoint_is_patch_relevant(watchpoint: &Value, changed_concepts: &[String]) -> bool {
    if changed_concepts.is_empty() {
        return true;
    }
    let scope = finding_scope(watchpoint);
    changed_concepts.iter().any(|concept| concept == &scope)
}

fn signal_band(score_0_10000: u32) -> &'static str {
    match score_0_10000 as u64 {
        8000..=u64::MAX => "very_high_signal",
        5000..=7999 => "high_signal",
        _ => "moderate_signal",
    }
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

#[cfg(test)]
mod tests {
    use super::{build_agent_brief, AgentBriefInput, AgentBriefMode};
    use serde_json::json;

    #[test]
    fn onboarding_brief_prioritizes_trusted_findings_and_starter_rules() {
        let brief = build_agent_brief(AgentBriefInput {
            mode: AgentBriefMode::RepoOnboarding,
            repo_shape: json!({
                "primary_archetype": "modular_nextjs_frontend",
                "effective_archetypes": ["modular_nextjs_frontend", "react_frontend"],
                "boundary_roots": [{ "root": "src/modules" }],
                "starter_rules_toml": "[[module_contract]]",
            }),
            findings: vec![json!({
                "kind": "concept_boundary_pressure",
                "scope": "api_endpoints_registry",
                "severity": "high",
                "summary": "Concept boundary pressure exists",
                "trust_tier": "trusted",
                "leverage_class": "boundary_discipline",
                "inspection_focus": ["inspect canonical access"],
                "likely_fix_sites": [{ "site": "src/hooks/use-api.ts" }],
                "concept_id": "api_endpoints_registry",
                "score_0_10000": 6500,
            })],
            experimental_findings: vec![json!({
                "kind": "dead_private_code_cluster",
                "scope": "src/unused.ts",
                "summary": "experimental dead code",
            })],
            missing_obligations: Vec::new(),
            watchpoints: vec![json!({
                "kind": "clone_family",
                "scope": "src/a.ts|src/b.ts",
                "summary": "watchpoint",
            })],
            resolved_findings: Vec::new(),
            changed_files: Vec::new(),
            changed_concepts: Vec::new(),
            decision: None,
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 9000 }),
            scan_trust: json!({ "overall_confidence_0_10000": 9000 }),
            freshness: json!({ "baseline_loaded": false }),
            strict: None,
            limit: 3,
        })
        .expect("agent brief");

        assert_eq!(brief["kind"], "agent_brief");
        assert_eq!(brief["mode"], "repo_onboarding");
        assert_eq!(
            brief["repo_shape"]["primary_archetype"],
            "modular_nextjs_frontend"
        );
        assert_eq!(brief["primary_target_count"], 1);
        assert_eq!(
            brief["primary_targets"][0]["scope"],
            "api_endpoints_registry"
        );
        assert_eq!(
            brief["primary_targets"][0]["next_tools"][0]["tool"],
            "explain_concept"
        );
        assert_eq!(brief["primary_targets"][0]["severity"], "high");
        assert_eq!(brief["primary_targets"][0]["trust_tier"], "trusted");
        assert_eq!(
            brief["primary_targets"][0]["leverage_class"],
            "boundary_discipline"
        );
        assert_eq!(brief["do_not_chase_count"], 1);
        assert_eq!(brief["do_not_chase"][0]["reason"], "experimental_detector");
    }

    #[test]
    fn patch_brief_prioritizes_missing_obligations_and_touched_concepts() {
        let brief = build_agent_brief(AgentBriefInput {
            mode: AgentBriefMode::Patch,
            repo_shape: json!({
                "primary_archetype": "react_frontend",
                "effective_archetypes": ["react_frontend"],
                "boundary_roots": [],
                "starter_rules_toml": null,
            }),
            findings: vec![json!({
                "kind": "concept_boundary_pressure",
                "scope": "task_git_status",
                "severity": "high",
                "summary": "Boundary pressure on task_git_status",
                "trust_tier": "trusted",
                "leverage_class": "architecture_signal",
                "inspection_focus": ["inspect write ownership"],
                "likely_fix_sites": [{ "site": "src/store/core.ts" }],
                "concept_id": "task_git_status",
                "score_0_10000": 9000,
            })],
            experimental_findings: Vec::new(),
            missing_obligations: vec![json!({
                "concept_id": "task_git_status",
                "summary": "task_git_status still misses a canonical projection update",
                "missing_sites": [{ "site": "src/app/task-presentation.ts" }],
            })],
            watchpoints: vec![json!({
                "kind": "clone_family",
                "scope": "src/a.ts|src/b.ts",
                "summary": "non-blocking watchpoint",
            })],
            resolved_findings: Vec::new(),
            changed_files: vec!["src/store/core.ts".to_string()],
            changed_concepts: vec!["task_git_status".to_string()],
            decision: Some("fail".to_string()),
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 9100 }),
            scan_trust: json!({ "overall_confidence_0_10000": 9100 }),
            freshness: json!({ "baseline_loaded": true }),
            strict: Some(false),
            limit: 3,
        })
        .expect("agent brief");

        assert_eq!(brief["mode"], "patch");
        assert_eq!(brief["decision"], "fix_now");
        assert_eq!(brief["primary_targets"][0]["kind"], "missing_obligation");
        assert_eq!(brief["primary_targets"][0]["blocking"], json!(true));
        assert!(
            brief["primary_targets"][0]["repair_packet"]["completeness_0_10000"]
                .as_u64()
                .is_some_and(|score| score >= 8000)
        );
        assert!(brief["primary_targets"][1]["why_now"]
            .as_array()
            .expect("why now")
            .iter()
            .any(|value| value == "touched_concept"));
        assert_eq!(brief["watchpoint_count"], 1);
    }

    #[test]
    fn onboarding_brief_demotes_raw_clone_groups_below_more_fixable_targets() {
        let brief = build_agent_brief(AgentBriefInput {
            mode: AgentBriefMode::RepoOnboarding,
            repo_shape: json!({
                "primary_archetype": "react_frontend",
                "effective_archetypes": ["react_frontend"],
                "boundary_roots": [],
                "starter_rules_toml": null,
            }),
            findings: vec![
                json!({
                    "kind": "exact_clone_group",
                    "scope": "src/a.ts|src/b.ts",
                    "severity": "high",
                    "summary": "Exact clone group exists",
                    "trust_tier": "watchpoint",
                    "presentation_class": "watchpoint",
                    "leverage_class": "secondary_cleanup",
                    "score_0_10000": 9400,
                    "files": ["src/a.ts", "src/b.ts"],
                }),
                json!({
                    "kind": "dependency_sprawl",
                    "scope": "src/app.ts",
                    "severity": "high",
                    "summary": "Entry surface fans out across too many owners",
                    "trust_tier": "trusted",
                    "presentation_class": "structural_debt",
                    "leverage_class": "architecture_signal",
                    "likely_fix_sites": [{ "site": "src/app.ts" }],
                    "score_0_10000": 8200,
                    "files": ["src/app.ts"],
                }),
            ],
            experimental_findings: Vec::new(),
            missing_obligations: Vec::new(),
            watchpoints: Vec::new(),
            resolved_findings: Vec::new(),
            changed_files: Vec::new(),
            changed_concepts: Vec::new(),
            decision: None,
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 9100 }),
            scan_trust: json!({ "overall_confidence_0_10000": 9100 }),
            freshness: json!({ "baseline_loaded": true }),
            strict: None,
            limit: 3,
        })
        .expect("agent brief");

        assert_eq!(brief["primary_targets"][0]["kind"], "dependency_sprawl");
        assert!(brief["primary_targets"]
            .as_array()
            .expect("primary targets")
            .iter()
            .all(|target| target["kind"] != "exact_clone_group"));
    }

    #[test]
    fn pre_merge_brief_blocks_on_gate_failures() {
        let brief = build_agent_brief(AgentBriefInput {
            mode: AgentBriefMode::PreMerge,
            repo_shape: json!({
                "primary_archetype": "sdk",
                "effective_archetypes": ["sdk"],
                "boundary_roots": [],
                "starter_rules_toml": null,
            }),
            findings: vec![json!({
                "kind": "authoritative_import_bypass",
                "scope": "api_endpoints_registry",
                "severity": "high",
                "summary": "Canonical API path is bypassed",
                "trust_tier": "trusted",
                "leverage_class": "boundary_discipline",
                "inspection_focus": ["inspect canonical service access"],
                "likely_fix_sites": [{ "site": "src/services/api.ts" }],
                "concept_id": "api_endpoints_registry",
                "score_0_10000": 7000,
            })],
            experimental_findings: vec![json!({
                "kind": "dead_private_code_cluster",
                "scope": "src/unused.ts",
                "summary": "experimental",
            })],
            missing_obligations: Vec::new(),
            watchpoints: vec![json!({
                "kind": "clone_family",
                "scope": "src/a.ts|src/b.ts",
                "summary": "watchpoint",
            })],
            resolved_findings: Vec::new(),
            changed_files: vec!["src/services/api.ts".to_string()],
            changed_concepts: vec!["api_endpoints_registry".to_string()],
            decision: Some("fail".to_string()),
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 8800 }),
            scan_trust: json!({ "overall_confidence_0_10000": 8800 }),
            freshness: json!({ "baseline_loaded": true }),
            strict: Some(true),
            limit: 3,
        })
        .expect("agent brief");

        assert_eq!(brief["mode"], "pre_merge");
        assert_eq!(brief["decision"], "block");
        assert_eq!(
            brief["primary_targets"][0]["kind"],
            "authoritative_import_bypass"
        );
        assert!(brief["primary_targets"][0]["why_now"]
            .as_array()
            .expect("why now")
            .iter()
            .any(|value| value == "merge_blocker_candidate"));
        assert_eq!(brief["do_not_chase"][0]["reason"], "experimental_detector");
    }

    #[test]
    fn pre_merge_brief_surfaces_medium_blockers_in_strict_mode() {
        let brief = build_agent_brief(AgentBriefInput {
            mode: AgentBriefMode::PreMerge,
            repo_shape: json!({
                "primary_archetype": "sdk",
                "effective_archetypes": ["sdk"],
                "boundary_roots": [],
                "starter_rules_toml": null,
            }),
            findings: vec![json!({
                "kind": "authoritative_import_bypass",
                "scope": "api_endpoints_registry",
                "severity": "medium",
                "summary": "Canonical API path is bypassed",
                "trust_tier": "trusted",
                "leverage_class": "boundary_discipline",
                "inspection_focus": ["inspect canonical service access"],
                "likely_fix_sites": [{ "site": "src/services/api.ts" }],
                "concept_id": "api_endpoints_registry",
                "score_0_10000": 7000,
            })],
            experimental_findings: Vec::new(),
            missing_obligations: Vec::new(),
            watchpoints: Vec::new(),
            resolved_findings: Vec::new(),
            changed_files: vec!["src/services/api.ts".to_string()],
            changed_concepts: vec!["api_endpoints_registry".to_string()],
            decision: Some("fail".to_string()),
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 8800 }),
            scan_trust: json!({ "overall_confidence_0_10000": 8800 }),
            freshness: json!({ "baseline_loaded": true }),
            strict: Some(true),
            limit: 3,
        })
        .expect("agent brief");

        assert_eq!(brief["mode"], "pre_merge");
        assert_eq!(brief["decision"], "block");
        assert_eq!(brief["primary_target_count"], 1);
        assert_eq!(brief["primary_targets"][0]["severity"], "medium");
        assert_eq!(
            brief["primary_targets"][0]["scope"],
            "api_endpoints_registry"
        );
    }
}
