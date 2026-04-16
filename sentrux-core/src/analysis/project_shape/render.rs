use super::{ModuleContractSuggestion, ProjectShapeReport};
use std::collections::BTreeSet;

pub(super) fn render_starter_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    let mut lines = render_base_rules_lines(shape, primary_language, existing_excludes);
    for contract in &shape.module_contracts {
        push_module_contract_lines(&mut lines, contract, false, true);
    }

    if !shape.boundary_roots.is_empty() {
        lines.push(String::new());
        lines.push("# Candidate boundary roots detected from repo shape:".to_string());
        for boundary in &shape.boundary_roots {
            lines.push(format!(
                "# - {} at {} ({})",
                boundary.kind,
                boundary.root,
                boundary.evidence.join(", ")
            ));
        }
    }

    lines.join("\n") + "\n"
}

pub(super) fn render_working_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    let mut lines = render_base_rules_lines(shape, primary_language, existing_excludes);
    for contract in &shape.module_contracts {
        push_module_contract_lines(&mut lines, contract, true, false);
    }
    lines.join("\n") + "\n"
}

fn push_module_contract_lines(
    lines: &mut Vec<String>,
    contract: &ModuleContractSuggestion,
    include_nested_public_api: bool,
    include_comments: bool,
) {
    lines.push(String::new());
    lines.push("[[module_contract]]".to_string());
    lines.push(format!("id = {:?}", contract.id));
    lines.push(format!("root = {:?}", contract.root));
    lines.push(format!(
        "public_api = [{}]",
        quoted_list(&contract.public_api)
    ));
    if include_nested_public_api && !contract.nested_public_api.is_empty() {
        lines.push(format!(
            "nested_public_api = [{}]",
            quoted_list(&contract.nested_public_api)
        ));
    }
    lines.push("forbid_cross_module_deep_imports = true".to_string());
    if include_comments {
        lines.push(format!("# confidence: {}", contract.confidence));
        if !contract.nested_public_api.is_empty() {
            lines.push(format!(
                "# observed nested public APIs: {}",
                contract.nested_public_api.join(", ")
            ));
        }
    }
}

fn render_base_rules_lines(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut excludes = existing_excludes.to_vec();
    excludes.extend(suggest_excludes(shape));
    excludes = dedupe_strings(excludes);

    lines.push("[project]".to_string());
    if let Some(language) = primary_language {
        lines.push(format!("primary_language = {:?}", language));
    }
    if !shape.effective_archetypes.is_empty() {
        lines.push(format!(
            "archetypes = [{}]",
            quoted_list(&shape.effective_archetypes)
        ));
    }
    if !excludes.is_empty() {
        lines.push(format!("exclude = [{}]", quoted_list(&excludes)));
    }
    lines
}

fn suggest_excludes(shape: &ProjectShapeReport) -> Vec<String> {
    let mut excludes = vec!["node_modules/**".to_string(), "coverage/**".to_string()];

    if shape
        .effective_archetypes
        .iter()
        .any(|entry| entry.contains("nextjs"))
    {
        excludes.push(".next/**".to_string());
    }
    if shape
        .capabilities
        .iter()
        .any(|entry| entry == "feature_modules")
    {
        excludes.push("tmp/**".to_string());
    }

    excludes
}

fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}
