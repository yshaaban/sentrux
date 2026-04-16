//! Generic repo-archetype and onboarding-shape detection.
//!
//! This is intentionally heuristic and evidence-first. It should help v2 adapt
//! to common TypeScript repo families without hardcoding repo names.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProjectArchetypeMatch {
    pub id: String,
    pub confidence: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BoundaryRootSuggestion {
    pub kind: String,
    pub root: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleContractSuggestion {
    pub id: String,
    pub root: String,
    pub public_api: Vec<String>,
    pub nested_public_api: Vec<String>,
    pub confidence: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProjectShapeReport {
    pub configured_archetypes: Vec<String>,
    pub detected_archetypes: Vec<ProjectArchetypeMatch>,
    pub effective_archetypes: Vec<String>,
    pub primary_archetype: Option<String>,
    pub capabilities: Vec<String>,
    pub boundary_roots: Vec<BoundaryRootSuggestion>,
    pub module_contracts: Vec<ModuleContractSuggestion>,
}

#[derive(Debug, Clone, Default)]
struct PackageManifestSignals {
    has_next: bool,
    has_react: bool,
    has_react_query: bool,
    has_zustand: bool,
}

pub fn detect_project_shape(
    root: Option<&Path>,
    file_paths: &[String],
    workspace_files: &[String],
    configured_archetypes: &[String],
) -> ProjectShapeReport {
    let package_signals = root
        .and_then(read_package_manifest_signals)
        .unwrap_or_default();
    let capabilities = detect_capabilities(file_paths, workspace_files, &package_signals);
    let detected_archetypes = detect_archetypes(
        file_paths,
        &capabilities,
        &package_signals,
        configured_archetypes,
    );
    let boundary_roots = detect_boundary_roots(file_paths, &capabilities);
    let module_contracts = detect_module_contracts(&boundary_roots, file_paths);
    let effective_archetypes = effective_archetypes(configured_archetypes, &detected_archetypes);
    let primary_archetype = effective_archetypes.first().cloned();

    ProjectShapeReport {
        configured_archetypes: configured_archetypes.to_vec(),
        detected_archetypes,
        effective_archetypes,
        primary_archetype,
        capabilities,
        boundary_roots,
        module_contracts,
    }
}

pub fn render_starter_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    let mut lines = render_base_rules_lines(shape, primary_language, existing_excludes);
    for contract in &shape.module_contracts {
        push_module_contract_lines(&mut lines, contract, false, true);
    }

    if !shape.boundary_roots.is_empty() {
        lines.push(String::new());
        lines.push("# Candidate boundary roots detected from repo shape:".to_string());
        for boundary in &shape.boundary_roots {
            lines.push(format!(
                "# - {} at {} ({})",
                boundary.kind,
                boundary.root,
                boundary.evidence.join(", ")
            ));
        }
    }

    lines.join("\n") + "\n"
}

pub fn render_working_rules(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> String {
    let mut lines = render_base_rules_lines(shape, primary_language, existing_excludes);
    for contract in &shape.module_contracts {
        push_module_contract_lines(&mut lines, contract, true, false);
    }
    lines.join("\n") + "\n"
}

fn push_module_contract_lines(
    lines: &mut Vec<String>,
    contract: &ModuleContractSuggestion,
    include_nested_public_api: bool,
    include_comments: bool,
) {
    lines.push(String::new());
    lines.push("[[module_contract]]".to_string());
    lines.push(format!("id = {:?}", contract.id));
    lines.push(format!("root = {:?}", contract.root));
    lines.push(format!(
        "public_api = [{}]",
        quoted_list(&contract.public_api)
    ));
    if include_nested_public_api && !contract.nested_public_api.is_empty() {
        lines.push(format!(
            "nested_public_api = [{}]",
            quoted_list(&contract.nested_public_api)
        ));
    }
    lines.push("forbid_cross_module_deep_imports = true".to_string());
    if include_comments {
        lines.push(format!("# confidence: {}", contract.confidence));
        if !contract.nested_public_api.is_empty() {
            lines.push(format!(
                "# observed nested public APIs: {}",
                contract.nested_public_api.join(", ")
            ));
        }
    }
}

fn render_base_rules_lines(
    shape: &ProjectShapeReport,
    primary_language: Option<&str>,
    existing_excludes: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut excludes = existing_excludes.to_vec();
    excludes.extend(suggest_excludes(shape));
    excludes = dedupe_strings(excludes);

    lines.push("[project]".to_string());
    if let Some(language) = primary_language {
        lines.push(format!("primary_language = {:?}", language));
    }
    if !shape.effective_archetypes.is_empty() {
        lines.push(format!(
            "archetypes = [{}]",
            quoted_list(&shape.effective_archetypes)
        ));
    }
    if !excludes.is_empty() {
        lines.push(format!("exclude = [{}]", quoted_list(&excludes)));
    }
    lines
}

fn read_package_manifest_signals(root: &Path) -> Option<PackageManifestSignals> {
    let package_json = root.join("package.json");
    let source = std::fs::read_to_string(package_json).ok()?;
    let payload = serde_json::from_str::<serde_json::Value>(&source).ok()?;

    let mut dependencies = BTreeSet::new();
    collect_manifest_keys(payload.get("dependencies"), &mut dependencies);
    collect_manifest_keys(payload.get("devDependencies"), &mut dependencies);

    Some(PackageManifestSignals {
        has_next: dependencies.contains("next"),
        has_react: dependencies.contains("react"),
        has_react_query: dependencies.contains("@tanstack/react-query"),
        has_zustand: dependencies.contains("zustand"),
    })
}

fn collect_manifest_keys(section: Option<&serde_json::Value>, dependencies: &mut BTreeSet<String>) {
    let Some(section) = section.and_then(|value| value.as_object()) else {
        return;
    };
    for key in section.keys() {
        dependencies.insert(key.to_string());
    }
}

fn any_matching_path(file_paths: &[String], predicate: impl Fn(&str) -> bool) -> bool {
    file_paths.iter().any(|path| predicate(path))
}

fn has_http_handlers(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/routes/")
            || path.starts_with("src/controllers/")
            || path.starts_with("src/api/")
            || path.starts_with("src/server/routes/")
            || path.starts_with("src/server/controllers/")
    })
}

