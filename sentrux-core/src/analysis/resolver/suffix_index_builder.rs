use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::analysis::resolver::helpers::{file_to_module_path, SuffixIndex};

use super::aliases::{apply_alias_transform, extract_name_from_manifest};

/// Add all suffixes of a module path to the index, pointing to the given file.
/// e.g. "a/b/c" generates suffixes ["a/b/c", "b/c", "c"].
fn add_module_suffixes<'a>(
    index: &mut HashMap<String, Vec<&'a str>>,
    module_path: &str,
    file_path: &'a str,
) {
    let mut pos = 0;
    loop {
        let suffix = &module_path[pos..];
        if !suffix.is_empty() {
            index.entry(suffix.to_string()).or_default().push(file_path);
        }
        match module_path[pos..].find('/') {
            Some(slash) => pos += slash + 1,
            None => break,
        }
    }
}

/// Extract a module path from a file content using a directive keyword.
/// Generic version: reads `<directive> <path>` from the first matching line.
fn extract_module_name_generic<'a>(content: &'a str, directive: &str) -> Option<&'a str> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(directive) {
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            return Some(rest.split_whitespace().next().unwrap_or(rest));
        }
    }
    None
}

/// Scan project roots for module prefix files and build a map of module paths to project dirs.
/// Reads module_prefix_file and module_prefix_directive from ALL loaded plugin profiles.
/// Sorted longest-first so more specific module paths match before shorter ones.
fn collect_module_prefixes(
    project_map: &HashMap<String, String>,
    scan_root: &Path,
) -> Vec<(String, String)> {
    let prefix_configs: Vec<(&str, &str)> = crate::analysis::lang_registry::all_profiles()
        .filter(|p| {
            !p.semantics.resolver.module_prefix_file.is_empty()
                && !p.semantics.resolver.module_prefix_directive.is_empty()
        })
        .map(|p| {
            (
                p.semantics.resolver.module_prefix_file.as_str(),
                p.semantics.resolver.module_prefix_directive.as_str(),
            )
        })
        .collect();

    if prefix_configs.is_empty() {
        return Vec::new();
    }

    let unique_roots: HashSet<&str> = project_map.values().map(|s| s.as_str()).collect();
    let mut prefixes = Vec::new();

    for &project_dir in &unique_roots {
        for &(prefix_file, directive) in &prefix_configs {
            let path = if project_dir.is_empty() {
                scan_root.join(prefix_file)
            } else {
                scan_root.join(project_dir).join(prefix_file)
            };

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(module_name) = extract_module_name_generic(&content, directive) {
                    prefixes.push((module_name.to_string(), project_dir.to_string()));
                }
            }
        }
    }
    prefixes.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    prefixes
}

/// Try to read a manifest and extract the transformed package name for an entry-point file.
fn extract_manifest_alias(
    file_path: &str,
    resolver: &crate::analysis::plugin::profile::ResolverConfig,
    scan_root: &Path,
) -> Option<String> {
    let project_dir = file_path
        .strip_suffix(&resolver.alias_entry_point)
        .unwrap_or("")
        .trim_end_matches('/');
    let manifest_path = if project_dir.is_empty() {
        scan_root.join(&resolver.alias_file)
    } else {
        scan_root.join(project_dir).join(&resolver.alias_file)
    };
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest_name = manifest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&resolver.alias_file);
    let name = extract_name_from_manifest(&content, &resolver.alias_field, manifest_name)?;
    let transformed = apply_alias_transform(&name, &resolver.alias_transform);
    if transformed.is_empty() {
        None
    } else {
        Some(transformed)
    }
}

/// Add exact package name → entry file to the manifest_name_aliases map.
fn inject_manifest_name_aliases<'a>(
    index: &mut HashMap<String, Vec<&'a str>>,
    known_files: &HashSet<&'a str>,
    scan_root: &Path,
) {
    for profile in crate::analysis::lang_registry::all_profiles() {
        let resolver = &profile.semantics.resolver;
        if resolver.alias_file.is_empty()
            || resolver.alias_field.is_empty()
            || resolver.alias_entry_point.is_empty()
        {
            continue;
        }

        let entry_filename = resolver
            .alias_entry_point
            .rsplit('/')
            .next()
            .unwrap_or(&resolver.alias_entry_point);

        for &file_path in known_files {
            let filename = file_path.rsplit('/').next().unwrap_or(file_path);
            if filename != entry_filename || !file_path.ends_with(&resolver.alias_entry_point) {
                continue;
            }
            if let Some(transformed) = extract_manifest_alias(file_path, resolver, scan_root) {
                index.entry(transformed).or_default().push(file_path);
            }
        }
    }
}

/// Map every suffix of every file's module path to that file.
/// Package index files use their parent directory as the module path.
pub(super) fn build_module_suffix_index<'a>(
    known_files: &HashSet<&'a str>,
    scan_root: &Path,
    project_map: &HashMap<String, String>,
) -> SuffixIndex<'a> {
    let mut index: HashMap<String, Vec<&'a str>> = HashMap::new();

    let dir_pkg_exts: Vec<String> = crate::analysis::lang_registry::all_profiles()
        .filter(|p| p.semantics.project.directory_is_package)
        .flat_map(|p| {
            crate::analysis::lang_registry::get(&p.name)
                .map(|c| c.extensions.clone())
                .unwrap_or_default()
        })
        .collect();

    for &file_path in known_files {
        let module_path = file_to_module_path(file_path);
        if module_path.is_empty() {
            continue;
        }

        add_module_suffixes(&mut index, module_path, file_path);

        if !dir_pkg_exts.is_empty() {
            let has_dir_pkg_ext = file_path
                .rsplit('.')
                .next()
                .is_some_and(|ext| dir_pkg_exts.iter().any(|e| e == ext));
            if has_dir_pkg_ext {
                if let Some((parent, _)) = module_path.rsplit_once('/') {
                    if !parent.is_empty() {
                        add_module_suffixes(&mut index, parent, file_path);
                    }
                }
            }
        }
    }

    let module_prefixes = collect_module_prefixes(project_map, scan_root);

    let mut manifest_name_aliases: HashMap<String, Vec<&'a str>> = HashMap::new();
    inject_manifest_name_aliases(&mut manifest_name_aliases, known_files, scan_root);

    SuffixIndex {
        index,
        manifest_name_aliases,
        module_prefixes,
    }
}
