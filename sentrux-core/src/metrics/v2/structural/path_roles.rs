use super::graph::StructuralGraph;
use super::utils::dedupe_strings_preserve_order;
use super::{
    FileFacts, StructuralDebtReport, StructuralLeverageClass, StructuralPresentationClass,
    StructuralTrustTier,
};
use std::collections::BTreeMap;

pub(super) fn has_role(facts: &FileFacts, role: &str) -> bool {
    facts.role_tags.iter().any(|tag| tag == role)
}

pub(super) fn has_role_tag(role_tags: &[String], role: &str) -> bool {
    role_tags.iter().any(|tag| tag == role)
}

fn looks_like_entry_surface_path(path: &str) -> bool {
    path.ends_with("/index.ts")
        || path.ends_with("/index.tsx")
        || path.ends_with("/main.ts")
        || path.ends_with("/main.tsx")
        || looks_like_route_surface_path(path)
        || looks_like_service_http_surface_path(path)
        || path == "src/middleware.ts"
        || path == "src/middleware.tsx"
}

fn looks_like_tooling_path(path: &str) -> bool {
    path.starts_with("scripts/")
}

fn looks_like_route_surface_path(path: &str) -> bool {
    path.starts_with("src/app/")
        && (path.ends_with("/page.ts")
            || path.ends_with("/page.tsx")
            || path.ends_with("/layout.ts")
            || path.ends_with("/layout.tsx")
            || path.ends_with("/loading.ts")
            || path.ends_with("/loading.tsx")
            || path.ends_with("/error.ts")
            || path.ends_with("/error.tsx")
            || path.ends_with("/not-found.ts")
            || path.ends_with("/not-found.tsx"))
}

fn looks_like_api_route_surface_path(path: &str) -> bool {
    path.starts_with("src/app/api/")
        && (path.ends_with("/route.ts") || path.ends_with("/route.tsx"))
}

fn looks_like_service_http_surface_path(path: &str) -> bool {
    path.starts_with("src/routes/")
        || path.starts_with("src/controllers/")
        || path.starts_with("src/api/")
        || path.starts_with("src/server/routes/")
        || path.starts_with("src/server/controllers/")
}

fn looks_like_provider_surface_path(path: &str) -> bool {
    path == "src/app/providers.tsx"
        || path == "src/app/providers.ts"
        || path.starts_with("src/providers/")
        || path.contains("/providers/")
        || path.starts_with("src/contexts/")
        || path.contains("/contexts/")
}

fn looks_like_service_layer_path(path: &str) -> bool {
    path.starts_with("src/services/") || path.contains("/services/")
}

fn looks_like_query_hook_surface_path(path: &str) -> bool {
    path.starts_with("src/hooks/queries/")
        || path.contains("/hooks/queries/")
        || path.ends_with("-queries.ts")
        || path.ends_with("-queries.tsx")
}

fn looks_like_repository_layer_path(path: &str) -> bool {
    path.starts_with("src/repositories/")
        || path.contains("/repositories/")
        || path.starts_with("src/db/")
        || path.contains("/db/")
        || path.starts_with("src/persistence/")
        || path.contains("/persistence/")
}

fn looks_like_middleware_surface_path(path: &str) -> bool {
    path == "src/middleware.ts"
        || path == "src/middleware.tsx"
        || path.starts_with("src/middleware/")
        || path.contains("/middleware/")
}

fn looks_like_feature_module_barrel_path(path: &str) -> bool {
    path.starts_with("src/modules/")
        && (path.ends_with("/index.ts") || path.ends_with("/index.tsx"))
        && path["src/modules/".len()..].split('/').count() == 2
}

fn looks_like_state_container_path(path: &str) -> bool {
    path.starts_with("src/store/")
        || path.contains("/store/")
        || path.ends_with(".store.ts")
        || path.ends_with(".store.tsx")
}