fn has_service_layer(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/services/") || path.contains("/services/")
    })
}

fn has_provider_stack(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path == "src/app/providers.tsx"
            || path.starts_with("src/providers/")
            || path.contains("/providers/")
            || path.starts_with("src/contexts/")
            || path.contains("/contexts/")
    })
}

fn has_state_layer(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/store/")
            || path.contains("/store/")
            || path.ends_with(".store.ts")
            || path.ends_with(".store.tsx")
    })
}

fn has_query_layer(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/hooks/queries/")
            || path.contains("/hooks/queries/")
            || path.contains("/controllers/use-")
            || path.contains("/hooks/use-")
    })
}

fn has_persistence_layer(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/repositories/")
            || path.starts_with("src/db/")
            || path.starts_with("src/persistence/")
            || path.contains("/repositories/")
            || path.contains("/persistence/")
    })
}

fn has_middleware_stack(file_paths: &[String]) -> bool {
    any_matching_path(file_paths, |path| {
        path.starts_with("src/middleware/")
            || path.contains("/middleware/")
            || path.ends_with("/middleware.ts")
            || path.ends_with("/middleware.tsx")
    })
}

fn push_capability_if(capabilities: &mut Vec<String>, enabled: bool, capability: &str) {
    if enabled {
        capabilities.push(capability.to_string());
    }
}

