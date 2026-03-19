//! V2 semantic findings built on explicit rules and semantic facts.

mod clones;
mod concentration;
mod obligations;
mod parity;
mod state;

use crate::analysis::semantic::{ReadFact, SemanticSnapshot, WriteFact};
use crate::core::snapshot::Snapshot;
use crate::metrics::rules::{self, ConceptRule, RulesConfig};
use crate::metrics::testgap::is_test_file;
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub use clones::{
    build_clone_drift_findings, build_clone_drift_report, build_clone_remediation_hints,
    CloneDriftFinding, CloneDriftInstance, CloneDriftReport, CloneFamilySummary,
    CloneRemediationHint,
};
pub use concentration::{
    build_concentration_findings, build_concentration_reports, ConcentrationFinding,
    ConcentrationHistory, ConcentrationReport,
};
pub use obligations::{
    build_obligation_findings, build_obligations, changed_concept_ids_from_files,
    changed_concepts_from_obligations, obligation_score_0_10000, ObligationReport, ObligationScope,
    ObligationSite,
};
pub use parity::{
    build_parity_findings, build_parity_reports, parity_score_0_10000, ContractParityReport,
    ParityCell, ParityScope,
};
pub use state::{
    build_state_integrity_findings, build_state_integrity_reports,
    changed_state_model_ids_from_files, state_integrity_score_0_10000, StateIntegrityReport,
    StateScope,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticFinding {
    pub kind: String,
    pub severity: String,
    pub concept_id: String,
    pub summary: String,
    pub files: Vec<String>,
    pub evidence: Vec<String>,
}

pub fn build_authority_and_access_findings(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    build_authority_and_access_findings_with_snapshot(config, semantic, None)
}

pub fn build_authority_and_access_findings_with_snapshot(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    snapshot: Option<&Snapshot>,
) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for concept in &config.concept {
        findings.extend(multi_writer_findings(concept, semantic));
        findings.extend(writer_policy_findings(concept, semantic));
        findings.extend(raw_access_findings(concept, semantic));
        if let Some(snapshot) = snapshot {
            findings.extend(authoritative_import_bypass_findings(concept, snapshot));
        }
    }

    findings
}

fn multi_writer_findings(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    let writes = relevant_production_writes(concept, semantic);
    let writer_files: BTreeSet<String> = writes.iter().map(|write| write.path.clone()).collect();
    if writer_files.len() <= 1 {
        return Vec::new();
    }

    vec![SemanticFinding {
        kind: "multi_writer_concept".to_string(),
        severity: "high".to_string(),
        concept_id: concept.id.clone(),
        summary: format!(
            "Concept '{}' is mutated from {} files",
            concept.id,
            writer_files.len()
        ),
        files: writer_files.iter().cloned().collect(),
        evidence: sorted_deduped_strings(writes.iter().map(|write| {
            format!(
                "{}::{} ({})",
                write.path, write.symbol_name, write.write_kind
            )
        })),
    }]
}

fn writer_policy_findings(
    concept: &ConceptRule,
    semantic: &SemanticSnapshot,
) -> Vec<SemanticFinding> {
    let writes = relevant_production_writes(concept, semantic);
    let mut forbidden_writes = BTreeMap::<String, BTreeSet<String>>::new();
    let mut outside_allowlist_writes = BTreeMap::<String, BTreeSet<String>>::new();

    for write in writes {
        let scoped_target = format!("{}::{}", write.path, write.symbol_name);
        if pattern_list_matches(&concept.forbid_writers, &write.path, &write.symbol_name) {
            forbidden_writes
                .entry(write.path.clone())
                .or_default()
                .insert(scoped_target);
            continue;
        }

        if !concept.allowed_writers.is_empty()
            && !pattern_list_matches(&concept.allowed_writers, &write.path, &write.symbol_name)
        {
            outside_allowlist_writes
                .entry(write.path.clone())
                .or_default()
                .insert(scoped_target);
        }
    }

    let forbidden_findings = forbidden_writes
        .into_iter()
        .map(|(path, evidence)| SemanticFinding {
            kind: "forbidden_writer".to_string(),
            severity: "high".to_string(),
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' is written from forbidden location {}",
                concept.id, path
            ),
            files: vec![path],
            evidence: evidence.into_iter().collect(),
        });
    let outside_allowlist_findings =
        outside_allowlist_writes
            .into_iter()
            .map(|(path, evidence)| SemanticFinding {
                kind: "writer_outside_allowlist".to_string(),
                severity: "high".to_string(),
                concept_id: concept.id.clone(),
                summary: format!(
                    "Concept '{}' is written outside its allowed writer set at {}",
                    concept.id, path
                ),
                files: vec![path],
                evidence: evidence.into_iter().collect(),
            });

    forbidden_findings
        .chain(outside_allowlist_findings)
        .collect()
}