pub(super) fn path_role_tags(path: &str) -> Vec<String> {
    let mut role_tags = Vec::new();
    if looks_like_route_surface_path(path) {
        role_tags.push("route_surface".to_string());
    }
    if looks_like_api_route_surface_path(path) {
        role_tags.push("api_route_surface".to_string());
        role_tags.push("entry_surface".to_string());
    }
    if looks_like_service_http_surface_path(path) {
        role_tags.push("http_handler_surface".to_string());
        role_tags.push("entry_surface".to_string());
    }
    if looks_like_provider_surface_path(path) {
        role_tags.push("provider_surface".to_string());
    }
    if looks_like_service_layer_path(path) {
        role_tags.push("service_layer".to_string());
    }
    if looks_like_query_hook_surface_path(path) {
        role_tags.push("query_hook_surface".to_string());
    }
    if looks_like_repository_layer_path(path) {
        role_tags.push("repository_layer".to_string());
    }
    if looks_like_middleware_surface_path(path) {
        role_tags.push("middleware_surface".to_string());
    }
    if looks_like_feature_module_barrel_path(path) {
        role_tags.push("feature_module_barrel".to_string());
    }
    if looks_like_state_container_path(path) {
        role_tags.push("state_container".to_string());
    }

    dedupe_strings_preserve_order(role_tags)
}

fn looks_like_transport_facade_path(path: &str) -> bool {
    path.contains("/ipc.")
        || path.contains("-ipc.")
        || path.ends_with("/ipc.ts")
        || path.ends_with("/ipc.tsx")
        || path.contains("/browser-http-ipc.")
}

pub(super) fn structural_presentation_class(
    kind: &str,
    path: &str,
    trust_tier: StructuralTrustTier,
    role_tags: &[String],
) -> StructuralPresentationClass {
    if trust_tier == StructuralTrustTier::Experimental {
        return StructuralPresentationClass::Experimental;
    }
    if trust_tier == StructuralTrustTier::Watchpoint
        || matches!(kind, "cycle_cluster" | "dead_island")
    {
        return StructuralPresentationClass::Watchpoint;
    }
    if looks_like_tooling_path(path) {
        return StructuralPresentationClass::ToolingDebt;
    }
    if has_role_tag(role_tags, "transport_facade") || has_role_tag(role_tags, "service_layer") {
        return StructuralPresentationClass::GuardedFacade;
    }

    StructuralPresentationClass::StructuralDebt
}

fn structural_leverage_class(report: &StructuralDebtReport) -> StructuralLeverageClass {
    if report.trust_tier == StructuralTrustTier::Experimental {
        return StructuralLeverageClass::Experimental;
    }
    if report.presentation_class == StructuralPresentationClass::ToolingDebt {
        return StructuralLeverageClass::ToolingDebt;
    }
    if report.presentation_class == StructuralPresentationClass::HardeningNote {
        return StructuralLeverageClass::HardeningNote;
    }
    if report.presentation_class == StructuralPresentationClass::GuardedFacade {
        return StructuralLeverageClass::BoundaryDiscipline;
    }
    if report.kind == "cycle_cluster" {
        if has_role_tag(&report.role_tags, "component_barrel")
            || has_role_tag(&report.role_tags, "guarded_boundary")
            || report.metrics.cut_candidate_count.unwrap_or(0) > 0
                && report.metrics.cycle_size.unwrap_or(0)
                    > report.metrics.largest_cycle_after_best_cut.unwrap_or(0)
        {
            return StructuralLeverageClass::ArchitectureSignal;
        }
        return StructuralLeverageClass::SecondaryCleanup;
    }
    if report.kind == "dead_island" {
        return StructuralLeverageClass::SecondaryCleanup;
    }
    if has_role_tag(&report.role_tags, "component_barrel")
        || has_role_tag(&report.role_tags, "guarded_boundary")
        || has_role_tag(&report.role_tags, "state_container")
        || has_role_tag(&report.role_tags, "feature_module_barrel")
    {
        return StructuralLeverageClass::ArchitectureSignal;
    }
    if has_role_tag(&report.role_tags, "composition_root")
        || has_role_tag(&report.role_tags, "entry_surface")
        || has_role_tag(&report.role_tags, "route_surface")
        || has_role_tag(&report.role_tags, "api_route_surface")
        || has_role_tag(&report.role_tags, "provider_surface")
    {
        return StructuralLeverageClass::RegrowthWatchpoint;
    }
    if has_role_tag(&report.role_tags, "facade_with_extracted_owners") {
        if extracted_owner_facade_needs_secondary_cleanup(
            report.kind.as_str(),
            &report.role_tags,
            report.metrics.line_count,
            report.metrics.max_complexity,
            report.metrics.fan_in,
        ) {
            return StructuralLeverageClass::SecondaryCleanup;
        }
        return StructuralLeverageClass::LocalRefactorTarget;
    }
    match report.kind.as_str() {
        "clone_family" | "clone_group" | "exact_clone_group" => {
            StructuralLeverageClass::SecondaryCleanup
        }
        "dependency_sprawl" | "unstable_hotspot" | "hotspot" => {
            StructuralLeverageClass::LocalRefactorTarget
        }
        _ => StructuralLeverageClass::SecondaryCleanup,
    }
}

