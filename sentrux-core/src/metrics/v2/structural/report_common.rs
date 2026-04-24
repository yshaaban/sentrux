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

#[derive(Debug, Clone)]
pub(super) struct LargeFileFirstCut {
    pub(super) extraction: String,
    pub(super) admissibility: String,
    pub(super) confidence_0_10000: u32,
}

pub(super) fn large_file_first_cut(
    facts: &FileFacts,
    role_tags: &[String],
    outgoing_paths: Option<&BTreeSet<String>>,
    candidate_split_axes: &[String],
) -> LargeFileFirstCut {
    let fan_out = graph_path_count(outgoing_paths);
    let first_axis = candidate_split_axes
        .first()
        .cloned()
        .unwrap_or_else(|| "orchestration boundary".to_string());
    let first_surface = outgoing_paths.and_then(|paths| paths.iter().next().cloned());
    let extraction = if has_role(facts, "facade_with_extracted_owners") {
        format!("move remaining facade owner behavior behind the {first_axis}")
    } else if has_role_tag(role_tags, "composition_root")
        || has_role_tag(role_tags, "entry_surface")
    {
        match first_surface {
            Some(surface) => {
                format!("move entry orchestration coupled to {surface} behind the {first_axis}")
            }
            None => format!("extract entry orchestration behind the {first_axis}"),
        }
    } else if facts.max_complexity >= 40 {
        "extract the highest-complexity helper path into a focused module".to_string()
    } else if facts.function_count >= 20 {
        "extract the private helper group into a focused module".to_string()
    } else if let Some(surface) = first_surface {
        format!("move behavior coupled to {surface} behind the {first_axis}")
    } else {
        format!("start with a narrow {first_axis} only after identifying a cohesive helper group")
    };

    let mut confidence = 1_200u32;
    let mut reasons = Vec::new();
    if facts.function_count >= 20 {
        confidence += 1_800;
        reasons.push(format!("symbol/function count: {}", facts.function_count));
    } else {
        reasons.push(format!("symbol/function count: {}", facts.function_count));
    }
    if facts.max_complexity >= 40 {
        confidence += 2_000;
        reasons.push(format!("peak complexity: {}", facts.max_complexity));
    } else if facts.max_complexity >= 25 {
        confidence += 1_000;
        reasons.push(format!("peak complexity: {}", facts.max_complexity));
    } else {
        reasons.push(format!("peak complexity: {}", facts.max_complexity));
    }
    if fan_out >= 2 {
        confidence += 1_600;
        reasons.push(format!("fan-out: {}", fan_out));
    } else {
        reasons.push(format!("fan-out: {}", fan_out));
    }
    if !facts.guardrail_tests.is_empty() {
        confidence += 1_700;
        reasons.push(format!("guardrail tests: {}", facts.guardrail_tests.len()));
    } else {
        reasons.push("guardrail tests: 0".to_string());
    }
    if role_tags.iter().any(|tag| {
        matches!(
            tag.as_str(),
            "composition_root" | "entry_surface" | "facade_with_extracted_owners" | "guarded_seam"
        )
    }) {
        confidence += 1_000;
        reasons.push(format!("role tags: {}", role_tags.join(", ")));
    } else if !role_tags.is_empty() {
        reasons.push(format!("role tags: {}", role_tags.join(", ")));
    } else {
        reasons.push("role tags: none".to_string());
    }

    let confidence = confidence.min(10_000);
    let label = match confidence {
        6_500..=10_000 => "high",
        4_000..=6_499 => "medium",
        _ => "watchpoint",
    };
    LargeFileFirstCut {
        extraction,
        admissibility: format!("{label} ({})", reasons.join("; ")),
        confidence_0_10000: confidence,
    }
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
