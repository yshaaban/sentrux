use super::is_package_index_for_path;
use crate::core::types::{EntryPoint, FileNode, ImportEdge};
use crate::metrics::testgap;
use crate::metrics::types::{
    DuplicateGroup, FileMetric, FileMetrics, FuncMetric, InstabilityMetric,
};
use std::collections::{HashMap, HashSet};

fn compute_fan_maps(
    import_edges: &[ImportEdge],
    call_edges: &[crate::core::types::CallEdge],
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut fan_out: HashMap<String, usize> = HashMap::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    for edge in import_edges {
        if seen.insert((edge.from_file.clone(), edge.to_file.clone())) {
            *fan_out.entry(edge.from_file.clone()).or_default() += 1;
            *fan_in.entry(edge.to_file.clone()).or_default() += 1;
        }
    }
    for edge in call_edges {
        if seen.insert((edge.from_file.clone(), edge.to_file.clone())) {
            *fan_out.entry(edge.from_file.clone()).or_default() += 1;
            *fan_in.entry(edge.to_file.clone()).or_default() += 1;
        }
    }
    (fan_out, fan_in)
}

fn fan_out_threshold_for_path(path: &str) -> usize {
    let ext = path.rsplit('.').next().unwrap_or("");
    let lang = crate::analysis::lang_registry::detect_lang_from_ext(ext);
    crate::analysis::lang_registry::profile(&lang)
        .thresholds
        .fan_out
}

fn fan_in_threshold_for_path(path: &str) -> usize {
    let ext = path.rsplit('.').next().unwrap_or("");
    let lang = crate::analysis::lang_registry::detect_lang_from_ext(ext);
    crate::analysis::lang_registry::profile(&lang)
        .thresholds
        .fan_in
}

fn detect_god_files(
    fan_out: &HashMap<String, usize>,
    entry_points: &[EntryPoint],
) -> Vec<FileMetric> {
    let entry_file_set: HashSet<&str> = entry_points.iter().map(|ep| ep.file.as_str()).collect();
    let mut values: Vec<FileMetric> = fan_out
        .iter()
        .filter(|(path, &count)| {
            let threshold = fan_out_threshold_for_path(path);
            count > threshold
                && !entry_file_set.contains(path.as_str())
                && !is_package_index_for_path(path)
        })
        .map(|(path, &count)| FileMetric {
            path: path.clone(),
            value: count,
        })
        .collect();
    values.sort_unstable_by(|left, right| right.value.cmp(&left.value));
    values
}

fn detect_hotspot_files(
    fan_in: &HashMap<String, usize>,
    fan_out: &HashMap<String, usize>,
) -> Vec<FileMetric> {
    let mut values: Vec<FileMetric> = fan_in
        .iter()
        .filter(|(path, &count)| {
            let threshold = fan_in_threshold_for_path(path);
            if count <= threshold {
                return false;
            }
            if is_package_index_for_path(path) {
                return false;
            }

            let fan_out_count = *fan_out.get(path.as_str()).unwrap_or(&0);
            let instability = fan_out_count as f64 / (count + fan_out_count) as f64;
            instability >= 0.15
        })
        .map(|(path, &count)| FileMetric {
            path: path.clone(),
            value: count,
        })
        .collect();
    values.sort_unstable_by(|left, right| right.value.cmp(&left.value));
    values
}