fn raw_access_findings(concept: &ConceptRule, semantic: &SemanticSnapshot) -> Vec<SemanticFinding> {
    let reads = relevant_production_reads(concept, semantic);
    let mut forbidden_reads = BTreeMap::<String, BTreeSet<String>>::new();

    for read in reads {
        if !pattern_list_matches(&concept.forbid_raw_reads, &read.path, &read.symbol_name) {
            continue;
        }

        let scoped_target = format!("{}::{}", read.path, read.symbol_name);
        forbidden_reads
            .entry(read.path.clone())
            .or_default()
            .insert(scoped_target);
    }

    forbidden_reads
        .into_iter()
        .map(|(path, evidence)| SemanticFinding {
            kind: "forbidden_raw_read".to_string(),
            severity: "medium".to_string(),
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' is read from a forbidden raw access path at {}",
                concept.id, path
            ),
            files: vec![path],
            evidence: evidence.into_iter().collect(),
        })
        .collect()
}

fn authoritative_import_bypass_findings(
    concept: &ConceptRule,
    snapshot: &Snapshot,
) -> Vec<SemanticFinding> {
    let authoritative_paths = concept_internal_boundary_paths(concept);
    if authoritative_paths.is_empty() {
        return Vec::new();
    }
    let preferred_entry_paths = concept_preferred_entry_paths(concept);
    if preferred_entry_paths.is_empty() {
        return Vec::new();
    }

    let allowed_importers = concept_allowed_importer_paths(concept);
    let mut bypasses = BTreeMap::<String, BTreeSet<String>>::new();

    for edge in &snapshot.import_graph {
        if !authoritative_paths.contains(&edge.to_file) {
            continue;
        }
        if edge.from_file == edge.to_file || is_test_file(&edge.from_file) {
            continue;
        }
        if allowed_importers
            .iter()
            .any(|pattern| path_matches_pattern(pattern, &edge.from_file))
        {
            continue;
        }
        let preference_detail =
            preferred_entry_detail(&preferred_entry_paths, edge.to_file.as_str());

        bypasses
            .entry(edge.from_file.clone())
            .or_default()
            .insert(format!(
                "{} -> {}{}",
                edge.from_file, edge.to_file, preference_detail
            ));
    }

    let severity = if concept.priority.as_deref() == Some("critical") {
        "high"
    } else {
        "medium"
    };
    let preferred_entry_summary = preferred_entry_summary(&preferred_entry_paths);
    let mut findings = bypasses
        .iter()
        .map(|(path, evidence)| SemanticFinding {
            kind: "authoritative_import_bypass".to_string(),
            severity: severity.to_string(),
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' bypasses {} at {}",
                concept.id, preferred_entry_summary, path
            ),
            files: vec![path.clone()],
            evidence: evidence.iter().cloned().collect(),
        })
        .collect::<Vec<_>>();

    if bypasses.len() > 1 {
        findings.push(SemanticFinding {
            kind: "concept_boundary_pressure".to_string(),
            severity: severity.to_string(),
            concept_id: concept.id.clone(),
            summary: format!(
                "Concept '{}' is bypassing {} from {} files",
                concept.id,
                preferred_entry_summary,
                bypasses.len()
            ),
            files: bypasses.keys().cloned().collect(),
            evidence: bypasses
                .iter()
                .flat_map(|(_, evidence)| evidence.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        });
    }

    findings
}

pub(crate) fn relevant_writes<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a WriteFact> {
    let targets = concept_write_targets(concept);
    semantic
        .writes
        .iter()
        .filter(|write| symbol_matches_targets(&write.symbol_name, &targets))
        .collect()
}

pub(crate) fn relevant_production_writes<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a WriteFact> {
    relevant_writes(concept, semantic)
        .into_iter()
        .filter(|write| !is_test_file(&write.path))
        .collect()
}

