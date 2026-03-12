//! Full directory scanner — walks filesystem, counts lines, parses structure, builds graphs.
//!
//! Uses `ignore` crate for gitignore-aware walking, `tokei` for line counting,
//! and `rayon` for parallel file processing. Produces a complete `Snapshot`.
//! Reports progress via callback for UI progress bars.

pub mod common;
mod tree;
pub mod rescan;

use self::common::{
    MAX_FILES, ScanLimits, count_lines_batch, detect_lang,
    should_ignore_dir, should_ignore_file,
};
use self::tree::build_tree;
use crate::core::types::AppError;
use crate::core::snapshot::{ScanProgress, Snapshot};
use crate::core::types::FileNode;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// Collected file info from the filesystem walk phase.
/// Captures path and mtime to avoid redundant metadata calls.
struct CollectedFile {
    path: PathBuf,
    mtime: f64,
}

/// Extract mtime as f64 seconds since UNIX_EPOCH from file metadata.
pub(crate) fn extract_mtime(meta: &fs::Metadata, path: &Path) -> f64 {
    meta.modified()
        .map(|t| {
            t.duration_since(UNIX_EPOCH)
                .unwrap_or_else(|e| {
                    eprintln!("[scanner] mtime before epoch for {:?}: {}", path, e);
                    std::time::Duration::ZERO
                })
                .as_secs_f64()
        })
        .unwrap_or(0.0) // Filesystem doesn't support mtime (some network mounts)
}

/// Process a single walker entry: check filters, extract metadata, send to channel.
/// Returns Quit if the file limit was reached or the channel is disconnected.
fn process_walk_entry(
    entry: &ignore::DirEntry,
    file_size_limit: u64,
    count: &std::sync::atomic::AtomicUsize,
    tx: &crossbeam_channel::Sender<CollectedFile>,
) -> ignore::WalkState {
    use std::sync::atomic::Ordering;

    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
        return ignore::WalkState::Continue;
    }
    let path = entry.path().to_path_buf();
    if should_ignore_file(&path) {
        return ignore::WalkState::Continue;
    }
    let meta = match fs::metadata(&path) {
        Ok(m) if m.len() <= file_size_limit => m,
        _ => return ignore::WalkState::Continue,
    };
    let prev = count.fetch_add(1, Ordering::AcqRel);
    if prev >= MAX_FILES {
        return ignore::WalkState::Quit;
    }
    let mtime = extract_mtime(&meta, &path);
    if tx.send(CollectedFile { path, mtime }).is_err() {
        return ignore::WalkState::Quit;
    }
    ignore::WalkState::Continue
}

/// Collect file paths using `git ls-files` for git repos (the universal, correct source
/// of "what files belong to this project"), falling back to filesystem walk for non-git dirs.
///
/// First-principles reasoning: the user's git index is the single source of truth for
/// what constitutes "their code." It handles .gitignore, monorepos, workspaces, and
/// any project structure without heuristics or hardcoded ignore lists.
fn collect_paths(root: &Path, file_size_limit: u64) -> Vec<CollectedFile> {
    // Try git ls-files first — the universal correct approach
    if let Some(files) = collect_paths_git(root, file_size_limit) {
        if !files.is_empty() {
            eprintln!("[scan] using git ls-files ({} tracked files)", files.len());
            return files;
        }
    }
    // Fallback: filesystem walk for non-git directories
    eprintln!("[scan] not a git repo, falling back to filesystem walk");
    collect_paths_walk(root, file_size_limit)
}

/// Collect files via `git ls-files` — returns None if not a git repo or git fails.
/// This is the primary path: git already knows every tracked file, respects .gitignore,
/// handles monorepos/workspaces, and requires zero heuristic filtering.
fn collect_paths_git(root: &Path, file_size_limit: u64) -> Option<Vec<CollectedFile>> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z"])  // null-delimited for safe path handling
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None; // not a git repo or git not available
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<CollectedFile> = stdout
        .split('\0')
        .filter(|s| !s.is_empty())
        .take(MAX_FILES)
        .filter_map(|rel| {
            let abs = root.join(rel);
            if should_ignore_file(&abs) {
                return None;
            }
            let meta = fs::metadata(&abs).ok()?;
            if !meta.is_file() || meta.len() > file_size_limit {
                return None;
            }
            let mtime = extract_mtime(&meta, &abs);
            Some(CollectedFile { path: abs, mtime })
        })
        .collect();

    Some(files)
}

