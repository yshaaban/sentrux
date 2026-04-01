use super::{FindingSeverity, SemanticFinding};
use crate::analysis::project_shape::ProjectShapeReport;
use crate::core::snapshot::Snapshot;
use crate::metrics::rules::{ModuleContractRule, RulesConfig};
use std::collections::BTreeSet;

pub fn build_zero_config_boundary_findings(
    config: &RulesConfig,
    snapshot: &Snapshot,
    shape: &ProjectShapeReport,
    changed_files: &BTreeSet<String>,
) -> Vec<SemanticFinding> {
    if changed_files.is_empty() || !config.module_contract.is_empty() {
        return Vec::new();
    }

    let mut findings = Vec::new();
    for suggestion in shape
        .module_contracts
        .iter()
        .filter(|suggestion| suggestion.confidence == "high")
    {
        let rule = ModuleContractRule {
            id: suggestion.id.clone(),
            root: suggestion.root.clone(),
            public_api: suggestion.public_api.clone(),
            forbid_cross_module_deep_imports: true,
        };
        let violations =
            crate::metrics::rules::check_module_contract(&rule, &snapshot.import_graph);
        for violation in violations {
            if !violation
                .files
                .iter()
                .any(|path| changed_files.contains(path))
            {
                continue;
            }

            let mut evidence = suggestion.evidence.clone();
            evidence.push(format!("inferred module root: {}", suggestion.root));
            evidence.push(format!(
                "public API files: {}",
                suggestion.public_api.join(", ")
            ));

            findings.push(SemanticFinding {
                kind: "zero_config_boundary_violation".to_string(),
                severity: FindingSeverity::Low,
                concept_id: suggestion.id.clone(),
                summary: format!(
                    "{} (inferred from project structure, not user-authored rules)",
                    violation.message
                ),
                files: violation.files,
                evidence,
            });
        }
    }

    findings.sort_by(|left, right| {
        left.summary
            .cmp(&right.summary)
            .then(left.files.cmp(&right.files))
    });
    findings.dedup_by(|left, right| left.summary == right.summary && left.files == right.files);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::project_shape::{ModuleContractSuggestion, ProjectShapeReport};
    use crate::core::types::{FileNode, ImportEdge};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn emits_changed_scope_findings_from_high_confidence_shape_suggestions() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let snapshot = test_snapshot(&[
            ("src/app/feature.ts", "src/module/internal.ts"),
            ("src/module/index.ts", "src/module/internal.ts"),
        ]);
        let shape = ProjectShapeReport {
            module_contracts: vec![ModuleContractSuggestion {
                id: "module_api".to_string(),
                root: "src/module".to_string(),
                public_api: vec!["src/module/index.ts".to_string()],
                nested_public_api: Vec::new(),
                confidence: "high".to_string(),
                evidence: vec!["detected boundary root".to_string()],
            }],
            ..ProjectShapeReport::default()
        };
        let changed_files = BTreeSet::from(["src/app/feature.ts".to_string()]);

        let findings =
            build_zero_config_boundary_findings(&config, &snapshot, &shape, &changed_files);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "zero_config_boundary_violation");
        assert!(findings[0]
            .summary
            .contains("inferred from project structure"));
    }

    #[test]
    fn skips_inferred_findings_when_explicit_module_contract_rules_exist() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[module_contract]]
                id = "module_api"
                root = "src/module"
                public_api = ["src/module/index.ts"]
                forbid_cross_module_deep_imports = true
            "#,
        )
        .expect("rules config");
        let snapshot = test_snapshot(&[("src/app/feature.ts", "src/module/internal.ts")]);
        let shape = ProjectShapeReport {
            module_contracts: vec![ModuleContractSuggestion {
                id: "module_api".to_string(),
                root: "src/module".to_string(),
                public_api: vec!["src/module/index.ts".to_string()],
                nested_public_api: Vec::new(),
                confidence: "high".to_string(),
                evidence: vec!["detected boundary root".to_string()],
            }],
            ..ProjectShapeReport::default()
        };
        let changed_files = BTreeSet::from(["src/app/feature.ts".to_string()]);

        let findings =
            build_zero_config_boundary_findings(&config, &snapshot, &shape, &changed_files);

        assert!(findings.is_empty());
    }

    #[test]
    fn skips_inferred_findings_when_changed_scope_does_not_touch_the_violation() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let snapshot = test_snapshot(&[("src/app/feature.ts", "src/module/internal.ts")]);
        let shape = ProjectShapeReport {
            module_contracts: vec![ModuleContractSuggestion {
                id: "module_api".to_string(),
                root: "src/module".to_string(),
                public_api: vec!["src/module/index.ts".to_string()],
                nested_public_api: Vec::new(),
                confidence: "high".to_string(),
                evidence: vec!["detected boundary root".to_string()],
            }],
            ..ProjectShapeReport::default()
        };
        let changed_files = BTreeSet::from(["src/app/unrelated.ts".to_string()]);

        let findings =
            build_zero_config_boundary_findings(&config, &snapshot, &shape, &changed_files);

        assert!(findings.is_empty());
    }

    fn test_snapshot(imports: &[(&str, &str)]) -> Snapshot {
        let mut paths = BTreeSet::new();
        let import_graph = imports
            .iter()
            .map(|(from_file, to_file)| {
                paths.insert((*from_file).to_string());
                paths.insert((*to_file).to_string());
                ImportEdge {
                    from_file: (*from_file).to_string(),
                    to_file: (*to_file).to_string(),
                }
            })
            .collect::<Vec<_>>();
        let children = paths
            .into_iter()
            .map(|path| FileNode {
                name: path
                    .split('/')
                    .next_back()
                    .unwrap_or(path.as_str())
                    .to_string(),
                path,
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
            .collect::<Vec<_>>();

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
                children: Some(children),
            }),
            total_files: imports.len().max(1) as u32,
            total_lines: (imports.len().max(1) * 10) as u32,
            total_dirs: 1,
            call_graph: Vec::new(),
            import_graph,
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        }
    }
}
