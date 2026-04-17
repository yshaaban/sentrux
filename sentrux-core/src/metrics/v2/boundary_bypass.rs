use super::concept_match::{relevant_production_reads, relevant_production_writes};
use super::*;
use crate::core::snapshot::Snapshot;
use crate::metrics::rules;

pub(super) fn authoritative_import_bypass_findings(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
    snapshot: &Snapshot,
) -> Vec<SemanticFinding> {
    let authoritative_paths = concept_internal_boundary_paths(concept);
    if authoritative_paths.is_empty() {
        return Vec::new();
    }
    let preferred_entry_paths = concept_preferred_entry_paths(concept);
    if preferred_entry_paths.is_empty() {
        return Vec::new();
    }

    let allowed_importers = concept_allowed_importer_paths(concept);
    let usage_paths = concept_boundary_usage_paths(concept, semantic);
    let mut bypasses = BTreeMap::<String, BTreeSet<String>>::new();

    for edge in &snapshot.import_graph {
        if !authoritative_paths.contains(&edge.to_file) {
            continue;
        }
        if edge.from_file == edge.to_file || is_test_file(&edge.from_file) {
            continue;
        }
        if !usage_paths.contains(&edge.from_file) {
            continue;
        }
        if allowed_importers
            .iter()
            .any(|pattern| path_matches_pattern(pattern, &edge.from_file))
        {
            continue;
        }
        let preference_detail =
            preferred_entry_detail(&preferred_entry_paths, edge.to_file.as_str());

        bypasses
            .entry(edge.from_file.clone())
            .or_default()
            .insert(format!(
                "{} -> {}{}",
                edge.from_file, edge.to_file, preference_detail
            ));
    }

    let severity = if concept.priority.as_deref() == Some("critical") {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    };
    let preferred_entry_summary = preferred_entry_summary(&preferred_entry_paths);
    let mut findings = bypasses
        .iter()
        .map(|(path, evidence)| SemanticFinding {
            kind: "authoritative_import_bypass".to_string(),
            severity,
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' bypasses {} at {}",
                concept.id, preferred_entry_summary, path
            ),
            files: vec![path.clone()],
            evidence: evidence.iter().cloned().collect(),
        })
        .collect::<Vec<_>>();

    if bypasses.len() > 1 {
        findings.push(SemanticFinding {
            kind: "concept_boundary_pressure".to_string(),
            severity,
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' is bypassing {} from {} files",
                concept.id,
                preferred_entry_summary,
                bypasses.len()
            ),
            files: bypasses.keys().cloned().collect(),
            evidence: bypasses
                .iter()
                .flat_map(|(_, evidence)| evidence.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        });
    }

    findings
}

fn concept_boundary_usage_paths(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
) -> HashSet<String> {
    relevant_production_reads(concept, semantic)
        .into_iter()
        .map(|read| read.path.clone())
        .chain(
            relevant_production_writes(concept, semantic)
                .into_iter()
                .map(|write| write.path.clone()),
        )
        .collect()
}

fn concept_internal_boundary_paths(concept: &ConceptRule) -> HashSet<String> {
    let mut paths = HashSet::new();

    if !concept.authoritative_inputs.is_empty() {
        for value in &concept.authoritative_inputs {
            insert_scoped_path(&mut paths, value);
        }
        if concept.kind == "projection" {
            return paths;
        }
    }

    if concept.kind == "authoritative_state" {
        for value in &concept.anchors {
            insert_scoped_path(&mut paths, value);
        }
    }

    paths
}

fn concept_preferred_entry_paths(concept: &ConceptRule) -> Vec<String> {
    let mut paths = BTreeSet::new();

    for value in &concept.canonical_accessors {
        insert_scoped_path(&mut paths, value);
    }

    if concept.kind == "projection" {
        for value in &concept.anchors {
            insert_scoped_path(&mut paths, value);
        }
    }

    paths.into_iter().collect()
}

fn concept_allowed_importer_paths(concept: &ConceptRule) -> Vec<String> {
    let mut patterns = BTreeSet::new();

    for value in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.related_tests.iter())
    {
        if let Some((path, _)) = value.split_once("::") {
            patterns.insert(path.to_string());
        } else {
            patterns.insert(value.clone());
        }
    }

    patterns.into_iter().collect()
}

fn preferred_entry_summary(preferred_entry_paths: &[String]) -> String {
    match preferred_entry_paths {
        [] => "canonical boundaries".to_string(),
        [path] => format!("canonical entrypoint {}", path),
        _ => format!("canonical entrypoints {}", preferred_entry_paths.join(", ")),
    }
}

fn preferred_entry_detail(preferred_entry_paths: &[String], imported_path: &str) -> String {
    let alternatives = preferred_entry_paths
        .iter()
        .filter(|path| path.as_str() != imported_path)
        .cloned()
        .collect::<Vec<_>>();
    if alternatives.is_empty() {
        return String::new();
    }

    format!(" (prefer {})", alternatives.join(", "))
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}

fn insert_scoped_path(paths: &mut impl Extend<String>, value: &str) {
    if let Some((path, _)) = value.split_once("::") {
        paths.extend(std::iter::once(path.to_string()));
    } else {
        paths.extend(std::iter::once(value.to_string()));
    }
}