fn compute_instability(
    import_edges: &[ImportEdge],
    fan_out: &HashMap<String, usize>,
    fan_in: &HashMap<String, usize>,
) -> Vec<InstabilityMetric> {
    let mut all_files: HashSet<&str> = HashSet::new();
    for edge in import_edges {
        all_files.insert(edge.from_file.as_str());
        all_files.insert(edge.to_file.as_str());
    }

    let mut values: Vec<InstabilityMetric> = all_files
        .iter()
        .filter(|&&path| !testgap::is_test_file(path))
        .map(|&path| {
            let ce = *fan_out.get(path).unwrap_or(&0);
            let ca = *fan_in.get(path).unwrap_or(&0);
            let total = ca + ce;
            let instability = if total == 0 {
                0.5
            } else {
                ce as f64 / total as f64
            };
            InstabilityMetric {
                path: path.to_string(),
                instability,
                fan_in: ca,
                fan_out: ce,
            }
        })
        .collect();
    values.sort_unstable_by(|left, right| {
        right
            .instability
            .partial_cmp(&left.instability)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    values.truncate(10);
    values
}

fn collect_functions_exceeding(
    files: &[&FileNode],
    extract_value: impl Fn(&FileNode, &crate::core::types::FuncInfo) -> Option<u32>,
) -> Vec<FuncMetric> {
    let mut result = Vec::new();
    for file in files {
        let funcs = match file.sa.as_ref().and_then(|sa| sa.functions.as_ref()) {
            Some(funcs) => funcs,
            None => continue,
        };
        for function in funcs {
            if let Some(value) = extract_value(file, function) {
                result.push(FuncMetric {
                    file: file.path.clone(),
                    func: function.n.clone(),
                    value,
                });
            }
        }
    }
    result.sort_unstable_by(|left, right| right.value.cmp(&left.value));
    result
}

fn collect_per_function_metrics(files: &[&FileNode]) -> (Vec<FuncMetric>, Vec<FuncMetric>) {
    let complex_functions = collect_functions_exceeding(files, |file, function| {
        let threshold = crate::analysis::lang_registry::profile(&file.lang)
            .thresholds
            .cc_high;
        function.cc.filter(|&cc| cc > threshold)
    });
    let long_functions = collect_functions_exceeding(files, |file, function| {
        let threshold = crate::analysis::lang_registry::profile(&file.lang)
            .thresholds
            .func_length;
        if function.ln > threshold {
            Some(function.ln)
        } else {
            None
        }
    });

    (complex_functions, long_functions)
}

pub(super) fn collect_all_function_ccs(files: &[&FileNode]) -> Vec<FuncMetric> {
    collect_functions_exceeding(files, |_file, function| function.cc)
}

pub(super) fn collect_all_function_lines(files: &[&FileNode]) -> Vec<FuncMetric> {
    collect_functions_exceeding(files, |_file, function| Some(function.ln))
}

pub(super) fn collect_all_file_lines(files: &[&FileNode]) -> Vec<FileMetric> {
    files
        .iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| FileMetric {
            path: file.path.clone(),
            value: file.lines as usize,
        })
        .collect()
}

fn compute_comment_ratio(files: &[&FileNode]) -> Option<f64> {
    let (total_comments, total_lines): (u64, u64) = files
        .iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .fold((0u64, 0u64), |(comments, lines), file| {
            (comments + file.comments as u64, lines + file.lines as u64)
        });
    if total_lines > 0 {
        Some(total_comments as f64 / total_lines as f64)
    } else {
        None
    }
}

fn compute_large_file_stats(files: &[&FileNode]) -> (Vec<FileMetric>, usize, f64) {
    let long_files: Vec<FileMetric> = files
        .iter()
        .filter(|file| {
            if file.lang.is_empty() || file.lang == "unknown" {
                return false;
            }
            let threshold = crate::analysis::lang_registry::profile(&file.lang)
                .thresholds
                .large_file_lines;
            file.lines > threshold
        })
        .map(|file| FileMetric {
            path: file.path.clone(),
            value: file.lines as usize,
        })
        .collect();
    let large_file_count = long_files.len();
    let code_file_count = files
        .iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .count();
    let large_file_ratio = if code_file_count == 0 || large_file_count == 0 {
        0.0
    } else {
        large_file_count as f64 / code_file_count as f64
    };

    (long_files, large_file_count, large_file_ratio)
}

fn collect_cog_complex_functions(files: &[&FileNode]) -> Vec<FuncMetric> {
    collect_functions_exceeding(files, |file, function| {
        let threshold = crate::analysis::lang_registry::profile(&file.lang)
            .thresholds
            .cog_high;
        function.cog.filter(|&cog| cog > threshold)
    })
}

fn collect_high_param_functions(files: &[&FileNode]) -> Vec<FuncMetric> {
    collect_functions_exceeding(files, |file, function| {
        let threshold = crate::analysis::lang_registry::profile(&file.lang)
            .thresholds
            .param_high;
        function.pc.filter(|&pc| pc > threshold)
    })
}

fn collect_file_body_hashes(
    file: &FileNode,
    hash_map: &mut HashMap<u64, Vec<(String, String, u32)>>,
) {
    let funcs = match file.sa.as_ref().and_then(|sa| sa.functions.as_ref()) {
        Some(funcs) => funcs,
        None => return,
    };
    for function in funcs {
        if let Some(body_hash) = function.bh {
            if body_hash != 0 {
                hash_map.entry(body_hash).or_default().push((
                    file.path.clone(),
                    function.n.clone(),
                    function.ln,
                ));
            }
        }
    }
}

fn build_body_hash_map(files: &[&FileNode]) -> HashMap<u64, Vec<(String, String, u32)>> {
    let mut hash_map: HashMap<u64, Vec<(String, String, u32)>> = HashMap::new();
    for file in files {
        collect_file_body_hashes(file, &mut hash_map);
    }
    hash_map
}

fn collect_duplicate_groups(files: &[&FileNode]) -> Vec<DuplicateGroup> {
    let hash_map = build_body_hash_map(files);
    let mut groups: Vec<DuplicateGroup> = hash_map
        .into_iter()
        .filter(|(_, instances)| instances.len() > 1)
        .map(|(hash, instances)| DuplicateGroup { hash, instances })
        .collect();
    groups.sort_unstable_by(|left, right| right.instances.len().cmp(&left.instances.len()));
    groups
}

fn insert_call_with_base(all_calls: &mut HashSet<String>, call: &str) {
    all_calls.insert(call.to_string());
    if let Some(base) = call.rsplit("::").next() {
        all_calls.insert(base.to_string());
    }
}

fn insert_calls_from_list(all_calls: &mut HashSet<String>, calls: &[String]) {
    for call in calls {
        insert_call_with_base(all_calls, call);
    }
}

fn collect_file_calls(
    all_calls: &mut HashSet<String>,
    structural_analysis: &crate::core::types::StructuralAnalysis,
) {
    if let Some(module_calls) = &structural_analysis.co {
        insert_calls_from_list(all_calls, module_calls);
    }
    if let Some(functions) = &structural_analysis.functions {
        for function in functions {
            if let Some(function_calls) = &function.co {
                insert_calls_from_list(all_calls, function_calls);
            }
        }
    }
}

fn build_call_target_set(files: &[&FileNode]) -> HashSet<String> {
    let mut all_calls: HashSet<String> = HashSet::new();
    for file in files {
        if let Some(structural_analysis) = &file.sa {
            collect_file_calls(&mut all_calls, structural_analysis);
        }
    }
    all_calls
}

const DEFAULT_IMPLICIT_ENTRY_POINTS: &[&str] = &[
    "main",
    "new",
    "default",
    "init",
    "setup",
    "teardown",
    "run",
    "start",
    "stop",
    "build",
    "configure",
    "register",
    "update",
    "draw",
    "render",
    "getDerivedStateFromError",
    "componentDidCatch",
    "serialize",
    "deserialize",
];

const DEFAULT_TEST_PREFIXES: &[&str] = &["test_"];

fn implicit_entry_points() -> HashSet<String> {
    let mut set: HashSet<String> = DEFAULT_IMPLICIT_ENTRY_POINTS
        .iter()
        .map(|value| value.to_string())
        .collect();
    for profile in crate::analysis::lang_registry::all_profiles() {
        for entry_point in &profile.semantics.implicit_entry_points {
            set.insert(entry_point.clone());
        }
    }
    set
}

fn is_dead_code_skip_file(file: &FileNode) -> bool {
    let profile = crate::analysis::lang_registry::profile(&file.lang);
    if profile.is_test_file(&file.path) {
        return true;
    }
    if file.path.contains("test") || file.path.contains("/tests/") {
        return true;
    }
    if let Some(structural_analysis) = &file.sa {
        if let Some(tags) = &structural_analysis.tags {
            if tags.iter().any(|tag| tag.contains("test")) {
                return true;
            }
        }
    }

    false
}

fn is_excluded_function(func_name: &str, implicit: &HashSet<String>, lang: &str) -> bool {
    let profile = crate::analysis::lang_registry::profile(lang);
    let semantics = &profile.semantics;

    if semantics.test_function_prefixes.is_empty() {
        for prefix in DEFAULT_TEST_PREFIXES {
            if func_name.starts_with(prefix) {
                return true;
            }
        }
    } else {
        for prefix in &semantics.test_function_prefixes {
            if func_name.starts_with(prefix.as_str()) {
                return true;
            }
        }
    }

    if !semantics.qualified_name_separator.is_empty()
        && func_name.contains(&semantics.qualified_name_separator)
    {
        return true;
    }

    let separator = if semantics.qualified_name_separator.is_empty() {
        "::"
    } else {
        &semantics.qualified_name_separator
    };
    let base_name = func_name.rsplit(separator).next().unwrap_or(func_name);
    implicit.contains(base_name)
}

fn is_called(func_name: &str, all_calls: &HashSet<String>) -> bool {
    let base_name = func_name.rsplit("::").next().unwrap_or(func_name);
    all_calls.contains(func_name) || all_calls.contains(base_name)
}

fn has_non_call_references(function: &crate::core::types::FuncInfo) -> bool {
    function.same_file_ref_count.is_some_and(|count| count > 0)
}

fn should_report_dead_function(
    file: &FileNode,
    function: &crate::core::types::FuncInfo,
    all_calls: &HashSet<String>,
    implicit: &HashSet<String>,
) -> bool {
    if function.is_public || function.is_method {
        return false;
    }
    if is_excluded_function(&function.n, implicit, &file.lang) {
        return false;
    }

    !is_called(&function.n, all_calls) && !has_non_call_references(function)
}

fn collect_dead_functions(files: &[&FileNode]) -> Vec<FuncMetric> {
    let all_calls = build_call_target_set(files);
    let implicit = implicit_entry_points();
    let mut result = Vec::new();

    for file in files {
        if is_dead_code_skip_file(file) {
            continue;
        }
        let functions = match file.sa.as_ref().and_then(|sa| sa.functions.as_ref()) {
            Some(functions) => functions,
            None => continue,
        };
        for function in functions {
            if should_report_dead_function(file, function, &all_calls, &implicit) {
                result.push(FuncMetric {
                    file: file.path.clone(),
                    func: function.n.clone(),
                    value: function.ln,
                });
            }
        }
    }

    result.sort_unstable_by(|left, right| right.value.cmp(&left.value));
    result
}

fn simple_ratio(count: usize, total: usize) -> f64 {
    if total == 0 || count == 0 {
        return 0.0;
    }
    count as f64 / total as f64
}

pub(super) fn count_total_funcs(files: &[&FileNode]) -> usize {
    files
        .iter()
        .filter_map(|file| file.sa.as_ref())
        .filter_map(|structural_analysis| structural_analysis.functions.as_ref())
        .map(|functions| functions.len())
        .sum()
}

pub(super) fn compute_file_metrics(
    files: &[&FileNode],
    import_edges: &[ImportEdge],
    call_edges: &[crate::core::types::CallEdge],
    entry_points: &[EntryPoint],
) -> FileMetrics {
    let (fan_out, fan_in) = compute_fan_maps(import_edges, call_edges);
    let god_files = detect_god_files(&fan_out, entry_points);
    let hotspot_files = detect_hotspot_files(&fan_in, &fan_out);
    let most_unstable = compute_instability(import_edges, &fan_out, &fan_in);
    let (complex_functions, long_functions) = collect_per_function_metrics(files);
    let cog_complex_functions = collect_cog_complex_functions(files);
    let high_param_functions = collect_high_param_functions(files);
    let duplicate_groups = collect_duplicate_groups(files);
    let dead_functions = collect_dead_functions(files);

    let total_funcs = count_total_funcs(files);
    let complex_fn_ratio = simple_ratio(complex_functions.len(), total_funcs);
    let long_fn_ratio = simple_ratio(long_functions.len(), total_funcs);
    let cog_complex_ratio = simple_ratio(cog_complex_functions.len(), total_funcs);
    let high_param_ratio = simple_ratio(high_param_functions.len(), total_funcs);
    let duplicate_func_count: usize = duplicate_groups
        .iter()
        .map(|group| group.instances.len())
        .sum();
    let duplication_ratio = simple_ratio(duplicate_func_count, total_funcs);
    let dead_code_ratio = simple_ratio(dead_functions.len(), total_funcs);

    let comment_ratio = compute_comment_ratio(files);
    let (long_files, large_file_count, large_file_ratio) = compute_large_file_stats(files);
    let code_file_count = files
        .iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .count();
    let god_ratio = simple_ratio(god_files.len(), code_file_count);
    let hotspot_ratio = simple_ratio(hotspot_files.len(), code_file_count);

    FileMetrics {
        fan_out,
        fan_in,
        god_files,
        hotspot_files,
        most_unstable,
        complex_functions,
        long_functions,
        long_files,
        complex_fn_ratio,
        long_fn_ratio,
        comment_ratio,
        large_file_count,
        large_file_ratio,
        god_ratio,
        hotspot_ratio,
        cog_complex_functions,
        high_param_functions,
        duplicate_groups,
        dead_functions,
        duplication_ratio,
        dead_code_ratio,
        high_param_ratio,
        cog_complex_ratio,
    }
}