pub(crate) fn relevant_reads<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ReadFact> {
    let targets = concept_read_targets(concept);
    semantic
        .reads
        .iter()
        .filter(|read| symbol_matches_targets(&read.symbol_name, &targets))
        .collect()
}

pub(crate) fn relevant_production_reads<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ReadFact> {
    relevant_reads(concept, semantic)
        .into_iter()
        .filter(|read| !is_test_file(&read.path))
        .collect()
}

pub(crate) fn concept_write_targets(concept: &ConceptRule) -> HashSet<String> {
    if concept.kind == "projection" {
        return scoped_symbols(&concept.anchors);
    }

    concept_targets(concept)
}

pub(crate) fn concept_read_targets(concept: &ConceptRule) -> HashSet<String> {
    if concept.kind == "projection" && !concept.authoritative_inputs.is_empty() {
        return scoped_symbols(&concept.authoritative_inputs);
    }

    concept_targets(concept)
}

pub(crate) fn concept_targets(concept: &ConceptRule) -> HashSet<String> {
    scoped_symbols(
        &concept
            .anchors
            .iter()
            .chain(concept.authoritative_inputs.iter())
            .cloned()
            .collect::<Vec<_>>(),
    )
}

fn concept_internal_boundary_paths(concept: &ConceptRule) -> HashSet<String> {
    let mut paths = HashSet::new();

    if concept.kind == "projection" && !concept.authoritative_inputs.is_empty() {
        for value in &concept.authoritative_inputs {
            insert_scoped_path(&mut paths, value);
        }
        return paths;
    }

    for value in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
    {
        insert_scoped_path(&mut paths, value);
    }

    paths
}

fn concept_preferred_entry_paths(concept: &ConceptRule) -> Vec<String> {
    let mut paths = BTreeSet::new();

    for value in &concept.canonical_accessors {
        insert_scoped_path(&mut paths, value);
    }

    if concept.kind == "projection" {
        for value in &concept.anchors {
            insert_scoped_path(&mut paths, value);
        }
    }

    paths.into_iter().collect()
}

fn concept_allowed_importer_paths(concept: &ConceptRule) -> Vec<String> {
    let mut patterns = BTreeSet::new();

    for value in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.related_tests.iter())
    {
        if let Some((path, _)) = value.split_once("::") {
            patterns.insert(path.to_string());
        } else {
            patterns.insert(value.clone());
        }
    }

    patterns.into_iter().collect()
}

fn preferred_entry_summary(preferred_entry_paths: &[String]) -> String {
    match preferred_entry_paths {
        [] => "canonical boundaries".to_string(),
        [path] => format!("canonical entrypoint {}", path),
        _ => format!("canonical entrypoints {}", preferred_entry_paths.join(", ")),
    }
}

fn preferred_entry_detail(preferred_entry_paths: &[String], imported_path: &str) -> String {
    let alternatives = preferred_entry_paths
        .iter()
        .filter(|path| path.as_str() != imported_path)
        .cloned()
        .collect::<Vec<_>>();
    if alternatives.is_empty() {
        return String::new();
    }

    format!(" (prefer {})", alternatives.join(", "))
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}

fn insert_scoped_path(paths: &mut impl Extend<String>, value: &str) {
    if let Some((path, _)) = value.split_once("::") {
        paths.extend(std::iter::once(path.to_string()));
    } else {
        paths.extend(std::iter::once(value.to_string()));
    }
}

pub(crate) fn symbol_from_scoped_path(value: &str) -> Option<String> {
    let (_, symbol) = value.split_once("::")?;
    Some(symbol.to_string())
}

fn scoped_symbols(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .filter_map(|value| symbol_from_scoped_path(value))
        .collect()
}

fn sorted_deduped_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn symbol_matches_targets(symbol_name: &str, targets: &HashSet<String>) -> bool {
    targets.iter().any(|target| {
        symbol_name == target
            || symbol_name.starts_with(&format!("{target}."))
            || symbol_name.starts_with(&format!("{target}[*]."))
            || symbol_name.starts_with(&format!("{target}.*."))
    })
}

pub(crate) fn pattern_list_matches(patterns: &[String], path: &str, symbol_name: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| scoped_pattern_matches(pattern, path, symbol_name))
}

