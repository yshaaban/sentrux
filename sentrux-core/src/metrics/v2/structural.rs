//! Structural debt reports built from existing health metrics and snapshot facts.

use crate::analysis::lang_registry;
use crate::core::snapshot::{flatten_files_ref, Snapshot};
use crate::core::types::{FileNode, ImportEdge};
use crate::metrics::testgap::is_test_file;
use crate::metrics::{is_mod_declaration_edge, is_package_index_for_path, HealthReport};
use ignore::WalkBuilder;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_out: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instability_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_symbol_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_reference_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_surface_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reachable_from_tests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_cycle_after_best_cut: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_test_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_count: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CycleCutCandidate {
    pub source: String,
    pub target: String,
    pub seam_kind: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub evidence: Vec<String>,
    pub reduction_file_count: usize,
    pub remaining_cycle_size: usize,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtReport {
    pub kind: String,
    pub trust_tier: String,
    pub presentation_class: String,
    pub leverage_class: String,
    pub scope: String,
    pub signal_class: String,
    pub signal_families: Vec<String>,
    pub severity: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub impact: String,
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leverage_reasons: Vec<String>,
    pub evidence: Vec<String>,
    pub inspection_focus: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_split_axes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_surfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cut_candidates: Vec<CycleCutCandidate>,
    pub metrics: StructuralDebtMetrics,
}

#[derive(Debug, Clone, Default)]
struct FileFacts {
    lang: String,
    lines: usize,
    function_count: u32,
    max_complexity: u32,
    is_test: bool,
    is_package_index: bool,
    has_entry_tag: bool,
    public_function_count: usize,
    role_tags: Vec<String>,
    guardrail_tests: Vec<String>,
    facade_owner_factories: Vec<String>,
    boundary_guard_literals: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct GuardrailFileEvidence {
    tests: Vec<String>,
    required_literals: Vec<String>,
    forbidden_literals: Vec<String>,
    facade_owner_factories: Vec<String>,
    boundary_guard_literals: Vec<String>,
}

#[derive(Debug, Default)]
struct StructuralGraph {
    outgoing: BTreeMap<String, BTreeSet<String>>,
    incoming: BTreeMap<String, BTreeSet<String>>,
    import_outgoing: BTreeMap<String, BTreeSet<String>>,
    import_incoming: BTreeMap<String, BTreeSet<String>>,
}

pub fn build_structural_debt_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
) -> Vec<StructuralDebtReport> {
    build_structural_debt_reports_internal(snapshot, health, None)
}

pub fn build_structural_debt_reports_with_root(
    root: &Path,
    snapshot: &Snapshot,
    health: &HealthReport,
) -> Vec<StructuralDebtReport> {
    build_structural_debt_reports_internal(snapshot, health, Some(root))
}

fn build_structural_debt_reports_internal(
    snapshot: &Snapshot,
    health: &HealthReport,
    root: Option<&Path>,
) -> Vec<StructuralDebtReport> {
    let file_facts = build_file_facts(snapshot, root);
    let graph = build_structural_graph(snapshot);
    let mut reports = Vec::new();

    reports.extend(build_large_file_reports(health, &file_facts, &graph));
    reports.extend(build_dependency_sprawl_reports(health, &file_facts, &graph));
    reports.extend(build_unstable_hotspot_reports(health, &file_facts, &graph));
    reports.extend(build_cycle_cluster_reports(health, &file_facts, &graph));
    reports.extend(build_dead_private_code_cluster_reports(health, &file_facts));
    reports.extend(build_dead_island_reports(
        snapshot,
        health,
        &file_facts,
        &graph,
    ));

    reports.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    reports
}

fn build_file_facts(snapshot: &Snapshot, root: Option<&Path>) -> BTreeMap<String, FileFacts> {
    let entry_surface_paths = snapshot
        .entry_points
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<BTreeSet<_>>();
    let file_paths = flatten_files_ref(&snapshot.root)
        .into_iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let guardrail_evidence = root
        .map(|root| detect_architecture_guardrails(root, &file_paths))
        .unwrap_or_default();

    flatten_files_ref(&snapshot.root)
        .into_iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| {
            let evidence = guardrail_evidence.get(&file.path);
            (
                file.path.clone(),
                file_facts(file, entry_surface_paths.contains(&file.path), evidence),
            )
        })
        .collect()
}

fn file_facts(
    file: &FileNode,
    is_entry_surface: bool,
    guardrail_evidence: Option<&GuardrailFileEvidence>,
) -> FileFacts {
    let max_complexity = file
        .sa
        .as_ref()
        .and_then(|analysis| analysis.functions.as_ref())
        .map(|functions| {
            functions
                .iter()
                .map(|function| function.cc.unwrap_or(0).max(function.cog.unwrap_or(0)))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let mut role_tags = Vec::new();
    if is_entry_surface {
        role_tags.push("entry_surface".to_string());
    }
    if let Some(evidence) = guardrail_evidence {
        if !evidence.tests.is_empty() {
            role_tags.push("guarded_seam".to_string());
        }
        if evidence.facade_owner_factories.len() >= 2 {
            role_tags.push("facade_with_extracted_owners".to_string());
        }
        if !evidence.boundary_guard_literals.is_empty() {
            role_tags.push("guarded_boundary".to_string());
        }
    }
    if file.path.contains("store/store.") && role_tags.iter().any(|tag| tag == "guarded_boundary") {
        role_tags.push("component_barrel".to_string());
    }
    role_tags.extend(path_role_tags(&file.path));

    FileFacts {
        lang: file.lang.clone(),
        lines: file.lines as usize,
        function_count: file.funcs,
        max_complexity,
        is_test: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag.contains("test")))
            || is_test_file(&file.path),
        is_package_index: is_package_index_for_path(&file.path),
        has_entry_tag: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag == "entry")),
        public_function_count: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.functions.as_ref())
            .map(|functions| {
                functions
                    .iter()
                    .filter(|function| function.is_public)
                    .count()
            })
            .unwrap_or(0),
        role_tags: dedupe_strings_preserve_order(role_tags),
        guardrail_tests: guardrail_evidence
            .map(|evidence| evidence.tests.clone())
            .unwrap_or_default(),
        facade_owner_factories: guardrail_evidence
            .map(|evidence| evidence.facade_owner_factories.clone())
            .unwrap_or_default(),
        boundary_guard_literals: guardrail_evidence
            .map(|evidence| evidence.boundary_guard_literals.clone())
            .unwrap_or_default(),
    }
}

fn build_structural_graph(snapshot: &Snapshot) -> StructuralGraph {
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut import_outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut import_incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut seen = HashSet::<(String, String)>::new();
    let mut import_seen = HashSet::<(String, String)>::new();

    for edge in filtered_import_edges(snapshot) {
        record_graph_edge(
            &mut outgoing,
            &mut incoming,
            &mut seen,
            &edge.from_file,
            &edge.to_file,
        );
        record_graph_edge(
            &mut import_outgoing,
            &mut import_incoming,
            &mut import_seen,
            &edge.from_file,
            &edge.to_file,
        );
    }
    for edge in &snapshot.call_graph {
        record_graph_edge(
            &mut outgoing,
            &mut incoming,
            &mut seen,
            &edge.from_file,
            &edge.to_file,
        );
    }
    StructuralGraph {
        outgoing,
        incoming,
        import_outgoing,
        import_incoming,
    }
}

fn filtered_import_edges(snapshot: &Snapshot) -> impl Iterator<Item = &ImportEdge> {
    snapshot
        .import_graph
        .iter()
        .filter(|edge| !is_mod_declaration_edge(edge))
}

fn record_graph_edge(
    outgoing: &mut BTreeMap<String, BTreeSet<String>>,
    incoming: &mut BTreeMap<String, BTreeSet<String>>,
    seen: &mut HashSet<(String, String)>,
    from_file: &str,
    to_file: &str,
) {
    let pair = (from_file.to_string(), to_file.to_string());
    if !seen.insert(pair.clone()) {
        return;
    }

    outgoing
        .entry(pair.0.clone())
        .or_default()
        .insert(pair.1.clone());
    incoming.entry(pair.1).or_default().insert(pair.0);
}

fn detect_architecture_guardrails(
    root: &Path,
    file_paths: &BTreeSet<String>,
) -> BTreeMap<String, GuardrailFileEvidence> {
    let mut evidence_by_file = BTreeMap::<String, GuardrailFileEvidence>::new();

    for (test_path, contents) in walk_guardrail_test_sources(root) {
        let named_targets = named_guardrail_targets(&test_path, file_paths);
        let explicit_targets = explicit_guardrail_targets(&contents, file_paths);
        let required_literals = test_contains_literals(&contents, false);
        let forbidden_literals = test_contains_literals(&contents, true);
        let facade_owner_factories = required_literals
            .iter()
            .filter(|literal| is_facade_owner_factory(literal))
            .cloned()
            .collect::<Vec<_>>();
        let boundary_targets = forbidden_literals
            .iter()
            .flat_map(|literal| resolve_module_literal_targets(literal, file_paths))
            .collect::<Vec<_>>();

        for target in named_targets
            .into_iter()
            .chain(explicit_targets.into_iter())
            .collect::<BTreeSet<_>>()
        {
            let entry = evidence_by_file.entry(target).or_default();
            entry.tests.push(test_path.clone());
            entry.required_literals.extend(required_literals.clone());
            entry.forbidden_literals.extend(forbidden_literals.clone());
            entry
                .facade_owner_factories
                .extend(facade_owner_factories.clone());
        }

        for target in boundary_targets {
            let entry = evidence_by_file.entry(target).or_default();
            entry.tests.push(test_path.clone());
            entry
                .boundary_guard_literals
                .extend(forbidden_literals.clone());
        }
    }

    for evidence in evidence_by_file.values_mut() {
        evidence.tests = dedupe_strings_preserve_order(std::mem::take(&mut evidence.tests));
        evidence.required_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.required_literals));
        evidence.forbidden_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.forbidden_literals));
        evidence.facade_owner_factories =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.facade_owner_factories));
        evidence.boundary_guard_literals =
            dedupe_strings_preserve_order(std::mem::take(&mut evidence.boundary_guard_literals));
    }

    evidence_by_file
}

