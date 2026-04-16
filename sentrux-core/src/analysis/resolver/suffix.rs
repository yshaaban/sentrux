//! Unified import resolver — suffix-index for ALL languages.
//!
//! Resolves import specifiers by matching against a suffix index of all known
//! file paths. Handles relative imports, path aliases (from plugin-declared
//! config files like tsconfig.json), and monorepo project boundaries.

use crate::core::types::FileNode;
use crate::core::types::ImportEdge;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[path = "suffix_aliases.rs"]
mod aliases;
#[path = "suffix_index_builder.rs"]
mod index_builder;
#[path = "suffix_project_map.rs"]
mod project_map;

use super::helpers::{
    resolve_relative, try_resolve_name, try_suffix_resolve, ResolveEnv, SuffixIndex,
};
use aliases::{apply_path_alias, collect_manifest_path_aliases, load_path_aliases, PathAlias};
use index_builder::build_module_suffix_index;
use project_map::build_project_map;
// Re-export normalize_path so existing callers (tests, graph) still find it here.
pub(crate) use super::helpers::normalize_path;

/// Source file context for import resolution.
pub(crate) struct SourceContext<'a> {
    /// The import specifier string to resolve
    pub specifier: &'a str,
    /// The file containing this import statement
    pub file: &'a FileNode,
    /// Parent directory of the importing file
    pub file_dir: &'a Path,
}

/// Shared indexes used for resolution lookups.
pub(crate) struct ResolutionIndex<'a> {
    #[allow(dead_code)]
    /// Set of all known file paths in the scan (reserved for future resolution strategies)
    pub known_files: &'a HashSet<&'a str>,
    #[allow(dead_code)]
    /// Module-path suffix index for fuzzy file matching (reserved for future resolution strategies)
    pub suffix_index: &'a SuffixIndex<'a>,
}

/// Import resolution summary surfaced to scan trust reporting.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImportResolutionSummary {
    pub resolved: usize,
    pub unresolved_internal: usize,
    pub unresolved_external: usize,
    pub unresolved_unknown: usize,
}

impl ImportResolutionSummary {
    pub fn total_specs(&self) -> usize {
        self.resolved
            + self.unresolved_internal
            + self.unresolved_external
            + self.unresolved_unknown
    }
}

/// Atomic counters for resolution statistics.
pub(crate) struct ResolutionStats {
    /// Number of imports successfully resolved to a file
    pub resolved_count: std::sync::atomic::AtomicUsize,
    /// Number of imports that should have resolved inside the project but did not
    pub unresolved_internal_count: std::sync::atomic::AtomicUsize,
    /// Number of unresolved bare imports that appear to target external packages
    pub unresolved_external_count: std::sync::atomic::AtomicUsize,
    /// Number of unresolved imports whose intent is ambiguous from syntax alone
    pub unresolved_unknown_count: std::sync::atomic::AtomicUsize,
}

