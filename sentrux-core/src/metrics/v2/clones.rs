//! Git-aware clone-drift findings built on duplicate groups plus evolution context.

use super::FindingSeverity;
use crate::metrics::evolution::EvolutionReport;
use crate::metrics::testgap::is_test_file;
use crate::metrics::DuplicateGroup;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

const MIN_CLONE_LINES: u32 = 3;
const RECENT_AGE_DAYS: u32 = 30;
const MIN_FAMILY_FILE_OVERLAP: usize = 2;
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneDriftInstance {
    pub file: String,
    pub func: String,
    pub lines: u32,
    pub commit_count: Option<u32>,
    pub age_days: Option<u32>,
    #[serde(skip_serializing)]
    pub last_modified_epoch: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneDriftFinding {
    pub kind: String,
    pub clone_id: String,
    pub severity: FindingSeverity,
    pub instance_count: usize,
    pub production_instance_count: usize,
    pub total_lines: u32,
    pub max_lines: u32,
    pub risk_score: u32,
    pub max_commit_count: u32,
    pub recently_touched_file_count: usize,
    pub youngest_age_days: Option<u32>,
    pub asymmetric_recent_change: bool,
    pub files: Vec<String>,
    pub reasons: Vec<String>,
    pub summary: String,
    pub instances: Vec<CloneDriftInstance>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneFamilySummary {
    pub kind: String,
    pub family_id: String,
    pub severity: FindingSeverity,
    pub family_score: u32,
    pub divergence_score: u32,
    pub member_count: usize,
    pub file_count: usize,
    pub distinct_file_set_count: usize,
    pub mixed_file_sets: bool,
    pub commit_count_gap: Option<u32>,
    pub age_days_gap: Option<u32>,
    pub representative_clone_id: String,
    pub clone_ids: Vec<String>,
    pub recently_touched_file_count: usize,
    pub asymmetric_recent_change: bool,
    pub files: Vec<String>,
    pub reasons: Vec<String>,
    pub summary: String,
    pub remediation_hints: Vec<CloneRemediationHint>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RemediationPriority {
    Low,
    #[default]
    Medium,
    High,
}

impl RemediationPriority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneRemediationHint {
    pub kind: String,
    pub priority: RemediationPriority,
    pub summary: String,
    pub files: Vec<String>,
    pub clone_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CloneDriftReport {
    pub findings: Vec<CloneDriftFinding>,
    pub prioritized_findings: Vec<CloneDriftFinding>,
    pub families: Vec<CloneFamilySummary>,
}

pub fn build_clone_drift_findings(
    groups: &[DuplicateGroup],
    evolution: Option<&EvolutionReport>,
    limit: usize,
) -> Vec<CloneDriftFinding> {
    let mut report = build_clone_drift_report(groups, evolution);
    report.findings.truncate(limit);
    report.findings
}

pub fn build_clone_drift_report(
    groups: &[DuplicateGroup],
    evolution: Option<&EvolutionReport>,
) -> CloneDriftReport {
    let mut findings = groups
        .iter()
        .filter_map(|group| clone_drift_finding(group, evolution))
        .collect::<Vec<_>>();
    findings.sort_by(compare_clone_findings);
    let families = build_clone_family_summaries(&findings);
    let prioritized_findings = prioritize_clone_findings(&findings, &families);

    CloneDriftReport {
        findings,
        prioritized_findings,
        families,
    }
}

pub fn build_clone_remediation_hints(
    families: &[CloneFamilySummary],
    limit: usize,
) -> Vec<CloneRemediationHint> {
    if limit == 0 || families.is_empty() {
        return Vec::new();
    }

    let max_hints = families
        .iter()
        .map(|family| family.remediation_hints.len())
        .max()
        .unwrap_or(0);
    let mut hints = Vec::new();

    for hint_index in 0..max_hints {
        for family in families {
            let Some(hint) = family.remediation_hints.get(hint_index) else {
                continue;
            };
            hints.push(hint.clone());
            if hints.len() >= limit {
                return hints;
            }
        }
    }

    hints
}

fn clone_drift_finding(
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
    let file_summaries = clone_file_summaries(&instances);
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
    let asymmetric_recent_change = active_files > 0 && inactive_files > 0;
    let risk_score = clone_risk_score(
        instance_count,
        max_lines,
        max_commit_count,
        recently_touched_file_count,
        asymmetric_recent_change,
    );
    let severity = clone_severity(
        instance_count,
        production_instance_count,
        max_lines,
        max_commit_count,
        asymmetric_recent_change,
    );
    let reasons = clone_reasons(
        files.len(),
        max_lines,
        max_commit_count,
        recently_touched_file_count,
        youngest_age_days,
        asymmetric_recent_change,
    );
    let summary = clone_summary(instance_count, asymmetric_recent_change, max_commit_count);

    Some(CloneDriftFinding {
        kind: "exact_clone_group".to_string(),
        clone_id: clone_id(group.hash),
        severity,
        instance_count,
        production_instance_count,
        total_lines,
        max_lines,
        risk_score,
        max_commit_count,
        recently_touched_file_count,
        youngest_age_days,
        asymmetric_recent_change,
        files,
        reasons,
        summary,
        instances,
    })
}

fn compare_clone_findings(left: &CloneDriftFinding, right: &CloneDriftFinding) -> Ordering {
    severity_priority(right.severity)
        .cmp(&severity_priority(left.severity))
        .then_with(|| right.risk_score.cmp(&left.risk_score))
        .then_with(|| left.clone_id.cmp(&right.clone_id))
}

fn compare_clone_families(left: &CloneFamilySummary, right: &CloneFamilySummary) -> Ordering {
    severity_priority(right.severity)
        .cmp(&severity_priority(left.severity))
        .then_with(|| right.divergence_score.cmp(&left.divergence_score))
        .then_with(|| right.family_score.cmp(&left.family_score))
        .then_with(|| left.family_id.cmp(&right.family_id))
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

fn build_clone_family_summaries(findings: &[CloneDriftFinding]) -> Vec<CloneFamilySummary> {
    let mut families = clone_family_clusters(findings)
        .into_iter()
        .filter_map(clone_family_summary)
        .collect::<Vec<_>>();
    families.sort_by(compare_clone_families);
    families
}

fn clone_family_clusters(findings: &[CloneDriftFinding]) -> Vec<Vec<&CloneDriftFinding>> {
    let mut visited = vec![false; findings.len()];
    let mut clusters = Vec::new();

    for start in 0..findings.len() {
        if visited[start] {
            continue;
        }

        visited[start] = true;
        let mut stack = vec![start];
        let mut cluster = Vec::new();

        while let Some(index) = stack.pop() {
            let finding = &findings[index];
            cluster.push(finding);

            for next_index in 0..findings.len() {
                if visited[next_index] {
                    continue;
                }
                if clone_findings_share_family(finding, &findings[next_index]) {
                    visited[next_index] = true;
                    stack.push(next_index);
                }
            }
        }

        if cluster.len() >= 2 {
            clusters.push(cluster);
        }
    }

    clusters
}

fn clone_findings_share_family(left: &CloneDriftFinding, right: &CloneDriftFinding) -> bool {
    overlapping_file_count(&left.files, &right.files) >= MIN_FAMILY_FILE_OVERLAP
}

fn overlapping_file_count(left: &[String], right: &[String]) -> usize {
    let right_files = right.iter().collect::<BTreeSet<_>>();
    left.iter()
        .filter(|file| right_files.contains(file))
        .count()
}

fn clone_family_summary(members: Vec<&CloneDriftFinding>) -> Option<CloneFamilySummary> {
    if members.len() < 2 {
        return None;
    }

    let mut members = members;
    members.sort_by(|left, right| compare_clone_findings(left, right));
    let representative = members.first()?;
    let files = clone_family_files(&members);
    let family_id = clone_family_id(&files);
    let clone_ids = members
        .iter()
        .map(|finding| finding.clone_id.clone())
        .collect::<Vec<_>>();
    let file_summaries = clone_family_file_summaries(&members);
    let distinct_file_set_count = members
        .iter()
        .map(|finding| finding.files.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<BTreeSet<_>>()
        .len();
    let family_metrics = clone_family_metrics(&file_summaries, distinct_file_set_count);
    let family_score = representative
        .risk_score
        .saturating_add(4 * ((members.len() - 1).min(3) as u32))
        .saturating_add(family_metrics.divergence_score);
    let reasons = clone_family_reasons(
        members.len(),
        files.len(),
        representative.risk_score,
        &family_metrics,
    );
    let summary = clone_family_summary_text(members.len(), files.len(), &family_metrics);
    let remediation_hints = clone_family_remediation_hints(
        representative,
        &family_metrics,
        members.len(),
        files.len(),
        &files,
        &clone_ids,
    );

    Some(CloneFamilySummary {
        kind: "clone_family".to_string(),
        family_id,
        severity: representative.severity,
        family_score,
        divergence_score: family_metrics.divergence_score,
        member_count: members.len(),
        file_count: files.len(),
        distinct_file_set_count: family_metrics.distinct_file_set_count,
        mixed_file_sets: family_metrics.mixed_file_sets,
        commit_count_gap: family_metrics.commit_count_gap,
        age_days_gap: family_metrics.age_days_gap,
        representative_clone_id: representative.clone_id.clone(),
        clone_ids,
        recently_touched_file_count: family_metrics.recently_touched_file_count,
        asymmetric_recent_change: family_metrics.asymmetric_recent_change,
        files,
        reasons,
        summary,
        remediation_hints,
    })
}

fn clone_family_files(findings: &[&CloneDriftFinding]) -> Vec<String> {
    findings
        .iter()
        .flat_map(|finding| finding.files.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn clone_family_id(files: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    format!("clone-family-{:#016x}", hasher.finish())
}

fn clone_family_file_summaries(
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

fn clone_family_reasons(
    member_count: usize,
    file_count: usize,
    representative_risk_score: u32,
    family_metrics: &CloneFamilyMetrics,
) -> Vec<String> {
    let mut reasons = vec![
        format!("{member_count} exact clone groups repeat across {file_count} files"),
        format!(
            "highest-risk representative exact clone scores {}",
            representative_risk_score
        ),
    ];
    if family_metrics.recently_touched_file_count > 0 {
        reasons.push(format!(
            "{} family file(s) changed within the last {} day(s)",
            family_metrics.recently_touched_file_count, RECENT_AGE_DAYS
        ));
    }
    if family_metrics.mixed_file_sets {
        reasons.push(format!(
            "family clone coverage spans {} overlapping file set(s)",
            family_metrics.distinct_file_set_count
        ));
    }
    if let Some(gap) = family_metrics.commit_count_gap {
        reasons.push(format!(
            "family churn spans a gap of {} recent commit(s)",
            gap
        ));
    }
    if let Some(gap) = family_metrics.age_days_gap {
        reasons.push(format!("family file age spans a gap of {} day(s)", gap));
    }
    if family_metrics.asymmetric_recent_change {
        reasons.push("recent activity is uneven across the clone family".to_string());
    }
    if family_metrics.divergence_score > 0 {
        reasons.push(format!(
            "family-level divergence contributes {} risk point(s)",
            family_metrics.divergence_score
        ));
    }
    reasons
}

fn clone_family_summary_text(
    member_count: usize,
    file_count: usize,
    family_metrics: &CloneFamilyMetrics,
) -> String {
    let mut details = Vec::new();

    if family_metrics.asymmetric_recent_change {
        details.push("recent edits are uneven across the family".to_string());
    }
    if family_metrics.mixed_file_sets {
        details.push(format!(
            "clone coverage spans {} overlapping sibling sets",
            family_metrics.distinct_file_set_count
        ));
    }

    if let Some(gap) = family_metrics.commit_count_gap {
        details.push(format!(
            "churn differs by {gap} recent commit(s) across siblings"
        ));
    }

    if let Some(gap) = family_metrics.age_days_gap {
        details.push(format!("sibling file age spans {gap} day(s)"));
    }

    if details.is_empty() {
        return format!("{member_count} exact clone groups repeat across {file_count} files");
    }

    format!(
        "{member_count} exact clone groups repeat across {file_count} files and {}",
        details.join("; ")
    )
}

fn clone_family_remediation_hints(
    representative: &CloneDriftFinding,
    family_metrics: &CloneFamilyMetrics,
    member_count: usize,
    file_count: usize,
    files: &[String],
    clone_ids: &[String],
) -> Vec<CloneRemediationHint> {
    let mut hints = Vec::new();

    if family_metrics.mixed_file_sets {
        hints.push(CloneRemediationHint {
            kind: "review_family_boundaries".to_string(),
            priority: if family_metrics.divergence_score >= 12 {
                RemediationPriority::High
            } else {
                RemediationPriority::Medium
            },
            summary: format!(
                "Review the overlapping clone family boundaries across {} sibling file set(s) and decide whether these copies should stay synchronized or be split into separate abstractions.",
                family_metrics.distinct_file_set_count
            ),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if family_metrics.divergence_score > 0 {
        let mut detail = Vec::new();
        if family_metrics.mixed_file_sets {
            detail.push(format!(
                "clone coverage spans {} overlapping sibling sets",
                family_metrics.distinct_file_set_count
            ));
        }
        if let Some(gap) = family_metrics.commit_count_gap {
            detail.push(format!(
                "commit churn spans a gap of {gap} recent commit(s)"
            ));
        }
        if let Some(gap) = family_metrics.age_days_gap {
            detail.push(format!("file age spans a gap of {gap} day(s)"));
        }
        if family_metrics.asymmetric_recent_change {
            detail.push("recent edits are uneven across the family".to_string());
        }

        hints.push(CloneRemediationHint {
            kind: "sync_recent_divergence".to_string(),
            priority: RemediationPriority::High,
            summary: if detail.is_empty() {
                "Review recent edits across clone siblings and either synchronize the shared behavior or intentionally split the implementations."
                    .to_string()
            } else {
                format!(
                    "Review recent edits across clone siblings: {}. Synchronize the shared behavior or intentionally split the implementations.",
                    detail.join("; ")
                )
            },
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if member_count >= 2 && file_count >= 2 {
        hints.push(CloneRemediationHint {
            kind: "extract_shared_helper".to_string(),
            priority: if representative.severity == FindingSeverity::High {
                RemediationPriority::High
            } else {
                RemediationPriority::Medium
            },
            summary: format!(
                "Extract the repeated logic into a shared helper or module used by the {} clone groups across these files.",
                member_count
            ),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if member_count >= 3 || file_count >= 3 {
        hints.push(CloneRemediationHint {
            kind: "collapse_clone_family".to_string(),
            priority: RemediationPriority::Medium,
            summary: format!(
                "Collapse the {} repeated clone groups behind one named abstraction instead of maintaining copies in {} files.",
                member_count, file_count
            ),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if representative.severity == FindingSeverity::High
        && family_metrics.recently_touched_file_count > 0
    {
        hints.push(CloneRemediationHint {
            kind: "add_shared_behavior_tests".to_string(),
            priority: RemediationPriority::Medium,
            summary:
                "Add focused tests around the shared behavior before deduplicating the clone family so the extraction does not hide drift."
                    .to_string(),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    hints
}

fn clone_family_metrics(
    file_summaries: &BTreeMap<String, CloneFileSummary>,
    distinct_file_set_count: usize,
) -> CloneFamilyMetrics {
    let mut metrics = CloneFamilyMetrics::default();
    metrics.file_count = file_summaries.len();
    metrics.distinct_file_set_count = distinct_file_set_count;
    metrics.mixed_file_sets = distinct_file_set_count > 1;
    metrics.recently_touched_file_count = file_summaries
        .values()
        .filter(|summary| is_recent_age(summary.age_days))
        .count();
    metrics.active_file_count = file_summaries
        .values()
        .filter(|summary| file_summary_has_recent_activity(summary))
        .count();
    metrics.inactive_file_count = metrics.file_count.saturating_sub(metrics.active_file_count);
    metrics.asymmetric_recent_change =
        metrics.active_file_count > 0 && metrics.inactive_file_count > 0;
    metrics.commit_count_gap = clone_file_commit_gap(file_summaries.values());
    metrics.age_days_gap = clone_file_age_gap_days(file_summaries.values());
    metrics.divergence_score = clone_family_divergence_score(&metrics);
    metrics
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

fn prioritize_clone_findings(
    findings: &[CloneDriftFinding],
    families: &[CloneFamilySummary],
) -> Vec<CloneDriftFinding> {
    let finding_by_id = findings
        .iter()
        .map(|finding| (finding.clone_id.as_str(), finding))
        .collect::<BTreeMap<_, _>>();
    let representative_ids = families
        .iter()
        .map(|family| family.representative_clone_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut prioritized = Vec::new();

    for family in families {
        if let Some(finding) = finding_by_id.get(family.representative_clone_id.as_str()) {
            prioritized.push((*finding).clone());
        }
    }

    prioritized.extend(
        findings
            .iter()
            .filter(|finding| !representative_ids.contains(finding.clone_id.as_str()))
            .cloned(),
    );
    prioritized
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

fn is_recent_age(age_days: Option<u32>) -> bool {
    age_days
        .map(|age_days| age_days <= RECENT_AGE_DAYS)
        .unwrap_or(false)
}

fn distinct_file_count(group: &DuplicateGroup) -> usize {
    clone_group_files(group).len()
}

fn clone_group_files(group: &DuplicateGroup) -> Vec<String> {
    group
        .instances
        .iter()
        .map(|(file, _, _)| file.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
struct CloneFileSummary {
    commit_count: Option<u32>,
    age_days: Option<u32>,
    last_modified_epoch: Option<i64>,
}

fn file_summary_has_recent_activity(summary: &CloneFileSummary) -> bool {
    summary.commit_count.unwrap_or(0) > 0 || is_recent_age(summary.age_days)
}

fn clone_file_summaries(instances: &[CloneDriftInstance]) -> BTreeMap<&str, CloneFileSummary> {
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

fn clone_file_commit_gap<'a>(summaries: impl Iterator<Item = &'a CloneFileSummary>) -> Option<u32> {
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

fn clone_file_age_gap_days<'a>(
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

fn max_present<T: Ord>(current: Option<T>, next: Option<T>) -> Option<T> {
    [current, next].into_iter().flatten().max()
}

fn min_present<T: Ord>(current: Option<T>, next: Option<T>) -> Option<T> {
    [current, next].into_iter().flatten().min()
}

#[derive(Debug, Clone, Default)]
struct CloneFamilyMetrics {
    file_count: usize,
    distinct_file_set_count: usize,
    active_file_count: usize,
    inactive_file_count: usize,
    recently_touched_file_count: usize,
    commit_count_gap: Option<u32>,
    age_days_gap: Option<u32>,
    asymmetric_recent_change: bool,
    mixed_file_sets: bool,
    divergence_score: u32,
}

fn group_max_lines(group: &DuplicateGroup) -> u32 {
    group
        .instances
        .iter()
        .map(|(_, _, lines)| *lines)
        .max()
        .unwrap_or(0)
}

fn has_production_instance(group: &DuplicateGroup) -> bool {
    group
        .instances
        .iter()
        .any(|(file, _, _)| !is_test_file(file))
}

fn production_instance_count(group: &DuplicateGroup) -> usize {
    group
        .instances
        .iter()
        .filter(|(file, _, _)| !is_test_file(file))
        .count()
}

#[cfg(test)]
mod tests {
    use super::{
        build_clone_drift_findings, build_clone_drift_report, build_clone_remediation_hints,
        CloneFamilySummary, CloneRemediationHint, FindingSeverity, RemediationPriority,
    };
    use crate::metrics::evolution::{
        AuthorInfo, CouplingPair, EvolutionReport, FileChurn, TemporalHotspot,
    };
    use crate::metrics::DuplicateGroup;
    use std::collections::HashMap;

    fn test_evolution() -> EvolutionReport {
        EvolutionReport {
            churn: HashMap::from([
                (
                    "src/a.ts".to_string(),
                    FileChurn {
                        commit_count: 4,
                        lines_added: 10,
                        lines_removed: 2,
                        total_churn: 12,
                    },
                ),
                (
                    "src/b.ts".to_string(),
                    FileChurn {
                        commit_count: 0,
                        lines_added: 0,
                        lines_removed: 0,
                        total_churn: 0,
                    },
                ),
            ]),
            coupling_pairs: Vec::<CouplingPair>::new(),
            hotspots: Vec::<TemporalHotspot>::new(),
            code_age: HashMap::from([("src/a.ts".to_string(), 3), ("src/b.ts".to_string(), 90)]),
            last_modified_epoch: HashMap::from([
                ("src/a.ts".to_string(), 1_000_000),
                ("src/b.ts".to_string(), 1_000_000 - (87 * 86_400)),
            ]),
            authors: HashMap::<String, AuthorInfo>::new(),
            single_author_ratio: 0.0,
            bus_factor_score: 1.0,
            churn_score: 1.0,
            evolution_score: 1.0,
            lookback_days: 90,
            commits_analyzed: 5,
        }
    }

    #[test]
    fn clone_drift_findings_include_stable_ids_and_git_context() {
        let groups = vec![DuplicateGroup {
            hash: 42,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/b.ts".to_string(), "dup_b".to_string(), 12),
            ],
        }];

        let findings = build_clone_drift_findings(&groups, Some(&test_evolution()), 10);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].clone_id, "clone-0x0000000000002a");
        assert_eq!(findings[0].max_commit_count, 4);
        assert_eq!(findings[0].youngest_age_days, Some(3));
        assert!(findings[0].asymmetric_recent_change);
        assert_eq!(findings[0].severity, FindingSeverity::High);
    }

    #[test]
    fn clone_drift_filters_test_only_and_tiny_groups() {
        let groups = vec![
            DuplicateGroup {
                hash: 1,
                instances: vec![
                    ("src/a.test.ts".to_string(), "dup_a".to_string(), 10),
                    ("src/b.test.ts".to_string(), "dup_b".to_string(), 10),
                ],
            },
            DuplicateGroup {
                hash: 2,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_a".to_string(), 1),
                    ("src/b.ts".to_string(), "dup_b".to_string(), 1),
                ],
            },
        ];

        let findings = build_clone_drift_findings(&groups, None, 10);

        assert!(findings.is_empty());
    }

    #[test]
    fn clone_drift_counts_recent_activity_per_file() {
        let groups = vec![DuplicateGroup {
            hash: 99,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/a.ts".to_string(), "dup_b".to_string(), 12),
                ("src/b.ts".to_string(), "dup_c".to_string(), 12),
            ],
        }];

        let findings = build_clone_drift_findings(&groups, Some(&test_evolution()), 10);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].recently_touched_file_count, 1);
        assert!(findings[0].asymmetric_recent_change);
        assert_eq!(findings[0].reasons[0], "identical logic spans 2 files");
        assert_eq!(findings[0].instances[0].file, "src/a.ts");
        assert_eq!(findings[0].instances[1].file, "src/a.ts");
        assert_eq!(findings[0].instances[2].file, "src/b.ts");
    }

    #[test]
    fn clone_drift_report_groups_same_file_set_into_families() {
        let groups = vec![
            DuplicateGroup {
                hash: 7,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                    ("src/b.ts".to_string(), "dup_b".to_string(), 12),
                ],
            },
            DuplicateGroup {
                hash: 8,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_c".to_string(), 9),
                    ("src/b.ts".to_string(), "dup_d".to_string(), 9),
                ],
            },
        ];

        let report = build_clone_drift_report(&groups, Some(&test_evolution()));

        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.families.len(), 1);
        assert_eq!(report.families[0].member_count, 2);
        assert_eq!(report.families[0].file_count, 2);
        assert_eq!(report.families[0].distinct_file_set_count, 1);
        assert!(!report.families[0].mixed_file_sets);
        assert_eq!(report.families[0].clone_ids.len(), 2);
        assert_eq!(report.families[0].commit_count_gap, Some(4));
        assert_eq!(report.families[0].age_days_gap, Some(87));
        assert!(report.families[0].divergence_score > 0);
        assert!(report.families[0]
            .summary
            .contains("churn differs by 4 recent commit(s)"));
        assert!(report.families[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("family churn spans a gap of 4 recent commit(s)")));
        assert!(report.families[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("family file age spans a gap of 87 day(s)")));
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "sync_recent_divergence"));
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "extract_shared_helper"));
        assert_eq!(
            report.prioritized_findings[0].clone_id,
            report.families[0].representative_clone_id
        );
    }

    #[test]
    fn clone_drift_report_prioritizes_more_divergent_families_first() {
        let groups = vec![
            DuplicateGroup {
                hash: 10,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                    ("src/b.ts".to_string(), "dup_b".to_string(), 12),
                ],
            },
            DuplicateGroup {
                hash: 11,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                    ("src/b.ts".to_string(), "dup_d".to_string(), 10),
                ],
            },
            DuplicateGroup {
                hash: 12,
                instances: vec![
                    ("src/c.ts".to_string(), "dup_e".to_string(), 10),
                    ("src/d.ts".to_string(), "dup_f".to_string(), 10),
                ],
            },
            DuplicateGroup {
                hash: 13,
                instances: vec![
                    ("src/c.ts".to_string(), "dup_g".to_string(), 10),
                    ("src/d.ts".to_string(), "dup_h".to_string(), 10),
                ],
            },
        ];

        let mut evolution = test_evolution();
        evolution.churn.insert(
            "src/a.ts".to_string(),
            crate::metrics::evolution::FileChurn {
                commit_count: 10,
                lines_added: 14,
                lines_removed: 2,
                total_churn: 16,
            },
        );
        evolution.churn.insert(
            "src/b.ts".to_string(),
            crate::metrics::evolution::FileChurn {
                commit_count: 9,
                lines_added: 12,
                lines_removed: 1,
                total_churn: 13,
            },
        );
        evolution.churn.insert(
            "src/c.ts".to_string(),
            crate::metrics::evolution::FileChurn {
                commit_count: 10,
                lines_added: 14,
                lines_removed: 2,
                total_churn: 16,
            },
        );
        evolution.churn.insert(
            "src/d.ts".to_string(),
            crate::metrics::evolution::FileChurn {
                commit_count: 1,
                lines_added: 1,
                lines_removed: 0,
                total_churn: 1,
            },
        );
        evolution.code_age.insert("src/a.ts".to_string(), 4);
        evolution.code_age.insert("src/b.ts".to_string(), 5);
        evolution.code_age.insert("src/c.ts".to_string(), 4);
        evolution.code_age.insert("src/d.ts".to_string(), 5);

        let report = build_clone_drift_report(&groups, Some(&evolution));

        assert_eq!(report.families.len(), 2);
        assert_eq!(report.prioritized_findings.len(), 4);
        assert!(report.families[0].divergence_score > report.families[1].divergence_score);
        assert_eq!(
            report.prioritized_findings[0].clone_id,
            report.families[0].representative_clone_id
        );
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "extract_shared_helper"));
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "sync_recent_divergence"));
        assert!(report.families[0]
            .summary
            .contains("churn differs by 9 recent commit(s)"));
        assert_eq!(report.families[0].commit_count_gap, Some(9));
    }

    #[test]
    fn clone_drift_report_groups_overlapping_file_sets_into_one_family() {
        let groups = vec![
            DuplicateGroup {
                hash: 21,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                    ("src/b.ts".to_string(), "dup_b".to_string(), 12),
                ],
            },
            DuplicateGroup {
                hash: 22,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                    ("src/b.ts".to_string(), "dup_d".to_string(), 10),
                    ("src/c.ts".to_string(), "dup_e".to_string(), 10),
                ],
            },
        ];

        let report = build_clone_drift_report(&groups, Some(&test_evolution()));

        assert_eq!(report.families.len(), 1);
        assert_eq!(report.families[0].member_count, 2);
        assert_eq!(report.families[0].file_count, 3);
        assert_eq!(report.families[0].distinct_file_set_count, 2);
        assert!(report.families[0].mixed_file_sets);
        assert!(report.families[0]
            .summary
            .contains("overlapping sibling sets"));
        assert!(report.families[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("overlapping file set(s)")));
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "review_family_boundaries"));
    }

    #[test]
    fn clone_family_age_gap_uses_stable_epoch_difference() {
        let groups = vec![
            DuplicateGroup {
                hash: 30,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                    ("src/b.ts".to_string(), "dup_b".to_string(), 12),
                ],
            },
            DuplicateGroup {
                hash: 31,
                instances: vec![
                    ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                    ("src/b.ts".to_string(), "dup_d".to_string(), 10),
                ],
            },
        ];

        let evolution = EvolutionReport {
            churn: HashMap::from([
                (
                    "src/a.ts".to_string(),
                    FileChurn {
                        commit_count: 1,
                        lines_added: 1,
                        lines_removed: 0,
                        total_churn: 1,
                    },
                ),
                (
                    "src/b.ts".to_string(),
                    FileChurn {
                        commit_count: 1,
                        lines_added: 1,
                        lines_removed: 0,
                        total_churn: 1,
                    },
                ),
            ]),
            coupling_pairs: Vec::<CouplingPair>::new(),
            hotspots: Vec::<TemporalHotspot>::new(),
            code_age: HashMap::from([("src/a.ts".to_string(), 1), ("src/b.ts".to_string(), 0)]),
            last_modified_epoch: HashMap::from([
                ("src/a.ts".to_string(), 1_000_000),
                ("src/b.ts".to_string(), 1_000_000 + (12 * 60 * 60)),
            ]),
            authors: HashMap::<String, AuthorInfo>::new(),
            single_author_ratio: 0.0,
            bus_factor_score: 1.0,
            churn_score: 1.0,
            evolution_score: 1.0,
            lookback_days: 90,
            commits_analyzed: 2,
        };

        let report = build_clone_drift_report(&groups, Some(&evolution));
        assert_eq!(report.families.len(), 1);
        assert_eq!(report.families[0].age_days_gap, Some(0));
    }

    #[test]
    fn clone_remediation_hints_round_robin_across_families() {
        let families = vec![
            CloneFamilySummary {
                family_id: "family-a".to_string(),
                remediation_hints: vec![
                    CloneRemediationHint {
                        kind: "sync_recent_divergence".to_string(),
                        priority: RemediationPriority::High,
                        summary: "sync".to_string(),
                        files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
                        clone_ids: vec!["clone-a".to_string()],
                    },
                    CloneRemediationHint {
                        kind: "extract_shared_helper".to_string(),
                        priority: RemediationPriority::Medium,
                        summary: "extract".to_string(),
                        files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
                        clone_ids: vec!["clone-a".to_string()],
                    },
                ],
                ..CloneFamilySummary::default()
            },
            CloneFamilySummary {
                family_id: "family-b".to_string(),
                remediation_hints: vec![CloneRemediationHint {
                    kind: "collapse_clone_family".to_string(),
                    priority: RemediationPriority::Medium,
                    summary: "collapse".to_string(),
                    files: vec!["src/c.ts".to_string(), "src/d.ts".to_string()],
                    clone_ids: vec!["clone-b".to_string()],
                }],
                ..CloneFamilySummary::default()
            },
        ];

        let hints = build_clone_remediation_hints(&families, 3);

        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].kind, "sync_recent_divergence");
        assert_eq!(hints[1].kind, "collapse_clone_family");
        assert_eq!(hints[2].kind, "extract_shared_helper");
    }

    #[test]
    fn remediation_priority_serializes_to_legacy_strings() {
        let hint = CloneRemediationHint {
            kind: "sync_recent_divergence".to_string(),
            priority: RemediationPriority::High,
            summary: "sync".to_string(),
            files: vec!["src/a.ts".to_string()],
            clone_ids: vec!["clone-a".to_string()],
        };

        let value = serde_json::to_value(&hint).expect("serialize hint");

        assert_eq!(value["priority"], "high");
    }
}
