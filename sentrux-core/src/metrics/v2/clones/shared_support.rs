use super::{
    CloneDriftFinding, CloneDriftInstance, MIN_FAMILY_FILE_OVERLAP, RECENT_AGE_DAYS,
    SECONDS_PER_DAY,
};
use crate::metrics::testgap::is_test_file;
use crate::metrics::DuplicateGroup;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CloneFileSummary {
    pub commit_count: Option<u32>,
    pub age_days: Option<u32>,
    pub last_modified_epoch: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CloneFamilyMetrics {
    pub distinct_file_set_count: usize,
    pub recently_touched_file_count: usize,
    pub commit_count_gap: Option<u32>,
    pub age_days_gap: Option<u32>,
    pub asymmetric_recent_change: bool,
    pub mixed_file_sets: bool,
    pub divergence_score: u32,
}

pub(super) fn is_recent_age(age_days: Option<u32>) -> bool {
    age_days
        .map(|age_days| age_days <= RECENT_AGE_DAYS)
        .unwrap_or(false)
}

pub(super) fn distinct_file_count(group: &DuplicateGroup) -> usize {
    clone_group_files(group).len()
}

pub(super) fn clone_group_files(group: &DuplicateGroup) -> Vec<String> {
    group
        .instances
        .iter()
        .map(|(file, _, _)| file.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(super) fn group_max_lines(group: &DuplicateGroup) -> u32 {
    group
        .instances
        .iter()
        .map(|(_, _, lines)| *lines)
        .max()
        .unwrap_or(0)
}

pub(super) fn has_production_instance(group: &DuplicateGroup) -> bool {
    group
        .instances
        .iter()
        .any(|(file, _, _)| !is_test_file(file))
}

pub(super) fn production_instance_count(group: &DuplicateGroup) -> usize {
    group
        .instances
        .iter()
        .filter(|(file, _, _)| !is_test_file(file))
        .count()
}

pub(super) fn file_summary_has_recent_activity(summary: &CloneFileSummary) -> bool {
    summary.commit_count.unwrap_or(0) > 0 || is_recent_age(summary.age_days)
}

pub(super) fn clone_file_summaries(
    instances: &[CloneDriftInstance],
) -> BTreeMap<&str, CloneFileSummary> {
    let mut summaries = BTreeMap::new();
    for instance in instances {
        summaries
            .entry(instance.file.as_str())
            .and_modify(|summary: &mut CloneFileSummary| {
                merge_clone_file_summary(summary, instance)
            })
            .or_insert_with(|| clone_file_summary(instance));
    }
    summaries
}

pub(super) fn clone_family_file_summaries(
    findings: &[&CloneDriftFinding],
) -> BTreeMap<String, CloneFileSummary> {
    let mut summaries = BTreeMap::new();
    for finding in findings {
        for instance in &finding.instances {
            summaries
                .entry(instance.file.clone())
                .and_modify(|summary: &mut CloneFileSummary| {
                    merge_clone_file_summary(summary, instance)
                })
                .or_insert_with(|| clone_file_summary(instance));
        }
    }
    summaries
}

pub(super) fn clone_family_files(findings: &[&CloneDriftFinding]) -> Vec<String> {
    findings
        .iter()
        .flat_map(|finding| finding.files.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn clone_file_commit_gap<'a>(
    summaries: impl Iterator<Item = &'a CloneFileSummary>,
) -> Option<u32> {
    let mut min_commit_count = None;
    let mut max_commit_count = None;

    for summary in summaries {
        min_commit_count = min_present(min_commit_count, summary.commit_count);
        max_commit_count = max_present(max_commit_count, summary.commit_count);
    }

    max_commit_count
        .zip(min_commit_count)
        .map(|(max, min)| max.saturating_sub(min))
}

pub(super) fn clone_file_age_gap_days<'a>(
    summaries: impl Iterator<Item = &'a CloneFileSummary>,
) -> Option<u32> {
    let mut youngest_age_days = None;
    let mut oldest_age_days = None;
    let mut oldest_epoch = None;
    let mut newest_epoch = None;

    for summary in summaries {
        youngest_age_days = min_present(youngest_age_days, summary.age_days);
        oldest_age_days = max_present(oldest_age_days, summary.age_days);
        oldest_epoch = min_present(oldest_epoch, summary.last_modified_epoch);
        newest_epoch = max_present(newest_epoch, summary.last_modified_epoch);
    }

    if let (Some(oldest_epoch), Some(newest_epoch)) = (oldest_epoch, newest_epoch) {
        let gap_days = newest_epoch.saturating_sub(oldest_epoch) / SECONDS_PER_DAY;
        return Some(gap_days as u32);
    }

    oldest_age_days
        .zip(youngest_age_days)
        .map(|(oldest_age_days, youngest_age_days)| {
            oldest_age_days.saturating_sub(youngest_age_days)
        })
}

pub(super) fn clone_family_metrics(
    file_summaries: &BTreeMap<String, CloneFileSummary>,
    distinct_file_set_count: usize,
) -> CloneFamilyMetrics {
    let file_count = file_summaries.len();
    let recently_touched_file_count = file_summaries
        .values()
        .filter(|summary| is_recent_age(summary.age_days))
        .count();
    let active_file_count = file_summaries
        .values()
        .filter(|summary| file_summary_has_recent_activity(summary))
        .count();
    let inactive_file_count = file_count.saturating_sub(active_file_count);
    let asymmetric_recent_change = active_file_count > 0 && inactive_file_count > 0;
    let mixed_file_sets = distinct_file_set_count > 1;
    let commit_count_gap = clone_file_commit_gap(file_summaries.values());
    let age_days_gap = clone_file_age_gap_days(file_summaries.values());

    let mut metrics = CloneFamilyMetrics {
        distinct_file_set_count,
        recently_touched_file_count,
        commit_count_gap,
        age_days_gap,
        asymmetric_recent_change,
        mixed_file_sets,
        divergence_score: 0,
    };
    metrics.divergence_score = clone_family_divergence_score(&metrics);
    metrics
}

pub(super) fn overlapping_file_count(left: &[String], right: &[String]) -> usize {
    let right_files = right.iter().collect::<BTreeSet<_>>();
    left.iter()
        .filter(|file| right_files.contains(file))
        .count()
}

pub(super) fn clone_findings_share_family(
    left: &CloneDriftFinding,
    right: &CloneDriftFinding,
) -> bool {
    overlapping_file_count(&left.files, &right.files) >= MIN_FAMILY_FILE_OVERLAP
}

fn clone_file_summary(instance: &CloneDriftInstance) -> CloneFileSummary {
    CloneFileSummary {
        commit_count: instance.commit_count,
        age_days: instance.age_days,
        last_modified_epoch: instance.last_modified_epoch,
    }
}

fn merge_clone_file_summary(summary: &mut CloneFileSummary, instance: &CloneDriftInstance) {
    summary.commit_count = max_present(summary.commit_count, instance.commit_count);
    summary.age_days = min_present(summary.age_days, instance.age_days);
    summary.last_modified_epoch =
        max_present(summary.last_modified_epoch, instance.last_modified_epoch);
}

fn max_present<T: Ord>(current: Option<T>, next: Option<T>) -> Option<T> {
    [current, next].into_iter().flatten().max()
}

fn min_present<T: Ord>(current: Option<T>, next: Option<T>) -> Option<T> {
    [current, next].into_iter().flatten().min()
}

fn clone_family_divergence_score(metrics: &CloneFamilyMetrics) -> u32 {
    let mut score = 0u32;

    if metrics.asymmetric_recent_change {
        score = score.saturating_add(18);
    }
    if metrics.mixed_file_sets {
        score = score.saturating_add(8);
    }

    if let Some(gap) = metrics.commit_count_gap {
        score = score.saturating_add(match gap {
            0 | 1 => 0,
            2..=3 => 6,
            4..=7 => 10,
            _ => 14,
        });
    }

    if let Some(gap) = metrics.age_days_gap {
        score = score.saturating_add(match gap {
            0..=7 => 0,
            8..=29 => 4,
            30..=59 => 8,
            _ => 12,
        });
    }

    if metrics.recently_touched_file_count > 0 {
        score = score.saturating_add((metrics.recently_touched_file_count as u32).min(3) * 2);
    }

    score.min(44)
}
