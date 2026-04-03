use super::*;

pub(crate) const SESSION_INTRODUCED_CLONE_KIND: &str = "session_introduced_clone";

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
    allow_cold_evolution: bool,
) -> (CloneFindingPayload, Option<String>) {
    let (evolution, evolution_error) =
        evolution_report_for_snapshot(state, root, snapshot, allow_cold_evolution);
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

pub(crate) fn is_clone_finding_kind(kind: &str) -> bool {
    matches!(
        kind,
        "exact_clone_group" | "clone_group" | "clone_family" | SESSION_INTRODUCED_CLONE_KIND
    )
}

pub(crate) fn build_session_introduced_clone_findings(
    current_findings: &[Value],
    session_v2: Option<&SessionV2Baseline>,
    changed_files: &BTreeSet<String>,
    limit: usize,
) -> Vec<Value> {
    let Some(session_v2) = session_v2 else {
        return Vec::new();
    };

    let baseline_clone_ids = session_v2
        .finding_payloads
        .values()
        .filter_map(exact_clone_id)
        .collect::<BTreeSet<_>>();
    let mut emitted_clone_ids = BTreeSet::new();
    let mut findings = Vec::new();

    for finding in current_findings {
        if finding_kind(finding) != "exact_clone_group" {
            continue;
        }

        let Some(clone_id) = exact_clone_id(finding) else {
            continue;
        };
        if baseline_clone_ids.contains(clone_id) || !emitted_clone_ids.insert(clone_id.to_string())
        {
            continue;
        }
        if !finding_touches_changed_files(finding, changed_files) {
            continue;
        }

        findings.push(session_introduced_clone_finding(finding, changed_files));
        if findings.len() >= limit {
            break;
        }
    }

    findings
}

pub(crate) fn merge_session_introduced_clone_findings(
    introduced_findings: Vec<Value>,
    current_findings: &[Value],
    session_v2: Option<&SessionV2Baseline>,
    changed_files: &BTreeSet<String>,
    limit: usize,
) -> Vec<Value> {
    if session_v2.is_none() {
        return introduced_findings;
    }

    let introduced_clone_findings =
        build_session_introduced_clone_findings(current_findings, session_v2, changed_files, limit);

    merge_findings(
        introduced_clone_findings,
        introduced_findings
            .into_iter()
            .filter(|finding| !is_clone_finding_kind(finding_kind(finding)))
            .collect(),
        limit,
    )
}

fn exact_clone_id(finding: &Value) -> Option<&str> {
    finding.get("clone_id").and_then(Value::as_str)
}

fn finding_touches_changed_files(finding: &Value, changed_files: &BTreeSet<String>) -> bool {
    if changed_files.is_empty() {
        return true;
    }

    finding_files(finding)
        .iter()
        .any(|path| changed_files.contains(path))
}

fn session_introduced_clone_finding(finding: &Value, changed_files: &BTreeSet<String>) -> Value {
    let files = finding_files(finding);
    let instances = clone_instance_labels(finding);
    let introduced_duplicate = instances
        .iter()
        .find(|(file, _)| changed_files.contains(file))
        .map(|(_, label)| label.clone());
    let preferred_owner = instances
        .iter()
        .find(|(file, _)| !changed_files.contains(file))
        .or_else(|| instances.first())
        .map(|(_, label)| label.clone());
    let file_count = files.len();
    let joined_files = match files.as_slice() {
        [] => "the changed surface".to_string(),
        [only] => only.clone(),
        [left, right] => format!("{left} and {right}"),
        _ => format!("{} files", file_count),
    };
    let clone_id = exact_clone_id(finding)
        .map(str::to_string)
        .unwrap_or_default();
    let mut evidence = Vec::new();
    if !clone_id.is_empty() {
        evidence.push(format!("introduced clone group: {clone_id}"));
    }
    if let Some(label) = &introduced_duplicate {
        evidence.push(format!("introduced duplicate: {label}"));
    }
    if let Some(label) = &preferred_owner {
        evidence.push(format!("preferred owner: {label}"));
    }
    evidence.extend(
        files
            .iter()
            .take(3)
            .map(|path| format!("duplicate surface: {path}")),
    );
    if let Some(summary) = finding.get("summary").and_then(Value::as_str) {
        evidence.push(summary.to_string());
    }
    let summary = match (&introduced_duplicate, &preferred_owner) {
        (Some(introduced), Some(owner)) if introduced != owner => format!(
            "This patch introduced a duplicate implementation in {introduced} instead of extending {owner}."
        ),
        _ => format!("This patch introduced a new duplicate implementation across {joined_files}."),
    };
    let impact = match &preferred_owner {
        Some(owner) => format!(
            "Fresh duplication is likely to drift on the next change unless the new path is folded back into {owner} now."
        ),
        None => "Fresh duplication is likely to drift on the next change unless the shared logic is collapsed now.".to_string(),
    };

    json!({
        "kind": SESSION_INTRODUCED_CLONE_KIND,
        "clone_id": clone_id,
        "scope": finding
            .get("scope")
            .cloned()
            .unwrap_or_else(|| json!(joined_files)),
        "files": files,
        "severity": "medium",
        "summary": summary,
        "impact": impact,
        "evidence": evidence,
        "trust_tier": "trusted",
        "presentation_class": "structural_debt",
        "leverage_class": "local_refactor_target",
        "leverage_reasons": [
            "duplicate_maintenance_pressure",
            "introduced_in_patch"
        ],
    })
}

fn clone_instance_labels(finding: &Value) -> Vec<(String, String)> {
    finding
        .get("instances")
        .and_then(Value::as_array)
        .map(|instances| {
            instances
                .iter()
                .filter_map(|instance| {
                    let file = instance.get("file").and_then(Value::as_str)?;
                    let label = instance
                        .get("func")
                        .and_then(Value::as_str)
                        .filter(|func| !func.is_empty())
                        .map(|func| format!("{file}::{func}"))
                        .unwrap_or_else(|| file.to_string());
                    Some((file.to_string(), label))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