fn walk_guardrail_test_sources(root: &Path) -> Vec<(String, String)> {
    let mut sources = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.into_path();
            if !path.is_file() {
                return None;
            }
            let relative_path = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if !is_guardrail_test_path(&relative_path) {
                return None;
            }
            let contents = std::fs::read_to_string(&path).ok()?;
            Some((relative_path, contents))
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

fn is_guardrail_test_path(path: &str) -> bool {
    path.ends_with(".architecture.test.ts")
        || path.ends_with(".architecture.test.tsx")
        || path.ends_with(".architecture.spec.ts")
        || path.ends_with(".architecture.spec.tsx")
}

fn named_guardrail_targets(test_path: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    let Some(file_name) = test_path.rsplit('/').next() else {
        return Vec::new();
    };
    let Some(stem) = file_name
        .strip_suffix(".architecture.test.ts")
        .or_else(|| file_name.strip_suffix(".architecture.test.tsx"))
        .or_else(|| file_name.strip_suffix(".architecture.spec.ts"))
        .or_else(|| file_name.strip_suffix(".architecture.spec.tsx"))
    else {
        return Vec::new();
    };
    let directory = test_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or_default();
    [".ts", ".tsx", ".js", ".jsx"]
        .into_iter()
        .map(|extension| {
            if directory.is_empty() {
                format!("{stem}{extension}")
            } else {
                format!("{directory}/{stem}{extension}")
            }
        })
        .filter(|candidate| file_paths.contains(candidate))
        .collect()
}

fn explicit_guardrail_targets(contents: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    quoted_literals(contents)
        .into_iter()
        .filter(|literal| looks_like_source_file_path(literal))
        .map(|literal| literal.trim_start_matches("./").replace('\\', "/"))
        .filter(|literal| file_paths.contains(literal))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn test_contains_literals(contents: &str, negated: bool) -> Vec<String> {
    let mut literals = Vec::new();
    let needle = if negated {
        ".not.toContain("
    } else {
        ".toContain("
    };
    let mut cursor = 0;
    while let Some(offset) = contents[cursor..].find(needle) {
        let start = cursor + offset + needle.len();
        let suffix = &contents[start..];
        let Some(quote) = suffix.chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' && quote != '`' {
            cursor = start;
            continue;
        }
        if let Some((literal, advance)) = quoted_literal_after(contents, start) {
            literals.push(literal);
            cursor = advance;
            continue;
        }
        cursor = start + 1;
    }
    dedupe_strings_preserve_order(literals)
}

fn resolve_module_literal_targets(literal: &str, file_paths: &BTreeSet<String>) -> Vec<String> {
    if !looks_like_import_fragment(literal) {
        return Vec::new();
    }

    file_paths
        .iter()
        .filter(|path| path_without_extension(path).ends_with(literal))
        .cloned()
        .collect()
}

fn looks_like_source_file_path(literal: &str) -> bool {
    (literal.ends_with(".ts")
        || literal.ends_with(".tsx")
        || literal.ends_with(".js")
        || literal.ends_with(".jsx"))
        && literal.contains('/')
}

fn looks_like_import_fragment(literal: &str) -> bool {
    literal.contains('/')
        && !literal.contains(' ')
        && !literal.ends_with(".ts")
        && !literal.ends_with(".tsx")
        && !literal.ends_with(".js")
        && !literal.ends_with(".jsx")
}

fn path_without_extension(path: &str) -> &str {
    path.strip_suffix(".tsx")
        .or_else(|| path.strip_suffix(".ts"))
        .or_else(|| path.strip_suffix(".jsx"))
        .or_else(|| path.strip_suffix(".js"))
        .unwrap_or(path)
}

fn is_facade_owner_factory(literal: &str) -> bool {
    literal.starts_with("create")
        && literal
            .chars()
            .nth(6)
            .is_some_and(|character| character.is_ascii_uppercase())
}

fn quoted_literals(contents: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut index = 0;
    while index < contents.len() {
        if let Some((literal, advance)) = quoted_literal_after(contents, index) {
            literals.push(literal);
            index = advance;
        } else {
            index += 1;
        }
    }
    literals
}

fn quoted_literal_after(contents: &str, start: usize) -> Option<(String, usize)> {
    let suffix = contents.get(start..)?;
    let quote = suffix.chars().next()?;
    if quote != '"' && quote != '\'' && quote != '`' {
        return None;
    }

    let mut escaped = false;
    let mut literal = String::new();
    for (offset, character) in suffix.char_indices().skip(1) {
        if escaped {
            literal.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            return Some((literal, start + offset + character.len_utf8()));
        }
        literal.push(character);
    }

    None
}

fn has_role(facts: &FileFacts, role: &str) -> bool {
    facts.role_tags.iter().any(|tag| tag == role)
}

fn has_role_tag(role_tags: &[String], role: &str) -> bool {
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

fn path_role_tags(path: &str) -> Vec<String> {
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

fn structural_presentation_class(
    kind: &str,
    path: &str,
    trust_tier: &str,
    role_tags: &[String],
) -> String {
    if trust_tier == "experimental" {
        return "experimental".to_string();
    }
    if trust_tier == "watchpoint" || matches!(kind, "cycle_cluster" | "dead_island") {
        return "watchpoint".to_string();
    }
    if looks_like_tooling_path(path) {
        return "tooling_debt".to_string();
    }
    if has_role_tag(role_tags, "transport_facade") || has_role_tag(role_tags, "service_layer") {
        return "guarded_facade".to_string();
    }

    "structural_debt".to_string()
}

fn structural_leverage_class(report: &StructuralDebtReport) -> String {
    if report.trust_tier == "experimental" {
        return "experimental".to_string();
    }
    if report.presentation_class == "tooling_debt" {
        return "tooling_debt".to_string();
    }
    if report.presentation_class == "hardening_note" {
        return "hardening_note".to_string();
    }
    if report.presentation_class == "guarded_facade" {
        return "boundary_discipline".to_string();
    }
    if report.kind == "cycle_cluster" {
        if has_role_tag(&report.role_tags, "component_barrel")
            || has_role_tag(&report.role_tags, "guarded_boundary")
            || report.metrics.cut_candidate_count.unwrap_or(0) > 0
                && report.metrics.cycle_size.unwrap_or(0)
                    > report.metrics.largest_cycle_after_best_cut.unwrap_or(0)
        {
            return "architecture_signal".to_string();
        }
        return "secondary_cleanup".to_string();
    }
    if report.kind == "dead_island" {
        return "secondary_cleanup".to_string();
    }
    if has_role_tag(&report.role_tags, "component_barrel")
        || has_role_tag(&report.role_tags, "guarded_boundary")
        || has_role_tag(&report.role_tags, "state_container")
        || has_role_tag(&report.role_tags, "feature_module_barrel")
    {
        return "architecture_signal".to_string();
    }
    if has_role_tag(&report.role_tags, "composition_root")
        || has_role_tag(&report.role_tags, "entry_surface")
        || has_role_tag(&report.role_tags, "route_surface")
        || has_role_tag(&report.role_tags, "api_route_surface")
        || has_role_tag(&report.role_tags, "provider_surface")
    {
        return "regrowth_watchpoint".to_string();
    }
    if has_role_tag(&report.role_tags, "facade_with_extracted_owners") {
        if extracted_owner_facade_needs_secondary_cleanup(
            report.kind.as_str(),
            &report.role_tags,
            report.metrics.line_count,
            report.metrics.max_complexity,
            report.metrics.fan_in,
        ) {
            return "secondary_cleanup".to_string();
        }
        return "local_refactor_target".to_string();
    }
    match report.kind.as_str() {
        "clone_family" | "clone_group" | "exact_clone_group" => "secondary_cleanup".to_string(),
        "dependency_sprawl" | "unstable_hotspot" | "hotspot" => "local_refactor_target".to_string(),
        _ => "secondary_cleanup".to_string(),
    }
}

fn structural_leverage_reasons(report: &StructuralDebtReport) -> Vec<String> {
    let mut reasons = Vec::new();
    match structural_leverage_class(report).as_str() {
        "experimental" => reasons.push("detector_under_evaluation".to_string()),
        "tooling_debt" => reasons.push("tooling_surface_maintenance_burden".to_string()),
        "hardening_note" => reasons.push("narrow_completeness_gap".to_string()),
        "boundary_discipline" => {
            reasons.push("guarded_or_transport_facade".to_string());
            if report.metrics.fan_in.unwrap_or(0) > 0 {
                reasons.push("heavy_inbound_seam_pressure".to_string());
            }
        }
        "architecture_signal" => {
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
        "regrowth_watchpoint" => {
            reasons.push("intentionally_central_surface".to_string());
            reasons.push("fan_out_regrowth_pressure".to_string());
            if has_role_tag(&report.role_tags, "route_surface")
                || has_role_tag(&report.role_tags, "api_route_surface")
            {
                reasons.push("framework_entry_surface".to_string());
            }
        }
        "local_refactor_target" => {
            if has_role_tag(&report.role_tags, "facade_with_extracted_owners") {
                reasons.push("extracted_owner_shell_pressure".to_string());
            }
            if report.metrics.guardrail_test_count.unwrap_or(0) > 0 {
                reasons.push("guardrail_backed_refactor_surface".to_string());
            }
            if is_contained_refactor_surface(
                &report.role_tags,
                report.metrics.fan_in.or(report.metrics.inbound_reference_count),
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
        "secondary_cleanup" => {
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
        _ => {}
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

fn annotate_structural_leverage(mut report: StructuralDebtReport) -> StructuralDebtReport {
    report.leverage_class = structural_leverage_class(&report);
    report.leverage_reasons = structural_leverage_reasons(&report);
    report
}

fn contextual_role_tags(
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

fn with_guardrail_evidence(facts: &FileFacts, mut evidence: Vec<String>) -> Vec<String> {
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

fn related_structural_surfaces(facts: &FileFacts, mut related: Vec<String>) -> Vec<String> {
    related.extend(facts.guardrail_tests.iter().cloned());
    dedupe_strings_preserve_order(related)
}

fn large_file_summary(
    path: &str,
    line_count: usize,
    lang: &str,
    threshold: u32,
    role_tags: &[String],
) -> String {
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return format!(
            "Guarded facade file '{}' is {} lines, above the {} threshold of {}",
            path, line_count, lang, threshold
        );
    }
    if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        return format!(
            "Composition root '{}' is {} lines, above the {} threshold of {}",
            path, line_count, lang, threshold
        );
    }
    format!(
        "File '{}' is {} lines, above the {} threshold of {}",
        path, line_count, lang, threshold
    )
}

fn large_file_impact(path: &str, role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return "The facade is still broad after extraction, which makes it harder to see whether new owner seams are actually shrinking the coordination surface.".to_string();
    }
    if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        return "A broad entry surface can leak shell, runtime, and presentation concerns into one composition root.".to_string();
    }
    "Responsibility concentration increases review friction and makes later splits harder to isolate.".to_string()
}

fn large_file_inspection_focus(path: &str, role_tags: &[String]) -> Vec<String> {
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return vec![
            "inspect whether remaining coordination belongs in extracted owner modules instead of the public facade".to_string(),
            "inspect whether guardrail-backed owner seams are staying thin or accumulating new logic".to_string(),
        ];
    }
    if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        return vec![
            "inspect whether shell composition and runtime wiring are staying separate".to_string(),
            "inspect whether the entry surface is acting as a coordinator rather than an implementation sink".to_string(),
        ];
    }
    vec![
        "inspect whether orchestration, adapters, and data shaping are accumulating in one file"
            .to_string(),
        "inspect whether the file can be split along responsibility boundaries instead of line-count slices".to_string(),
    ]
}

fn dependency_sprawl_summary(
    path: &str,
    fan_out: usize,
    lang: &str,
    threshold: usize,
    role_tags: &[String],
) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return format!(
            "Guarded transport facade '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "component_barrel") {
        return format!(
            "Component-facing barrel '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return format!(
            "Guarded boundary file '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return format!(
            "Composition root '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    format!(
        "File '{}' depends on {} real surfaces, above the {} threshold of {}",
        path, fan_out, lang, threshold
    )
}

fn dependency_sprawl_impact(role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return "A broad transport facade makes it harder to keep lifecycle and domain policy out of glue code.".to_string();
    }
    if has_role_tag(role_tags, "component_barrel") {
        return "A broad component-facing barrel can stay intentional, but it still needs narrow boundaries so app and runtime layers do not grow back through it.".to_string();
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return "A broad boundary surface increases change surface and makes it harder to keep consumers on narrow, intended entry paths.".to_string();
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return "Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.".to_string();
    }
    "Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.".to_string()
}

fn dependency_sprawl_focus(role_tags: &[String]) -> Vec<String> {
    if has_role_tag(role_tags, "transport_facade") {
        return vec![
            "inspect whether transport glue is accumulating lifecycle or domain policy".to_string(),
            "inspect whether callers can depend on narrower transport contracts instead of the broad facade".to_string(),
        ];
    }
    if has_role_tag(role_tags, "component_barrel") {
        return vec![
            "inspect whether component-facing access is staying narrow while app and runtime imports remain outside the barrel".to_string(),
            "inspect whether mixed responsibilities belong in dedicated owner modules instead of the shared barrel".to_string(),
        ];
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return vec![
            "inspect whether callers are forced through a broad boundary instead of narrower owner modules".to_string(),
            "inspect whether policy-compliant imports are still pushing too much responsibility through one surface".to_string(),
        ];
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return vec![
            "inspect whether view composition can stay separate from runtime or session wiring"
                .to_string(),
            "inspect whether shell responsibilities are spreading across too many direct imports"
                .to_string(),
        ];
    }
    vec![
        "inspect whether orchestration and policy code can move behind narrower helpers"
            .to_string(),
        "inspect whether unrelated adapter dependencies are accumulating in one module".to_string(),
    ]
}

fn unstable_hotspot_summary(path: &str, fan_in: usize, role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return format!(
            "Guarded transport facade '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "component_barrel") {
        return format!(
            "Component-facing barrel '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return format!(
            "Guarded boundary file '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return format!(
            "Guarded facade '{}' still has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    format!(
        "File '{}' has {} inbound references and remains unstable",
        path, fan_in
    )
}

fn unstable_hotspot_impact(role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return "A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.".to_string();
    }
    if has_role_tag(role_tags, "component_barrel") {
        return "A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.".to_string();
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return "A volatile boundary surface increases blast radius even when callers stay inside the intended layer.".to_string();
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return "A volatile public facade can hide whether the real extracted owners are taking the intended load or whether coordination is flowing back uphill.".to_string();
    }
    "High fan-in plus instability increases blast radius and makes small edits harder to contain."
        .to_string()
}

fn unstable_hotspot_focus(role_tags: &[String]) -> Vec<String> {
    if has_role_tag(role_tags, "transport_facade") {
        return vec![
            "inspect whether lifecycle or domain policy is accumulating inside transport glue"
                .to_string(),
            "inspect whether callers or owner modules can take decisions outside the facade"
                .to_string(),
        ];
    }
    if has_role_tag(role_tags, "component_barrel") {
        return vec![
            "inspect which component-facing reads really need the shared barrel and which can move behind narrower selectors".to_string(),
            "inspect whether broad inbound traffic is hiding a smaller set of volatility-heavy owner modules".to_string(),
        ];
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return vec![
            "inspect whether a narrower public boundary can serve the common consumers".to_string(),
            "inspect whether intended callers are mixed with broader orchestration traffic"
                .to_string(),
        ];
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return vec![
            "inspect whether volatile logic belongs in extracted owners instead of the facade"
                .to_string(),
            "inspect whether too many callers still depend on coordination-heavy facade behavior"
                .to_string(),
        ];
    }
    vec![
        "inspect whether a stable contract can be split from the volatile implementation"
            .to_string(),
        "inspect whether too many callers depend directly on an orchestration-heavy file"
            .to_string(),
    ]
}

fn cycle_role_tags(
    files: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<String> {
    dedupe_strings_preserve_order(
        files
            .iter()
            .filter_map(|path| {
                file_facts
                    .get(path)
                    .map(|facts| contextual_role_tags(path, facts, graph, file_facts))
            })
            .flat_map(|role_tags| role_tags.into_iter())
            .collect(),
    )
}

fn role_tags_summary(role_tags: &[String]) -> String {
    if role_tags.is_empty() {
        return "role tags: none".to_string();
    }
    format!("role tags in cycle: {}", role_tags.join(", "))
}

fn cycle_cluster_impact(role_tags: &[String]) -> String {
    if role_tags.iter().any(|tag| tag == "component_barrel") {
        return "The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.".to_string();
    }
    if role_tags.iter().any(|tag| tag == "guarded_boundary") {
        return "The cycle crosses a guardrail-backed boundary, which increases refactor friction and makes it harder to keep the intended layering intact.".to_string();
    }
    if role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners")
    {
        return "The cycle still touches a guarded facade, which can hide whether extracted owners are actually reducing the coordination surface.".to_string();
    }
    "The cycle prevents clean layering and makes initialization order and refactors harder to isolate.".to_string()
}

fn cycle_cluster_focus(role_tags: &[String]) -> Vec<String> {
    if role_tags.iter().any(|tag| tag == "component_barrel") {
        return vec![
            "inspect whether the best cut keeps component-facing barrel access while moving deeper orchestration behind a narrower seam".to_string(),
            "inspect whether app or runtime dependencies can stop flowing back through the shared barrel".to_string(),
        ];
    }
    if role_tags.iter().any(|tag| tag == "guarded_boundary") {
        return vec![
            "inspect whether the best cut preserves the guardrail-backed boundary instead of widening it".to_string(),
            "inspect whether boundary callers can move to narrower authority modules".to_string(),
        ];
    }
    if role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners")
    {
        return vec![
            "inspect whether the cycle runs through a facade that should stay thin".to_string(),
            "inspect whether extracted owner modules can absorb the back-edge instead of routing it through the facade".to_string(),
        ];
    }
    vec![
        "inspect whether one back-edge can be removed by splitting contracts from implementations"
            .to_string(),
        "inspect whether shared types can move to a lower-dependency seam".to_string(),
    ]
}

fn build_large_file_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .long_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let threshold = lang_registry::profile(&facts.lang)
                .thresholds
                .large_file_lines;
            let score_0_10000 =
                large_file_score(file_metric.value, threshold, facts.max_complexity);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "large_file".to_string(),
                trust_tier: "trusted".to_string(),
                presentation_class: structural_presentation_class(
                    "large_file",
                    &file_metric.path,
                    "trusted",
                    &role_tags,
                ),
                leverage_class: String::new(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["size".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: large_file_summary(
                    &file_metric.path,
                    file_metric.value,
                    &facts.lang,
                    threshold,
                    &role_tags,
                ),
                impact: large_file_impact(&file_metric.path, &role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(
                    facts,
                    vec![
                        format!("line count: {}", file_metric.value),
                        format!("large-file threshold: {}", threshold),
                        format!("function count: {}", facts.function_count),
                        format!("peak function complexity: {}", facts.max_complexity),
                        format!(
                            "outbound dependencies: {}",
                            graph
                                .outgoing
                                .get(&file_metric.path)
                                .map(|paths| paths.len())
                                .unwrap_or(0)
                        ),
                    ],
                )),
                inspection_focus: large_file_inspection_focus(&file_metric.path, &role_tags),
                candidate_split_axes: large_file_split_axes(facts, graph, &file_metric.path),
                related_surfaces: related_structural_surfaces(
                    facts,
                    sample_paths(graph.outgoing.get(&file_metric.path), 5),
                ),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(file_metric.value),
                    function_count: Some(facts.function_count),
                    fan_out: Some(
                        graph
                            .outgoing
                            .get(&file_metric.path)
                            .map(|paths| paths.len())
                            .unwrap_or(0),
                    ),
                    max_complexity: Some(facts.max_complexity),
                    guardrail_test_count: Some(facts.guardrail_tests.len()),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn build_dependency_sprawl_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .god_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let fan_in = graph
                .incoming
                .get(&file_metric.path)
                .map(|paths| paths.len())
                .unwrap_or(0);
            let fan_out = graph
                .outgoing
                .get(&file_metric.path)
                .map(|paths| paths.len())
                .unwrap_or(0);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_out;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = dependency_sprawl_score(fan_out, threshold, instability);
            let dependency_examples = sample_paths(graph.outgoing.get(&file_metric.path), 3);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dependency_sprawl".to_string(),
                trust_tier: "trusted".to_string(),
                presentation_class: structural_presentation_class(
                    "dependency_sprawl",
                    &file_metric.path,
                    "trusted",
                    &role_tags,
                ),
                leverage_class: String::new(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["coupling".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: dependency_sprawl_summary(
                    &file_metric.path,
                    fan_out,
                    &facts.lang,
                    threshold,
                    &role_tags,
                ),
                impact: dependency_sprawl_impact(&role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(
                    facts,
                    vec![
                        format!("fan-out: {}", fan_out),
                        format!("fan-out threshold: {}", threshold),
                        format!("instability: {:.2}", instability as f64 / 10_000.0),
                        format!(
                            "dominant dependency categories: {}",
                            join_or_none(&dependency_category_summaries(
                                graph.outgoing.get(&file_metric.path),
                                3,
                            ))
                        ),
                        if dependency_examples.is_empty() {
                            "sample dependencies: none".to_string()
                        } else {
                            format!("sample dependencies: {}", dependency_examples.join(", "))
                        },
                    ],
                )),
                inspection_focus: dependency_sprawl_focus(&role_tags),
                candidate_split_axes: dependency_category_axes(
                    graph.outgoing.get(&file_metric.path),
                    3,
                ),
                related_surfaces: related_structural_surfaces(facts, dependency_examples),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    fan_in: Some(fan_in),
                    fan_out: Some(fan_out),
                    instability_0_10000: Some(instability),
                    max_complexity: Some(facts.max_complexity),
                    guardrail_test_count: Some(facts.guardrail_tests.len()),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn build_unstable_hotspot_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .hotspot_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let fan_in = graph
                .incoming
                .get(&file_metric.path)
                .map(|paths| paths.len())
                .unwrap_or(0);
            let fan_out = graph
                .outgoing
                .get(&file_metric.path)
                .map(|paths| paths.len())
                .unwrap_or(0);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_in;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = unstable_hotspot_score(fan_in, threshold, instability);
            let dependent_examples = sample_paths(graph.incoming.get(&file_metric.path), 3);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "unstable_hotspot".to_string(),
                trust_tier: "trusted".to_string(),
                presentation_class: structural_presentation_class(
                    "unstable_hotspot",
                    &file_metric.path,
                    "trusted",
                    &role_tags,
                ),
                leverage_class: String::new(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["coupling".to_string(), "blast_radius".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: unstable_hotspot_summary(&file_metric.path, fan_in, &role_tags),
                impact: unstable_hotspot_impact(&role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(
                    facts,
                    vec![
                        format!("fan-in: {}", fan_in),
                        format!("hotspot threshold: {}", threshold),
                        format!("fan-out: {}", fan_out),
                        format!("instability: {:.2}", instability as f64 / 10_000.0),
                        format!(
                            "dominant dependent categories: {}",
                            join_or_none(&dependency_category_summaries(
                                graph.incoming.get(&file_metric.path),
                                3,
                            ))
                        ),
                        if dependent_examples.is_empty() {
                            "sample dependents: none".to_string()
                        } else {
                            format!("sample dependents: {}", dependent_examples.join(", "))
                        },
                    ],
                )),
                inspection_focus: unstable_hotspot_focus(&role_tags),
                candidate_split_axes: hotspot_split_axes(
                    facts,
                    graph.incoming.get(&file_metric.path),
                    graph.outgoing.get(&file_metric.path),
                    3,
                ),
                related_surfaces: related_structural_surfaces(facts, dependent_examples),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    fan_in: Some(fan_in),
                    fan_out: Some(fan_out),
                    instability_0_10000: Some(instability),
                    max_complexity: Some(facts.max_complexity),
                    guardrail_test_count: Some(facts.guardrail_tests.len()),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn build_cycle_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .circular_dep_files
        .iter()
        .map(|files| {
            let scope = format!("cycle:{}", files.join("|"));
            let total_lines = files
                .iter()
                .map(|path| file_facts.get(path).map(|facts| facts.lines).unwrap_or(0))
                .sum::<usize>();
            let max_complexity = files
                .iter()
                .filter_map(|path| file_facts.get(path).map(|facts| facts.max_complexity))
                .max()
                .unwrap_or(0);
            let role_tags = cycle_role_tags(files, file_facts, graph);
            let cut_candidates = cycle_cut_candidates(files, file_facts, graph);
            let score_0_10000 =
                cycle_cluster_score(files.len(), total_lines, &role_tags, &cut_candidates);
            let cut_candidate_count = cut_candidates.len();
            let largest_cycle_after_best_cut = cut_candidates
                .first()
                .map(|candidate| candidate.remaining_cycle_size)
                .unwrap_or(files.len());
            let related_surfaces = cycle_related_surfaces(files, &cut_candidates);
            let candidate_split_axes = cycle_split_axes(&cut_candidates);

            annotate_structural_leverage(StructuralDebtReport {
                kind: "cycle_cluster".to_string(),
                trust_tier: "watchpoint".to_string(),
                presentation_class: structural_presentation_class(
                    "cycle_cluster",
                    files.first().map(String::as_str).unwrap_or_default(),
                    "watchpoint",
                    &role_tags,
                ),
                leverage_class: String::new(),
                scope,
                signal_class: "watchpoint".to_string(),
                signal_families: vec!["dependency".to_string(), "layering".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!("Files {} form a dependency cycle", files.join(", ")),
                impact: cycle_cluster_impact(&role_tags),
                files: files.clone(),
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(vec![
                    format!("cycle size: {}", files.len()),
                    format!("total lines in cycle: {}", total_lines),
                    format!("peak function complexity inside cycle: {}", max_complexity),
                    format!("candidate cuts: {}", cut_candidates.len()),
                    best_cycle_cut_evidence(&cut_candidates),
                    role_tags_summary(&role_tags),
                ]),
                inspection_focus: cycle_cluster_focus(&role_tags),
                candidate_split_axes,
                related_surfaces,
                cut_candidates,
                metrics: StructuralDebtMetrics {
                    file_count: Some(files.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(files.len()),
                    max_complexity: Some(max_complexity),
                    cut_candidate_count: Some(cut_candidate_count),
                    largest_cycle_after_best_cut: Some(largest_cycle_after_best_cut),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn build_dead_private_code_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
) -> Vec<StructuralDebtReport> {
    let mut dead_by_file = BTreeMap::<String, Vec<_>>::new();
    for function in &health.dead_functions {
        dead_by_file
            .entry(function.file.clone())
            .or_default()
            .push(function.clone());
    }

    dead_by_file
        .into_iter()
        .filter_map(|(path, functions)| {
            let dead_symbol_count = functions.len();
            let dead_line_count = functions.iter().map(|function| function.value as usize).sum::<usize>();
            if dead_symbol_count < 2 && dead_line_count < 40 {
                return None;
            }
            let facts = file_facts.get(&path)?;
            let score_0_10000 = dead_private_cluster_score(dead_symbol_count, dead_line_count);
            let function_names = functions
                .iter()
                .take(3)
                .map(|function| function.func.clone())
                .collect::<Vec<_>>();

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dead_private_code_cluster".to_string(),
                trust_tier: "experimental".to_string(),
                presentation_class: structural_presentation_class(
                    "dead_private_code_cluster",
                    &path,
                    "experimental",
                    &facts.role_tags,
                ),
                leverage_class: String::new(),
                scope: path.clone(),
                signal_class: "watchpoint".to_string(),
                signal_families: vec!["staleness".to_string(), "maintainability".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "File '{}' contains {} uncalled private functions totaling {} lines",
                    path, dead_symbol_count, dead_line_count
                ),
                impact: "Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.".to_string(),
                files: vec![path.clone()],
                role_tags: facts.role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(vec![
                    format!("dead private functions: {}", dead_symbol_count),
                    format!("dead private lines: {}", dead_line_count),
                    format!("sample dead functions: {}", function_names.join(", ")),
                    format!("total file lines: {}", facts.lines),
                ]),
                inspection_focus: vec![
                    "inspect whether the dead helpers should be deleted or intentionally reconnected".to_string(),
                    "inspect whether the file still reflects the supported control flow".to_string(),
                ],
                candidate_split_axes: Vec::new(),
                related_surfaces: Vec::new(),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    dead_symbol_count: Some(dead_symbol_count),
                    dead_line_count: Some(dead_line_count),
                    max_complexity: Some(facts.max_complexity),
                    role_count: Some(facts.role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn build_dead_island_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    let app_roots = application_root_files(snapshot, file_facts, graph);
    if app_roots.is_empty() {
        return Vec::new();
    }

    let test_roots = file_facts
        .iter()
        .filter(|(_, facts)| facts.is_test)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let app_reachable = reachable_files(graph, &app_roots);
    let test_reachable = reachable_files(graph, &test_roots);
    let cycle_size_by_file = cycle_size_by_file(health);

    weak_components(file_facts, graph)
        .into_iter()
        .filter_map(|component| {
            let component_set = component.iter().cloned().collect::<BTreeSet<_>>();
            let is_app_reachable = component.iter().any(|path| app_reachable.contains(path));
            if is_app_reachable {
                return None;
            }

            let public_surface_count = component
                .iter()
                .map(|path| {
                    file_facts
                        .get(path)
                        .map(|facts| facts.public_function_count)
                        .unwrap_or(0)
                })
                .sum::<usize>();
            if public_surface_count > 0 {
                return None;
            }
            let has_entry_or_package_surface = component.iter().any(|path| {
                file_facts
                    .get(path)
                    .is_some_and(|facts| facts.is_package_index || facts.has_entry_tag)
            });
            if has_entry_or_package_surface {
                return None;
            }

            let inbound_reference_count =
                external_non_test_inbound_count(&component_set, file_facts, graph);
            if inbound_reference_count > 0 {
                return None;
            }

            let cycle_size = component
                .iter()
                .filter_map(|path| cycle_size_by_file.get(path).copied())
                .max()
                .unwrap_or(0);
            let total_lines = component
                .iter()
                .map(|path| file_facts.get(path).map(|facts| facts.lines).unwrap_or(0))
                .sum::<usize>();
            let reachable_from_tests = component.iter().any(|path| test_reachable.contains(path));

            if component.len() < 2 && cycle_size < 2 {
                return None;
            }

            let score_0_10000 =
                dead_island_score(component.len(), total_lines, cycle_size, reachable_from_tests);
            let scope = format!("dead_island:{}", component.join("|"));
            let sample_files = component
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let evidence = dedupe_strings_preserve_order(vec![
                format!("component file count: {}", component.len()),
                format!("component lines: {}", total_lines),
                format!("largest internal cycle: {}", cycle_size),
                format!("external inbound references from app graph: {}", inbound_reference_count),
                format!("reachable from tests: {}", reachable_from_tests),
                format!("sample files: {}", sample_files),
            ]);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dead_island".to_string(),
                trust_tier: "watchpoint".to_string(),
                presentation_class: structural_presentation_class(
                    "dead_island",
                    component.first().map(String::as_str).unwrap_or_default(),
                    "watchpoint",
                    &Vec::new(),
                ),
                leverage_class: String::new(),
                scope,
                signal_class: if reachable_from_tests {
                    "watchpoint".to_string()
                } else {
                    "debt".to_string()
                },
                signal_families: vec!["reachability".to_string(), "staleness".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: if reachable_from_tests {
                    format!(
                        "Files {} form an internally connected component that is not reachable from app roots",
                        component.join(", ")
                    )
                } else {
                    format!(
                        "Files {} form an internally connected component that is disconnected from the app-reachable graph",
                        component.join(", ")
                    )
                },
                impact: if reachable_from_tests {
                    "A test-only internal component may be stale production code or an accidentally disconnected subsystem.".to_string()
                } else {
                    "A disconnected internal component adds maintenance noise and can hide obsolete or unsupported code paths.".to_string()
                },
                files: component.clone(),
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence,
                inspection_focus: vec![
                    "inspect whether this component is intentionally disconnected or stale".to_string(),
                    "inspect whether it should be deleted, archived, or wired through an explicit root".to_string(),
                ],
                candidate_split_axes: vec![
                    "reachable entry surface".to_string(),
                    "public contract boundary".to_string(),
                ],
                related_surfaces: component.iter().take(5).cloned().collect(),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(component.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(cycle_size),
                    inbound_reference_count: Some(inbound_reference_count),
                    public_surface_count: Some(public_surface_count),
                    reachable_from_tests: Some(reachable_from_tests),
                    cut_candidate_count: Some(0),
                    largest_cycle_after_best_cut: Some(cycle_size),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn application_root_files(
    snapshot: &Snapshot,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> BTreeSet<String> {
    let explicit_roots = snapshot
        .entry_points
        .iter()
        .map(|entry| entry.file.clone())
        .filter(|path| file_facts.get(path).is_some_and(|facts| !facts.is_test))
        .collect::<BTreeSet<_>>();

    let mut roots = explicit_roots;
    roots.extend(
        file_facts
            .iter()
            .filter(|(_, facts)| facts.has_entry_tag || facts.is_package_index)
            .filter(|(_, facts)| !facts.is_test)
            .map(|(path, _)| path.clone()),
    );

    if !roots.is_empty() {
        return roots;
    }

    file_facts
        .iter()
        .filter(|(_, facts)| !facts.is_test)
        .filter(|(path, facts)| is_zero_inbound_root_candidate(path, facts, file_facts, graph))
        .map(|(path, _)| path.clone())
        .collect()
}

#[cfg(test)]
fn has_dead_island_report(reports: &[StructuralDebtReport], expected_files: &[&str]) -> bool {
    let expected_files = expected_files
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    reports
        .iter()
        .any(|report| report.kind == "dead_island" && report.files == expected_files)
}

fn is_zero_inbound_root_candidate(
    path: &str,
    facts: &FileFacts,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> bool {
    let inbound_only_from_tests = graph.incoming.get(path).is_none_or(|sources| {
        sources.iter().all(|source| {
            file_facts
                .get(source)
                .is_some_and(|source_facts| source_facts.is_test)
        })
    });
    let has_surface = graph
        .outgoing
        .get(path)
        .is_some_and(|targets| !targets.is_empty())
        || facts.public_function_count > 0;

    inbound_only_from_tests && has_surface
}

fn reachable_files(graph: &StructuralGraph, roots: &BTreeSet<String>) -> BTreeSet<String> {
    let mut visited = roots.clone();
    let mut queue = roots.iter().cloned().collect::<Vec<_>>();

    while let Some(path) = queue.pop() {
        if let Some(targets) = graph.outgoing.get(&path) {
            for target in targets {
                if visited.insert(target.clone()) {
                    queue.push(target.clone());
                }
            }
        }
    }

    visited
}

fn weak_components(
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<Vec<String>> {
    let relevant_files = file_facts
        .iter()
        .filter(|(_, facts)| !facts.is_test)
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut components = Vec::new();

    for start in relevant_files {
        if !visited.insert(start.clone()) {
            continue;
        }
        let mut queue = vec![start.clone()];
        let mut component = vec![start];
        while let Some(path) = queue.pop() {
            for neighbor in weak_neighbors(graph, &path) {
                if visited.insert(neighbor.clone()) {
                    queue.push(neighbor.clone());
                    component.push(neighbor);
                }
            }
        }
        component.sort();
        components.push(component);
    }

    components
}

fn weak_neighbors(graph: &StructuralGraph, path: &str) -> Vec<String> {
    let mut neighbors = BTreeSet::new();
    if let Some(targets) = graph.outgoing.get(path) {
        neighbors.extend(targets.iter().cloned());
    }
    if let Some(sources) = graph.incoming.get(path) {
        neighbors.extend(sources.iter().cloned());
    }
    neighbors.into_iter().collect()
}

fn external_non_test_inbound_count(
    component: &BTreeSet<String>,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> usize {
    let mut sources = BTreeSet::new();
    for path in component {
        if let Some(incoming) = graph.incoming.get(path) {
            for source in incoming {
                if component.contains(source) {
                    continue;
                }
                if file_facts.get(source).is_some_and(|facts| !facts.is_test) {
                    sources.insert(source.clone());
                }
            }
        }
    }
    sources.len()
}

fn cycle_size_by_file(health: &HealthReport) -> BTreeMap<String, usize> {
    let mut sizes = BTreeMap::new();
    for cycle in &health.circular_dep_files {
        for path in cycle {
            sizes
                .entry(path.clone())
                .and_modify(|size: &mut usize| *size = (*size).max(cycle.len()))
                .or_insert(cycle.len());
        }
    }
    sizes
}

fn sample_paths(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| paths.iter().take(limit).cloned().collect())
        .unwrap_or_default()
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn dependency_category_summaries(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    let Some(paths) = paths else {
        return Vec::new();
    };

    let mut counts = BTreeMap::<String, usize>::new();
    for path in paths {
        let category = path_category(path);
        counts
            .entry(category)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }

    let mut categories = counts.into_iter().collect::<Vec<_>>();
    categories.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    categories
        .into_iter()
        .take(limit)
        .map(|(category, count)| format!("{category}({count})"))
        .collect()
}

fn dependency_category_axes(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    let Some(paths) = paths else {
        return vec!["orchestration boundary".to_string()];
    };

    let categories = dominant_categories(paths, limit);
    if categories.is_empty() {
        return vec!["orchestration boundary".to_string()];
    }

    categories
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect()
}

fn hotspot_split_axes(
    facts: &FileFacts,
    incoming: Option<&BTreeSet<String>>,
    outgoing: Option<&BTreeSet<String>>,
    limit: usize,
) -> Vec<String> {
    let inbound_categories = dominant_categories_from_option(incoming, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} caller boundary"))
        .collect::<Vec<_>>();
    let outbound_categories = dominant_categories_from_option(outgoing, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect::<Vec<_>>();

    let mut axes = inbound_categories;
    axes.extend(outbound_categories);
    if has_role(facts, "guarded_boundary") {
        axes.push("guarded boundary".to_string());
    }
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    let mut axes = dedupe_strings_preserve_order(axes);
    axes.truncate(limit.max(1));
    if axes.is_empty() {
        axes.push("stable contract boundary".to_string());
    }
    axes
}

fn large_file_split_axes(facts: &FileFacts, graph: &StructuralGraph, path: &str) -> Vec<String> {
    let mut axes = dependency_category_axes(graph.outgoing.get(path), 3);
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    if has_role(facts, "entry_surface") {
        axes.push("entry surface split".to_string());
    }
    if facts.max_complexity >= 40 {
        axes.push("high-complexity helper extraction".to_string());
    }
    if facts.function_count >= 20 {
        axes.push("private helper surface split".to_string());
    }
    dedupe_strings_preserve_order(axes)
}

fn dominant_categories(paths: &BTreeSet<String>, limit: usize) -> Vec<String> {
    dependency_category_summaries(Some(paths), limit)
        .into_iter()
        .filter_map(|summary| {
            summary
                .split_once('(')
                .map(|(category, _)| category.to_string())
        })
        .collect()
}

fn dominant_categories_from_option(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| dominant_categories(paths, limit))
        .unwrap_or_default()
}

fn path_category(path: &str) -> String {
    let normalized = path.strip_prefix("./").unwrap_or(path);
    if let Some(rest) = normalized.strip_prefix("src/") {
        return rest.split('/').next().unwrap_or("src").to_string();
    }
    normalized
        .split('/')
        .next()
        .unwrap_or(normalized)
        .to_string()
}

fn cycle_cut_candidates(
    files: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<CycleCutCandidate> {
    let nodes = files.iter().cloned().collect::<BTreeSet<_>>();
    let original_cycle_size = nodes.len();
    let internal_edges = cycle_internal_import_edges(&nodes, graph);
    if internal_edges.is_empty() {
        return Vec::new();
    }

    let mut candidates = internal_edges
        .into_iter()
        .filter_map(|(source, target)| {
            let (remaining_cycle_size, cyclic_node_count) =
                cyclic_sizes_without_edge(&nodes, graph, (&source, &target));
            let reduction_file_count = original_cycle_size.saturating_sub(cyclic_node_count);
            if reduction_file_count == 0 {
                return None;
            }

            let seam_kind = cycle_seam_kind_with_roles(&source, &target, file_facts);
            let score_0_10000 = cycle_cut_candidate_score(
                original_cycle_size,
                reduction_file_count,
                remaining_cycle_size,
                &source,
                &target,
                graph,
                seam_kind,
            );
            let source_lines = file_facts
                .get(&source)
                .map(|facts| facts.lines)
                .unwrap_or(0);
            let target_lines = file_facts
                .get(&target)
                .map(|facts| facts.lines)
                .unwrap_or(0);
            Some(CycleCutCandidate {
                source: source.clone(),
                target: target.clone(),
                seam_kind: seam_kind.to_string(),
                score_0_10000,
                summary: format!(
                    "Inspect import edge '{}' -> '{}' to reduce the cyclic footprint by {} file(s)",
                    source, target, reduction_file_count
                ),
                evidence: vec![
                    format!("seam kind: {}", seam_kind),
                    format!(
                        "remaining largest cycle after cut: {}",
                        remaining_cycle_size
                    ),
                    format!("cyclic files removed by cut: {}", reduction_file_count),
                    format!("source lines: {}", source_lines),
                    format!("target lines: {}", target_lines),
                ],
                reduction_file_count,
                remaining_cycle_size,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .score_0_10000
            .cmp(&left.score_0_10000)
            .then_with(|| right.reduction_file_count.cmp(&left.reduction_file_count))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
    });
    candidates.truncate(3);
    candidates
}

fn cycle_internal_import_edges(
    nodes: &BTreeSet<String>,
    graph: &StructuralGraph,
) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for source in nodes {
        let Some(targets) = graph.import_outgoing.get(source) else {
            continue;
        };
        for target in targets {
            if nodes.contains(target) {
                edges.push((source.clone(), target.clone()));
            }
        }
    }
    edges
}

fn cyclic_sizes_without_edge(
    nodes: &BTreeSet<String>,
    graph: &StructuralGraph,
    removed_edge: (&str, &str),
) -> (usize, usize) {
    let adjacency = cycle_adjacency(nodes, graph, Some(removed_edge));
    let components = strongly_connected_components(nodes, &adjacency);
    let cyclic_components = components
        .iter()
        .filter(|component| is_cyclic_component(component, &adjacency))
        .collect::<Vec<_>>();
    let largest_cycle_size = cyclic_components
        .iter()
        .map(|component| component.len())
        .max()
        .unwrap_or(0);
    let cyclic_node_count = cyclic_components
        .iter()
        .map(|component| component.len())
        .sum::<usize>();
    (largest_cycle_size, cyclic_node_count)
}

fn cycle_adjacency(
    nodes: &BTreeSet<String>,
    graph: &StructuralGraph,
    removed_edge: Option<(&str, &str)>,
) -> BTreeMap<String, Vec<String>> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for node in nodes {
        let neighbors = graph
            .import_outgoing
            .get(node)
            .map(|targets| {
                targets
                    .iter()
                    .filter(|target| nodes.contains(*target))
                    .filter(|target| {
                        removed_edge.is_none_or(|(source, removed_target)| {
                            !(node == source && target.as_str() == removed_target)
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        adjacency.insert(node.clone(), neighbors);
    }
    adjacency
}

fn strongly_connected_components(
    nodes: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for node in nodes {
        dfs_order(node, adjacency, &mut visited, &mut order);
    }

    let reverse = reverse_adjacency(nodes, adjacency);
    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for node in order.into_iter().rev() {
        if assigned.contains(&node) {
            continue;
        }
        let mut component = Vec::new();
        dfs_collect(&node, &reverse, &mut assigned, &mut component);
        component.sort();
        components.push(component);
    }
    components
}

fn dfs_order(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    visited: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) {
    if !visited.insert(node.to_string()) {
        return;
    }
    if let Some(neighbors) = adjacency.get(node) {
        for neighbor in neighbors {
            dfs_order(neighbor, adjacency, visited, order);
        }
    }
    order.push(node.to_string());
}

fn reverse_adjacency(
    nodes: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut reverse = nodes
        .iter()
        .map(|node| (node.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in adjacency {
        for target in targets {
            reverse
                .entry(target.clone())
                .or_default()
                .push(source.clone());
        }
    }
    reverse
}

fn dfs_collect(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    visited: &mut BTreeSet<String>,
    component: &mut Vec<String>,
) {
    if !visited.insert(node.to_string()) {
        return;
    }
    component.push(node.to_string());
    if let Some(neighbors) = adjacency.get(node) {
        for neighbor in neighbors {
            dfs_collect(neighbor, adjacency, visited, component);
        }
    }
}

fn is_cyclic_component(component: &[String], adjacency: &BTreeMap<String, Vec<String>>) -> bool {
    if component.len() > 1 {
        return true;
    }
    component.first().is_some_and(|node| {
        adjacency
            .get(node)
            .is_some_and(|neighbors| neighbors.iter().any(|neighbor| neighbor == node))
    })
}

fn cycle_seam_kind(source: &str, target: &str) -> &'static str {
    let source_category = path_category(source);
    let target_category = path_category(target);
    if is_app_store_boundary(&source_category, &target_category) {
        return "app_store_boundary";
    }
    if path_has_contract_hint(source) || path_has_contract_hint(target) {
        return "contract_or_type_extraction";
    }
    if source_category != target_category {
        return "cross_layer_boundary";
    }
    "local_module_split"
}

fn cycle_seam_kind_with_roles(
    source: &str,
    target: &str,
    file_facts: &BTreeMap<String, FileFacts>,
) -> &'static str {
    let source_facts = file_facts.get(source);
    let target_facts = file_facts.get(target);
    if source_facts.is_some_and(|facts| has_role(facts, "guarded_boundary"))
        || target_facts.is_some_and(|facts| has_role(facts, "guarded_boundary"))
    {
        let source_category = path_category(source);
        let target_category = path_category(target);
        if is_app_store_boundary(&source_category, &target_category) {
            return "guarded_app_store_boundary";
        }
        return "guarded_boundary_cut";
    }
    if source_facts.is_some_and(|facts| has_role(facts, "facade_with_extracted_owners"))
        || target_facts.is_some_and(|facts| has_role(facts, "facade_with_extracted_owners"))
    {
        return "facade_owner_boundary";
    }
    cycle_seam_kind(source, target)
}

fn cycle_cut_candidate_score(
    original_cycle_size: usize,
    reduction_file_count: usize,
    remaining_cycle_size: usize,
    source: &str,
    target: &str,
    graph: &StructuralGraph,
    seam_kind: &str,
) -> u32 {
    let reduction_bonus = if original_cycle_size == 0 {
        0
    } else {
        ((reduction_file_count as f64 / original_cycle_size as f64) * 4500.0).round() as u32
    };
    let seam_bonus = match seam_kind {
        "guarded_app_store_boundary" => 2200,
        "guarded_boundary_cut" => 2000,
        "facade_owner_boundary" => 1900,
        "app_store_boundary" => 1800,
        "contract_or_type_extraction" => 1500,
        "cross_layer_boundary" => 1200,
        _ => 700,
    };
    let source_internal_out = graph
        .import_outgoing
        .get(source)
        .map(|targets| targets.len())
        .unwrap_or(0) as u32;
    let target_internal_in = graph
        .import_incoming
        .get(target)
        .map(|sources| sources.len())
        .unwrap_or(0) as u32;
    let pressure_bonus = ((source_internal_out + target_internal_in) * 180).min(1800);
    let cleanup_bonus =
        (original_cycle_size.saturating_sub(remaining_cycle_size) as u32 * 120).min(1200);

    (2000 + reduction_bonus + seam_bonus + pressure_bonus + cleanup_bonus).min(10_000)
}

fn best_cycle_cut_evidence(cut_candidates: &[CycleCutCandidate]) -> String {
    match cut_candidates.first() {
        Some(candidate) => format!(
            "best cut candidate: {} -> {} (removes {} cyclic files)",
            candidate.source, candidate.target, candidate.reduction_file_count
        ),
        None => "best cut candidate: none".to_string(),
    }
}

fn cycle_related_surfaces(files: &[String], cut_candidates: &[CycleCutCandidate]) -> Vec<String> {
    let mut related = cut_candidates
        .iter()
        .flat_map(|candidate| [candidate.source.clone(), candidate.target.clone()])
        .collect::<Vec<_>>();
    related.extend(files.iter().take(3).cloned());
    dedupe_strings_preserve_order(related)
}

fn cycle_split_axes(cut_candidates: &[CycleCutCandidate]) -> Vec<String> {
    let mut axes = cut_candidates
        .iter()
        .map(|candidate| candidate.seam_kind.replace('_', " "))
        .collect::<Vec<_>>();
    if axes.is_empty() {
        axes.push("contract boundary".to_string());
    }
    dedupe_strings_preserve_order(axes)
}

fn path_has_contract_hint(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    ["contract", "schema", "types", "state", "model"]
        .iter()
        .any(|segment| normalized.contains(segment))
}

fn is_app_store_boundary(source_category: &str, target_category: &str) -> bool {
    (source_category == "app" && target_category == "store")
        || (source_category == "store" && target_category == "app")
}

fn large_file_score(line_count: usize, threshold: u32, max_complexity: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(line_count, threshold as usize, 3600);
    let complexity_bonus = max_complexity.saturating_sub(20).min(20) * 120;
    (2400 + over_threshold + complexity_bonus).min(10_000)
}

fn dependency_sprawl_score(fan_out: usize, threshold: usize, instability_0_10000: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_out, threshold as usize, 3200);
    let instability_bonus = instability_0_10000 / 4;
    (2800 + over_threshold + instability_bonus).min(10_000)
}

fn unstable_hotspot_score(fan_in: usize, threshold: usize, instability_0_10000: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_in, threshold as usize, 3000);
    let instability_bonus = instability_0_10000 / 3;
    (3200 + over_threshold + instability_bonus).min(10_000)
}

fn cycle_cluster_score(
    file_count: usize,
    total_lines: usize,
    role_tags: &[String],
    cut_candidates: &[CycleCutCandidate],
) -> u32 {
    let size_bonus = (file_count as u32 * 800).min(3200);
    let line_bonus = (total_lines as u32 / 14).min(1800);
    let role_bonus = [
        ("component_barrel", 1700),
        ("guarded_boundary", 1500),
        ("composition_root", 500),
        ("entry_surface", 400),
    ]
    .into_iter()
    .filter(|(tag, _)| has_role_tag(role_tags, tag))
    .map(|(_, bonus)| bonus)
    .sum::<u32>()
    .min(2800);
    let cut_bonus = cut_candidates
        .first()
        .map(|candidate| {
            let seam_bonus = match candidate.seam_kind.as_str() {
                "guarded_app_store_boundary" => 1300,
                "guarded_boundary_cut" => 1100,
                "facade_owner_boundary" => 900,
                "app_store_boundary" => 700,
                "contract_or_type_extraction" => 600,
                "cross_layer_boundary" => 500,
                _ => 300,
            };
            let reduction_bonus = (candidate.reduction_file_count as u32 * 160).min(1100);
            let reduction_ratio_bonus = if file_count == 0 {
                0
            } else {
                ((candidate.reduction_file_count as f64 / file_count as f64) * 1500.0).round()
                    as u32
            };
            let remainder_penalty = if file_count == 0 {
                0
            } else {
                ((candidate.remaining_cycle_size as f64 / file_count as f64) * 700.0).round()
                    as u32
            };
            let contained_remainder_bonus = 700u32.saturating_sub(remainder_penalty);
            seam_bonus + reduction_bonus + reduction_ratio_bonus + contained_remainder_bonus
        })
        .unwrap_or(0);
    (2400 + size_bonus + line_bonus + role_bonus + cut_bonus).min(10_000)
}

fn dead_private_cluster_score(dead_symbol_count: usize, dead_line_count: usize) -> u32 {
    let symbol_bonus = (dead_symbol_count as u32 * 900).min(3600);
    let line_bonus = (dead_line_count as u32 * 18).min(2800);
    (1500 + symbol_bonus + line_bonus).min(10_000)
}

fn dead_island_score(
    file_count: usize,
    total_lines: usize,
    cycle_size: usize,
    reachable_from_tests: bool,
) -> u32 {
    let file_bonus = (file_count as u32 * 900).min(3600);
    let line_bonus = (total_lines as u32 / 10).min(2600);
    let cycle_bonus = (cycle_size as u32 * 700).min(2100);
    let test_penalty = if reachable_from_tests { 1200 } else { 0 };
    (2800 + file_bonus + line_bonus + cycle_bonus).saturating_sub(test_penalty)
}

fn scaled_ratio_pressure(value: usize, threshold: usize, max_bonus: u32) -> u32 {
    if threshold == 0 || value <= threshold {
        return 0;
    }

    let pressure = ((value - threshold) as f64 / threshold as f64).min(1.0);
    (pressure * max_bonus as f64).round() as u32
}

fn instability_0_10000(fan_in: usize, fan_out: usize) -> u32 {
    let total = fan_in + fan_out;
    let instability = if total == 0 {
        0.5
    } else {
        fan_out as f64 / total as f64
    };
    (instability * 10_000.0).round() as u32
}

fn signal_severity(score_0_10000: u32) -> &'static str {
    match score_0_10000 {
        6500..=10_000 => "high",
        3000..=6499 => "medium",
        _ => "low",
    }
}

fn severity_priority(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{CallEdge, EntryPoint, FileNode, StructuralAnalysis};
    use crate::metrics::root_causes::{RootCauseRaw, RootCauseScores};
    use crate::metrics::{FileMetric, FuncMetric};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reports_large_files_sprawl_hotspots_cycles_and_dead_private_clusters() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/app.ts", 720, 6, 28),
                    test_file("src/a.ts", 120, 3, 12),
                    test_file("src/b.ts", 120, 3, 16),
                    test_file("src/unused.ts", 110, 5, 8),
                ]),
            }),
            total_files: 4,
            total_lines: 1070,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/a.ts".into(),
                },
                ImportEdge {
                    from_file: "src/a.ts".into(),
                    to_file: "src/b.ts".into(),
                },
                ImportEdge {
                    from_file: "src/b.ts".into(),
                    to_file: "src/a.ts".into(),
                },
            ],
            call_graph: vec![
                CallEdge {
                    from_file: "src/app.ts".into(),
                    from_func: "main".into(),
                    to_file: "src/a.ts".into(),
                    to_func: "helper".into(),
                },
                CallEdge {
                    from_file: "src/app.ts".into(),
                    from_func: "main".into(),
                    to_file: "src/b.ts".into(),
                    to_func: "helper".into(),
                },
            ],
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 1,
            circular_dep_files: vec![vec!["src/a.ts".into(), "src/b.ts".into()]],
            total_import_edges: 3,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 2,
            god_files: vec![FileMetric {
                path: "src/app.ts".into(),
                value: 8,
            }],
            hotspot_files: vec![FileMetric {
                path: "src/a.ts".into(),
                value: 4,
            }],
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: vec![
                FuncMetric {
                    file: "src/unused.ts".into(),
                    func: "orphanAlpha".into(),
                    value: 24,
                },
                FuncMetric {
                    file: "src/unused.ts".into(),
                    func: "orphanBeta".into(),
                    value: 20,
                },
            ],
            long_files: vec![FileMetric {
                path: "src/app.ts".into(),
                value: 720,
            }],
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 1,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 1,
                max_depth: 2,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        };

        let reports = build_structural_debt_reports(&snapshot, &health);
        let kinds = reports
            .iter()
            .map(|report| report.kind.as_str())
            .collect::<Vec<_>>();

        assert!(kinds.contains(&"large_file"));
        assert!(kinds.contains(&"dependency_sprawl"));
        assert!(kinds.contains(&"unstable_hotspot"));
        assert!(kinds.contains(&"cycle_cluster"));
        assert!(kinds.contains(&"dead_private_code_cluster"));
    }

    #[test]
    fn reports_dead_island_for_disconnected_internal_cycle() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/app.ts", 120, 2, 10),
                    test_file("src/live.ts", 80, 2, 8),
                    test_file("src/orphan-a.ts", 90, 2, 6),
                    test_file("src/orphan-b.ts", 95, 2, 7),
                ]),
            }),
            total_files: 4,
            total_lines: 385,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/live.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-a.ts".into(),
                    to_file: "src/orphan-b.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-b.ts".into(),
                    to_file: "src/orphan-a.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 1,
            circular_dep_files: vec![vec!["src/orphan-a.ts".into(), "src/orphan-b.ts".into()]],
            total_import_edges: 3,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 1,
            god_files: Vec::new(),
            hotspot_files: Vec::new(),
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: Vec::new(),
            long_files: Vec::new(),
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 0,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 1,
                max_depth: 1,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        };

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(has_dead_island_report(
            &reports,
            &["src/orphan-a.ts", "src/orphan-b.ts"]
        ));
    }

    #[test]
    fn reports_dead_island_for_disconnected_non_cycle_component_when_entry_points_exist() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/app.ts", 120, 2, 10),
                    test_file("src/live.ts", 80, 2, 8),
                    test_file("src/orphan-root.ts", 90, 2, 6),
                    test_file("src/orphan-leaf.ts", 95, 2, 7),
                ]),
            }),
            total_files: 4,
            total_lines: 385,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/live.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-root.ts".into(),
                    to_file: "src/orphan-leaf.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = empty_health_report();

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(has_dead_island_report(
            &reports,
            &["src/orphan-leaf.ts", "src/orphan-root.ts"]
        ));
    }

    #[test]
    fn does_not_report_dead_island_for_zero_inbound_root_when_no_entry_points_exist() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/root.ts", 120, 2, 10),
                    test_file("src/helper.ts", 80, 2, 8),
                ]),
            }),
            total_files: 2,
            total_lines: 200,
            total_dirs: 1,
            import_graph: vec![ImportEdge {
                from_file: "src/root.ts".into(),
                to_file: "src/helper.ts".into(),
            }],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let health = empty_health_report();

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(!reports.iter().any(|report| report.kind == "dead_island"));
    }

    #[test]
    fn large_file_guarded_facade_reports_role_tags_and_guardrail_evidence() {
        let root = temp_root("guarded-facade");
        write_file(
            &root,
            "src/components/terminal-session.architecture.test.ts",
            "\
                expect(source).toContain('createTerminalInputPipeline');\n\
                expect(source).toContain('createTerminalOutputPipeline');\n\
            ",
        );
        write_file(
            &root,
            "src/components/terminal-session.ts",
            "export function main() {}\n",
        );

        let snapshot = Snapshot {
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
                children: Some(vec![test_file(
                    "src/components/terminal-session.ts",
                    720,
                    24,
                    51,
                )]),
            }),
            total_files: 1,
            total_lines: 720,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.long_files = vec![FileMetric {
            path: "src/components/terminal-session.ts".into(),
            value: 720,
        }];

        let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "large_file")
            .expect("large-file report");

        assert!(report.role_tags.iter().any(|tag| tag == "guarded_seam"));
        assert!(report
            .role_tags
            .iter()
            .any(|tag| tag == "facade_with_extracted_owners"));
        assert_eq!(report.leverage_class, "secondary_cleanup");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "secondary_facade_cleanup"));
        assert!(report.summary.contains("Guarded facade file"));
        assert!(report
            .evidence
            .iter()
            .any(|entry| entry.contains("guardrail tests:")));
        assert!(report
            .candidate_split_axes
            .iter()
            .any(|axis| axis == "facade owner boundary"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_sprawl_marks_contained_extracted_owner_shell_as_local_refactor_target() {
        let root = temp_root("extracted-owner-shell");
        write_file(
            &root,
            "src/components/TaskPanel.architecture.test.ts",
            "\
                expect(source).toContain('createTaskPanelFocusRuntime');\n\
                expect(source).toContain('createTaskPanelPreviewController');\n\
                expect(source).toContain('createTaskPanelDialogState');\n\
            ",
        );
        write_file(
            &root,
            "src/components/TaskPanel.tsx",
            "export function TaskPanel() {}\n",
        );

        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/components/TaskPanel.tsx", 423, 59, 4),
                    test_file("src/app/task-ports.ts", 40, 2, 3),
                    test_file("src/components/CloseTaskDialog.tsx", 60, 3, 4),
                    test_file("src/components/DiffViewerDialog.tsx", 70, 4, 4),
                ]),
            }),
            total_files: 4,
            total_lines: 593,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/components/TaskPanel.tsx".into(),
                    to_file: "src/app/task-ports.ts".into(),
                },
                ImportEdge {
                    from_file: "src/components/TaskPanel.tsx".into(),
                    to_file: "src/components/CloseTaskDialog.tsx".into(),
                },
                ImportEdge {
                    from_file: "src/components/TaskPanel.tsx".into(),
                    to_file: "src/components/DiffViewerDialog.tsx".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/components/TaskPanel.tsx".into(),
            value: 28,
        }];

        let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.scope == "src/components/TaskPanel.tsx")
            .expect("task-panel dependency-sprawl report");

        assert!(report.role_tags.iter().any(|tag| tag == "guarded_seam"));
        assert!(report
            .role_tags
            .iter()
            .any(|tag| tag == "facade_with_extracted_owners"));
        assert_eq!(report.leverage_class, "local_refactor_target");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "extracted_owner_shell_pressure"));
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "guardrail_backed_refactor_surface"));
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "contained_refactor_surface"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_sprawl_reports_guarded_boundary_role_from_architecture_test_literal() {
        let root = temp_root("guarded-boundary");
        write_file(
            &root,
            "src/app/store-boundary.architecture.test.ts",
            "expect(source).not.toContain('store/store');\n",
        );
        write_file(
            &root,
            "src/store/store.ts",
            "export function selectStore() {}\n",
        );

        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/store/store.ts", 220, 6, 18),
                    test_file("src/components/App.tsx", 120, 3, 8),
                    test_file("src/components/TaskPanel.tsx", 120, 3, 8),
                ]),
            }),
            total_files: 3,
            total_lines: 460,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/store/store.ts".into(),
                    to_file: "src/components/App.tsx".into(),
                },
                ImportEdge {
                    from_file: "src/store/store.ts".into(),
                    to_file: "src/components/TaskPanel.tsx".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/store/store.ts".into(),
            value: 17,
        }];

        let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "dependency_sprawl")
            .expect("dependency-sprawl report");

        assert!(report.role_tags.iter().any(|tag| tag == "component_barrel"));
        assert!(report.role_tags.iter().any(|tag| tag == "guarded_boundary"));
        assert_eq!(report.leverage_class, "architecture_signal");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "shared_barrel_boundary_hub"));
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "guardrail_backed_boundary_pressure"));
        assert!(report.summary.contains("Component-facing barrel"));
        assert!(report
            .evidence
            .iter()
            .any(|entry| entry.contains("guarded boundary literals:")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_sprawl_softens_direct_entry_composition_root() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/main.tsx", 40, 1, 1),
                    test_file("src/App.tsx", 260, 8, 24),
                    test_file("src/components/app-shell/Chrome.tsx", 80, 2, 5),
                    test_file("src/app/desktop-session.ts", 80, 2, 5),
                ]),
            }),
            total_files: 4,
            total_lines: 460,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/main.tsx".into(),
                    to_file: "src/App.tsx".into(),
                },
                ImportEdge {
                    from_file: "src/App.tsx".into(),
                    to_file: "src/components/app-shell/Chrome.tsx".into(),
                },
                ImportEdge {
                    from_file: "src/App.tsx".into(),
                    to_file: "src/app/desktop-session.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/main.tsx".into(),
                func: "bootstrap".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/App.tsx".into(),
            value: 22,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "dependency_sprawl")
            .expect("dependency-sprawl report");

        assert!(report.role_tags.iter().any(|tag| tag == "composition_root"));
        assert_eq!(report.leverage_class, "regrowth_watchpoint");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "intentionally_central_surface"));
        assert!(report.summary.contains("Composition root"));
        assert!(report.impact.contains("composition root"));
    }

    #[test]
    fn dependency_sprawl_softens_direct_index_import_when_entry_points_are_missing() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/index.tsx", 40, 1, 1),
                    test_file("src/App.tsx", 260, 8, 24),
                    test_file("src/components/app-shell/Chrome.tsx", 80, 2, 5),
                ]),
            }),
            total_files: 3,
            total_lines: 380,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/index.tsx".into(),
                    to_file: "src/App.tsx".into(),
                },
                ImportEdge {
                    from_file: "src/App.tsx".into(),
                    to_file: "src/components/app-shell/Chrome.tsx".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/App.tsx".into(),
            value: 18,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "dependency_sprawl")
            .expect("dependency-sprawl report");

        assert!(report.role_tags.iter().any(|tag| tag == "composition_root"));
        assert!(report.summary.contains("Composition root"));
    }

    #[test]
    fn large_file_keeps_script_entry_surfaces_generic() {
        let snapshot = Snapshot {
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
                children: Some(vec![test_file("scripts/session-stress.mjs", 2048, 12, 14)]),
            }),
            total_files: 1,
            total_lines: 2048,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "scripts/session-stress.mjs".into(),
                func: "main".into(),
                lang: "javascript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.long_files = vec![FileMetric {
            path: "scripts/session-stress.mjs".into(),
            value: 2048,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "large_file")
            .expect("large-file report");

        assert_eq!(report.leverage_class, "tooling_debt");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "tooling_surface_maintenance_burden"));
        assert!(report
            .summary
            .starts_with("File 'scripts/session-stress.mjs'"));
        assert!(!report.impact.contains("composition root"));
    }

    #[test]
    fn unstable_hotspot_marks_transport_facades_as_boundary_discipline() {
        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/lib/ipc.ts", 180, 5, 12),
                    test_file("src/app/browser-session.ts", 90, 2, 5),
                    test_file("src/app/electron-session.ts", 90, 2, 5),
                ]),
            }),
            total_files: 3,
            total_lines: 360,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app/browser-session.ts".into(),
                    to_file: "src/lib/ipc.ts".into(),
                },
                ImportEdge {
                    from_file: "src/app/electron-session.ts".into(),
                    to_file: "src/lib/ipc.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.hotspot_files = vec![FileMetric {
            path: "src/lib/ipc.ts".into(),
            value: 24,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "unstable_hotspot")
            .expect("unstable-hotspot report");

        assert!(report.role_tags.iter().any(|tag| tag == "transport_facade"));
        assert_eq!(report.leverage_class, "boundary_discipline");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "guarded_or_transport_facade"));
    }

    #[test]
    fn cycle_cluster_score_boosts_boundary_hubs_and_cut_leverage() {
        let base_score = cycle_cluster_score(6, 900, &[], &[]);
        let boosted_score = cycle_cluster_score(
            6,
            900,
            &[
                "component_barrel".to_string(),
                "guarded_boundary".to_string(),
            ],
            &[CycleCutCandidate {
                seam_kind: "guarded_boundary_cut".to_string(),
                reduction_file_count: 3,
                remaining_cycle_size: 2,
                ..CycleCutCandidate::default()
            }],
        );

        assert!(boosted_score > base_score);
    }

    #[test]
    fn cycle_cluster_score_prefers_stronger_cut_reduction_over_weaker_remainder() {
        let stronger_cut = cycle_cluster_score(
            10,
            1_400,
            &["guarded_boundary".to_string()],
            &[CycleCutCandidate {
                seam_kind: "guarded_boundary_cut".to_string(),
                reduction_file_count: 6,
                remaining_cycle_size: 3,
                ..CycleCutCandidate::default()
            }],
        );
        let weaker_cut = cycle_cluster_score(
            10,
            1_400,
            &["guarded_boundary".to_string()],
            &[CycleCutCandidate {
                seam_kind: "guarded_boundary_cut".to_string(),
                reduction_file_count: 2,
                remaining_cycle_size: 8,
                ..CycleCutCandidate::default()
            }],
        );

        assert!(stronger_cut > weaker_cut);
    }

    #[test]
    fn dependency_sprawl_marks_nextjs_route_surfaces_as_regrowth_watchpoints() {
        let snapshot = Snapshot {
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
                children: Some(vec![test_file("src/app/[locale]/layout.tsx", 220, 6, 12)]),
            }),
            total_files: 1,
            total_lines: 220,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/app/[locale]/layout.tsx".into(),
            value: 18,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.scope == "src/app/[locale]/layout.tsx")
            .expect("route surface report");

        assert!(report.role_tags.iter().any(|tag| tag == "route_surface"));
        assert_eq!(report.leverage_class, "regrowth_watchpoint");
    }

    #[test]
    fn unstable_hotspot_marks_state_containers_as_architecture_signals() {
        let snapshot = Snapshot {
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
                children: Some(vec![test_file("src/store/chat-input.store.ts", 204, 5, 9)]),
            }),
            total_files: 1,
            total_lines: 204,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.hotspot_files = vec![FileMetric {
            path: "src/store/chat-input.store.ts".into(),
            value: 17,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.scope == "src/store/chat-input.store.ts")
            .expect("state container report");

        assert!(report.role_tags.iter().any(|tag| tag == "state_container"));
        assert_eq!(report.leverage_class, "architecture_signal");
    }

    #[test]
    fn dependency_sprawl_marks_service_http_surfaces_as_entry_surfaces() {
        let snapshot = Snapshot {
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
                children: Some(vec![test_file("src/routes/users.ts", 180, 4, 11)]),
            }),
            total_files: 1,
            total_lines: 180,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.god_files = vec![FileMetric {
            path: "src/routes/users.ts".into(),
            value: 15,
        }];

        let reports = build_structural_debt_reports(&snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.scope == "src/routes/users.ts")
            .expect("http handler report");

        assert!(report
            .role_tags
            .iter()
            .any(|tag| tag == "http_handler_surface"));
        assert!(report.role_tags.iter().any(|tag| tag == "entry_surface"));
    }

    #[test]
    fn cycle_cut_candidates_prefer_guarded_app_store_boundary_edges() {
        let root = temp_root("cycle-guarded-boundary");
        write_file(
            &root,
            "src/app/store-boundary.architecture.test.ts",
            "expect(source).not.toContain('store/store');\n",
        );
        write_file(
            &root,
            "src/store/store.ts",
            "export function selectStore() {}\n",
        );

        let snapshot = Snapshot {
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
                children: Some(vec![
                    test_file("src/store/store.ts", 220, 6, 18),
                    test_file("src/store/core.ts", 180, 4, 11),
                    test_file("src/app/task-workflows.ts", 240, 5, 17),
                ]),
            }),
            total_files: 3,
            total_lines: 640,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/store/store.ts".into(),
                    to_file: "src/app/task-workflows.ts".into(),
                },
                ImportEdge {
                    from_file: "src/app/task-workflows.ts".into(),
                    to_file: "src/store/store.ts".into(),
                },
                ImportEdge {
                    from_file: "src/store/core.ts".into(),
                    to_file: "src/store/store.ts".into(),
                },
                ImportEdge {
                    from_file: "src/store/store.ts".into(),
                    to_file: "src/store/core.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let mut health = empty_health_report();
        health.circular_dep_files = vec![vec![
            "src/app/task-workflows.ts".into(),
            "src/store/core.ts".into(),
            "src/store/store.ts".into(),
        ]];
        health.circular_dep_count = 1;

        let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
        let report = reports
            .iter()
            .find(|report| report.kind == "cycle_cluster")
            .expect("cycle-cluster report");
        let best_cut = report.cut_candidates.first().expect("best cut");

        assert_eq!(best_cut.seam_kind, "guarded_app_store_boundary");
        assert_eq!(report.leverage_class, "architecture_signal");
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "mixed_cycle_pressure"));
        assert!(report
            .leverage_reasons
            .iter()
            .any(|reason| reason == "high_leverage_cycle_cut"));
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_file(path: &str, lines: u32, funcs: u32, max_complexity: u32) -> FileNode {
        FileNode {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            is_dir: false,
            lines,
            logic: lines.saturating_sub(10),
            comments: 5,
            blanks: 5,
            funcs,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(StructuralAnalysis {
                functions: Some(vec![crate::core::types::FuncInfo {
                    n: "main".to_string(),
                    sl: 1,
                    el: lines,
                    ln: lines,
                    cc: Some(max_complexity),
                    cog: Some(max_complexity),
                    pc: Some(0),
                    bh: Some(1),
                    d: None,
                    co: None,
                    same_file_ref_count: None,
                    is_public: false,
                    is_method: false,
                }]),
                cls: None,
                imp: None,
                co: None,
                tags: None,
                comment_lines: None,
            }),
            children: None,
        }
    }

    fn empty_health_report() -> HealthReport {
        HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 0,
            circular_dep_files: Vec::new(),
            total_import_edges: 0,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 0,
            god_files: Vec::new(),
            hotspot_files: Vec::new(),
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: Vec::new(),
            long_files: Vec::new(),
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 0,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 0,
                max_depth: 0,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        }
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sentrux-structural-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let absolute_path = root.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&absolute_path, contents).expect("write file");
    }
}
