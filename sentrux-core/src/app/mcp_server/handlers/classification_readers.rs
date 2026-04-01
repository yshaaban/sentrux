use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn finding_payload_map(findings: &[Value]) -> BTreeMap<String, Value> {
    let mut payloads = BTreeMap::new();
    for finding in findings {
        payloads.insert(stable_json_key(finding), finding.clone());
    }
    payloads
}

fn stable_json_key(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

pub(crate) fn serialized_values<T: serde::Serialize>(values: &[T]) -> Vec<Value> {
    values
        .iter()
        .filter_map(|value| serde_json::to_value(value).ok())
        .collect()
}

pub(crate) fn combined_other_finding_values(
    semantic_findings: &[crate::metrics::v2::SemanticFinding],
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
) -> Vec<Value> {
    let mut findings = serialized_values(semantic_findings);
    findings.extend(serialized_values(structural_reports));
    findings
}

pub(crate) fn finding_values(clone_findings: &[Value], other_findings: &[Value]) -> Vec<Value> {
    let mut findings = clone_findings.to_vec();
    findings.extend(other_findings.iter().cloned());
    findings
}

pub(crate) fn finding_kind(finding: &Value) -> &str {
    finding
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

pub(crate) fn finding_concept_id(finding: &Value) -> Option<&str> {
    finding.get("concept_id").and_then(|value| value.as_str())
}

pub(super) fn finding_string_values(finding: &Value, field: &str) -> Vec<String> {
    finding
        .get(field)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn finding_files(finding: &Value) -> Vec<String> {
    let files = finding_string_values(finding, "files");
    if !files.is_empty() {
        return files;
    }

    if let Some(path) = finding.get("path").and_then(|value| value.as_str()) {
        return vec![path.to_string()];
    }

    finding
        .get("instances")
        .and_then(|value| value.as_array())
        .map(|instances| {
            instances
                .iter()
                .filter_map(|instance| {
                    instance
                        .get("file")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn finding_scope(finding: &Value) -> String {
    if let Some(scope) = finding.get("scope").and_then(|value| value.as_str()) {
        return scope.to_string();
    }

    if let Some(concept_id) = finding_concept_id(finding) {
        return concept_id.to_string();
    }

    let files = finding_files(finding);
    if !files.is_empty() {
        if files.len() == 1 {
            return files[0].clone();
        }
        return files.join("|");
    }

    finding_kind(finding).to_string()
}

pub(crate) fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
