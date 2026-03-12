//! Shared types and utilities for scanner and scanner_rescan.
//!
//! Extracted to break the circular dependency between scanner.rs and scanner_rescan.rs.
//! Both modules import from here instead of from each other.

use crate::core::snapshot::Snapshot;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokei::{Config, Languages};

pub(crate) const MAX_FILES: usize = 100_000;

/// Interface for scanning a directory into a structured snapshot.
/// Enables alternative implementations for testing or cached scanning.
pub trait DirectoryScanner {
    /// Scan a directory and produce a snapshot of its structure and dependencies.
    fn scan(&self, root: &Path, max_file_size_kb: u64, max_parse_size_kb: u64) -> Result<ScanResult, crate::core::types::AppError>;
}

/// Return type that bundles the scan result.
pub struct ScanResult {
    /// The complete snapshot produced by the scan
    pub snapshot: Snapshot,
}

/// Resource limits for scanning — always passed together across scan functions.
#[derive(Debug, Clone, Copy)]
pub struct ScanLimits {
    /// Maximum file size in KB to include in scan
    pub max_file_size_kb: u64,
    /// Maximum file size in KB for structural parsing
    pub max_parse_size_kb: usize,
    /// Maximum call targets per call name before skipping
    pub max_call_targets: usize,
}

/// Detect language from file extension — delegates to lang_registry,
/// with fallback for non-parseable languages (display-only).
/// Also checks filename for extensionless files like Dockerfile.
pub(crate) fn detect_lang(path: &Path) -> String {
    // Check filename first for extensionless files (Dockerfile, Makefile, etc.)
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if let Some(lang) = crate::analysis::lang_registry::detect_lang_from_filename(name) {
            return lang.to_string();
        }
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => crate::analysis::lang_registry::detect_lang_from_ext(ext),
        None => "unknown".into(),
    }
}

/// Directories to always ignore
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    ".DS_Store",
    "target",
    ".mypy_cache",
    ".pytest_cache",
    "venv",
    ".venv",
    ".claude",
    ".cognitive",
    ".beemem",
    "site-packages",
    "lib64",
    "include",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "coverage",
    ".tox",
    ".eggs",
    ".cargo",
    ".rustup",
];

/// Extensions to ignore
const IGNORED_EXTENSIONS: &[&str] = &[
    "pyc", "pyo", "swp", "swo", "tmp", "bak", "orig", "db", "sqlite", "sqlite3", "o", "so",
    "dylib", "a", "dll", "exe", "wasm", "class", "jar", "png", "jpg", "jpeg", "gif", "ico",
    "svg", "mp3", "mp4", "wav", "webp", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "lock",
    "parquet", "csv", "tsv", "h5", "hdf5", "pkl", "pickle", "npy", "npz", "bin", "dat", "pack",
    "idx", "onnx", "pt", "pth", "safetensors", "gguf", "log", "pdf", "dmg",
];

/// Check if a directory name should be ignored during scanning.
pub(crate) fn should_ignore_dir(name: &str) -> bool {
    IGNORED_DIRS.contains(&name)
}

/// Check if a file should be ignored based on its extension.
pub(crate) fn should_ignore_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if IGNORED_EXTENSIONS.contains(&ext) {
            return true;
        }
    }
    false
}

/// Batch line counting using tokei — processes ALL files at once instead of per-file.
/// Returns map keyed by the ORIGINAL input paths (not tokei's internal form).
/// tokei may canonicalize/normalize paths internally, so we build a reverse map
/// from tokei's report.name back to the original input path. Without this,
/// scan_file's HashMap lookup misses for every file, cascading to expensive
/// canonicalize() syscalls and fs::read_to_string() fallbacks. [ref:93cf32d4]
pub(crate) fn count_lines_batch(paths: &[PathBuf]) -> HashMap<PathBuf, (u32, u32, u32, u32)> {
    if paths.is_empty() {
        return HashMap::new();
    }
    let cfg = Config::default();
    let mut langs = Languages::new();
    let path_list: Vec<PathBuf> = paths.to_vec();
    // tokei can panic on directories with no recognizable source files.
    // Catch the panic to avoid crashing the scanner thread.
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        langs.get_statistics(&path_list, &[], &cfg);
    }));
    if panicked.is_err() {
        eprintln!("[scan] tokei panicked on input paths, returning empty line counts");
        return HashMap::new();
    }

    // Build reverse map: tokei's report.name → original input path.
    // tokei may return canonicalized or normalized paths that differ from
    // what we passed in. Build lookup from canonical → original to remap.
    let mut canonical_to_original: HashMap<PathBuf, PathBuf> = HashMap::with_capacity(paths.len());
    for p in paths {
        // Store both the raw path AND the canonical form as keys pointing to the original.
        // This covers cases where tokei returns either form.
        canonical_to_original.insert(p.clone(), p.clone());
        if let Ok(cp) = p.canonicalize() {
            canonical_to_original.insert(cp, p.clone());
        }
    }

    let mut result = HashMap::with_capacity(paths.len());

    for (_lang_type, lang) in &langs {
        insert_reports(&lang.reports, &canonical_to_original, &mut result);
        insert_child_reports(&lang.children, &canonical_to_original, &mut result);
    }

    result
}

/// Extract (total, code, comments, blanks) from a tokei report and remap the key.
fn remap_report_key(
    report: &tokei::Report,
    canonical_to_original: &HashMap<PathBuf, PathBuf>,
) -> (PathBuf, (u32, u32, u32, u32)) {
    let stats = &report.stats;
    // Saturate to u32::MAX instead of silent truncation for huge files. [H6 fix]
    let total = (stats.code + stats.comments + stats.blanks).min(u32::MAX as usize) as u32;
    let code = stats.code.min(u32::MAX as usize) as u32;
    let comments = stats.comments.min(u32::MAX as usize) as u32;
    let blanks = stats.blanks.min(u32::MAX as usize) as u32;
    let key = canonical_to_original.get(&report.name)
        .cloned()
        .unwrap_or_else(|| report.name.clone());
    (key, (total, code, comments, blanks))
}

/// Insert primary language reports into the result map.
fn insert_reports(
    reports: &[tokei::Report],
    canonical_to_original: &HashMap<PathBuf, PathBuf>,
    result: &mut HashMap<PathBuf, (u32, u32, u32, u32)>,
) {
    for report in reports {
        let (key, counts) = remap_report_key(report, canonical_to_original);
        result.insert(key, counts);
    }
}

/// Insert child (embedded language) reports, skipping files already covered by parent.
fn insert_child_reports(
    children: &std::collections::BTreeMap<tokei::LanguageType, Vec<tokei::Report>>,
    canonical_to_original: &HashMap<PathBuf, PathBuf>,
    result: &mut HashMap<PathBuf, (u32, u32, u32, u32)>,
) {
    for child_reports in children.values() {
        for report in child_reports {
            let (key, counts) = remap_report_key(report, canonical_to_original);
            if !result.contains_key(&key) {
                result.insert(key, counts);
            }
        }
    }
}
