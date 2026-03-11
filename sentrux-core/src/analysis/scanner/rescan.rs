//! Incremental rescan: patches an existing snapshot with changes to specific files.
//!
//! Extracted from scanner.rs — handles file change detection, re-parsing, and
//! tree/graph rebuilding for changed files only.

use super::common::{
    ScanLimits, ScanResult, count_lines_batch, detect_lang,
    should_ignore_dir, should_ignore_file, MAX_FILES,
};
use super::tree::build_tree;
use crate::core::types::AppError;
use crate::core::snapshot::Snapshot;
use crate::core::types::FileNode;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// Incremental rescan: patch an existing snapshot with changes to specific files.
/// Re-parses only changed files, rebuilds tree + graphs.
/// Accepts `on_tree_ready` to emit partial snapshot before graph rebuild. [ref:7f9a39c8]
pub fn rescan_changed(
    root_path: &str,
    old_snap: &Snapshot,
    changed_rel_paths: &[String],
    on_tree_ready: Option<&dyn Fn(Snapshot)>,
    limits: &ScanLimits,
) -> Result<ScanResult, AppError> {
    let root = Path::new(root_path);
    let max_file_size_bytes = limits.max_file_size_kb * 1024;
    let max_parse_size = limits.max_parse_size_kb;
    let max_call_targets = limits.max_call_targets;

    // Flatten old snapshot into a mutable file list (clone cost ~ file count, not content)
    let mut files: Vec<FileNode> = crate::core::snapshot::flatten_files(&old_snap.root);

    // Expand directories and classify into reparse vs deleted
    let expanded = expand_directory_events(root, changed_rel_paths, max_file_size_bytes);
    let (to_reparse, deleted) = classify_changed_paths(root, &expanded, max_file_size_bytes);

    // Remove deleted files
    files.retain(|f| !deleted.contains(&f.path));

    // Batch line counts + structural analysis + git statuses
    let line_counts = batch_line_counts(&to_reparse);
    let sa_map = batch_parse_files(&to_reparse, max_parse_size);
    let git_statuses = crate::analysis::git::get_statuses(root_path);

    // Update or insert changed files into the file list
    upsert_changed_files(&mut files, &to_reparse, &line_counts, &sa_map, &git_statuses);

    // Enforce MAX_FILES limit (same as initial scan) [ref:93cf32d4]
    enforce_max_files(&mut files);

    // Build tree, emit partial snapshot, build graphs, return final result
    build_snapshot_with_graphs(root, files, on_tree_ready, max_call_targets)
}

/// Walk directories in `changed_rel_paths` to discover new files inside.
/// Non-directory paths are passed through. Applies same ignore/size filters
/// as collect_paths. [ref:93cf32d4]
fn expand_directory_events(
    root: &Path,
    changed_rel_paths: &[String],
    max_file_size_bytes: u64,
) -> Vec<String> {
    let mut expanded: Vec<String> = Vec::new();
    for rel in changed_rel_paths {
        let abs = root.join(rel);
        if abs.exists() && abs.is_dir() {
            expand_single_dir(root, &abs, max_file_size_bytes, &mut expanded);
        } else {
            expanded.push(rel.clone());
        }
        if expanded.len() >= MAX_FILES {
            break;
        }
    }
    expanded
}

/// Check if a walked entry is a valid file for expansion (not ignored, within size limit).
/// Returns Some(rel_path) if valid, None if the entry should be skipped.
fn validate_walk_entry(
    entry: &ignore::DirEntry,
    root: &Path,
    max_file_size_bytes: u64,
) -> Option<String> {
    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
        return None;
    }
    let path = entry.path().to_path_buf();
    if should_ignore_file(&path) {
        return None;
    }
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > max_file_size_bytes {
            return None;
        }
    }
    path.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().to_string())
}

/// Walk a single directory and append discovered file rel-paths to `out`.
/// Same filters as collect_paths: ignore dirs, ignore files, size limit. [ref:93cf32d4]
fn expand_single_dir(
    root: &Path,
    dir_abs: &Path,
    max_file_size_bytes: u64,
    out: &mut Vec<String>,
) {
    for entry in ignore::WalkBuilder::new(dir_abs)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(20))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                return !should_ignore_dir(&name);
            }
            true
        })
        .build()
    {
        if out.len() >= MAX_FILES {
            eprintln!("[rescan] expanded_paths hit MAX_FILES limit ({}), truncating", MAX_FILES);
            break;
        }
        if let Ok(e) = entry {
            if let Some(rel_path) = validate_walk_entry(&e, root, max_file_size_bytes) {
                out.push(rel_path);
            }
        }
    }
}

/// Separate expanded paths into files to reparse (exist on disk) and deleted files.
/// Applies same ignore/size filters as collect_paths for direct file paths. [ref:6c60c4ee]
fn classify_changed_paths(
    root: &Path,
    expanded: &[String],
    max_file_size_bytes: u64,
) -> (Vec<(String, PathBuf)>, HashSet<String>) {
    let mut to_reparse: Vec<(String, PathBuf)> = Vec::new();
    let mut deleted: HashSet<String> = HashSet::new();
    for rel in expanded {
        let abs = root.join(rel);
        if abs.exists() && abs.is_file() {
            if should_ignore_file(&abs) {
                continue;
            }
            if let Ok(meta) = fs::metadata(&abs) {
                if meta.len() > max_file_size_bytes {
                    continue;
                }
            }
            to_reparse.push((rel.clone(), abs));
        } else if !abs.exists() {
            deleted.insert(rel.clone());
        }
    }
    (to_reparse, deleted)
}

