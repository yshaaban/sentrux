//! Git-aware clone-drift findings built on duplicate groups plus evolution context.

use crate::metrics::evolution::EvolutionReport;
use crate::metrics::testgap::is_test_file;
use crate::metrics::DuplicateGroup;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

const MIN_CLONE_LINES: u32 = 3;
const RECENT_AGE_DAYS: u32 = 30;

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneDriftInstance {
    pub file: String,
    pub func: String,
    pub lines: u32,
    pub commit_count: Option<u32>,
    pub age_days: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneDriftFinding {
    pub kind: String,
    pub clone_id: String,
    pub severity: String,
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
    pub severity: String,
    pub family_score: u32,
    pub member_count: usize,
    pub file_count: usize,
    pub representative_clone_id: String,
    pub clone_ids: Vec<String>,
    pub recently_touched_file_count: usize,
    pub asymmetric_recent_change: bool,
    pub files: Vec<String>,
    pub reasons: Vec<String>,
    pub summary: String,
    pub remediation_hints: Vec<CloneRemediationHint>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CloneRemediationHint {
    pub kind: String,
    pub priority: String,
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
        severity: severity.to_string(),
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
    severity_priority(&right.severity)
        .cmp(&severity_priority(&left.severity))
        .then_with(|| right.risk_score.cmp(&left.risk_score))
        .then_with(|| left.clone_id.cmp(&right.clone_id))
}

fn compare_clone_families(left: &CloneFamilySummary, right: &CloneFamilySummary) -> Ordering {
    severity_priority(&right.severity)
        .cmp(&severity_priority(&left.severity))
        .then_with(|| right.family_score.cmp(&left.family_score))
        .then_with(|| left.family_id.cmp(&right.family_id))
}

fn severity_priority(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
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
) -> &'static str {
    if max_lines >= 20
        || (production_instance_count >= 2
            && max_lines >= 10
            && (asymmetric_recent_change || max_commit_count >= 3))
        || (instance_count >= 3 && max_lines >= 8)
    {
        "high"
    } else if max_lines >= 8
        || (asymmetric_recent_change && max_commit_count >= 1)
        || (instance_count >= 3 && max_lines >= 5)
    {
        "medium"
    } else {
        "low"
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
    let mut families = findings
        .iter()
        .fold(
            BTreeMap::<Vec<String>, Vec<&CloneDriftFinding>>::new(),
            |mut map, finding| {
                map.entry(finding.files.clone()).or_default().push(finding);
                map
            },
        )
        .into_iter()
        .filter_map(|(files, members)| clone_family_summary(files, members))
        .collect::<Vec<_>>();
    families.sort_by(compare_clone_families);
    families
}

fn clone_family_summary(
    files: Vec<String>,
    members: Vec<&CloneDriftFinding>,
) -> Option<CloneFamilySummary> {
    if members.len() < 2 {
        return None;
    }

    let mut members = members;
    members.sort_by(|left, right| compare_clone_findings(left, right));
    let representative = members.first()?;
    let family_id = clone_family_id(&files);
    let clone_ids = members
        .iter()
        .map(|finding| finding.clone_id.clone())
        .collect::<Vec<_>>();
    let file_summaries = clone_family_file_summaries(&members);
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
    let family_score = representative
        .risk_score
        .saturating_add(4 * ((members.len() - 1).min(3) as u32))
        .saturating_add((recently_touched_file_count as u32).saturating_mul(3))
        .saturating_add(if asymmetric_recent_change { 12 } else { 0 });
    let reasons = clone_family_reasons(
        members.len(),
        files.len(),
        representative.risk_score,
        recently_touched_file_count,
        asymmetric_recent_change,
    );
    let summary = clone_family_summary_text(members.len(), files.len(), asymmetric_recent_change);
    let remediation_hints = clone_family_remediation_hints(
        representative,
        members.len(),
        files.len(),
        recently_touched_file_count,
        asymmetric_recent_change,
        &files,
        &clone_ids,
    );

    Some(CloneFamilySummary {
        kind: "clone_family".to_string(),
        family_id,
        severity: representative.severity.clone(),
        family_score,
        member_count: members.len(),
        file_count: files.len(),
        representative_clone_id: representative.clone_id.clone(),
        clone_ids,
        recently_touched_file_count,
        asymmetric_recent_change,
        files,
        reasons,
        summary,
        remediation_hints,
    })
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
                    summary.commit_count = Some(
                        summary
                            .commit_count
                            .unwrap_or(0)
                            .max(instance.commit_count.unwrap_or(0)),
                    );
                    summary.age_days = match (summary.age_days, instance.age_days) {
                        (Some(current), Some(next)) => Some(current.min(next)),
                        (Some(current), None) => Some(current),
                        (None, Some(next)) => Some(next),
                        (None, None) => None,
                    };
                })
                .or_insert(CloneFileSummary {
                    commit_count: instance.commit_count,
                    age_days: instance.age_days,
                });
        }
    }
    summaries
}

