//! Shared types and utilities for scanner and scanner_rescan.
//!
//! Extracted to break the circular dependency between scanner.rs and scanner_rescan.rs.
//! Both modules import from here instead of from each other.

use crate::analysis::resolver::suffix::ImportResolutionSummary;
use crate::core::snapshot::Snapshot;
use std::path::Path;

pub(crate) const MAX_FILES: usize = 100_000;

/// Normalize a path to forward slashes. Called at every entry point where
/// OS paths become string keys. All downstream code uses `/` only.
#[inline]
pub(crate) fn normalize_path(path: std::borrow::Cow<'_, str>) -> String {
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    }
}

/// Interface for scanning a directory into a structured snapshot.
/// Enables alternative implementations for testing or cached scanning.
pub trait DirectoryScanner {
    /// Scan a directory and produce a snapshot of its structure and dependencies.
    fn scan(
        &self,
        root: &Path,
        max_file_size_kb: u64,
        max_parse_size_kb: u64,
    ) -> Result<ScanResult, crate::core::types::AppError>;
}

/// Return type that bundles the scan result.
pub struct ScanResult {
    /// The complete snapshot produced by the scan
    pub snapshot: Snapshot,
    /// Scan-scope and resolution metadata used for confidence reporting
    pub metadata: ScanMetadata,
}

/// How files were collected for this scan.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ScanMode {
    Git,
    Walk,
}

impl ScanMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Walk => "walk",
        }
    }
}

/// High-noise path categories that are typically excluded from scans.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ExclusionBucket {
    Vendor,
    Generated,
    Build,
    Fixture,
    Cache,
}

impl ExclusionBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vendor => "vendor",
            Self::Generated => "generated",
            Self::Build => "build",
            Self::Fixture => "fixture",
            Self::Cache => "cache",
        }
    }
}

/// Counts for excluded files by bucket.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExclusionBucketCounts {
    pub vendor: usize,
    pub generated: usize,
    pub build: usize,
    pub fixture: usize,
    pub cache: usize,
}

impl ExclusionBucketCounts {
    pub fn increment(&mut self, bucket: ExclusionBucket) {
        match bucket {
            ExclusionBucket::Vendor => self.vendor += 1,
            ExclusionBucket::Generated => self.generated += 1,
            ExclusionBucket::Build => self.build += 1,
            ExclusionBucket::Fixture => self.fixture += 1,
            ExclusionBucket::Cache => self.cache += 1,
        }
    }
}

/// Counts for excluded files by reason.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExclusionSummary {
    pub bucketed: ExclusionBucketCounts,
    pub ignored_extension: usize,
    pub too_large: usize,
    pub metadata_error: usize,
}

impl ExclusionSummary {
    pub fn total(&self) -> usize {
        self.bucketed.vendor
            + self.bucketed.generated
            + self.bucketed.build
            + self.bucketed.fixture
            + self.bucketed.cache
            + self.ignored_extension
            + self.too_large
            + self.metadata_error
    }
}

/// Scan-scope and resolution metadata used for confidence reporting.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanMetadata {
    pub mode: ScanMode,
    pub candidate_files: usize,
    pub tracked_candidates: usize,
    pub untracked_candidates: usize,
    pub kept_files: usize,
    pub exclusions: ExclusionSummary,
    pub resolution: ImportResolutionSummary,
    pub fallback_reason: Option<String>,
    pub partial: bool,
    pub truncated: bool,
}

impl ScanMetadata {
    pub fn empty(mode: ScanMode) -> Self {
        Self {
            mode,
            candidate_files: 0,
            tracked_candidates: 0,
            untracked_candidates: 0,
            kept_files: 0,
            exclusions: ExclusionSummary::default(),
            resolution: ImportResolutionSummary::default(),
            fallback_reason: None,
            partial: false,
            truncated: false,
        }
    }
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

/// Global ignored dirs (OS/tool artifacts, not language-specific).
/// Language-specific ignored dirs come from plugin.toml [semantics.project].
const GLOBAL_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".DS_Store",
    ".claude",
    ".cognitive",
    ".beemem",
    "lib64",
    "include",
];

