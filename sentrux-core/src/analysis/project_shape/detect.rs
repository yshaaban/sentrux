use super::{boundaries, dedupe_strings, ProjectArchetypeMatch, ProjectShapeReport};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Default)]
struct PackageManifestSignals {
    has_next: bool,
    has_react: bool,
    has_react_query: bool,
    has_zustand: bool,
}

pub(super) fn detect_project_shape(
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
    let boundary_roots = boundaries::detect_boundary_roots(file_paths, &capabilities);
    let module_contracts = boundaries::detect_module_contracts(&boundary_roots, file_paths);
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
