use super::{
    build_authority_and_access_findings, build_authority_and_access_findings_with_snapshot,
    FindingSeverity,
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
        transition_sites: Vec::new(),
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
        transition_sites: Vec::new(),
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
        transition_sites: Vec::new(),
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
fn forbidden_raw_reads_carry_preferred_accessor_evidence() {
    let config: RulesConfig = toml::from_str(
        r#"
            [[concept]]
            id = "task_presentation_status"
            kind = "projection"
            anchors = ["src/app/task-presentation-status.ts::getTaskDotStatus"]
            authoritative_inputs = ["src/store/core.ts::store.taskGitStatus"]
            canonical_accessors = [
                "src/app/task-presentation-status.ts::getTaskDotStatus",
                "src/app/task-presentation-status.ts::getTaskDotStatusLabel",
            ]
            forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
        "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Reads],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: vec![ReadFact {
            path: "src/components/SidebarTaskRow.tsx".to_string(),
            symbol_name: "store.taskGitStatus".to_string(),
            read_kind: "property_access".to_string(),
            line: 42,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };

    let findings = build_authority_and_access_findings(&config, &semantic);
    let raw_read = findings
        .iter()
        .find(|finding| finding.kind == "forbidden_raw_read")
        .expect("forbidden raw read finding");

    assert!(raw_read.evidence.iter().any(|entry| {
        entry == "preferred accessor: src/app/task-presentation-status.ts::getTaskDotStatus"
    }));
    assert_eq!(
        raw_read.evidence[1],
        "preferred accessor: src/app/task-presentation-status.ts::getTaskDotStatus"
    );
    assert_eq!(
        raw_read.evidence[2],
        "preferred accessor: src/app/task-presentation-status.ts::getTaskDotStatusLabel"
    );
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
        transition_sites: Vec::new(),
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
        capabilities: vec![SemanticCapability::Reads],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: vec![ReadFact {
            path: "src/app/task-workflows.ts".to_string(),
            symbol_name: "store.taskGitStatus".to_string(),
            read_kind: "property_access".to_string(),
            line: 21,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
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
    assert_eq!(bypasses[0].severity, FindingSeverity::High);
    assert_eq!(bypasses[0].files, vec!["src/app/task-workflows.ts"]);
    assert_eq!(bypasses[0].summary, "Concept 'task_git_status' bypasses canonical entrypoint src/store/store.ts at src/app/task-workflows.ts");
    assert_eq!(
        bypasses[0].evidence,
        vec!["src/app/task-workflows.ts -> src/store/core.ts (prefer src/store/store.ts)"]
    );
}

#[test]
fn ignores_internal_imports_without_matching_concept_usage() {
    let config: RulesConfig = toml::from_str(
        r#"
            [[concept]]
            id = "task_git_status"
            kind = "authoritative_state"
            priority = "critical"
            anchors = ["src/store/core.ts::store.taskGitStatus"]
            canonical_accessors = ["src/store/store.ts::getTaskGitStatus"]
        "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Reads],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: vec![ReadFact {
            path: "src/app/sidebar.ts".to_string(),
            symbol_name: "store.otherValue".to_string(),
            read_kind: "property_access".to_string(),
            line: 14,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let snapshot = snap_with_edges(
        vec![
            edge("src/app/sidebar.ts", "src/store/core.ts"),
            edge("src/store/store.ts", "src/store/core.ts"),
        ],
        vec![
            file("src/app/sidebar.ts"),
            file("src/store/core.ts"),
            file("src/store/store.ts"),
        ],
    );

    let findings =
        build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

    assert!(findings
        .iter()
        .all(|finding| finding.kind != "authoritative_import_bypass"));
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
        reads: vec![ReadFact {
            path: "src/components/SidebarTaskRow.tsx".to_string(),
            symbol_name: "store.taskGitStatus".to_string(),
            read_kind: "property_access".to_string(),
            line: 42,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
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
fn runtime_contract_concepts_do_not_treat_domain_anchors_as_boundary_bypasses() {
    let config: RulesConfig = toml::from_str(
        r#"
            [[concept]]
            id = "server_state_bootstrap"
            kind = "runtime_contract"
            priority = "critical"
            anchors = [
              "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES",
              "src/domain/server-state-bootstrap.ts::ServerStateBootstrapCategory",
            ]
            canonical_accessors = [
              "src/app/server-state-bootstrap.ts::replaceServerStateBootstrap",
              "src/app/server-state-bootstrap-registry.ts::createServerStateBootstrapCategoryDescriptors",
            ]
        "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Reads],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: vec![ReadFact {
            path: "src/app/runtime-diagnostics.ts".to_string(),
            symbol_name: "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                .to_string(),
            read_kind: "identifier".to_string(),
            line: 3,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let snapshot = snap_with_edges(
        vec![
            edge(
                "src/app/runtime-diagnostics.ts",
                "src/domain/server-state-bootstrap.ts",
            ),
            edge(
                "src/app/server-state-bootstrap.ts",
                "src/domain/server-state-bootstrap.ts",
            ),
            edge(
                "src/app/server-state-bootstrap-registry.ts",
                "src/domain/server-state-bootstrap.ts",
            ),
        ],
        vec![
            file("src/app/runtime-diagnostics.ts"),
            file("src/app/server-state-bootstrap.ts"),
            file("src/app/server-state-bootstrap-registry.ts"),
            file("src/domain/server-state-bootstrap.ts"),
        ],
    );

    let findings =
        build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

    assert!(findings
        .iter()
        .all(|finding| finding.kind != "authoritative_import_bypass"));
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
        capabilities: vec![SemanticCapability::Reads],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: vec![
            ReadFact {
                path: "src/app/task-workflows.ts".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                read_kind: "property_access".to_string(),
                line: 21,
            },
            ReadFact {
                path: "src/app/sidebar.ts".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                read_kind: "property_access".to_string(),
                line: 33,
            },
        ],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let snapshot = snap_with_edges(
        vec![
            edge("src/app/task-workflows.ts", "src/store/core.ts"),
            edge("src/app/sidebar.ts", "src/store/core.ts"),
            edge("src/store/internal-status.ts", "src/store/core.ts"),
            edge("src/store/store.ts", "src/store/core.ts"),
        ],
        vec![
            file("src/app/task-workflows.ts"),
            file("src/app/sidebar.ts"),
            file("src/store/core.ts"),
            file("src/store/internal-status.ts"),
            file("src/store/store.ts"),
        ],
    );

    let findings =
        build_authority_and_access_findings_with_snapshot(&config, &semantic, Some(&snapshot));

    let pressure = findings
        .iter()
        .find(|finding| finding.kind == "concept_boundary_pressure")
        .expect("concept boundary pressure finding");
    assert_eq!(pressure.severity, FindingSeverity::Medium);
    assert_eq!(
        pressure.files,
        vec!["src/app/sidebar.ts", "src/app/task-workflows.ts"]
    );
    assert!(pressure.summary.contains("2 files"));
}