fn structural_leverage_reasons(report: &StructuralDebtReport) -> Vec<String> {
    let mut reasons = Vec::new();
    match structural_leverage_class(report) {
        StructuralLeverageClass::Experimental => {
            reasons.push("detector_under_evaluation".to_string())
        }
        StructuralLeverageClass::ToolingDebt => {
            reasons.push("tooling_surface_maintenance_burden".to_string())
        }
        StructuralLeverageClass::HardeningNote => {
            reasons.push("narrow_completeness_gap".to_string())
        }
        StructuralLeverageClass::BoundaryDiscipline => {
            reasons.push("guarded_or_transport_facade".to_string());
            if report.metrics.fan_in.unwrap_or(0) > 0 {
                reasons.push("heavy_inbound_seam_pressure".to_string());
            }
        }
        StructuralLeverageClass::ArchitectureSignal => {
            if has_role_tag(&report.role_tags, "component_barrel") {
                reasons.push("shared_barrel_boundary_hub".to_string());
            }
            if has_role_tag(&report.role_tags, "guarded_boundary") {
                reasons.push("guardrail_backed_boundary_pressure".to_string());
            }
            if has_role_tag(&report.role_tags, "state_container") {
                reasons.push("client_state_hub_pressure".to_string());
            }
            if has_role_tag(&report.role_tags, "feature_module_barrel") {
                reasons.push("feature_module_public_api_pressure".to_string());
            }
            if report.kind == "cycle_cluster" {
                reasons.push("mixed_cycle_pressure".to_string());
                if report.metrics.cut_candidate_count.unwrap_or(0) > 0 {
                    reasons.push("high_leverage_cycle_cut".to_string());
                }
            }
            if report.metrics.fan_in.unwrap_or(0) > 0 {
                reasons.push("high_inbound_dependency_pressure".to_string());
            }
        }
        StructuralLeverageClass::RegrowthWatchpoint => {
            reasons.push("intentionally_central_surface".to_string());
            reasons.push("fan_out_regrowth_pressure".to_string());
            if has_role_tag(&report.role_tags, "route_surface")
                || has_role_tag(&report.role_tags, "api_route_surface")
            {
                reasons.push("framework_entry_surface".to_string());
            }
        }
        StructuralLeverageClass::LocalRefactorTarget => {
            if has_role_tag(&report.role_tags, "facade_with_extracted_owners") {
                reasons.push("extracted_owner_shell_pressure".to_string());
            }
            if report.metrics.guardrail_test_count.unwrap_or(0) > 0 {
                reasons.push("guardrail_backed_refactor_surface".to_string());
            }
            if is_contained_refactor_surface(
                &report.role_tags,
                report
                    .metrics
                    .fan_in
                    .or(report.metrics.inbound_reference_count),
                report.metrics.fan_out,
                report.metrics.cycle_size,
                report.metrics.guardrail_test_count,
            ) {
                reasons.push("contained_refactor_surface".to_string());
            }
            if report.metrics.fan_out.unwrap_or(0) > 0 {
                reasons.push("contained_dependency_pressure".to_string());
            }
        }
        StructuralLeverageClass::SecondaryCleanup => {
            if report.kind == "dead_island" {
                reasons.push("disconnected_internal_component".to_string());
            } else if report.kind == "cycle_cluster" {
                reasons.push("smaller_cycle_watchpoint".to_string());
            } else if has_role_tag(&report.role_tags, "query_hook_surface") {
                reasons.push("query_surface_cleanup".to_string());
            } else if has_role_tag(&report.role_tags, "facade_with_extracted_owners") {
                reasons.push("secondary_facade_cleanup".to_string());
            } else if report.kind == "large_file" {
                reasons.push("supporting_size_pressure".to_string());
            } else {
                reasons.push("real_but_lower_leverage_cleanup".to_string());
            }
        }
    }
    dedupe_strings_preserve_order(reasons)
}