fn detect_capabilities(
    file_paths: &[String],
    workspace_files: &[String],
    package_signals: &PackageManifestSignals,
) -> Vec<String> {
    let mut capabilities = Vec::new();
    let has_src_app = any_matching_path(file_paths, |path| path.starts_with("src/app/"));
    let has_app_router_entries = any_matching_path(file_paths, is_app_router_entry_path);
    let has_api_routes = any_matching_path(file_paths, |path| {
        path.starts_with("src/app/api/") && is_route_handler_path(path)
    });
    let has_modules_root = any_matching_path(file_paths, |path| path.starts_with("src/modules/"));
    let feature_module_count = feature_module_names(file_paths).len();
    let has_localized_routing =
        any_matching_path(file_paths, |path| path.starts_with("src/app/[locale]/"));

    push_capability_if(
        &mut capabilities,
        package_signals.has_next
            || workspace_files
                .iter()
                .any(|path| path.starts_with("next.config")),
        "nextjs",
    );
    push_capability_if(&mut capabilities, package_signals.has_react, "react");
    push_capability_if(
        &mut capabilities,
        has_src_app && has_app_router_entries,
        "app_router",
    );
    push_capability_if(&mut capabilities, has_api_routes, "api_routes");
    push_capability_if(
        &mut capabilities,
        has_http_handlers(file_paths),
        "http_handlers",
    );
    push_capability_if(
        &mut capabilities,
        has_modules_root && feature_module_count >= 1,
        "feature_modules",
    );
    push_capability_if(
        &mut capabilities,
        has_service_layer(file_paths),
        "service_layer",
    );
    push_capability_if(
        &mut capabilities,
        has_provider_stack(file_paths),
        "provider_stack",
    );
    push_capability_if(
        &mut capabilities,
        has_state_layer(file_paths) || package_signals.has_zustand,
        "client_state",
    );
    push_capability_if(
        &mut capabilities,
        has_query_layer(file_paths) || package_signals.has_react_query,
        "query_layer",
    );
    push_capability_if(
        &mut capabilities,
        has_persistence_layer(file_paths),
        "persistence_layer",
    );
    push_capability_if(
        &mut capabilities,
        has_middleware_stack(file_paths),
        "middleware_stack",
    );
    push_capability_if(
        &mut capabilities,
        has_localized_routing,
        "localized_routing",
    );

    dedupe_strings(capabilities)
}

fn detect_archetypes(
    file_paths: &[String],
    capabilities: &[String],
    package_signals: &PackageManifestSignals,
    configured_archetypes: &[String],
) -> Vec<ProjectArchetypeMatch> {
    let mut archetypes = Vec::new();
    let has = |capability: &str| capabilities.iter().any(|entry| entry == capability);

    if has("nextjs") && has("app_router") && has("feature_modules") {
        archetypes.push(ProjectArchetypeMatch {
            id: "modular_nextjs_frontend".to_string(),
            confidence: "high".to_string(),
            reasons: dedupe_strings(vec![
                "nextjs_runtime_detected".to_string(),
                "app_router_entries_detected".to_string(),
                "feature_module_packages_detected".to_string(),
            ]),
        });
    } else if has("nextjs") && has("app_router") {
        archetypes.push(ProjectArchetypeMatch {
            id: "nextjs_app_router_frontend".to_string(),
            confidence: "high".to_string(),
            reasons: dedupe_strings(vec![
                "nextjs_runtime_detected".to_string(),
                "app_router_entries_detected".to_string(),
            ]),
        });
    } else if has("feature_modules") && package_signals.has_react {
        archetypes.push(ProjectArchetypeMatch {
            id: "modular_react_frontend".to_string(),
            confidence: "medium".to_string(),
            reasons: dedupe_strings(vec![
                "react_runtime_detected".to_string(),
                "feature_module_packages_detected".to_string(),
            ]),
        });
    }

    if !has("app_router") && has("service_layer") && has("http_handlers") {
        archetypes.push(ProjectArchetypeMatch {
            id: "layered_node_service".to_string(),
            confidence: if has("persistence_layer") || has("middleware_stack") {
                "high".to_string()
            } else {
                "medium".to_string()
            },
            reasons: dedupe_strings(vec![
                "http_handlers_detected".to_string(),
                "service_layer_detected".to_string(),
                has("persistence_layer")
                    .then(|| "persistence_layer_detected".to_string())
                    .unwrap_or_default(),
                has("middleware_stack")
                    .then(|| "middleware_stack_detected".to_string())
                    .unwrap_or_default(),
            ]),
        });
    } else if !has("app_router") && has("http_handlers") {
        archetypes.push(ProjectArchetypeMatch {
            id: "node_service".to_string(),
            confidence: "medium".to_string(),
            reasons: dedupe_strings(vec![
                "http_handlers_detected".to_string(),
                has("middleware_stack")
                    .then(|| "middleware_stack_detected".to_string())
                    .unwrap_or_default(),
            ]),
        });
    }

    if configured_archetypes.is_empty() && package_signals.has_react && !file_paths.is_empty() {
        archetypes.push(ProjectArchetypeMatch {
            id: "react_frontend".to_string(),
            confidence: "medium".to_string(),
            reasons: vec!["react_runtime_detected".to_string()],
        });
    }

    dedupe_archetypes(archetypes)
}

