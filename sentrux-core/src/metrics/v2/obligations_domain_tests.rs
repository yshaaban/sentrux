use super::*;

#[test]
fn computes_missing_variants_and_related_test_obligations() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[concept]]
                id = "task_presentation_status"
                anchors = ["src/app/task-presentation-status.ts::TaskDotStatus"]
                related_tests = ["src/app/task-presentation-status.test.ts"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/app/task-presentation-status.ts".to_string(),
            symbol_name: "TaskDotStatus".to_string(),
            variants: vec!["idle".to_string(), "busy".to_string(), "error".to_string()],
            line: 4,
            defining_file: Some("src/app/task-presentation-status.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/components/Sidebar.tsx".to_string(),
            domain_symbol_name: "TaskDotStatus".to_string(),
            defining_file: Some("src/app/task-presentation-status.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::AssertNever,
            covered_variants: vec!["idle".to_string(), "busy".to_string()],
            line: 20,
        }],
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from([
        "src/app/task-presentation-status.ts".to_string(),
        "src/components/Sidebar.tsx".to_string(),
    ]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].missing_variants, vec!["error".to_string()]);
    assert_eq!(obligations[0].missing_sites.len(), 2);
    assert!(obligations[0]
        .missing_sites
        .iter()
        .any(|site| site.kind == "related_test"));
    assert!(obligation_score_0_10000(&obligations) < 10000);
}

#[test]
fn closed_domain_exhaustiveness_findings_are_high_severity_without_site_coverage() {
    let findings = build_obligation_findings(&[super::ObligationReport {
        id: "task_status".to_string(),
        kind: "closed_domain_exhaustiveness".to_string(),
        concept_id: None,
        domain_symbol_name: Some("TaskStatus".to_string()),
        origin: super::ObligationOrigin::ZeroConfig,
        trust_tier: super::ObligationTrustTier::Watchpoint,
        confidence: super::ObligationConfidence::Medium,
        severity: crate::metrics::v2::FindingSeverity::High,
        score_0_10000: 8900,
        summary: "missing exhaustive coverage".to_string(),
        files: vec!["src/task-status.ts".to_string()],
        required_sites: vec![ObligationSite {
            path: "src/task-status.ts".to_string(),
            kind: "closed_domain".to_string(),
            line: Some(10),
            detail: "no exhaustive mapping or switch site found".to_string(),
        }],
        satisfied_sites: Vec::new(),
        missing_sites: vec![ObligationSite {
            path: "src/task-status.ts".to_string(),
            kind: "closed_domain".to_string(),
            line: Some(10),
            detail: "no exhaustive mapping or switch site found".to_string(),
        }],
        missing_variants: Vec::new(),
        context_burden: 1,
    }]);

    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].severity,
        crate::metrics::v2::FindingSeverity::High
    );
}

#[test]
fn changed_scope_includes_allowed_writer_paths() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[concept]]
                id = "task_state"
                anchors = ["src/domain/task-state.ts::TaskState"]
                allowed_writers = ["src/app/task-state-writer.ts::*"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/domain/task-state.ts".to_string(),
            symbol_name: "TaskState".to_string(),
            variants: vec!["idle".to_string(), "running".to_string()],
            line: 1,
            defining_file: Some("src/domain/task-state.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/app/presenter.ts".to_string(),
            domain_symbol_name: "TaskState".to_string(),
            defining_file: Some("src/domain/task-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::AssertNever,
            covered_variants: vec!["idle".to_string()],
            line: 10,
        }],
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/task-state-writer.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].concept_id.as_deref(), Some("task_state"));
}

#[test]
fn zero_config_domains_ignore_test_only_sites() {
    let config: RulesConfig = toml::from_str("").expect("empty rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/domain/task-state.ts".to_string(),
            symbol_name: "TaskState".to_string(),
            variants: vec!["idle".to_string(), "running".to_string()],
            line: 1,
            defining_file: Some("src/domain/task-state.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/domain/task-state.test.ts".to_string(),
            domain_symbol_name: "TaskState".to_string(),
            defining_file: Some("src/domain/task-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::AssertNever,
            covered_variants: vec!["idle".to_string()],
            line: 10,
        }],
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    assert!(obligations.is_empty());
}

#[test]
fn zero_config_domains_ignore_large_variant_sets() {
    let config: RulesConfig = toml::from_str("").expect("empty rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/domain/ipc.ts".to_string(),
            symbol_name: "IPC".to_string(),
            variants: (0..20).map(|index| format!("Variant{index}")).collect(),
            line: 1,
            defining_file: Some("src/domain/ipc.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/app/ipc-switch.ts".to_string(),
            domain_symbol_name: "IPC".to_string(),
            defining_file: Some("src/domain/ipc.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::Switch,
            covered_variants: vec!["Variant0".to_string()],
            line: 10,
        }],
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    assert!(obligations.is_empty());
}

#[test]
fn zero_config_domain_matching_prefers_defining_file() {
    let config: RulesConfig = toml::from_str("").expect("empty rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![
            ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
                defining_file: Some("src/domain/task-state.ts".to_string()),
            },
            ClosedDomain {
                path: "src/legacy/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["ready".to_string(), "done".to_string()],
                line: 1,
                defining_file: Some("src/legacy/task-state.ts".to_string()),
            },
        ],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/app/presenter.ts".to_string(),
            domain_symbol_name: "TaskState".to_string(),
            defining_file: Some("src/legacy/task-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::AssertNever,
            covered_variants: vec!["ready".to_string()],
            line: 14,
        }],
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    assert_eq!(obligations.len(), 1);
    assert!(obligations[0].summary.contains("src/legacy/task-state.ts"));
    assert_eq!(obligations[0].missing_variants, vec!["done".to_string()]);

    let findings = build_obligation_findings(&obligations);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]
        .evidence
        .iter()
        .any(|entry| entry.contains("src/legacy/task-state.ts")));
    assert!(findings[0]
        .evidence
        .iter()
        .any(|entry| entry.contains("ready, done")));
}

#[test]
fn zero_config_domains_are_not_in_changed_scope_when_unrelated_files_change() {
    let config: RulesConfig = toml::from_str("").expect("empty rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 1,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: vec![SemanticFileFact::default()],
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/domain/task-state.ts".to_string(),
            symbol_name: "TaskState".to_string(),
            variants: vec!["idle".to_string(), "running".to_string()],
            line: 1,
            defining_file: Some("src/domain/task-state.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/app/presenter.ts".to_string(),
            domain_symbol_name: "TaskState".to_string(),
            defining_file: Some("src/domain/task-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::AssertNever,
            covered_variants: vec!["idle".to_string()],
            line: 10,
        }],
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/unrelated.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    assert!(obligations.is_empty());
}
