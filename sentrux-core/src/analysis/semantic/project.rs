//! Project discovery for semantic frontends.

use super::types::ProjectModel;
use crate::analysis::project_shape::detect_project_shape;
use crate::analysis::scanner::common::normalize_path;
use ignore::WalkBuilder;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const PROJECT_DISCOVERY_MAX_DEPTH: usize = 4;
const PROJECT_SHAPE_DISCOVERY_MAX_DEPTH: usize = 8;
const WORKSPACE_FILENAMES: &[&str] = &[
    "package.json",
    "pnpm-workspace.yaml",
    "turbo.json",
    "lerna.json",
    "next.config.js",
    "next.config.mjs",
    "next.config.cjs",
    "next.config.ts",
];

pub fn discover_project(root: &Path) -> Result<ProjectModel, String> {
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let tsconfig_paths = collect_named_files(root, "tsconfig.json");
    let workspace_files = collect_workspace_files(root);
    let primary_language = if !tsconfig_paths.is_empty() {
        Some("typescript".to_string())
    } else {
        None
    };
    let fingerprint = fingerprint_paths(&tsconfig_paths, &workspace_files);
    let discovery_paths = collect_discovery_paths(root);
    let shape = detect_project_shape(Some(root), &discovery_paths, &workspace_files, &[]);

    Ok(ProjectModel {
        root: normalize_path(root.to_string_lossy()),
        tsconfig_paths,
        workspace_files,
        primary_language,
        fingerprint,
        repo_archetype: shape.primary_archetype,
        detected_archetypes: shape.detected_archetypes,
    })
}

fn collect_named_files(root: &Path, filename: &str) -> Vec<String> {
    WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(PROJECT_DISCOVERY_MAX_DEPTH))
        .build()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some(filename))
        .filter_map(|path| relative_normalized(root, &path))
        .collect()
}

fn collect_workspace_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for filename in WORKSPACE_FILENAMES {
        files.extend(collect_named_files(root, filename));
    }
    files.sort();
    files.dedup();
    files
}

fn collect_discovery_paths(root: &Path) -> Vec<String> {
    WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(PROJECT_SHAPE_DISCOVERY_MAX_DEPTH))
        .build()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file())
        .filter_map(|path| relative_normalized(root, &path))
        .collect()
}

fn relative_normalized(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(normalize_path(relative.to_string_lossy()))
}

fn fingerprint_paths(tsconfig_paths: &[String], workspace_files: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    for path in tsconfig_paths {
        path.hash(&mut hasher);
    }
    for path in workspace_files {
        path.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

#[allow(dead_code)]
fn normalize_paths(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| relative_normalized(root, path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::fingerprint_paths;

    #[test]
    fn fingerprint_is_stable_for_same_inputs() {
        let first = fingerprint_paths(&["a/tsconfig.json".into()], &["package.json".into()]);
        let second = fingerprint_paths(&["a/tsconfig.json".into()], &["package.json".into()]);

        assert_eq!(first, second);
    }
}
