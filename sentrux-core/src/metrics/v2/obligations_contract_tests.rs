use super::*;

#[test]
fn changed_scope_requires_related_contract_surfaces() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 4,
        capabilities: vec![SemanticCapability::Symbols],
        files: vec![
            SemanticFileFact {
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/runtime/browser-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/desktop-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
        ],
        symbols: vec![
            SymbolFact {
                id: "cats".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                kind: "const".to_string(),
                line: 3,
            },
            SymbolFact {
                id: "payload".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "ServerStateBootstrapPayloadMap".to_string(),
                kind: "type_alias".to_string(),
                line: 8,
            },
            SymbolFact {
                id: "registry".to_string(),
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                kind: "const".to_string(),
                line: 5,
            },
        ],
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/domain/server-state-bootstrap.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert_eq!(
        contract.concept_id.as_deref(),
        Some("server_state_bootstrap")
    );
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "registry_symbol"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "browser_entry"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "electron_entry"));
}

#[test]
fn changed_scope_triggers_contract_when_registry_surface_changes() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 4,
        capabilities: vec![SemanticCapability::Symbols],
        files: vec![
            SemanticFileFact {
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/runtime/browser-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/desktop-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
        ],
        symbols: vec![
            SymbolFact {
                id: "cats".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                kind: "const".to_string(),
                line: 3,
            },
            SymbolFact {
                id: "payload".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "ServerStateBootstrapPayloadMap".to_string(),
                kind: "type_alias".to_string(),
                line: 8,
            },
            SymbolFact {
                id: "registry".to_string(),
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                kind: "const".to_string(),
                line: 5,
            },
        ],
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/server-state-bootstrap-registry.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert!(contract
        .satisfied_sites
        .iter()
        .any(|site| site.kind == "registry_symbol"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "categories_symbol"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "browser_entry"));
}

#[test]
fn changed_scope_triggers_from_semantically_related_contract_reader() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 5,
        capabilities: vec![SemanticCapability::Symbols, SemanticCapability::Reads],
        files: vec![
            SemanticFileFact {
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/runtime/browser-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/bootstrap-persist.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/bootstrap-adapter.ts".to_string(),
                ..SemanticFileFact::default()
            },
        ],
        symbols: vec![
            SymbolFact {
                id: "cats".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                kind: "const".to_string(),
                line: 3,
            },
            SymbolFact {
                id: "payload".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "ServerStateBootstrapPayloadMap".to_string(),
                kind: "type_alias".to_string(),
                line: 8,
            },
            SymbolFact {
                id: "registry".to_string(),
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                kind: "const".to_string(),
                line: 5,
            },
            SymbolFact {
                id: "persist".to_string(),
                path: "src/app/bootstrap-persist.ts".to_string(),
                name: "serializeBootstrapPayload".to_string(),
                kind: "function".to_string(),
                line: 12,
            },
        ],
        reads: vec![ReadFact {
            path: "src/app/bootstrap-adapter.ts".to_string(),
            symbol_name: "ServerStateBootstrapPayloadMap".to_string(),
            read_kind: "type_reference".to_string(),
            line: 21,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/bootstrap-adapter.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert!(contract
        .files
        .contains(&"src/app/bootstrap-adapter.ts".to_string()));
    assert!(contract.summary.contains("bootstrap-adapter.ts"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "required_symbol"));
}

#[test]
fn changed_scope_triggers_from_semantically_related_contract_symbol_declaration() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 3,
        capabilities: vec![SemanticCapability::Symbols],
        files: vec![
            SemanticFileFact {
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/bootstrap-persist.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/bootstrap-field-adapter.ts".to_string(),
                ..SemanticFileFact::default()
            },
        ],
        symbols: vec![
            SymbolFact {
                id: "payload".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "ServerStateBootstrapPayloadMap".to_string(),
                kind: "type_alias".to_string(),
                line: 8,
            },
            SymbolFact {
                id: "persist".to_string(),
                path: "src/app/bootstrap-persist.ts".to_string(),
                name: "serializeBootstrapPayload".to_string(),
                kind: "function".to_string(),
                line: 12,
            },
            SymbolFact {
                id: "field".to_string(),
                path: "src/app/bootstrap-field-adapter.ts".to_string(),
                name: "ServerStateBootstrapPayloadMap.snapshot".to_string(),
                kind: "property_signature".to_string(),
                line: 4,
            },
        ],
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/bootstrap-field-adapter.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert!(contract
        .files
        .contains(&"src/app/bootstrap-field-adapter.ts".to_string()));
    assert!(contract.summary.contains("bootstrap-field-adapter.ts"));
}