fn extracted_owner_facade_needs_secondary_cleanup(
    kind: &str,
    role_tags: &[String],
    line_count: Option<usize>,
    max_complexity: Option<u32>,
    fan_in: Option<usize>,
) -> bool {
    if has_role_tag(role_tags, "entry_surface") {
        return true;
    }
    if kind == "large_file" {
        return true;
    }
    if line_count.unwrap_or(0) >= 500 {
        return true;
    }
    if max_complexity.unwrap_or(0) >= 20 {
        return true;
    }
    fan_in.unwrap_or(0) >= 20
}

fn is_contained_refactor_surface(
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    guardrail_test_count: Option<usize>,
) -> bool {
    let has_extracted_owner_surface = has_role_tag(role_tags, "facade_with_extracted_owners");
    let guardrail_count = guardrail_test_count.unwrap_or(0);
    let inbound_pressure = fan_in.unwrap_or(0);
    let dependency_breadth = fan_out.unwrap_or(0);
    let cycle_span = cycle_size.unwrap_or(0);

    (has_extracted_owner_surface || guardrail_count > 0)
        && dependency_breadth >= 3
        && (inbound_pressure == 0 || inbound_pressure <= 12)
        && (cycle_span == 0 || cycle_span <= 6)
}

pub(super) fn annotate_structural_leverage(
    mut report: StructuralDebtReport,
) -> StructuralDebtReport {
    report.leverage_class = structural_leverage_class(&report);
    report.leverage_reasons = structural_leverage_reasons(&report);
    report
}

pub(super) fn contextual_role_tags(
    path: &str,
    facts: &FileFacts,
    graph: &StructuralGraph,
    file_facts: &BTreeMap<String, FileFacts>,
) -> Vec<String> {
    let mut role_tags = facts.role_tags.clone();

    if looks_like_transport_facade_path(path) {
        role_tags.push("transport_facade".to_string());
    }

    let imported_by_entry_surface = graph.import_incoming.get(path).is_some_and(|sources| {
        sources.iter().any(|source| {
            file_facts.get(source).is_some_and(|source_facts| {
                has_role(source_facts, "entry_surface")
                    || source_facts.has_entry_tag
                    || looks_like_entry_surface_path(source)
            })
        })
    });
    if imported_by_entry_surface
        && path.starts_with("src/")
        && !looks_like_api_route_surface_path(path)
        && !looks_like_route_surface_path(path)
    {
        role_tags.push("composition_root".to_string());
    }

    dedupe_strings_preserve_order(role_tags)
}

pub(super) fn with_guardrail_evidence(facts: &FileFacts, mut evidence: Vec<String>) -> Vec<String> {
    if !facts.guardrail_tests.is_empty() {
        evidence.push(format!(
            "guardrail tests: {}",
            facts.guardrail_tests.join(", ")
        ));
    }
    if !facts.facade_owner_factories.is_empty() {
        evidence.push(format!(
            "extracted owner factories: {}",
            facts.facade_owner_factories.join(", ")
        ));
    }
    if !facts.boundary_guard_literals.is_empty() {
        evidence.push(format!(
            "guarded boundary literals: {}",
            facts.boundary_guard_literals.join(", ")
        ));
    }
    evidence
}

pub(super) fn related_structural_surfaces(
    facts: &FileFacts,
    mut related: Vec<String>,
) -> Vec<String> {
    related.extend(facts.guardrail_tests.iter().cloned());
    dedupe_strings_preserve_order(related)
}