/// Fallback: filesystem walk for non-git directories.
/// Uses `ignore` crate with hardcoded ignore list (only for non-git repos).
fn collect_paths_walk(root: &Path, file_size_limit: u64) -> Vec<CollectedFile> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let count = Arc::new(AtomicUsize::new(0));
    // MUST be unbounded: run() blocks until all walker threads finish, and
    // rx.iter() only runs after run() returns. A bounded channel deadlocks
    // when walker threads fill it and block on send() — nobody is reading.
    let (tx, rx) = crossbeam_channel::unbounded::<CollectedFile>();

    let count_w = Arc::clone(&count);
    WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(20))
        .threads(rayon::current_num_threads().min(8))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                return !should_ignore_dir(&name);
            }
            true
        })
        .build_parallel()
        .run(|| {
            let tx = tx.clone();
            let count = Arc::clone(&count_w);
            Box::new(move |result| {
                if count.load(Ordering::Acquire) >= MAX_FILES {
                    return ignore::WalkState::Quit;
                }
                if let Ok(entry) = result {
                    return process_walk_entry(&entry, file_size_limit, &count, &tx);
                }
                ignore::WalkState::Continue
            })
        });
    drop(tx); // close sender so rx.iter() terminates

    rx.iter().collect()
}


/// Scan a single file into a FileNode (no tokei call — line counts provided externally)
fn scan_file(
    collected: &CollectedFile,
    root: &Path,
    line_counts: &HashMap<PathBuf, (u32, u32, u32, u32)>,
) -> FileNode {
    let rel = collected.path.strip_prefix(root).unwrap_or(&collected.path);
    let (lines, logic, comments, blanks) = line_counts
        .get(&collected.path)
        .or_else(|| {
            // Try canonicalized path as fallback key (symlinks, case-insensitive FS).
            // Log errors at trace level — canonicalize can fail benignly. [ref:4540215f]
            match collected.path.canonicalize() {
                Ok(cp) => line_counts.get(&cp),
                Err(_) => None,
            }
        })
        .copied()
        .unwrap_or_else(|| {
            // Fallback: raw line count for files tokei didn't recognize.
            // Set logic=0 to avoid inflating code metrics — we only know total lines.
            if let Ok(content) = fs::read_to_string(&collected.path) {
                let total = content.lines().count() as u32;
                (total, 0, 0, 0)
            } else {
                (0, 0, 0, 0)
            }
        });

    FileNode {
        path: rel.to_string_lossy().to_string(),
        name: collected
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        is_dir: false,
        lines,
        logic,
        comments,
        blanks,
        funcs: 0,
        mtime: collected.mtime,
        gs: String::new(),
        lang: detect_lang(&collected.path),
        sa: None,
        children: None,
    }
}


/// Collect files from disk, count lines, and produce FileNode records.
/// Returns (files, total_files_count).
fn walk_and_scan_files(
    root: &Path,
    max_file_size: u64,
    scan_t0: std::time::Instant,
    emit: &dyn Fn(&str, u8),
) -> Vec<FileNode> {
    emit("Collecting files\u{2026}", 5);
    let collected = collect_paths(root, max_file_size * 1024);
    let total_files = collected.len() as u32;
    eprintln!("[scan] collect_paths: {:.1}ms ({} files)", scan_t0.elapsed().as_secs_f64() * 1000.0, total_files);

    emit(&format!("Counting lines ({total_files} files)"), 15);
    let all_paths: Vec<PathBuf> = collected.iter().map(|c| c.path.clone()).collect();
    let tokei_t0 = std::time::Instant::now();
    let line_counts = count_lines_batch(&all_paths);
    eprintln!("[scan] count_lines: {:.1}ms (tokei: {:.1}ms, {} entries returned)", scan_t0.elapsed().as_secs_f64() * 1000.0, tokei_t0.elapsed().as_secs_f64() * 1000.0, line_counts.len());

    {
        let hits = collected.iter().filter(|c| line_counts.contains_key(&c.path)).count();
        let misses = collected.len() - hits;
        eprintln!("[scan] tokei key hit rate: {}/{} ({} misses)", hits, collected.len(), misses);
    }

    emit(&format!("Scanning files ({total_files} files)"), 30);
    let files: Vec<FileNode> = collected
        .par_iter()
        .map(|c| scan_file(c, root, &line_counts))
        .collect();
    eprintln!("[scan] scan_files: {:.1}ms", scan_t0.elapsed().as_secs_f64() * 1000.0);
    files
}

