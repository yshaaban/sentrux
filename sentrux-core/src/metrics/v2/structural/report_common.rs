use super::path_roles::has_role;
use super::utils::{dedupe_strings_preserve_order, path_category};
use super::FileFacts;
use std::collections::BTreeSet;

pub(super) use super::path_roles::has_role_tag;

pub(super) fn graph_path_count(paths: Option<&BTreeSet<String>>) -> usize {
    paths.map(BTreeSet::len).unwrap_or(0)
}

pub(super) fn dependency_category_summaries(
    paths: Option<&BTreeSet<String>>,
    limit: usize,
) -> Vec<String> {
    let Some(paths) = paths else {
        return Vec::new();
    };

    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for path in paths {
        let category = path_category(path);
        counts
            .entry(category)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }

    let mut categories = counts.into_iter().collect::<Vec<_>>();
    categories.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    categories
        .into_iter()
        .take(limit)
        .map(|(category, count)| format!("{category}({count})"))
        .collect()
}

pub(super) fn dependency_category_axes(
    paths: Option<&BTreeSet<String>>,
    limit: usize,
) -> Vec<String> {
    let Some(paths) = paths else {
        return vec!["orchestration boundary".to_string()];
    };

    let categories = dominant_categories(paths, limit);
    if categories.is_empty() {
        return vec!["orchestration boundary".to_string()];
    }

    categories
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect()
}

pub(super) fn hotspot_split_axes(
    facts: &FileFacts,
    incoming: Option<&BTreeSet<String>>,
    outgoing: Option<&BTreeSet<String>>,
    limit: usize,
) -> Vec<String> {
    let inbound_categories = dominant_categories_from_option(incoming, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} caller boundary"))
        .collect::<Vec<_>>();
    let outbound_categories = dominant_categories_from_option(outgoing, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect::<Vec<_>>();

    let mut axes = inbound_categories;
    axes.extend(outbound_categories);
    if has_role(facts, "guarded_boundary") {
        axes.push("guarded boundary".to_string());
    }
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    let mut axes = dedupe_strings_preserve_order(axes);
    axes.truncate(limit.max(1));
    if axes.is_empty() {
        axes.push("stable contract boundary".to_string());
    }
    axes
}

pub(super) fn large_file_split_axes(
    facts: &FileFacts,
    outgoing_paths: Option<&BTreeSet<String>>,
) -> Vec<String> {
    let mut axes = outgoing_paths
        .map(|paths| dependency_category_axes(Some(paths), 3))
        .unwrap_or_default();
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    if has_role(facts, "entry_surface") {
        axes.push("entry surface split".to_string());
    }
    if facts.max_complexity >= 40 {
        axes.push("high-complexity helper extraction".to_string());
    }
    if facts.function_count >= 20 {
        axes.push("private helper surface split".to_string());
    }
    let mut axes = dedupe_strings_preserve_order(axes);
    if axes.is_empty() {
        axes.push("orchestration boundary".to_string());
    }
    axes
}

fn dominant_categories(paths: &BTreeSet<String>, limit: usize) -> Vec<String> {
    dependency_category_summaries(Some(paths), limit)
        .into_iter()
        .filter_map(|summary| {
            summary
                .split_once('(')
                .map(|(category, _)| category.to_string())
        })
        .collect()
}

fn dominant_categories_from_option(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| dominant_categories(paths, limit))
        .unwrap_or_default()
}
