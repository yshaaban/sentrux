use std::collections::BTreeSet;

pub(super) fn sample_paths(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| paths.iter().take(limit).cloned().collect())
        .unwrap_or_default()
}

pub(super) fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

pub(super) fn path_category(path: &str) -> String {
    let normalized = path.strip_prefix("./").unwrap_or(path);
    if let Some(rest) = normalized.strip_prefix("src/") {
        return rest.split('/').next().unwrap_or("src").to_string();
    }
    normalized
        .split('/')
        .next()
        .unwrap_or(normalized)
        .to_string()
}

pub(super) fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
