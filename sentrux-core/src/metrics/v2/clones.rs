//! Git-aware clone-drift findings built on duplicate groups plus evolution context.

mod family_support;
mod group_support;
mod shared_support;

#[cfg(test)]
mod tests;

use super::FindingSeverity;
use crate::metrics::evolution::EvolutionReport;
use crate::metrics::DuplicateGroup;
use crate::string_enum::impl_str_enum;
use family_support::{build_clone_family_summaries, prioritize_clone_findings};
use group_support::{clone_drift_finding, compare_clone_findings};

pub(super) const MIN_CLONE_LINES: u32 = 3;
pub(super) const RECENT_AGE_DAYS: u32 = 30;
pub(super) const MIN_FAMILY_FILE_OVERLAP: usize = 2;
pub(super) const SECONDS_PER_DAY: i64 = 86_400;

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

impl_str_enum!(RemediationPriority {
    Low => "low",
    Medium => "medium",
    High => "high",
});

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
