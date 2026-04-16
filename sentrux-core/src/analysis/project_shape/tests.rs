use super::{detect_project_shape, render_starter_rules, render_working_rules};

#[test]
fn detects_modular_nextjs_frontend_shape() {
    let file_paths = vec![
        "src/app/[locale]/layout.tsx".to_string(),
        "src/app/api/rag/jobs/route.ts".to_string(),
        "src/modules/dashboard/index.ts".to_string(),
        "src/modules/file-manager/index.ts".to_string(),
        "src/services/users.ts".to_string(),
        "src/providers/query-provider.tsx".to_string(),
        "src/store/chat-input.store.ts".to_string(),
    ];

    let shape = detect_project_shape(
        None,
        &file_paths,
        &["package.json".to_string(), "next.config.ts".to_string()],
        &[],
    );

    assert_eq!(
        shape.primary_archetype.as_deref(),
        Some("modular_nextjs_frontend")
    );
    assert!(shape.capabilities.iter().any(|entry| entry == "app_router"));
    assert!(shape
        .capabilities
        .iter()
        .any(|entry| entry == "feature_modules"));
    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.root == "src/modules"));
    assert!(shape
        .module_contracts
        .iter()
        .any(|contract| contract.root == "src/modules"));
}

#[test]
fn renders_starter_rules_with_module_contracts() {
    let file_paths = vec![
        "src/app/layout.tsx".to_string(),
        "src/modules/dashboard/index.ts".to_string(),
        "src/modules/file-manager/index.ts".to_string(),
    ];
    let shape = detect_project_shape(
        None,
        &file_paths,
        &["package.json".to_string(), "next.config.ts".to_string()],
        &[],
    );

    let rendered = render_starter_rules(&shape, Some("typescript"), &[]);

    assert!(rendered.contains("[project]"));
    assert!(rendered.contains("archetypes = ["));
    assert!(rendered.contains("[[module_contract]]"));
    assert!(rendered.contains("root = \"src/modules\""));
    assert!(rendered.contains("# confidence: high"));
}

#[test]
fn renders_working_rules_without_commentary() {
    let file_paths = vec![
        "src/app/layout.tsx".to_string(),
        "src/modules/dashboard/index.ts".to_string(),
        "src/modules/file-manager/index.ts".to_string(),
    ];
    let shape = detect_project_shape(
        None,
        &file_paths,
        &["package.json".to_string(), "next.config.ts".to_string()],
        &[],
    );

    let rendered = render_working_rules(&shape, Some("typescript"), &[]);

    assert!(rendered.contains("[[module_contract]]"));
    assert!(rendered.contains("forbid_cross_module_deep_imports = true"));
    if shape
        .module_contracts
        .iter()
        .any(|contract| !contract.nested_public_api.is_empty())
    {
        assert!(rendered.contains("nested_public_api"));
    }
    assert!(!rendered.contains("# confidence:"));
    assert!(!rendered.contains("Candidate boundary roots"));
}

#[test]
fn detects_nextjs_from_config_and_single_feature_module() {
    let file_paths = vec![
        "src/app/layout.tsx".to_string(),
        "src/modules/dashboard/index.ts".to_string(),
    ];

    let shape = detect_project_shape(None, &file_paths, &["next.config.ts".to_string()], &[]);

    assert_eq!(
        shape.primary_archetype.as_deref(),
        Some("modular_nextjs_frontend")
    );
}

#[test]
fn detects_provider_and_query_boundary_roots() {
    let file_paths = vec![
        "src/app/layout.tsx".to_string(),
        "src/providers/auth-provider.tsx".to_string(),
        "src/contexts/organization.tsx".to_string(),
        "src/hooks/queries/use-users-queries.ts".to_string(),
    ];

    let shape = detect_project_shape(
        None,
        &file_paths,
        &["package.json".to_string(), "next.config.ts".to_string()],
        &[],
    );

    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.kind == "provider_stack" && boundary.root == "src/providers"));
    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.kind == "provider_stack" && boundary.root == "src/contexts"));
    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.kind == "query_layer" && boundary.root == "src/hooks/queries"));
}

#[test]
fn detects_layered_node_service_shape() {
    let file_paths = vec![
        "src/routes/users.ts".to_string(),
        "src/controllers/users-controller.ts".to_string(),
        "src/services/users-service.ts".to_string(),
        "src/repositories/users-repository.ts".to_string(),
        "src/middleware/auth.ts".to_string(),
    ];

    let shape = detect_project_shape(None, &file_paths, &["package.json".to_string()], &[]);

    assert_eq!(
        shape.primary_archetype.as_deref(),
        Some("layered_node_service")
    );
    assert!(shape
        .capabilities
        .iter()
        .any(|entry| entry == "http_handlers"));
    assert!(shape
        .capabilities
        .iter()
        .any(|entry| entry == "persistence_layer"));
    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.root == "src/routes"));
    assert!(shape
        .boundary_roots
        .iter()
        .any(|boundary| boundary.root == "src/repositories"));
}

#[test]
fn infers_nested_feature_module_public_api_patterns() {
    let file_paths = vec![
        "src/app/layout.tsx".to_string(),
        "src/modules/dashboard/index.ts".to_string(),
        "src/modules/dashboard/components/index.ts".to_string(),
        "src/modules/dashboard/hooks/index.ts".to_string(),
        "src/modules/users/index.ts".to_string(),
        "src/modules/users/components/index.ts".to_string(),
        "src/modules/users/hooks/index.ts".to_string(),
    ];

    let shape = detect_project_shape(
        None,
        &file_paths,
        &["package.json".to_string(), "next.config.ts".to_string()],
        &[],
    );

    let contract = shape
        .module_contracts
        .iter()
        .find(|entry| entry.id == "feature_modules")
        .expect("feature modules contract");

    assert_eq!(contract.confidence, "high");
    assert!(contract
        .nested_public_api
        .iter()
        .any(|path| path == "components/index.ts"));
    assert!(contract
        .nested_public_api
        .iter()
        .any(|path| path == "hooks/index.ts"));
}
