use super::shared_support::{
    clone_file_summaries, clone_group_files, distinct_file_count, file_summary_has_recent_activity,
    group_max_lines, has_production_instance, is_recent_age, production_instance_count,
};
use super::{
    CloneDriftFinding, CloneDriftInstance, FindingSeverity, MIN_CLONE_LINES, RECENT_AGE_DAYS,
};
use crate::metrics::evolution::EvolutionReport;
use crate::metrics::DuplicateGroup;
use std::cmp::Ordering;

pub(super) fn clone_drift_finding(
    group: &DuplicateGroup,
    evolution: Option<&EvolutionReport>,
) -> Option<CloneDriftFinding> {
    if distinct_file_count(group) <= 1 {
        return None;
    }
    if !has_production_instance(group) {
        return None;
    }

    let max_lines = group_max_lines(group);
    if max_lines < MIN_CLONE_LINES {
        return None;
    }

    let instance_count = group.instances.len();
    let production_instance_count = production_instance_count(group);
    let total_lines = group.instances.iter().map(|(_, _, lines)| *lines).sum();
    let files = clone_group_files(group);
    let instances = build_clone_instances(group, evolution);
    let activity = summarize_clone_activity(&instances);
    let risk_score = clone_risk_score(
        instance_count,
        max_lines,
        activity.max_commit_count,
        activity.recently_touched_file_count,
        activity.asymmetric_recent_change,
    );
    let severity = clone_severity(
        instance_count,
        production_instance_count,
        max_lines,
        activity.max_commit_count,
        activity.asymmetric_recent_change,
    );
    let reasons = clone_reasons(
        files.len(),
        max_lines,
        activity.max_commit_count,
        activity.recently_touched_file_count,
        activity.youngest_age_days,
        activity.asymmetric_recent_change,
    );
    let summary = clone_summary(
        instance_count,
        activity.asymmetric_recent_change,
        activity.max_commit_count,
    );

    Some(CloneDriftFinding {
        kind: "exact_clone_group".to_string(),
        clone_id: clone_id(group.hash),
        severity,
        instance_count,
        production_instance_count,
        total_lines,
        max_lines,
        risk_score,
        max_commit_count: activity.max_commit_count,
        recently_touched_file_count: activity.recently_touched_file_count,
        youngest_age_days: activity.youngest_age_days,
        asymmetric_recent_change: activity.asymmetric_recent_change,
        files,
        reasons,
        summary,
        instances,
    })
}

#[derive(Debug, Clone, Copy)]
struct CloneActivitySummary {
    max_commit_count: u32,
    youngest_age_days: Option<u32>,
    recently_touched_file_count: usize,
    asymmetric_recent_change: bool,
}

fn build_clone_instances(
    group: &DuplicateGroup,
    evolution: Option<&EvolutionReport>,
) -> Vec<CloneDriftInstance> {
    let mut instances = group
        .instances
        .iter()
        .map(|(file, func, lines)| CloneDriftInstance {
            file: file.clone(),
            func: func.clone(),
            lines: *lines,
            commit_count: evolution
                .and_then(|report| report.churn.get(file))
                .map(|churn| churn.commit_count),
            age_days: evolution.and_then(|report| report.code_age.get(file).copied()),
            last_modified_epoch: evolution
                .and_then(|report| report.last_modified_epoch.get(file))
                .copied(),
        })
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.func.cmp(&right.func))
            .then_with(|| left.lines.cmp(&right.lines))
    });
    instances
}