/// Apply git statuses to file nodes in-place.
fn apply_git_statuses(files: &mut [FileNode], root_path: &str, scan_t0: std::time::Instant, emit: &dyn Fn(&str, u8)) {
    let total_files = files.len();
    emit(&format!("Git status ({total_files} files)"), 40);
    let git_statuses = crate::analysis::git::get_statuses(root_path);
    for file in files.iter_mut() {
        if let Some(gs) = git_statuses.get(&file.path) {
            file.gs = gs.clone();
        }
    }
    eprintln!("[scan] git_status: {:.1}ms", scan_t0.elapsed().as_secs_f64() * 1000.0);
}

/// Poll parse progress until completion, emitting progress updates.
/// Accepts the parse thread handle to detect panics — if the thread dies
/// before all work is done, we break instead of spinning forever. [C2 fix]
fn poll_parse_progress(
    progress: &std::sync::Arc<crate::analysis::parser::ParseProgress>,
    parse_handle: &std::thread::JoinHandle<Vec<(String, crate::core::types::StructuralAnalysis)>>,
    emit: &dyn Fn(&str, u8),
) {
    let real_total = progress.total;
    if real_total == 0 {
        emit("Parsing structure (0/0)", 80);
        return;
    }
    loop {
        let done = progress.done.load(std::sync::atomic::Ordering::Relaxed);
        let pct = 50 + (done * 30 / real_total);
        emit(&format!("Parsing structure ({done}/{real_total})"), pct as u8);
        if done >= real_total { break; }
        // Bail out if the parse thread has finished (panicked or completed early)
        // to avoid spinning forever when done < real_total due to a panic.
        if parse_handle.is_finished() { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Parse structural analysis for all parseable files, polling progress.
fn parse_structure(
    files: &mut [FileNode],
    root: &Path,
    max_parse_size: usize,
    scan_t0: std::time::Instant,
    emit: &dyn Fn(&str, u8),
) {
    let parseable_count = files.iter().filter(|f| !f.lang.is_empty() && f.lang != "unknown").count();
    emit(&format!("Parsing structure ({parseable_count} files)"), 50);
    let parse_inputs: Vec<(String, String, String)> = files
        .iter()
        .filter(|f| !f.lang.is_empty() && f.lang != "unknown")
        .map(|f| {
            let abs = root.join(&f.path).to_string_lossy().to_string();
            (abs, f.path.clone(), f.lang.clone())
        })
        .collect();

    let progress = std::sync::Arc::new(crate::analysis::parser::ParseProgress {
        done: std::sync::atomic::AtomicUsize::new(0),
        total: parse_inputs.len(),
    });
    let prog_clone = progress.clone();
    let parse_handle = std::thread::spawn({
        move || {
            crate::analysis::parser::parse_files_batch_with_progress(&parse_inputs, max_parse_size, Some(&prog_clone))
        }
    });

    poll_parse_progress(&progress, &parse_handle, emit);

    let sa_results = parse_handle.join().expect("parse thread panicked");
    eprintln!("[scan] parse_files: {:.1}ms ({} parseable)", scan_t0.elapsed().as_secs_f64() * 1000.0, parseable_count);
    let sa_map: HashMap<String, crate::core::types::StructuralAnalysis> =
        sa_results.into_iter().collect();
    for file in files.iter_mut() {
        if let Some(sa) = sa_map.get(&file.path) {
            file.funcs = sa.functions.as_ref().map_or(0, |v| v.len() as u32);
            file.sa = Some(sa.clone());
        }
    }
}

/// Context for the tree-building and graph-building phase of a scan.
struct BuildContext<'a> {
    root: &'a Path,
    max_call_targets: usize,
    scan_t0: std::time::Instant,
    emit: &'a dyn Fn(&str, u8),
    on_tree_ready: Option<&'a dyn Fn(Snapshot)>,
}

/// Build the file tree and emit a tree-ready snapshot, then build graphs.
fn build_tree_and_graphs(
    files: Vec<FileNode>,
    bctx: &BuildContext<'_>,
) -> ScanResult {
    // Use u64 to prevent overflow when summing line counts across many files. [ref:4e8f1175]
    let total_lines: u32 = files.iter().map(|f| f.lines as u64).sum::<u64>().min(u32::MAX as u64) as u32;
    let total_files = files.len() as u32;
    let root_name = bctx.root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    (bctx.emit)(&format!("Building tree ({total_files} files)"), 65);
    let (tree, total_dirs) = build_tree(files, &root_name);
    let tree = Arc::new(tree);

    if let Some(cb) = bctx.on_tree_ready {
        cb(Snapshot {
            root: Arc::clone(&tree),
            total_files, total_lines, total_dirs,
            call_graph: Vec::new(),
            import_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        });
    }

    eprintln!("[scan] tree_ready sent at: {:.1}ms", bctx.scan_t0.elapsed().as_secs_f64() * 1000.0);
    (bctx.emit)(&format!("Building graphs ({total_files} files, {total_dirs} dirs)"), 85);
    let flat_files = crate::core::snapshot::flatten_files_ref(&tree);
    let gr = crate::analysis::graph::build_graphs(&flat_files, Some(bctx.root), bctx.max_call_targets);

    eprintln!("[scan] build_graphs done at: {:.1}ms | {} import, {} call, {} inherit edges",
        bctx.scan_t0.elapsed().as_secs_f64() * 1000.0, gr.import_edges.len(), gr.call_edges.len(), gr.inherit_edges.len());
    (bctx.emit)("Done", 100);

    ScanResult {
        snapshot: Snapshot {
            root: tree, total_files, total_lines, total_dirs,
            call_graph: gr.call_edges,
            import_graph: gr.import_edges,
            inherit_graph: gr.inherit_edges,
            entry_points: gr.entry_points,
            exec_depth: gr.exec_depth,
        },
    }
}

/// Main scan function: walk directory, count lines in batch, build tree.
/// Accepts an optional progress callback to report scan phases.
/// Accepts an optional `on_tree_ready` callback that fires after tree building
/// but BEFORE graph building — allows the caller to emit a partial snapshot
/// so the frontend can render rectangles ~1-2s earlier. [ref:7f9a39c8]
pub fn scan_directory(
    root_path: &str,
    on_progress: Option<&dyn Fn(ScanProgress)>,
    on_tree_ready: Option<&dyn Fn(Snapshot)>,
    limits: &ScanLimits,
) -> Result<ScanResult, AppError> {
    let scan_t0 = std::time::Instant::now();
    let root = Path::new(root_path);
    if !root.exists() || !root.is_dir() {
        return Err(AppError::Path(format!("Not a valid directory: {}", root_path)));
    }

    let emit = |step: &str, pct: u8| {
        if let Some(cb) = on_progress {
            cb(ScanProgress { step: step.into(), pct });
        }
    };

    let mut files = walk_and_scan_files(root, limits.max_file_size_kb, scan_t0, &emit);
    apply_git_statuses(&mut files, root_path, scan_t0, &emit);
    parse_structure(&mut files, root, limits.max_parse_size_kb, scan_t0, &emit);
    let bctx = BuildContext {
        root, max_call_targets: limits.max_call_targets, scan_t0, emit: &emit, on_tree_ready,
    };
    Ok(build_tree_and_graphs(files, &bctx))
}

/// Re-export for backward compatibility.
pub use self::common::ScanResult;