/// Batch tokei line counting for all reparse targets.
fn batch_line_counts(to_reparse: &[(String, PathBuf)]) -> HashMap<PathBuf, (u32, u32, u32, u32)> {
    let abs_paths: Vec<PathBuf> = to_reparse.iter().map(|(_, abs)| abs.clone()).collect();
    if abs_paths.is_empty() {
        HashMap::new()
    } else {
        count_lines_batch(&abs_paths)
    }
}

/// Batch structural analysis parsing in parallel for all reparse targets.
fn batch_parse_files(
    to_reparse: &[(String, PathBuf)],
    max_parse_size: usize,
) -> HashMap<String, crate::core::types::StructuralAnalysis> {
    let parse_inputs: Vec<(String, String, String)> = to_reparse
        .iter()
        .map(|(rel, abs)| {
            let lang = detect_lang(abs);
            (abs.to_string_lossy().to_string(), rel.clone(), lang)
        })
        .collect();
    crate::analysis::parser::parse_files_batch(&parse_inputs, max_parse_size)
        .into_iter()
        .collect()
}

/// Look up line counts for a file, with canonicalized-path fallback and read fallback.
fn lookup_line_counts(
    abs: &Path,
    line_counts: &HashMap<PathBuf, (u32, u32, u32, u32)>,
) -> (u32, u32, u32, u32) {
    line_counts
        .get(abs)
        .or_else(|| match abs.canonicalize() {
            Ok(cp) => line_counts.get(&cp),
            Err(_) => None,
        })
        .copied()
        .unwrap_or_else(|| {
            if let Ok(content) = fs::read_to_string(abs) {
                let total = content.lines().count() as u32;
                (total, 0, 0, 0)
            } else {
                (0, 0, 0, 0)
            }
        })
}

/// Build a FileNode from a changed file's metadata, line counts, and structural analysis.
fn build_file_node(
    rel: &str,
    abs: &Path,
    line_counts: &HashMap<PathBuf, (u32, u32, u32, u32)>,
    sa_map: &HashMap<String, crate::core::types::StructuralAnalysis>,
    git_statuses: &HashMap<String, String>,
) -> FileNode {
    let mtime = match fs::metadata(abs).and_then(|m| m.modified()) {
        Ok(t) => t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64(),
        Err(_) => 0.0,
    };
    let (lines, logic, comments, blanks) = lookup_line_counts(abs, line_counts);
    let lang = detect_lang(abs);
    let sa = sa_map.get(rel).cloned();
    let funcs = sa.as_ref().and_then(|s| s.functions.as_ref()).map_or(0, |v| v.len() as u32);
    let gs = git_statuses.get(rel).cloned().unwrap_or_default();
    let name = abs.file_name().unwrap_or_default().to_string_lossy().to_string();
    FileNode {
        path: rel.to_string(), name, is_dir: false,
        lines, logic, comments, blanks, funcs, mtime, gs, lang, sa,
        children: None,
    }
}

/// Update existing files or insert new ones from reparse results. O(1) per file via HashMap.
fn upsert_changed_files(
    files: &mut Vec<FileNode>,
    to_reparse: &[(String, PathBuf)],
    line_counts: &HashMap<PathBuf, (u32, u32, u32, u32)>,
    sa_map: &HashMap<String, crate::core::types::StructuralAnalysis>,
    git_statuses: &HashMap<String, String>,
) {
    let mut file_map: HashMap<String, usize> = files.iter().enumerate()
        .map(|(i, f)| (f.path.clone(), i)).collect();
    for (rel, abs) in to_reparse {
        let node = build_file_node(rel, abs, line_counts, sa_map, git_statuses);
        if let Some(&idx) = file_map.get(rel) {
            files[idx] = node;
        } else {
            file_map.insert(rel.clone(), files.len());
            files.push(node);
        }
    }
}

/// Enforce MAX_FILES limit: keep most recent files by mtime. [ref:93cf32d4]
fn enforce_max_files(files: &mut Vec<FileNode>) {
    if files.len() > MAX_FILES {
        files.sort_unstable_by(|a, b| b.mtime.total_cmp(&a.mtime));
        files.truncate(MAX_FILES);
    }
}

/// Build tree, emit partial snapshot via callback, then build graphs and return final result.
fn build_snapshot_with_graphs(
    root: &Path,
    files: Vec<FileNode>,
    on_tree_ready: Option<&dyn Fn(Snapshot)>,
    max_call_targets: usize,
) -> Result<ScanResult, AppError> {
    let total_files = files.len() as u32;
    let total_lines: u32 = files.iter().map(|f| f.lines as u64).sum::<u64>().min(u32::MAX as u64) as u32;
    let root_name = root.file_name().unwrap_or_default().to_string_lossy().to_string();

    let (tree, total_dirs) = build_tree(files, &root_name);
    let tree = Arc::new(tree);

    // Emit tree-ready with empty graphs — frontend renders rectangles immediately
    if let Some(cb) = on_tree_ready {
        cb(Snapshot {
            root: Arc::clone(&tree),
            total_files, total_lines, total_dirs,
            call_graph: Vec::new(), import_graph: Vec::new(),
            inherit_graph: Vec::new(), entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        });
    }

    // Build graphs from flattened tree (zero-copy flatten)
    let flat_files = crate::core::snapshot::flatten_files_ref(&tree);
    let gr = crate::analysis::graph::build_graphs(&flat_files, Some(root), max_call_targets);

    Ok(ScanResult {
        snapshot: Snapshot {
            root: tree,
            total_files, total_lines, total_dirs,
            call_graph: gr.call_edges, import_graph: gr.import_edges,
            inherit_graph: gr.inherit_edges, entry_points: gr.entry_points,
            exec_depth: gr.exec_depth,
        },
    })
}

