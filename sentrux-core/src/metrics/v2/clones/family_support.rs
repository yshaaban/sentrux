use super::group_support::compare_clone_findings;
use super::shared_support::{
    clone_family_file_summaries, clone_family_files, clone_family_metrics,
    clone_findings_share_family, CloneFamilyMetrics,
};
use super::{
    CloneDriftFinding, CloneFamilySummary, CloneRemediationHint, FindingSeverity,
    RemediationPriority, RECENT_AGE_DAYS,
};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

pub(super) fn build_clone_family_summaries(
    findings: &[CloneDriftFinding],
) -> Vec<CloneFamilySummary> {
    let mut families = clone_family_clusters(findings)
        .into_iter()
        .filter_map(clone_family_summary)
        .collect::<Vec<_>>();
    families.sort_by(compare_clone_families);
    families
}

pub(super) fn prioritize_clone_findings(
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

fn compare_clone_families(left: &CloneFamilySummary, right: &CloneFamilySummary) -> Ordering {
    right
        .severity
        .priority()
        .cmp(&left.severity.priority())
        .then_with(|| right.divergence_score.cmp(&left.divergence_score))
        .then_with(|| right.family_score.cmp(&left.family_score))
        .then_with(|| left.family_id.cmp(&right.family_id))
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

fn clone_family_id(files: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    format!("clone-family-{:#016x}", hasher.finish())
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
        hints.push(build_boundary_review_hint(family_metrics, files, clone_ids));
    }

    if family_metrics.divergence_score > 0 {
        hints.push(build_divergence_hint(family_metrics, files, clone_ids));
    }

    if member_count >= 2 && file_count >= 2 {
        hints.push(build_shared_helper_hint(
            representative,
            member_count,
            files,
            clone_ids,
        ));
    }

    if member_count >= 3 || file_count >= 3 {
        hints.push(build_collapse_hint(
            member_count,
            file_count,
            files,
            clone_ids,
        ));
    }

    if representative.severity == FindingSeverity::High
        && family_metrics.recently_touched_file_count > 0
    {
        hints.push(build_shared_behavior_test_hint(files, clone_ids));
    }

    hints
}

fn build_boundary_review_hint(
    family_metrics: &CloneFamilyMetrics,
    files: &[String],
    clone_ids: &[String],
) -> CloneRemediationHint {
    CloneRemediationHint {
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
    }
}

fn build_divergence_hint(
    family_metrics: &CloneFamilyMetrics,
    files: &[String],
    clone_ids: &[String],
) -> CloneRemediationHint {
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

    CloneRemediationHint {
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
    }
}

fn build_shared_helper_hint(
    representative: &CloneDriftFinding,
    member_count: usize,
    files: &[String],
    clone_ids: &[String],
) -> CloneRemediationHint {
    CloneRemediationHint {
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
    }
}

fn build_collapse_hint(
    member_count: usize,
    file_count: usize,
    files: &[String],
    clone_ids: &[String],
) -> CloneRemediationHint {
    CloneRemediationHint {
        kind: "collapse_clone_family".to_string(),
        priority: RemediationPriority::Medium,
        summary: format!(
            "Collapse the {} repeated clone groups behind one named abstraction instead of maintaining copies in {} files.",
            member_count, file_count
        ),
        files: files.to_vec(),
        clone_ids: clone_ids.to_vec(),
    }
}

fn build_shared_behavior_test_hint(files: &[String], clone_ids: &[String]) -> CloneRemediationHint {
    CloneRemediationHint {
        kind: "add_shared_behavior_tests".to_string(),
        priority: RemediationPriority::Medium,
        summary:
            "Add focused tests around the shared behavior before deduplicating the clone family so the extraction does not hide drift."
                .to_string(),
        files: files.to_vec(),
        clone_ids: clone_ids.to_vec(),
    }
}
