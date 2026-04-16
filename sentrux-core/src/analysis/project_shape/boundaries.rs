use super::{BoundaryRootSuggestion, ModuleContractSuggestion};
use std::collections::BTreeMap;

pub(super) fn detect_boundary_roots(
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

pub(super) fn detect_module_contracts(
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

fn detect_module_public_api_patterns(file_paths: &[String], root: &str) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
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