fn detect_boundary_roots(
    file_paths: &[String],
    capabilities: &[String],
) -> Vec<BoundaryRootSuggestion> {
    let mut roots = Vec::new();
    for (capability, kind, root, evidence) in capability_boundary_root_specs() {
        push_capability_boundary_root(&mut roots, capabilities, capability, kind, root, evidence);
    }
    for (prefix, kind, root, evidence) in default_path_boundary_root_specs() {
        push_path_boundary_root(&mut roots, file_paths, prefix, kind, root, evidence);
    }
    if has_capability(capabilities, "provider_stack") {
        for (prefix, root, evidence) in provider_stack_boundary_root_specs() {
            push_path_boundary_root(
                &mut roots,
                file_paths,
                prefix,
                "provider_stack",
                root,
                evidence,
            );
        }
    }
    if has_capability(capabilities, "query_layer") {
        push_path_boundary_root(
            &mut roots,
            file_paths,
            "src/hooks/queries/",
            "query_layer",
            "src/hooks/queries",
            &["top-level query hook layer detected"],
        );
    }
    for (prefix, kind, root, evidence) in persistence_boundary_root_specs() {
        push_path_boundary_root(&mut roots, file_paths, prefix, kind, root, evidence);
    }
    roots
}

fn capability_boundary_root_specs() -> [(
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
); 2] {
    [
        (
            "feature_modules",
            "feature_modules",
            "src/modules",
            &[
                "feature-module barrels detected",
                "cross-module public API likely lives in index.ts",
            ],
        ),
        (
            "api_routes",
            "api_routes",
            "src/app/api",
            &["Next.js route handlers detected"],
        ),
    ]
}

fn default_path_boundary_root_specs() -> [(
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
); 4] {
    [
        (
            "src/routes/",
            "http_handlers",
            "src/routes",
            &["top-level route handlers detected"],
        ),
        (
            "src/controllers/",
            "http_handlers",
            "src/controllers",
            &["top-level controller layer detected"],
        ),
        (
            "src/services/",
            "service_layer",
            "src/services",
            &["top-level service layer detected"],
        ),
        (
            "src/store/",
            "client_state",
            "src/store",
            &["top-level client state layer detected"],
        ),
    ]
}

fn provider_stack_boundary_root_specs() -> [(&'static str, &'static str, &'static [&'static str]); 2]
{
    [
        (
            "src/providers/",
            "src/providers",
            &["top-level provider stack detected"],
        ),
        (
            "src/contexts/",
            "src/contexts",
            &["top-level shared context layer detected"],
        ),
    ]
}

fn persistence_boundary_root_specs() -> [(
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
); 3] {
    [
        (
            "src/repositories/",
            "persistence_layer",
            "src/repositories",
            &["top-level repository layer detected"],
        ),
        (
            "src/db/",
            "persistence_layer",
            "src/db",
            &["top-level database layer detected"],
        ),
        (
            "src/middleware/",
            "middleware_stack",
            "src/middleware",
            &["top-level middleware stack detected"],
        ),
    ]
}

fn push_capability_boundary_root(
    roots: &mut Vec<BoundaryRootSuggestion>,
    capabilities: &[String],
    capability: &str,
    kind: &str,
    root: &str,
    evidence: &[&str],
) {
    if !has_capability(capabilities, capability) {
        return;
    }

    push_boundary_root(roots, kind, root, evidence);
}

fn push_path_boundary_root(
    roots: &mut Vec<BoundaryRootSuggestion>,
    file_paths: &[String],
    prefix: &str,
    kind: &str,
    root: &str,
    evidence: &[&str],
) {
    if !has_path_prefix(file_paths, prefix) {
        return;
    }

    push_boundary_root(roots, kind, root, evidence);
}

fn push_boundary_root(
    roots: &mut Vec<BoundaryRootSuggestion>,
    kind: &str,
    root: &str,
    evidence: &[&str],
) {
    roots.push(BoundaryRootSuggestion {
        kind: kind.to_string(),
        root: root.to_string(),
        evidence: evidence.iter().map(|entry| (*entry).to_string()).collect(),
    });
}

fn has_capability(capabilities: &[String], capability: &str) -> bool {
    capabilities.iter().any(|entry| entry == capability)
}

fn has_path_prefix(file_paths: &[String], prefix: &str) -> bool {
    file_paths.iter().any(|path| path.starts_with(prefix))
}