/// Merged ignored dirs: global + all plugins. Cached at first access.
static ALL_IGNORED_DIRS: std::sync::LazyLock<std::collections::HashSet<String>> =
    std::sync::LazyLock::new(|| {
        let mut set: std::collections::HashSet<String> =
            GLOBAL_IGNORED_DIRS.iter().map(|s| s.to_string()).collect();
        for dir in crate::analysis::lang_registry::all_ignored_dirs() {
            set.insert(dir.to_string());
        }
        set
    });

/// Extensions to ignore
const IGNORED_EXTENSIONS: &[&str] = &[
    "pyc",
    "pyo",
    "swp",
    "swo",
    "tmp",
    "bak",
    "orig",
    "db",
    "sqlite",
    "sqlite3",
    "o",
    "so",
    "dylib",
    "a",
    "dll",
    "exe",
    "wasm",
    "class",
    "jar",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "ico",
    "svg",
    "mp3",
    "mp4",
    "wav",
    "webp",
    "zip",
    "tar",
    "gz",
    "bz2",
    "xz",
    "7z",
    "rar",
    "lock",
    "parquet",
    "csv",
    "tsv",
    "h5",
    "hdf5",
    "pkl",
    "pickle",
    "npy",
    "npz",
    "bin",
    "dat",
    "pack",
    "idx",
    "onnx",
    "pt",
    "pth",
    "safetensors",
    "gguf",
    "log",
    "pdf",
    "dmg",
];

/// Check if a directory name should be ignored during scanning.
/// Checks both global ignored dirs and per-language ignored dirs from plugins.
pub(crate) fn should_ignore_dir(name: &str) -> bool {
    ALL_IGNORED_DIRS.contains(name)
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

/// Classify a high-noise path bucket from a relative or absolute path.
pub(crate) fn classify_exclusion_bucket(path: &Path) -> Option<ExclusionBucket> {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => {
                Some(value.to_string_lossy().to_ascii_lowercase())
            }
            _ => None,
        })
        .collect();

    for component in &components {
        match component.as_str() {
            "vendor" | "vendors" | "third_party" | "third-party" | "external" => {
                return Some(ExclusionBucket::Vendor);
            }
            "__generated__" | "generated" => {
                return Some(ExclusionBucket::Generated);
            }
            "dist" | "build" | "out" | "coverage" | ".next" | ".nuxt" => {
                return Some(ExclusionBucket::Build);
            }
            "fixture" | "fixtures" | "__fixtures__" | "testdata" | "__snapshots__"
            | "snapshots" => {
                return Some(ExclusionBucket::Fixture);
            }
            ".cache" | ".turbo" | ".parcel-cache" | ".yarn" | ".pnpm-store" | ".sentrux" => {
                return Some(ExclusionBucket::Cache);
            }
            _ => {}
        }
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if filename.contains(".generated.") || filename.ends_with(".generated") {
        return Some(ExclusionBucket::Generated);
    }

    None
}

/// Line counts computed from raw file content (no external dependency).
pub(crate) struct LineCounts {
    pub total: u32,
    pub blanks: u32,
}

/// Count total lines and blank lines from raw bytes.
/// Single pass, zero allocation, O(N) in file size.
/// Replaces the entire tokei dependency.
pub(crate) fn count_lines_from_bytes(content: &[u8]) -> LineCounts {
    if content.is_empty() {
        return LineCounts {
            total: 0,
            blanks: 0,
        };
    }
    let mut total: u32 = 0;
    let mut blanks: u32 = 0;
    let mut line_has_non_ws = false;

    for &b in content {
        if b == b'\n' {
            total += 1;
            if !line_has_non_ws {
                blanks += 1;
            }
            line_has_non_ws = false;
        } else if b != b' ' && b != b'\t' && b != b'\r' {
            line_has_non_ws = true;
        }
    }
    // Handle last line without trailing newline
    if content.last() != Some(&b'\n') {
        total += 1;
        if !line_has_non_ws {
            blanks += 1;
        }
    }
    LineCounts { total, blanks }
}