impl ResolutionStats {
    /// Create a new zeroed ResolutionStats.
    pub fn new() -> Self {
        Self {
            resolved_count: std::sync::atomic::AtomicUsize::new(0),
            unresolved_internal_count: std::sync::atomic::AtomicUsize::new(0),
            unresolved_external_count: std::sync::atomic::AtomicUsize::new(0),
            unresolved_unknown_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn summary(&self) -> ImportResolutionSummary {
        use std::sync::atomic::Ordering;

        ImportResolutionSummary {
            resolved: self.resolved_count.load(Ordering::Relaxed),
            unresolved_internal: self.unresolved_internal_count.load(Ordering::Relaxed),
            unresolved_external: self.unresolved_external_count.load(Ordering::Relaxed),
            unresolved_unknown: self.unresolved_unknown_count.load(Ordering::Relaxed),
        }
    }
}

/// Unified import resolution for ALL languages via suffix-index.
/// No tier split — JS/TS goes through the same resolver with path alias support.
pub(crate) fn resolve_path_imports_ref(
    files: &[&FileNode],
    scan_root: Option<&Path>,
) -> (Vec<ImportEdge>, ImportResolutionSummary) {
    let t0 = std::time::Instant::now();
    let scan_root = match scan_root {
        Some(r) => r,
        None => return (Vec::new(), ImportResolutionSummary::default()),
    };

    let known_files: HashSet<&str> = files
        .iter()
        .filter(|f| !f.is_dir)
        .map(|f| f.path.as_str())
        .collect();

    let mut exts = crate::analysis::lang_registry::all_extensions();
    exts.sort_unstable();

    let project_map = build_project_map(files, scan_root);
    let t_project_map = t0.elapsed();

    let suffix_index = build_module_suffix_index(&known_files, scan_root, &project_map);

    // Load path aliases from two sources:
    // 1. Config files (tsconfig.json paths) — declared in plugin.toml
    // 2. Manifest names (package.json "name", Cargo.toml "package.name") — auto-discovered
    let mut path_aliases = load_path_aliases(&project_map, scan_root);
    let manifest_aliases = collect_manifest_path_aliases(&project_map, scan_root);
    if !manifest_aliases.is_empty() {
        path_aliases
            .entry(String::new())
            .or_default()
            .extend(manifest_aliases);
    }
    let t_suffix = t0.elapsed();

    let (edges, summary) = resolve_tier2_imports(
        files,
        &known_files,
        &project_map,
        &suffix_index,
        &exts,
        &path_aliases,
    );
    let t_total = t0.elapsed();

    eprintln!(
        "[resolve_imports] project_map {:.1}ms, suffix_idx {:.1}ms, suffix_resolve {:.1}ms, total {:.1}ms",
        t_project_map.as_secs_f64() * 1000.0,
        (t_suffix - t_project_map).as_secs_f64() * 1000.0,
        (t_total - t_suffix).as_secs_f64() * 1000.0,
        t_total.as_secs_f64() * 1000.0,
    );

    (edges, summary)
}

/// Resolve a single import specifier for a file and classify the result.
/// Returns Some(ImportEdge) if resolved within the same project, None otherwise.
fn resolve_single_specifier(
    src: &SourceContext<'_>,
    _idx: &ResolutionIndex<'_>,
    env: &ResolveEnv<'_>,
) -> Option<ImportEdge> {
    if src.specifier.starts_with('<') {
        return None;
    }
    let resolved = resolve_module_import(src.specifier, src.file_dir, env, &src.file.lang);
    match resolved {
        Some(target) if target != src.file.path => {
            // Accept ALL resolved edges. The user chose to scan this directory —
            // everything in it is their project. Cross-sub-project imports are
            // real dependencies that the tool should show, not hide.
            Some(ImportEdge {
                from_file: src.file.path.clone(),
                to_file: target,
            })
        }
        None => None,
        _ => None,
    }
}

/// Resolve non-JS/TS imports in parallel using suffix-index and relative-path strategies.
fn resolve_tier2_imports(
    files: &[&FileNode],
    known_files: &HashSet<&str>,
    project_map: &HashMap<String, String>,
    suffix_index: &SuffixIndex<'_>,
    exts: &[&str],
    path_aliases: &HashMap<String, Vec<PathAlias>>,
) -> (Vec<ImportEdge>, ImportResolutionSummary) {
    let stats = ResolutionStats::new();
    let idx = ResolutionIndex {
        known_files,
        suffix_index,
    };
    let edges: Vec<ImportEdge> = files
        .par_iter()
        .filter(|f| !f.is_dir)
        .flat_map_iter(|file| {
            resolve_file_imports(
                file,
                &idx,
                exts,
                known_files,
                project_map,
                path_aliases,
                &stats,
            )
        })
        .collect();

    let summary = stats.summary();
    let total_specs = summary.total_specs();
    if total_specs > 0 {
        eprintln!(
            "[resolve] {} resolved, {} unresolved_internal, {} unresolved_external, {} unresolved_unknown (of {} total specs)",
            summary.resolved,
            summary.unresolved_internal,
            summary.unresolved_external,
            summary.unresolved_unknown,
            total_specs
        );
    }
    (edges, summary)
}

fn resolve_file_imports<'a>(
    file: &'a FileNode,
    idx: &ResolutionIndex<'a>,
    exts: &[&str],
    known_files: &HashSet<&'a str>,
    project_map: &HashMap<String, String>,
    path_aliases: &HashMap<String, Vec<PathAlias>>,
    stats: &ResolutionStats,
) -> Vec<ImportEdge> {
    let imports = match file.sa.as_ref().and_then(|sa| sa.imp.as_ref()) {
        Some(imports) => imports,
        None => return Vec::new(),
    };
    let profile = crate::analysis::lang_registry::profile(&file.lang);
    let env = ResolveEnv {
        suffix_index: idx.suffix_index,
        known_files,
        exts,
        directory_is_package: profile.semantics.project.directory_is_package,
    };
    let file_dir = Path::new(&file.path).parent().unwrap_or(Path::new(""));
    let src_project = project_map
        .get(&file.path)
        .map(|value| value.as_str())
        .unwrap_or("");
    let project_aliases = path_aliases
        .get(src_project)
        .map(|values| values.as_slice())
        .unwrap_or(&[]);
    let root_aliases = path_aliases
        .get("")
        .map(|values| values.as_slice())
        .unwrap_or(&[]);

    imports
        .iter()
        .filter_map(|specifier| {
            resolve_import_specifier(
                file,
                file_dir,
                specifier,
                idx,
                &env,
                project_aliases,
                root_aliases,
                stats,
            )
        })
        .collect()
}

fn resolve_import_specifier(
    file: &FileNode,
    file_dir: &Path,
    specifier: &str,
    idx: &ResolutionIndex<'_>,
    env: &ResolveEnv<'_>,
    project_aliases: &[PathAlias],
    root_aliases: &[PathAlias],
    stats: &ResolutionStats,
) -> Option<ImportEdge> {
    let resolution_kind = classify_specifier_kind(specifier, project_aliases, root_aliases);
    let alias_specs = [
        apply_path_alias(specifier, project_aliases),
        apply_path_alias(specifier, root_aliases),
    ];
    let resolved_edge = alias_specs
        .iter()
        .filter_map(|aliased| aliased.as_deref())
        .find_map(|aliased| {
            let src = SourceContext {
                specifier: aliased,
                file,
                file_dir,
            };
            resolve_single_specifier(&src, idx, env)
        })
        .or_else(|| {
            let src = SourceContext {
                specifier,
                file,
                file_dir,
            };
            resolve_single_specifier(&src, idx, env)
        });

    if resolved_edge.is_some() {
        stats
            .resolved_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    } else {
        increment_unresolved(stats, resolution_kind);
    }

    resolved_edge
}

#[derive(Clone, Copy)]
enum SpecifierKind {
    Internal,
    External,
    Unknown,
}

fn increment_unresolved(stats: &ResolutionStats, kind: SpecifierKind) {
    use std::sync::atomic::Ordering;

    match kind {
        SpecifierKind::Internal => {
            stats
                .unresolved_internal_count
                .fetch_add(1, Ordering::Relaxed);
        }
        SpecifierKind::External => {
            stats
                .unresolved_external_count
                .fetch_add(1, Ordering::Relaxed);
        }
        SpecifierKind::Unknown => {
            stats
                .unresolved_unknown_count
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn classify_specifier_kind(
    specifier: &str,
    project_aliases: &[PathAlias],
    root_aliases: &[PathAlias],
) -> SpecifierKind {
    if specifier.starts_with('.')
        || specifier.starts_with('/')
        || specifier.starts_with("crate::")
        || specifier.starts_with("self::")
        || specifier.starts_with("super::")
    {
        return SpecifierKind::Internal;
    }
    if matches_path_alias(specifier, project_aliases) || matches_path_alias(specifier, root_aliases)
    {
        return SpecifierKind::Internal;
    }
    if specifier.starts_with("node:") || (!specifier.contains('/') && !specifier.starts_with('@')) {
        return SpecifierKind::External;
    }
    SpecifierKind::Unknown
}

fn matches_path_alias(specifier: &str, aliases: &[PathAlias]) -> bool {
    aliases.iter().any(|alias| {
        specifier == alias.prefix.trim_end_matches('/')
            || specifier.starts_with(alias.prefix.as_str())
    })
}

/// Language-agnostic module resolver.
///
/// Resolution strategy (tried in order):
///   1. Relative (leading '.') -> resolve from file dir
///   2. Multi-segment absolute -> suffix-index with progressive prefix stripping
///   3. Single-segment -> dir-relative, then root-relative
///   4. Package index files -> try __init__.py, mod.rs, index.{js,ts,...} for dirs
///
/// Key design rule: single-segment absolute imports never use suffix-index.
fn resolve_module_import(
    specifier: &str,
    file_dir: &Path,
    env: &ResolveEnv<'_>,
    _lang: &str,
) -> Option<String> {
    if specifier.is_empty() {
        return None;
    }

    // 1. Relative imports (leading dots)
    if specifier.starts_with('.') {
        return resolve_relative(specifier, file_dir, env.known_files, env.exts);
    }

    // 2. Direct file path check
    {
        let cleaned = specifier.trim_start_matches("./").trim_start_matches('/');
        let joined = file_dir.join(cleaned);
        let normalized = normalize_path(&joined);
        if env.known_files.contains(normalized.as_str()) {
            return Some(normalized);
        }
        let from_root = normalize_path(Path::new(cleaned));
        if env.known_files.contains(from_root.as_str()) {
            return Some(from_root);
        }
    }

    // 3+4. Module-name resolution
    let file_dir_str = file_dir.to_str().unwrap_or("");

    if specifier.contains('/') {
        if let Some(found) = try_suffix_resolve(specifier, env, file_dir_str, file_dir) {
            return Some(found);
        }

        // Previously fell back to parent module when submodule didn't resolve,
        // creating false-positive import edges. Removed: if the exact specifier
        // doesn't resolve, return None rather than silently return the wrong file.
        // [ref:4540215f]
    }

    // Single-segment: try dir-relative first (handles `mod foo` -> foo.rs)
    if let Some(found) = try_resolve_name(specifier, file_dir, env.known_files, env.exts) {
        return Some(found);
    }
    // Then root-relative
    if let Some(found) = try_resolve_name(specifier, Path::new(""), env.known_files, env.exts) {
        return Some(found);
    }
    // Finally: manifest name aliases (crate names, package names)
    // These are high-confidence (from actual manifest files), safe for single-segment lookup.
    if let Some(candidates) = env.suffix_index.manifest_name_aliases.get(specifier) {
        if candidates.len() == 1 {
            return Some(candidates[0].to_string());
        }
    }
    None
}
