use super::{
    finding_scope, string_field, AgentBriefInput, AgentBriefMode, AgentBriefTarget, BriefSeverity,
    BriefTrustTier, DeferredItem, Value,
};

pub(super) fn build_do_not_chase(input: &AgentBriefInput) -> Vec<DeferredItem> {
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

pub(super) fn decide(input: &AgentBriefInput, primary_targets: &[AgentBriefTarget]) -> String {
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

pub(super) fn summarize(
    input: &AgentBriefInput,
    decision: &str,
    primary_target_count: usize,
) -> String {
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

fn watchpoint_is_patch_relevant(watchpoint: &Value, changed_concepts: &[String]) -> bool {
    if changed_concepts.is_empty() {
        return true;
    }
    let scope = finding_scope(watchpoint);
    changed_concepts.iter().any(|concept| concept == &scope)
}