fn detect_module_contracts(
    boundary_roots: &[BoundaryRootSuggestion],
    file_paths: &[String],
) -> Vec<ModuleContractSuggestion> {
    boundary_roots
        .iter()
        .filter(|boundary| boundary.kind == "feature_modules")
        .map(|boundary| ModuleContractSuggestion {
            id: "feature_modules".to_string(),
            root: boundary.root.clone(),
            public_api: vec!["index.ts".to_string(), "index.tsx".to_string()],
            nested_public_api: detect_module_public_api_patterns(file_paths, &boundary.root),
            confidence: "high".to_string(),
            evidence: boundary.evidence.clone(),
        })
        .collect()
}

fn effective_archetypes(
    configured_archetypes: &[String],
    detected_archetypes: &[ProjectArchetypeMatch],
) -> Vec<String> {
    let mut effective = configured_archetypes.to_vec();
    effective.extend(
        detected_archetypes
            .iter()
            .map(|archetype| archetype.id.clone()),
    );
    dedupe_strings(effective)
}

fn suggest_excludes(shape: &ProjectShapeReport) -> Vec<String> {
    let mut excludes = vec!["node_modules/**".to_string(), "coverage/**".to_string()];

    if shape
        .effective_archetypes
        .iter()
        .any(|entry| entry.contains("nextjs"))
    {
        excludes.push(".next/**".to_string());
    }
    if shape
        .capabilities
        .iter()
        .any(|entry| entry == "feature_modules")
    {
        excludes.push("tmp/**".to_string());
    }

    excludes
}

fn feature_module_names(file_paths: &[String]) -> BTreeSet<String> {
    file_paths
        .iter()
        .filter_map(|path| {
            let remainder = path.strip_prefix("src/modules/")?;
            let module_name = remainder.split('/').next()?;
            (!module_name.is_empty()).then(|| module_name.to_string())
        })
        .collect()
}

fn detect_module_public_api_patterns(file_paths: &[String], root: &str) -> Vec<String> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let root_prefix = format!("{root}/");

    for path in file_paths {
        let Some(remainder) = path.strip_prefix(&root_prefix) else {
            continue;
        };
        let mut parts = remainder.split('/');
        let Some(_module_name) = parts.next() else {
            continue;
        };
        let nested = parts.collect::<Vec<_>>();
        if nested.len() < 2 {
            continue;
        }
        let nested_path = nested.join("/");
        if nested_path.ends_with("index.ts") || nested_path.ends_with("index.tsx") {
            *counts.entry(nested_path).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .filter_map(|(pattern, count)| (count >= 2).then_some(pattern))
        .collect()
}

fn is_app_router_entry_path(path: &str) -> bool {
    path.starts_with("src/app/")
        && (path.ends_with("/page.tsx")
            || path.ends_with("/page.ts")
            || path.ends_with("/layout.tsx")
            || path.ends_with("/layout.ts")
            || path.ends_with("/loading.tsx")
            || path.ends_with("/loading.ts")
            || path.ends_with("/error.tsx")
            || path.ends_with("/error.ts")
            || path.ends_with("/not-found.tsx")
            || path.ends_with("/not-found.ts")
            || is_route_handler_path(path))
}

fn is_route_handler_path(path: &str) -> bool {
    path.ends_with("/route.ts") || path.ends_with("/route.tsx")
}

fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn dedupe_archetypes(values: Vec<ProjectArchetypeMatch>) -> Vec<ProjectArchetypeMatch> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.id.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::{detect_project_shape, render_starter_rules, render_working_rules};

    #[test]
    fn detects_modular_nextjs_frontend_shape() {
        let file_paths = vec![
            "src/app/[locale]/layout.tsx".to_string(),
            "src/app/api/rag/jobs/route.ts".to_string(),
            "src/modules/home/index.ts".to_string(),
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
            "src/modules/home/index.ts".to_string(),
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
            "src/modules/home/index.ts".to_string(),
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
            "src/modules/home/index.ts".to_string(),
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
        assert!(
            shape
                .boundary_roots
                .iter()
                .any(|boundary| boundary.kind == "query_layer"
                    && boundary.root == "src/hooks/queries")
        );
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
            "src/modules/home/index.ts".to_string(),
            "src/modules/home/components/index.ts".to_string(),
            "src/modules/home/hooks/index.ts".to_string(),
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
}