fn summarize_clone_activity(instances: &[CloneDriftInstance]) -> CloneActivitySummary {
    let file_summaries = clone_file_summaries(instances);
    let max_commit_count = instances
        .iter()
        .filter_map(|instance| instance.commit_count)
        .max()
        .unwrap_or(0);
    let youngest_age_days = instances
        .iter()
        .filter_map(|instance| instance.age_days)
        .min();
    let recently_touched_file_count = file_summaries
        .values()
        .filter(|summary| is_recent_age(summary.age_days))
        .count();
    let active_files = file_summaries
        .values()
        .filter(|summary| file_summary_has_recent_activity(summary))
        .count();
    let inactive_files = file_summaries.len().saturating_sub(active_files);

    CloneActivitySummary {
        max_commit_count,
        youngest_age_days,
        recently_touched_file_count,
        asymmetric_recent_change: active_files > 0 && inactive_files > 0,
    }
}

pub(super) fn compare_clone_findings(
    left: &CloneDriftFinding,
    right: &CloneDriftFinding,
) -> Ordering {
    severity_priority(right.severity)
        .cmp(&severity_priority(left.severity))
        .then_with(|| right.risk_score.cmp(&left.risk_score))
        .then_with(|| left.clone_id.cmp(&right.clone_id))
}

fn severity_priority(severity: FindingSeverity) -> u8 {
    severity.priority()
}

fn clone_risk_score(
    instance_count: usize,
    max_lines: u32,
    max_commit_count: u32,
    recently_touched_file_count: usize,
    asymmetric_recent_change: bool,
) -> u32 {
    let base = (instance_count as u32).saturating_mul(max_lines);
    let churn_bonus = max_commit_count.saturating_mul(4);
    let recent_bonus = (recently_touched_file_count as u32).saturating_mul(3);
    let asymmetry_bonus = if asymmetric_recent_change { 12 } else { 0 };
    base.saturating_add(churn_bonus)
        .saturating_add(recent_bonus)
        .saturating_add(asymmetry_bonus)
}

fn clone_severity(
    instance_count: usize,
    production_instance_count: usize,
    max_lines: u32,
    max_commit_count: u32,
    asymmetric_recent_change: bool,
) -> FindingSeverity {
    if max_lines >= 20
        || (production_instance_count >= 2
            && max_lines >= 10
            && (asymmetric_recent_change || max_commit_count >= 3))
        || (instance_count >= 3 && max_lines >= 8)
    {
        FindingSeverity::High
    } else if max_lines >= 8
        || (asymmetric_recent_change && max_commit_count >= 1)
        || (instance_count >= 3 && max_lines >= 5)
    {
        FindingSeverity::Medium
    } else {
        FindingSeverity::Low
    }
}

fn clone_summary(
    instance_count: usize,
    asymmetric_recent_change: bool,
    max_commit_count: u32,
) -> String {
    if asymmetric_recent_change {
        return format!(
            "{instance_count} functions share an identical normalized body and recent edits are asymmetric across clone files"
        );
    }
    if max_commit_count > 0 {
        return format!(
            "{instance_count} functions share an identical normalized body across recently changed files"
        );
    }
    format!("{instance_count} functions share an identical normalized body")
}

fn clone_reasons(
    file_count: usize,
    max_lines: u32,
    max_commit_count: u32,
    recently_touched_file_count: usize,
    youngest_age_days: Option<u32>,
    asymmetric_recent_change: bool,
) -> Vec<String> {
    let mut reasons = vec![
        format!("identical logic spans {} files", file_count),
        format!("largest clone instance is {} lines", max_lines),
    ];
    if max_commit_count > 0 {
        reasons.push(format!(
            "at least one clone file changed in {} recent commit(s)",
            max_commit_count
        ));
    }
    if recently_touched_file_count > 0 {
        reasons.push(format!(
            "{} clone file(s) changed within the last {} day(s)",
            recently_touched_file_count, RECENT_AGE_DAYS
        ));
    }
    if let Some(age_days) = youngest_age_days {
        reasons.push(format!(
            "youngest clone file was touched {} day(s) ago",
            age_days
        ));
    }
    if asymmetric_recent_change {
        reasons.push("recent activity is uneven across clone instances".to_string());
    }
    reasons
}

fn clone_id(hash: u64) -> String {
    format!("clone-{hash:#016x}")
}
