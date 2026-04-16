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

    let normalized_root = canonical_root_string(root)?;
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
        root: normalized_root,
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

fn canonical_root_string(root: &Path) -> Result<String, String> {
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        format!(
            "Failed to canonicalize project root {}: {error}",
            root.display()
        )
    })?;
    Ok(normalize_path(canonical_root.to_string_lossy()))
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
    use super::{discover_project, fingerprint_paths};
    use std::path::Path;

    #[test]
    fn fingerprint_is_stable_for_same_inputs() {
        let first = fingerprint_paths(&["a/tsconfig.json".into()], &["package.json".into()]);
        let second = fingerprint_paths(&["a/tsconfig.json".into()], &["package.json".into()]);

        assert_eq!(first, second);
    }

    #[test]
    fn discover_project_normalizes_relative_root_to_absolute_path() {
        let cwd = std::env::current_dir().expect("current dir");
        let local_root = cwd.join("target/discover-project-relative-root");
        if local_root.exists() {
            std::fs::remove_dir_all(&local_root).expect("remove previous local root");
        }
        std::fs::create_dir_all(local_root.join("src")).expect("create local root");
        std::fs::write(
            local_root.join("tsconfig.json"),
            r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler"
  },
  "include": ["src/**/*.ts"]
}
"#,
        )
        .expect("write tsconfig");
        std::fs::write(local_root.join("src/index.ts"), "export const value = 1;\n")
            .expect("write source");

        let relative_root = local_root.strip_prefix(&cwd).expect("local root under cwd");
        let project = discover_project(Path::new(relative_root)).expect("project discovery");
        let expected_root = std::fs::canonicalize(&local_root).expect("canonical local root");

        assert_eq!(
            project.root,
            expected_root.to_string_lossy().replace('\\', "/")
        );

        let _ = std::fs::remove_dir_all(local_root);
    }
}
