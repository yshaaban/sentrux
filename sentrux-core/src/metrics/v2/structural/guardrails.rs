use super::utils::dedupe_strings_preserve_order;
use ignore::WalkBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub(super) struct GuardrailFileEvidence {
    pub(super) tests: Vec<String>,
    pub(super) required_literals: Vec<String>,
    pub(super) forbidden_literals: Vec<String>,
    pub(super) facade_owner_factories: Vec<String>,
    pub(super) boundary_guard_literals: Vec<String>,
}

pub(super) fn detect_architecture_guardrails(
    root: &Path,
    file_paths: &BTreeSet<String>,
) -> BTreeMap<String, GuardrailFileEvidence> {
    let mut evidence_by_file = BTreeMap::<String, GuardrailFileEvidence>::new();

    for (test_path, contents) in walk_guardrail_test_sources(root) {
        let named_targets = named_guardrail_targets(&test_path, file_paths);
        let explicit_targets = explicit_guardrail_targets(&contents, file_paths);
        let required_literals = test_contains_literals(&contents, false);
        let forbidden_literals = test_contains_literals(&contents, true);
        let facade_owner_factories = required_literals
            .iter()
            .filter(|literal| is_facade_owner_factory(literal))
            .cloned()
            .collect::<Vec<_>>();
        let boundary_targets = forbidden_literals
            .iter()
            .flat_map(|literal| resolve_module_literal_targets(literal, file_paths))
            .collect::<Vec<_>>();

        for target in named_targets
            .into_iter()
            .chain(explicit_targets.into_iter())
            .collect::<BTreeSet<_>>()
        {
            let entry = evidence_by_file.entry(target).or_default();
            entry.tests.push(test_path.clone());
            entry
                .required_literals
                .extend(required_literals.iter().cloned());
            entry
                .forbidden_literals
                .extend(forbidden_literals.iter().cloned());
            entry
                .facade_owner_factories
                .extend(facade_owner_factories.iter().cloned());
        }

        for target in boundary_targets {
            let entry = evidence_by_file.entry(target).or_default();
            entry.tests.push(test_path.clone());
            entry
                .boundary_guard_literals
                .extend(forbidden_literals.iter().cloned());
        }
    }

    for evidence in evidence_by_file.values_mut() {
        evidence.tests = dedupe_strings_preserve_order(std::mem::take(&mut evidence.tests));
        evidence.required_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.required_literals));
        evidence.forbidden_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.forbidden_literals));
        evidence.facade_owner_factories =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.facade_owner_factories));
        evidence.boundary_guard_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.boundary_guard_literals));
    }

    evidence_by_file
}

fn walk_guardrail_test_sources(root: &Path) -> Vec<(String, String)> {
    let mut sources = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.into_path();
            if !path.is_file() {
                return None;
            }
            let relative_path = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if !is_guardrail_test_path(&relative_path) {
                return None;
            }
            let contents = std::fs::read_to_string(&path).ok()?;
            Some((relative_path, contents))
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

fn is_guardrail_test_path(path: &str) -> bool {
    path.ends_with(".architecture.test.ts")
        || path.ends_with(".architecture.test.tsx")
        || path.ends_with(".architecture.spec.ts")
        || path.ends_with(".architecture.spec.tsx")
}

fn named_guardrail_targets(test_path: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    let Some(file_name) = test_path.rsplit('/').next() else {
        return Vec::new();
    };
    let Some(stem) = file_name
        .strip_suffix(".architecture.test.ts")
        .or_else(|| file_name.strip_suffix(".architecture.test.tsx"))
        .or_else(|| file_name.strip_suffix(".architecture.spec.ts"))
        .or_else(|| file_name.strip_suffix(".architecture.spec.tsx"))
    else {
        return Vec::new();
    };
    let directory = test_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or_default();
    [".ts", ".tsx", ".js", ".jsx"]
        .into_iter()
        .map(|extension| {
            if directory.is_empty() {
                format!("{stem}{extension}")
            } else {
                format!("{directory}/{stem}{extension}")
            }
        })
        .filter(|candidate| file_paths.contains(candidate))
        .collect()
}

fn explicit_guardrail_targets(contents: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    quoted_literals(contents)
        .into_iter()
        .filter(|literal| looks_like_source_file_path(literal))
        .map(|literal| literal.trim_start_matches("./").replace('\\', "/"))
        .filter(|literal| file_paths.contains(literal))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn test_contains_literals(contents: &str, negated: bool) -> Vec<String> {
    let mut literals = Vec::new();
    let needle = if negated {
        ".not.toContain("
    } else {
        ".toContain("
    };
    let mut cursor = 0;
    while let Some(offset) = contents[cursor..].find(needle) {
        let start = cursor + offset + needle.len();
        let suffix = &contents[start..];
        let Some(quote) = suffix.chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' && quote != '`' {
            cursor = start;
            continue;
        }
        if let Some((literal, advance)) = quoted_literal_after(contents, start) {
            literals.push(literal);
            cursor = advance;
            continue;
        }
        cursor = start + 1;
    }
    dedupe_strings_preserve_order(literals)
}

fn resolve_module_literal_targets(literal: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    if !looks_like_import_fragment(literal) {
        return Vec::new();
    }

    file_paths
        .iter()
        .filter(|path| path_without_extension(path).ends_with(literal))
        .cloned()
        .collect()
}

fn looks_like_source_file_path(literal: &str) -> bool {
    (literal.ends_with(".ts")
        || literal.ends_with(".tsx")
        || literal.ends_with(".js")
        || literal.ends_with(".jsx"))
        && literal.contains('/')
}

fn looks_like_import_fragment(literal: &str) -> bool {
    literal.contains('/')
        && !literal.contains(' ')
        && !literal.ends_with(".ts")
        && !literal.ends_with(".tsx")
        && !literal.ends_with(".js")
        && !literal.ends_with(".jsx")
}

fn path_without_extension(path: &str) -> &str {
    path.strip_suffix(".tsx")
        .or_else(|| path.strip_suffix(".ts"))
        .or_else(|| path.strip_suffix(".jsx"))
        .or_else(|| path.strip_suffix(".js"))
        .unwrap_or(path)
}

fn is_facade_owner_factory(literal: &str) -> bool {
    literal.starts_with("create")
        && literal
            .chars()
            .nth(6)
            .is_some_and(|character| character.is_ascii_uppercase())
}

fn quoted_literals(contents: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut index = 0;
    while index < contents.len() {
        if let Some((literal, advance)) = quoted_literal_after(contents, index) {
            literals.push(literal);
            index = advance;
        } else {
            index += 1;
        }
    }
    literals
}

fn quoted_literal_after(contents: &str, start: usize) -> Option<(String, usize)> {
    let suffix = contents.get(start..)?;
    let quote = suffix.chars().next()?;
    if quote != '"' && quote != '\'' && quote != '`' {
        return None;
    }

    let mut escaped = false;
    let mut literal = String::new();
    for (offset, character) in suffix.char_indices().skip(1) {
        if escaped {
            literal.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            return Some((literal, start + offset + character.len_utf8()));
        }
        literal.push(character);
    }

    None
}