#[test]
fn changed_scope_ignores_test_only_contract_readers() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 4,
        capabilities: vec![SemanticCapability::Symbols, SemanticCapability::Reads],
        files: vec![
            SemanticFileFact {
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/runtime/browser-session.ts".to_string(),
                ..SemanticFileFact::default()
            },
            SemanticFileFact {
                path: "src/app/server-state-bootstrap.test.ts".to_string(),
                ..SemanticFileFact::default()
            },
        ],
        symbols: vec![
            SymbolFact {
                id: "cats".to_string(),
                path: "src/domain/server-state-bootstrap.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                kind: "const".to_string(),
                line: 3,
            },
            SymbolFact {
                id: "registry".to_string(),
                path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                kind: "const".to_string(),
                line: 5,
            },
        ],
        reads: vec![ReadFact {
            path: "src/app/server-state-bootstrap.test.ts".to_string(),
            symbol_name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
            read_kind: "property_access".to_string(),
            line: 12,
        }],
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };
    let changed_files = BTreeSet::from(["src/app/server-state-bootstrap.test.ts".to_string()]);

    let obligations =
        build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

    assert!(obligations.is_empty());
}

#[test]
fn all_scope_reports_missing_declared_contract_sites() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                browser_entry = "src/runtime/browser-session.ts"
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Symbols],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert_eq!(contract.missing_sites.len(), 2);
    assert!(contract
        .missing_sites
        .iter()
        .all(|site| site.detail.contains("declared contract site is missing")));
}

#[test]
fn all_scope_reports_required_contract_extensions() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
                required_files = ["src/runtime/server-state-bootstrap.ts"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Symbols],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "required_symbol"));
    assert!(contract
        .missing_sites
        .iter()
        .any(|site| site.kind == "required_file"));
}

#[test]
fn contract_missing_sites_prioritize_runtime_and_registry_surfaces() {
    let config: RulesConfig = toml::from_str(
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                required_files = ["src/app/bootstrap-persist.ts"]
            "#,
    )
    .expect("rules config");
    let semantic = SemanticSnapshot {
        project: ProjectModel::default(),
        analyzed_files: 0,
        capabilities: vec![SemanticCapability::Symbols],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: Vec::new(),
        closed_domain_sites: Vec::new(),
        transition_sites: Vec::new(),
    };

    let obligations = build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

    let contract = obligations
        .iter()
        .find(|obligation| obligation.kind == "contract_surface_completeness")
        .expect("contract obligation");
    assert_eq!(
        contract
            .missing_sites
            .first()
            .map(|site| site.kind.as_str()),
        Some("browser_entry")
    );
    assert_eq!(
        contract.missing_sites.get(1).map(|site| site.kind.as_str()),
        Some("registry_symbol")
    );
    assert_eq!(
        contract.missing_sites.get(2).map(|site| site.kind.as_str()),
        Some("required_file")
    );
}

#[test]
fn contract_missing_site_summary_dedupes_non_adjacent_labels() {
    let summary = summarize_contract_missing_sites(&[
        ObligationSite {
            path: "src/app/bootstrap-adapter.ts".to_string(),
            kind: "required_symbol".to_string(),
            line: None,
            detail: "update adapter".to_string(),
        },
        ObligationSite {
            path: "src/runtime/browser-session.ts".to_string(),
            kind: "browser_entry".to_string(),
            line: None,
            detail: "update browser runtime entry".to_string(),
        },
        ObligationSite {
            path: "src/app/bootstrap-persist.ts".to_string(),
            kind: "required_symbol".to_string(),
            line: None,
            detail: "update required contract symbol".to_string(),
        },
    ]);

    assert_eq!(
        summary,
        "the required symbol and browser runtime entry surfaces"
    );
}
