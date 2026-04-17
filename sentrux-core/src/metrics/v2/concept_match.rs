use super::*;
use crate::metrics::rules;

pub(crate) fn relevant_writes<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a WriteFact> {
    let targets = concept_write_targets(concept);
    semantic
        .writes
        .iter()
        .filter(|write| symbol_matches_targets(&write.symbol_name, &targets))
        .collect()
}

pub(crate) fn relevant_production_writes<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a WriteFact> {
    relevant_writes(concept, semantic)
        .into_iter()
        .filter(|write| !is_test_file(&write.path))
        .collect()
}

pub(crate) fn relevant_reads<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ReadFact> {
    let targets = concept_read_targets(concept);
    semantic
        .reads
        .iter()
        .filter(|read| symbol_matches_targets(&read.symbol_name, &targets))
        .collect()
}

pub(crate) fn relevant_production_reads<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ReadFact> {
    relevant_reads(concept, semantic)
        .into_iter()
        .filter(|read| !is_test_file(&read.path))
        .collect()
}

fn concept_write_targets(concept: &ConceptRule) -> HashSet<String> {
    if concept.kind == "projection" {
        return scoped_symbols(&concept.anchors);
    }

    concept_targets(concept)
}

fn concept_read_targets(concept: &ConceptRule) -> HashSet<String> {
    if concept.kind == "projection" && !concept.authoritative_inputs.is_empty() {
        return scoped_symbols(&concept.authoritative_inputs);
    }

    concept_targets(concept)
}

pub(crate) fn concept_targets(concept: &ConceptRule) -> HashSet<String> {
    scoped_symbols(
        &concept
            .anchors
            .iter()
            .chain(concept.authoritative_inputs.iter())
            .cloned()
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn symbol_from_scoped_path(value: &str) -> Option<String> {
    let (_, symbol) = value.split_once("::")?;
    Some(symbol.to_string())
}

fn scoped_symbols(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .filter_map(|value| symbol_from_scoped_path(value))
        .collect()
}

pub(super) fn sorted_deduped_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn symbol_matches_targets(symbol_name: &str, targets: &HashSet<String>) -> bool {
    targets.iter().any(|target| {
        symbol_name == target
            || symbol_name.starts_with(&format!("{target}."))
            || symbol_name.starts_with(&format!("{target}[*]."))
            || symbol_name.starts_with(&format!("{target}.*."))
    })
}

pub(crate) fn pattern_list_matches(patterns: &[String], path: &str, symbol_name: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| scoped_pattern_matches(pattern, path, symbol_name))
}

pub(crate) fn scoped_pattern_matches(pattern: &str, path: &str, symbol_name: &str) -> bool {
    let (path_pattern, symbol_pattern) = match pattern.split_once("::") {
        Some((path_pattern, symbol_pattern)) => (path_pattern, Some(symbol_pattern)),
        None => (pattern, None),
    };
    if !rules::glob_match(path_pattern, path) {
        return false;
    }

    match symbol_pattern {
        None => true,
        Some("*") | Some("**") => true,
        Some(symbol_pattern) => symbol_pattern_matches(symbol_pattern, symbol_name),
    }
}

pub(crate) fn symbol_pattern_matches(pattern: &str, symbol_name: &str) -> bool {
    if pattern == symbol_name {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return symbol_name == prefix || symbol_name.starts_with(&format!("{prefix}."));
    }
    if let Some(prefix) = pattern.strip_suffix(".**") {
        return symbol_name == prefix || symbol_name.starts_with(&format!("{prefix}."));
    }

    false
}
