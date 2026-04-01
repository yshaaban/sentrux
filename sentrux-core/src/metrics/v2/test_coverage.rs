use super::{FindingSeverity, SemanticFinding};
use crate::core::snapshot::{flatten_files_ref, Snapshot};
use crate::metrics::testgap::is_test_file;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn build_missing_test_findings(
    snapshot: &Snapshot,
    changed_files: &BTreeSet<String>,
    baseline_files: &BTreeSet<String>,
) -> Vec<SemanticFinding> {
    if changed_files.is_empty() || baseline_files.is_empty() {
        return Vec::new();
    }

    let snapshot_paths = flatten_files_ref(snapshot.root.as_ref())
        .into_iter()
        .filter(|file| !file.is_dir)
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let missing_test_files = changed_files
        .iter()
        .filter(|path| snapshot_paths.contains(path.as_str()))
        .filter(|path| !baseline_files.contains(*path))
        .filter(|path| is_production_source_path(path))
        .filter(|path| !has_plausible_sibling_test(path, &snapshot_paths))
        .cloned()
        .collect::<Vec<_>>();
    if missing_test_files.is_empty() {
        return Vec::new();
    }

    let mut by_directory = BTreeMap::<String, Vec<String>>::new();
    for path in missing_test_files {
        let directory = Path::new(&path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        by_directory.entry(directory).or_default().push(path);
    }

    let mut findings = Vec::new();
    for (directory, mut files) in by_directory {
        files.sort();
        if files.len() > 3 {
            findings.push(SemanticFinding {
                kind: "missing_test_coverage".to_string(),
                severity: FindingSeverity::Low,
                concept_id: directory.clone(),
                summary: format!(
                    "{} new production files in {} do not have sibling tests",
                    files.len(),
                    directory
                ),
                files: files.clone(),
                evidence: files,
            });
            continue;
        }

        for path in files {
            findings.push(SemanticFinding {
                kind: "missing_test_coverage".to_string(),
                severity: FindingSeverity::Low,
                concept_id: path.clone(),
                summary: format!("New production file {} does not have a sibling test", path),
                files: vec![path.clone()],
                evidence: vec![path],
            });
        }
    }

    findings
}

fn is_production_source_path(path: &str) -> bool {
    if is_test_file(path) {
        return false;
    }

    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    matches!(
        extension,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "rs" | "java" | "kt" | "py"
    ) && (path.starts_with("src/") || path.starts_with("app/") || path.starts_with("lib/"))
}

fn has_plausible_sibling_test(path: &str, snapshot_paths: &BTreeSet<&str>) -> bool {
    if is_test_file(path) {
        return true;
    }

    let file_path = Path::new(path);
    let Some(stem) = file_path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let extension = file_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let parent = file_path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();

    let candidates = [
        format!("{parent}/{stem}.test.{extension}"),
        format!("{parent}/{stem}.spec.{extension}"),
        format!("{parent}/__tests__/{stem}.test.{extension}"),
        format!("{parent}/__tests__/{stem}.spec.{extension}"),
        format!("{parent}/{stem}Test.{extension}"),
        format!("{parent}/{stem}.test.ts"),
        format!("{parent}/{stem}.spec.ts"),
        format!("{parent}/{stem}.test.tsx"),
        format!("{parent}/{stem}.spec.tsx"),
        format!("{parent}/{stem}.test.js"),
        format!("{parent}/{stem}.spec.js"),
        format!("{parent}/{stem}.test.jsx"),
        format!("{parent}/{stem}.spec.jsx"),
        format!("{parent}/{stem}Test.java"),
    ];

    candidates
        .iter()
        .any(|candidate| snapshot_paths.contains(candidate.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::FileNode;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn reports_new_production_file_without_sibling_test() {
        let snapshot = test_snapshot(&["src/app/task-health-monitor.ts"]);
        let changed_files = BTreeSet::from(["src/app/task-health-monitor.ts".to_string()]);
        let baseline_files = BTreeSet::from(["src/app/existing.ts".to_string()]);

        let findings = build_missing_test_findings(&snapshot, &changed_files, &baseline_files);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "missing_test_coverage");
        assert!(findings[0].summary.contains("task-health-monitor.ts"));
    }

    #[test]
    fn groups_multiple_new_files_in_the_same_directory() {
        let snapshot = test_snapshot(&[
            "src/app/a.ts",
            "src/app/b.ts",
            "src/app/c.ts",
            "src/app/d.ts",
        ]);
        let changed_files = BTreeSet::from([
            "src/app/a.ts".to_string(),
            "src/app/b.ts".to_string(),
            "src/app/c.ts".to_string(),
            "src/app/d.ts".to_string(),
        ]);
        let baseline_files = BTreeSet::from(["src/app/existing.ts".to_string()]);

        let findings = build_missing_test_findings(&snapshot, &changed_files, &baseline_files);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].summary.contains("4 new production files"));
        assert_eq!(findings[0].files.len(), 4);
    }

    #[test]
    fn ignores_files_when_a_sibling_test_exists() {
        let snapshot = test_snapshot(&[
            "src/app/task-health-monitor.ts",
            "src/app/task-health-monitor.test.ts",
        ]);
        let changed_files = BTreeSet::from(["src/app/task-health-monitor.ts".to_string()]);
        let baseline_files = BTreeSet::from(["src/app/existing.ts".to_string()]);

        let findings = build_missing_test_findings(&snapshot, &changed_files, &baseline_files);

        assert!(findings.is_empty());
    }

    fn test_snapshot(files: &[&str]) -> Snapshot {
        Snapshot {
            root: Arc::new(FileNode {
                path: ".".to_string(),
                name: ".".to_string(),
                is_dir: true,
                lines: 0,
                logic: 0,
                comments: 0,
                blanks: 0,
                funcs: 0,
                mtime: 0.0,
                gs: String::new(),
                lang: String::new(),
                sa: None,
                children: Some(
                    files
                        .iter()
                        .map(|path| FileNode {
                            path: (*path).to_string(),
                            name: Path::new(path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or(path)
                                .to_string(),
                            is_dir: false,
                            lines: 10,
                            logic: 0,
                            comments: 0,
                            blanks: 0,
                            funcs: 0,
                            mtime: 0.0,
                            gs: String::new(),
                            lang: "ts".to_string(),
                            sa: None,
                            children: None,
                        })
                        .collect(),
                ),
            }),
            total_files: files.len() as u32,
            total_lines: (files.len() * 10) as u32,
            total_dirs: 1,
            call_graph: Vec::new(),
            import_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        }
    }
}