pub(crate) fn scoped_pattern_matches(pattern: &str, path: &str, symbol_name: &str) -> bool {
    let (path_pattern, symbol_pattern) = match pattern.split_once("::") {
        Some((path_pattern, symbol_pattern)) => (path_pattern, Some(symbol_pattern)),
        None => (pattern, None),
    };
    if !rules::glob_match(path_pattern, path) {
        return false;
    }

    match symbol_pattern {
        None => true,
        Some("*") | Some("**") => true,
        Some(symbol_pattern) => symbol_pattern_matches(symbol_pattern, symbol_name),
    }
}

pub(crate) fn symbol_pattern_matches(pattern: &str, symbol_name: &str) -> bool {
    if pattern == symbol_name {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return symbol_name == prefix || symbol_name.starts_with(&format!("{prefix}."));
    }
    if let Some(prefix) = pattern.strip_suffix(".**") {
        return symbol_name == prefix || symbol_name.starts_with(&format!("{prefix}."));
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        build_authority_and_access_findings, build_authority_and_access_findings_with_snapshot,
    };
    use crate::analysis::semantic::{
        ProjectModel, ReadFact, SemanticCapability, SemanticSnapshot, WriteFact,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::metrics::test_helpers::{edge, file, snap_with_edges};

    #[test]
    fn reports_multi_writer_and_forbidden_raw_read_findings() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
                forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Reads, SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: vec![ReadFact {
                path: "src/components/Sidebar.tsx".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                read_kind: "property_access".to_string(),
                line: 10,
            }],
            writes: vec![
                WriteFact {
                    path: "src/app/git-status-sync.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 5,
                },
                WriteFact {
                    path: "src/store/git-status-polling.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 8,
                },
            ],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let findings = build_authority_and_access_findings(&config, &semantic);

        assert!(findings
            .iter()
            .any(|finding| finding.kind == "multi_writer_concept"));
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "writer_outside_allowlist"));
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "forbidden_raw_read"));
    }

    #[test]
    fn ignores_test_writes_and_reads_for_authority_findings() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
                forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Reads, SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: vec![
                ReadFact {
                    path: "src/components/Sidebar.tsx".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 10,
                },
                ReadFact {
                    path: "src/components/Sidebar.test.tsx".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 20,
                },
            ],
            writes: vec![
                WriteFact {
                    path: "src/app/git-status-sync.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 5,
                },
                WriteFact {
                    path: "src/app/task-presentation-status.test.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 18,
                },
            ],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let findings = build_authority_and_access_findings(&config, &semantic);

        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.kind == "multi_writer_concept")
                .count(),
            0
        );
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.kind == "writer_outside_allowlist")
                .count(),
            0
        );
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.kind == "forbidden_raw_read")
                .count(),
            1
        );
        assert!(findings.iter().all(|finding| !finding
            .files
            .iter()
            .any(|path| path.ends_with(".test.ts") || path.ends_with(".test.tsx"))));
    }

    #[test]
    fn projection_concepts_use_authoritative_inputs_for_reads_not_writes() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_presentation_status"
                kind = "projection"
                anchors = ["src/app/task-presentation-status.ts::getTaskDotStatus"]
                authoritative_inputs = [
                    "src/store/core.ts::store.agentSupervision",
                    "src/store/core.ts::store.taskGitStatus",
                ]
                forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Reads, SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: vec![ReadFact {
                path: "src/components/SidebarTaskRow.tsx".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                read_kind: "property_access".to_string(),
                line: 42,
            }],
            writes: vec![
                WriteFact {
                    path: "src/app/git-status-sync.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 5,
                },
                WriteFact {
                    path: "src/store/git-status-polling.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 8,
                },
            ],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let findings = build_authority_and_access_findings(&config, &semantic);

        assert!(findings
            .iter()
            .any(|finding| finding.kind == "forbidden_raw_read"));
        assert!(findings
            .iter()
            .all(|finding| finding.kind != "multi_writer_concept"));
        assert!(findings
            .iter()
            .all(|finding| finding.kind != "writer_outside_allowlist"));
    }

    #[test]
    fn writer_policy_findings_are_deduped_per_file() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                forbid_writers = ["src/store/git-status-polling.ts::store.taskGitStatus.*"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: vec![
                WriteFact {
                    path: "src/store/git-status-polling.ts".to_string(),
                    symbol_name: "store.taskGitStatus.*".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 61,
                },
                WriteFact {
                    path: "src/store/git-status-polling.ts".to_string(),
                    symbol_name: "store.taskGitStatus.*".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 113,
                },
            ],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let findings = build_authority_and_access_findings(&config, &semantic);
        let forbidden = findings
            .iter()
            .filter(|finding| finding.kind == "forbidden_writer")
            .collect::<Vec<_>>();

        assert_eq!(forbidden.len(), 1);
        assert_eq!(forbidden[0].files, vec!["src/store/git-status-polling.ts"]);
        assert_eq!(
            forbidden[0].evidence,
            vec!["src/store/git-status-polling.ts::store.taskGitStatus.*"]
        );
    }

    #[test]
    fn reports_direct_imports_of_authoritative_modules() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                kind = "authoritative_state"
                priority = "critical"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                authoritative_inputs = ["src/store/internal-status.ts::taskGitStatusSource"]
                canonical_accessors = ["src/store/store.ts::getTaskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let snapshot = snap_with_edges(
            vec![
                edge("src/app/task-workflows.ts", "src/store/core.ts"),
                edge("src/store/internal-status.ts", "src/store/core.ts"),
                edge("src/store/store.ts", "src/store/core.ts"),
                edge("src/app/git-status-sync.ts", "src/store/core.ts"),
            ],
            vec![
                file("src/app/task-workflows.ts"),
                file("src/store/core.ts"),
                file("src/store/internal-status.ts"),
                file("src/store/store.ts"),
                file("src/app/git-status-sync.ts"),
            ],
        );

        let findings =
            build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

        let bypasses = findings
            .iter()
            .filter(|finding| finding.kind == "authoritative_import_bypass")
            .collect::<Vec<_>>();
        assert_eq!(bypasses.len(), 1);
        assert_eq!(bypasses[0].severity, "high");
        assert_eq!(bypasses[0].files, vec!["src/app/task-workflows.ts"]);
        assert_eq!(bypasses[0].summary, "Concept 'task_git_status' bypasses canonical entrypoint src/store/store.ts at src/app/task-workflows.ts");
        assert_eq!(
            bypasses[0].evidence,
            vec!["src/app/task-workflows.ts -> src/store/core.ts (prefer src/store/store.ts)"]
        );
    }

    #[test]
    fn reports_projection_import_bypass_through_authoritative_inputs() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_presentation_status"
                kind = "projection"
                anchors = ["src/app/task-presentation-status.ts::getTaskDotStatus"]
                authoritative_inputs = [
                    "src/store/core.ts::store.agentSupervision",
                    "src/store/core.ts::store.taskGitStatus",
                ]
                canonical_accessors = ["src/app/task-presentation-status.ts::getTaskDotStatus"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Reads],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let snapshot = snap_with_edges(
            vec![
                edge("src/components/SidebarTaskRow.tsx", "src/store/core.ts"),
                edge("src/app/task-presentation-status.ts", "src/store/core.ts"),
            ],
            vec![
                file("src/components/SidebarTaskRow.tsx"),
                file("src/app/task-presentation-status.ts"),
                file("src/store/core.ts"),
            ],
        );

        let findings =
            build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

        let bypass = findings
            .iter()
            .find(|finding| finding.kind == "authoritative_import_bypass")
            .expect("projection bypass finding");
        assert_eq!(bypass.files, vec!["src/components/SidebarTaskRow.tsx"]);
        assert!(bypass
            .summary
            .contains("canonical entrypoint src/app/task-presentation-status.ts"));
    }

    #[test]
    fn reports_concept_boundary_pressure_when_multiple_files_bypass_same_boundary() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                kind = "authoritative_state"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                authoritative_inputs = ["src/store/internal-status.ts::taskGitStatusSource"]
                canonical_accessors = ["src/store/store.ts::getTaskGitStatus"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let snapshot = snap_with_edges(
            vec![
                edge("src/app/task-workflows.ts", "src/store/core.ts"),
                edge("src/app/sidebar.ts", "src/store/core.ts"),
                edge("src/store/store.ts", "src/store/core.ts"),
            ],
            vec![
                file("src/app/task-workflows.ts"),
                file("src/app/sidebar.ts"),
                file("src/store/core.ts"),
                file("src/store/store.ts"),
            ],
        );

        let findings =
            build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

        let pressure = findings
            .iter()
            .find(|finding| finding.kind == "concept_boundary_pressure")
            .expect("boundary pressure finding");
        assert_eq!(
            pressure.files,
            vec!["src/app/sidebar.ts", "src/app/task-workflows.ts"]
        );
        assert!(pressure.summary.contains("2 files"));
    }
}
