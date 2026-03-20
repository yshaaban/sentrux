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
    severity: String,
    trust_tier: String,
    leverage_class: String,
    summary: String,
    why_now: Vec<String>,
    likely_fix_sites: Vec<String>,
    inspection_focus: Vec<String>,
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

pub fn build_agent_brief(input: AgentBriefInput) -> Value {
    let mut primary_targets = match input.mode {
        AgentBriefMode::RepoOnboarding => select_onboarding_targets(&input),
        AgentBriefMode::Patch => select_patch_targets(&input),
        AgentBriefMode::PreMerge => select_pre_merge_targets(&input),
    };
    primary_targets.truncate(input.limit.max(1));

    let do_not_chase = build_do_not_chase(&input);
    let decision = decide(&input, &primary_targets);
    let summary = summarize(&input, &decision, primary_targets.len());

    serde_json::to_value(AgentBrief {
        kind: "agent_brief",
        mode: input.mode.as_str(),
        decision,
        summary,
        repo_shape: summarize_repo_shape(&input.repo_shape),
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
    .unwrap_or_else(|_| json!({}))
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
    })
}

fn select_onboarding_targets(input: &AgentBriefInput) -> Vec<AgentBriefTarget> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();

    for finding in &input.findings {
        let target = target_from_finding(finding, input);
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

fn select_patch_targets(input: &AgentBriefInput) -> Vec<AgentBriefTarget> {
    let mut selected = select_obligation_targets(input);
    let mut seen = selected
        .iter()
        .map(|target| format!("{}:{}", target.scope, target.kind))
        .collect::<BTreeSet<_>>();

    for finding in &input.findings {
        if !is_patch_relevant(finding, &input.changed_concepts) {
            continue;
        }
        let target = target_from_finding(finding, input);
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

fn select_pre_merge_targets(input: &AgentBriefInput) -> Vec<AgentBriefTarget> {
    let mut selected = select_obligation_targets(input);
    let mut seen = selected
        .iter()
        .map(|target| format!("{}:{}", target.scope, target.kind))
        .collect::<BTreeSet<_>>();
    let include_medium = input.strict.unwrap_or(false);

    for finding in &input.findings {
        let severity = severity_of_value(finding);
        if severity != "high" && !(include_medium && severity == "medium") {
            continue;
        }
        let target = target_from_finding(finding, input);
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

fn select_obligation_targets(input: &AgentBriefInput) -> Vec<AgentBriefTarget> {
    input
        .missing_obligations
        .iter()
        .map(|obligation| target_from_obligation(obligation, input))
        .collect()
}

fn target_from_obligation(obligation: &Value, input: &AgentBriefInput) -> AgentBriefTarget {
    let concept = obligation
        .get("concept_id")
        .or_else(|| obligation.get("concept"))
        .and_then(Value::as_str)
        .unwrap_or("changed_concept")
        .to_string();
    let missing_sites = obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let likely_fix_sites = missing_sites
        .iter()
        .filter_map(|site| site.get("site").and_then(Value::as_str))
        .take(5)
        .map(str::to_string)
        .collect::<Vec<_>>();
    AgentBriefTarget {
        scope: concept.clone(),
        kind: "missing_obligation".to_string(),
        severity: "high".to_string(),
        trust_tier: "trusted".to_string(),
        leverage_class: "hardening_note".to_string(),
        summary: obligation
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "Concept '{concept}' still has {} missing update site(s)",
                    likely_fix_sites.len()
                )
            }),
        why_now: vec![
            "blocking_obligation".to_string(),
            "changed_concept".to_string(),
            "clear_fix_surface".to_string(),
        ],
        likely_fix_sites,
        inspection_focus: vec![
            "inspect the missing update sites implied by the changed concept".to_string(),
        ],
        next_tools: concept_tools(&concept, input.mode),
    }
}

fn target_from_finding(finding: &Value, input: &AgentBriefInput) -> AgentBriefTarget {
    let concept_id = finding
        .get("concept_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let scope = finding_scope(finding);
    let kind = string_field(finding, "kind", "finding");
    let summary = format!(
        "{} ({})",
        string_field(finding, "summary", &kind),
        signal_band(finding)
    );
    let leverage_class = string_field(finding, "leverage_class", "secondary_cleanup");
    let likely_fix_sites = likely_fix_sites(finding);

    AgentBriefTarget {
        scope: scope.clone(),
        kind: kind.clone(),
        severity: severity_of_value(finding).to_string(),
        trust_tier: string_field(finding, "trust_tier", "trusted"),
        leverage_class: leverage_class.clone(),
        summary,
        why_now: why_now(finding, input),
        likely_fix_sites,
        inspection_focus: string_array_field(finding, "inspection_focus", 3),
        next_tools: concept_id
            .as_deref()
            .map(|concept| concept_tools(concept, input.mode))
            .unwrap_or_default(),
    }
}

fn why_now(finding: &Value, input: &AgentBriefInput) -> Vec<String> {
    let mut reasons = Vec::new();
    let concept_id = finding.get("concept_id").and_then(Value::as_str);
    if concept_id.is_some_and(|concept| input.changed_concepts.iter().any(|value| value == concept))
    {
        reasons.push("touched_concept".to_string());
    }
    if string_field(finding, "trust_tier", "trusted") == "trusted" {
        reasons.push("high_trust".to_string());
    }
    let leverage_class = string_field(finding, "leverage_class", "secondary_cleanup");
    if matches!(
        leverage_class.as_str(),
        "architecture_signal" | "local_refactor_target" | "boundary_discipline"
    ) {
        reasons.push("high_leverage".to_string());
    }
    if !likely_fix_sites(finding).is_empty() {
        reasons.push("clear_fix_surface".to_string());
    }
    if matches!(input.mode, AgentBriefMode::PreMerge) && severity_of_value(finding) == "high" {
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
                .any(|target| target.trust_tier == "trusted")
            {
                "fix_now".to_string()
            } else {
                "continue".to_string()
            }
        }
        AgentBriefMode::Patch => {
            if !input.missing_obligations.is_empty()
                || primary_targets
                    .iter()
                    .any(|target| target.severity == "high" && target.trust_tier == "trusted")
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

fn likely_fix_sites(finding: &Value) -> Vec<String> {
    let from_tool = finding
        .get("likely_fix_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| site.get("site").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !from_tool.is_empty() {
        return from_tool;
    }
    let files = file_list(finding);
    files.into_iter().take(5).collect()
}

fn is_patch_relevant(finding: &Value, changed_concepts: &[String]) -> bool {
    if changed_concepts.is_empty() {
        return true;
    }
    finding
        .get("concept_id")
        .and_then(Value::as_str)
        .is_some_and(|concept| changed_concepts.iter().any(|value| value == concept))
}

fn watchpoint_is_patch_relevant(watchpoint: &Value, changed_concepts: &[String]) -> bool {
    if changed_concepts.is_empty() {
        return true;
    }
    let scope = finding_scope(watchpoint);
    changed_concepts.iter().any(|concept| concept == &scope)
}

fn signal_band(finding: &Value) -> &'static str {
    match finding
        .get("score_0_10000")
        .and_then(Value::as_u64)
        .unwrap_or(0)
    {
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

fn string_array_field(value: &Value, key: &str, limit: usize) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(limit)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn severity_of_value(value: &Value) -> &str {
    value
        .get("severity")
        .and_then(Value::as_str)
        .unwrap_or("low")
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
        });

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
        });

        assert_eq!(brief["mode"], "patch");
        assert_eq!(brief["decision"], "fix_now");
        assert_eq!(brief["primary_targets"][0]["kind"], "missing_obligation");
        assert!(brief["primary_targets"][1]["why_now"]
            .as_array()
            .expect("why now")
            .iter()
            .any(|value| value == "touched_concept"));
        assert_eq!(brief["watchpoint_count"], 1);
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
        });

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
        });

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
