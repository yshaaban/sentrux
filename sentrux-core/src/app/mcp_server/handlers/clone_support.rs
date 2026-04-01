use super::*;

pub(crate) struct CloneFindingPayload {
    pub(crate) exact_findings: Vec<Value>,
    pub(crate) prioritized_findings: Vec<Value>,
    pub(crate) families: Vec<Value>,
    pub(crate) remediation_hints: Vec<Value>,
    pub(crate) clone_group_count: usize,
    pub(crate) clone_family_count: usize,
}

pub(crate) fn clone_findings_for_health(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    limit: usize,
) -> (CloneFindingPayload, Option<String>) {
    let (evolution, evolution_error) = evolution_report_for_snapshot(state, root, snapshot);
    let report =
        crate::metrics::v2::build_clone_drift_report(&health.duplicate_groups, evolution.as_ref());
    let prioritized_findings = report
        .prioritized_findings
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let remediation_limit = report.families.len().saturating_mul(4);

    (
        CloneFindingPayload {
            clone_group_count: report.findings.len(),
            clone_family_count: report.families.len(),
            exact_findings: serialized_values(&report.findings),
            prioritized_findings: serialized_values(&prioritized_findings),
            families: serialized_values(&report.families),
            remediation_hints: serialized_values(
                &crate::metrics::v2::build_clone_remediation_hints(
                    &report.families,
                    remediation_limit,
                ),
            ),
        },
        evolution_error,
    )
}

pub(crate) fn visible_clone_ids(findings: &[Value]) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding_kind(finding) == "exact_clone_group")
        .filter_map(|finding| {
            finding
                .get("clone_id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect()
}

fn clone_value_matches_visible_clone_ids(
    value: &Value,
    visible_clone_ids: &BTreeSet<String>,
    key: &str,
) -> bool {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .map(|clone_ids| {
            clone_ids.iter().any(|clone_id| {
                clone_id
                    .as_str()
                    .map(|clone_id| visible_clone_ids.contains(clone_id))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn filter_clone_values_by_visible_clone_ids(
    values: Vec<Value>,
    visible_clone_ids: &BTreeSet<String>,
    key: &str,
    limit: usize,
) -> Vec<Value> {
    if visible_clone_ids.is_empty() {
        return Vec::new();
    }

    values
        .into_iter()
        .filter(|value| clone_value_matches_visible_clone_ids(value, visible_clone_ids, key))
        .take(limit)
        .collect()
}

#[cfg(test)]
pub(crate) fn build_clone_drift_finding_values(
    groups: &[crate::metrics::DuplicateGroup],
    evolution: Option<&crate::metrics::evo::EvolutionReport>,
    limit: usize,
) -> Vec<Value> {
    serialized_values(&crate::metrics::v2::build_clone_drift_findings(
        groups, evolution, limit,
    ))
}