fn clone_family_reasons(
    member_count: usize,
    file_count: usize,
    representative_risk_score: u32,
    recently_touched_file_count: usize,
    asymmetric_recent_change: bool,
) -> Vec<String> {
    let mut reasons = vec![
        format!("{member_count} exact clone groups repeat across {file_count} files"),
        format!(
            "highest-risk representative exact clone scores {}",
            representative_risk_score
        ),
    ];
    if recently_touched_file_count > 0 {
        reasons.push(format!(
            "{} family file(s) changed within the last {} day(s)",
            recently_touched_file_count, RECENT_AGE_DAYS
        ));
    }
    if asymmetric_recent_change {
        reasons.push("recent activity is uneven across the clone family".to_string());
    }
    reasons
}

fn clone_family_summary_text(
    member_count: usize,
    file_count: usize,
    asymmetric_recent_change: bool,
) -> String {
    if asymmetric_recent_change {
        return format!(
            "{member_count} exact clone groups repeat across {file_count} files and recent edits are uneven across the family"
        );
    }

    format!("{member_count} exact clone groups repeat across {file_count} files")
}

fn clone_family_remediation_hints(
    representative: &CloneDriftFinding,
    member_count: usize,
    file_count: usize,
    recently_touched_file_count: usize,
    asymmetric_recent_change: bool,
    files: &[String],
    clone_ids: &[String],
) -> Vec<CloneRemediationHint> {
    let mut hints = Vec::new();

    if asymmetric_recent_change {
        hints.push(CloneRemediationHint {
            kind: "sync_recent_divergence".to_string(),
            priority: "high".to_string(),
            summary:
                "Review recent edits across clone siblings and either synchronize the shared behavior or intentionally split the implementations."
                    .to_string(),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if member_count >= 2 && file_count >= 2 {
        hints.push(CloneRemediationHint {
            kind: "extract_shared_helper".to_string(),
            priority: if representative.severity == "high" {
                "high".to_string()
            } else {
                "medium".to_string()
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
            priority: "medium".to_string(),
            summary: format!(
                "Collapse the {} repeated clone groups behind one named abstraction instead of maintaining copies in {} files.",
                member_count, file_count
            ),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    if representative.severity == "high" && recently_touched_file_count > 0 {
        hints.push(CloneRemediationHint {
            kind: "add_shared_behavior_tests".to_string(),
            priority: "medium".to_string(),
            summary:
                "Add focused tests around the shared behavior before deduplicating the clone family so the extraction does not hide drift."
                    .to_string(),
            files: files.to_vec(),
            clone_ids: clone_ids.to_vec(),
        });
    }

    hints
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
        .map(|family| family.representative_clone_id.clone())
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
            .filter(|finding| !representative_ids.contains(&finding.clone_id))
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
        .map(|(file, _, _)| file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
struct CloneFileSummary {
    commit_count: Option<u32>,
    age_days: Option<u32>,
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
                summary.commit_count = Some(
                    summary
                        .commit_count
                        .unwrap_or(0)
                        .max(instance.commit_count.unwrap_or(0)),
                );
                summary.age_days = match (summary.age_days, instance.age_days) {
                    (Some(current), Some(next)) => Some(current.min(next)),
                    (Some(current), None) => Some(current),
                    (None, Some(next)) => Some(next),
                    (None, None) => None,
                };
            })
            .or_insert(CloneFileSummary {
                commit_count: instance.commit_count,
                age_days: instance.age_days,
            });
    }
    summaries
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
        CloneFamilySummary, CloneRemediationHint,
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
        assert_eq!(findings[0].severity, "high");
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
        assert_eq!(report.families[0].clone_ids.len(), 2);
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
    fn clone_drift_report_prioritizes_family_representatives_before_siblings() {
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
                    ("src/c.ts".to_string(), "dup_e".to_string(), 8),
                    ("src/d.ts".to_string(), "dup_f".to_string(), 8),
                ],
            },
        ];

        let report = build_clone_drift_report(&groups, Some(&test_evolution()));

        assert_eq!(report.families.len(), 1);
        assert_eq!(report.prioritized_findings.len(), 3);
        assert_eq!(
            report.prioritized_findings[0].clone_id,
            report.families[0].representative_clone_id
        );
        assert!(report.families[0]
            .remediation_hints
            .iter()
            .any(|hint| hint.kind == "extract_shared_helper"));
        assert!(
            report.prioritized_findings[1].clone_id != report.families[0].representative_clone_id
        );
    }

    #[test]
    fn clone_remediation_hints_round_robin_across_families() {
        let families = vec![
            CloneFamilySummary {
                family_id: "family-a".to_string(),
                remediation_hints: vec![
                    CloneRemediationHint {
                        kind: "sync_recent_divergence".to_string(),
                        priority: "high".to_string(),
                        summary: "sync".to_string(),
                        files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
                        clone_ids: vec!["clone-a".to_string()],
                    },
                    CloneRemediationHint {
                        kind: "extract_shared_helper".to_string(),
                        priority: "medium".to_string(),
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
                    priority: "medium".to_string(),
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
}
