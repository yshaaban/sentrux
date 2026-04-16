use crate::core::types::FileNode;
use std::collections::HashMap;
use std::path::Path;

/// Only manifests that truly define a project boundary. Makefile and
/// CMakeLists.txt are excluded: they routinely appear at multiple directory
/// levels within a single project (CMake per-directory, recursive Make),
/// causing the boundary gate to silently drop valid cross-directory imports.
/// Manifest files aggregated from all loaded plugins. Cached at first access.
static MANIFEST_FILES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    crate::analysis::lang_registry::all_manifest_files()
        .into_iter()
        .map(|s| s.to_string())
        .collect()
});

/// Backfill all visited directories with the found project root.
fn backfill_cache(cache: &mut HashMap<String, String>, visited: &[String], result: &str) {
    for v in visited {
        cache.insert(v.clone(), result.to_string());
    }
}

/// Check if any manifest file exists in the given directory.
fn has_manifest(dir: &Path) -> bool {
    MANIFEST_FILES
        .iter()
        .any(|manifest| dir.join(manifest).exists())
}

/// Detect which project a file belongs to by walking up from its directory
/// to find the nearest manifest file. Caches ALL intermediate directories
/// visited during the walk so sibling files sharing ancestor dirs skip the
/// filesystem entirely (previous code only cached the leaf dir).
fn detect_project_root_cached(
    file_rel_path: &str,
    scan_root: &Path,
    cache: &mut HashMap<String, String>,
) -> String {
    let abs = scan_root.join(file_rel_path);
    let mut dir = abs.parent().unwrap_or(scan_root).to_path_buf();
    let mut visited: Vec<String> = Vec::new();

    while dir.starts_with(scan_root) {
        let rel = dir
            .strip_prefix(scan_root)
            .unwrap_or(&dir)
            .to_string_lossy()
            .to_string();

        if let Some(cached) = cache.get(&rel) {
            let result = cached.clone();
            backfill_cache(cache, &visited, &result);
            return result;
        }

        if has_manifest(&dir) {
            cache.insert(rel.clone(), rel.clone());
            backfill_cache(cache, &visited, &rel);
            return rel;
        }

        visited.push(rel);
        if dir == *scan_root || !dir.pop() {
            break;
        }
    }

    backfill_cache(cache, &visited, "");
    String::new()
}

/// Build project membership map: file_path -> project_root.
/// Computed once per scan for all files. Caches intermediate directories
/// to avoid redundant filesystem walks up shared ancestor paths.
pub(super) fn build_project_map(files: &[&FileNode], scan_root: &Path) -> HashMap<String, String> {
    let t0 = std::time::Instant::now();
    let mut dir_cache: HashMap<String, String> = HashMap::new();
    let mut project_map = HashMap::new();
    let mut cache_misses = 0usize;

    for file in files {
        if file.is_dir {
            continue;
        }
        let dir = Path::new(&file.path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();
        let project_root = if let Some(cached) = dir_cache.get(&dir) {
            cached.clone()
        } else {
            cache_misses += 1;
            detect_project_root_cached(&file.path, scan_root, &mut dir_cache)
        };
        project_map.insert(file.path.clone(), project_root);
    }
    eprintln!(
        "[build_project_map] {} files, {} unique dirs, {} cache misses, {:.1}ms",
        files.len(),
        dir_cache.len(),
        cache_misses,
        t0.elapsed().as_secs_f64() * 1000.0
    );
    project_map
}
