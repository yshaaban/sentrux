//! Code health metrics (Constantine & Yourdon 1979, McCabe 1976, Martin).
//!
//! Top-level module that orchestrates all metric computations: structural
//! coupling, cyclic dependencies, god-file detection, cyclomatic complexity,
//! and overall quality signal scoring. Sub-modules provide architecture
//! analysis, DSM construction, evolutionary metrics, rule enforcement,
//! test-gap analysis, and what-if scenario simulation.
//! Key function: `compute_health` produces a `HealthReport` from a `Snapshot`.

// ── Sub-modules (directory modules with internal cohesion) ──
pub mod arch; // arch/mod.rs + graph.rs + distance.rs
pub mod evo; // evo/mod.rs + git_walker.rs
pub mod rules; // rules/mod.rs + checks.rs
pub mod v2;

// ── Flat modules (remain at metrics root) ──
pub mod cross_validation; // FREE: compression-based quality cross-check
pub mod dsm;
mod file_analysis;
mod graph_analysis;
pub mod root_causes;
pub mod stability;
pub mod testgap;
pub mod types;
pub mod whatif;

pub use types::*;

// ── Re-exports for backward compatibility ──
// External code (app/mcp_handlers_evo.rs) imports crate::metrics::evolution.
// After restructure, evolution lives in crate::metrics::evo.
pub use evo as evolution;

#[cfg(test)]
mod mod_tests;
#[cfg(test)]
mod mod_tests2;
#[cfg(test)]
pub(crate) mod test_helpers;

use crate::core::snapshot::Snapshot;
use crate::core::types::ImportEdge;
use crate::metrics::types::is_mod_declaration_edge;
use file_analysis::{
    collect_all_file_lines, collect_all_function_ccs, collect_all_function_lines,
    compute_file_metrics, count_total_funcs,
};
use graph_analysis::compute_module_metrics;

/// Check if a file is a package-index / barrel file via its language profile.
/// Now reads from plugin.toml [semantics] package_index_files per language,
/// instead of a single hardcoded list.
pub(crate) fn is_package_index_for_path(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    let lang = crate::analysis::lang_registry::detect_lang_from_ext(ext);
    crate::analysis::lang_registry::profile(&lang).is_package_index_file(path)
}

/// Compute a comprehensive code health report from a scan snapshot.
/// Evaluates coupling, complexity, dead code, duplication, and more.
/// Quality signal is derived from root causes (modularity, cycles, depth,
/// complexity equality, redundancy).
pub fn compute_health(snapshot: &Snapshot) -> HealthReport {
    let files = crate::core::snapshot::flatten_files_ref(&snapshot.root);
    let dependency_edges: Vec<ImportEdge> = snapshot
        .import_graph
        .iter()
        .filter(|edge| !is_mod_declaration_edge(edge))
        .cloned()
        .collect();

    let file_metrics = compute_file_metrics(
        &files,
        &dependency_edges,
        &snapshot.call_graph,
        &snapshot.entry_points,
    );
    let module_metrics = compute_module_metrics(
        &files,
        &dependency_edges,
        &snapshot.call_graph,
        &snapshot.entry_points,
    );

    let all_function_ccs = collect_all_function_ccs(&files);
    let all_function_lines = collect_all_function_lines(&files);
    let all_file_lines = collect_all_file_lines(&files);

    let modularity_q =
        root_causes::compute_modularity_q(&dependency_edges, &snapshot.call_graph, &files);
    let complexity_gini = root_causes::compute_complexity_gini(&files);
    let duplicate_function_count: usize = file_metrics
        .duplicate_groups
        .iter()
        .map(|group| group.instances.len())
        .sum();
    let total_functions = count_total_funcs(&files);
    let redundancy_ratio = root_causes::compute_redundancy_ratio(
        file_metrics.dead_functions.len(),
        duplicate_function_count,
        total_functions,
    );

    let root_cause_raw = root_causes::RootCauseRaw {
        modularity_q,
        cycle_count: module_metrics.circular_dep_count,
        max_depth: module_metrics.max_depth,
        complexity_gini,
        redundancy_ratio,
    };
    let (root_cause_scores, quality_signal) =
        root_causes::compute_root_cause_scores(&root_cause_raw);

    HealthReport {
        coupling_score: module_metrics.coupling_score,
        circular_dep_count: module_metrics.circular_dep_count,
        circular_dep_files: module_metrics.circular_dep_files,
        total_import_edges: dependency_edges.len(),
        cross_module_edges: module_metrics.cross_module_edges,
        entropy: module_metrics.entropy,
        entropy_bits: module_metrics.entropy_bits,
        avg_cohesion: module_metrics.avg_cohesion,
        max_depth: module_metrics.max_depth,
        god_files: file_metrics.god_files,
        hotspot_files: file_metrics.hotspot_files,
        most_unstable: file_metrics.most_unstable,
        complex_functions: file_metrics.complex_functions,
        long_functions: file_metrics.long_functions,
        cog_complex_functions: file_metrics.cog_complex_functions,
        high_param_functions: file_metrics.high_param_functions,
        duplicate_groups: file_metrics.duplicate_groups,
        dead_functions: file_metrics.dead_functions,
        long_files: file_metrics.long_files,
        all_function_ccs,
        all_function_lines,
        all_file_lines,
        god_file_ratio: file_metrics.god_ratio,
        hotspot_ratio: file_metrics.hotspot_ratio,
        complex_fn_ratio: file_metrics.complex_fn_ratio,
        long_fn_ratio: file_metrics.long_fn_ratio,
        comment_ratio: file_metrics.comment_ratio,
        large_file_count: file_metrics.large_file_count,
        large_file_ratio: file_metrics.large_file_ratio,
        duplication_ratio: file_metrics.duplication_ratio,
        dead_code_ratio: file_metrics.dead_code_ratio,
        high_param_ratio: file_metrics.high_param_ratio,
        cog_complex_ratio: file_metrics.cog_complex_ratio,
        quality_signal,
        root_cause_raw,
        root_cause_scores,
    }
}

// ── Pro metrics extension point ──

/// Trait for injecting additional metrics from Pro crate.
/// Pro implements this to add Type Coupling, LCOM, etc.
pub trait MetricsExtension: Send + Sync {
    /// Compute additional metrics and return as JSON value.
    /// Called after the standard health report is computed.
    fn compute(&self, snapshot: &crate::core::snapshot::Snapshot) -> serde_json::Value;

    /// Name of the metric (for display in health panel).
    fn name(&self) -> &str;
}

/// Global registry of Pro metrics extensions.
static METRICS_EXTENSIONS: std::sync::OnceLock<Vec<Box<dyn MetricsExtension>>> =
    std::sync::OnceLock::new();

/// Register Pro metrics extensions (called by an optional integration at startup).
pub fn register_extensions(extensions: Vec<Box<dyn MetricsExtension>>) {
    let _ = METRICS_EXTENSIONS.set(extensions);
}

/// Get registered extensions (returns empty slice if no Pro).
pub fn extensions() -> &'static [Box<dyn MetricsExtension>] {
    METRICS_EXTENSIONS
        .get()
        .map(|extensions| extensions.as_slice())
        .unwrap_or(&[])
}
