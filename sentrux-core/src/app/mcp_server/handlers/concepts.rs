use super::*;

pub fn check_rules_def() -> ToolDef {
    ToolDef {
        name: "check_rules",
        description: "Check .sentrux/rules.toml architectural constraints. Returns pass/fail with specific violations.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_check_rules,
        invalidates_evolution: false,
    }
}

pub fn concepts_def() -> ToolDef {
    ToolDef {
        name: "concepts",
        description: "List configured v2 concepts plus guardrail-test evidence, conservative concept suggestions, and rule coverage.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_concepts,
        invalidates_evolution: false,
    }
}

pub fn project_shape_def() -> ToolDef {
    ToolDef {
        name: "project_shape",
        description: "Detect repo archetypes, candidate boundary roots, module public-API contracts, and starter v2 rules for onboarding a new project shape.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_project_shape,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_project_shape(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let (config, rules_error) = load_v2_rules_config(state, &root);
    let project_shape = project_shape_json_cached(state, &root, &snapshot, &config);

    let mut response = json!({
        "kind": "project_shape",
        "project": config.project,
        "project_shape": project_shape,
    });
    if let Some(object) = response.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![DiagnosticEntry::new("rules", rules_error.clone())],
            Vec::new(),
        );
    }
    Ok(response)
}

pub(crate) fn handle_concepts(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (config, rules_error) = load_v2_rules_config(state, &root);
    let graph = crate::analysis::concepts::extract_concept_graph(&config);
    let coverage = config.v2_rule_coverage();
    let guardrail_tests = crate::analysis::concepts::detect_guardrail_tests(&root, &config);
    let (inferred_concepts, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => (
            crate::analysis::concepts::infer_concepts(&config, &semantic),
            None,
        ),
        Ok(None) => (Vec::new(), None),
        Err(error) => (
            Vec::new(),
            merge_optional_errors(rules_error.clone(), Some(error)),
        ),
    };
    let matched_guardrail_tests = guardrail_tests
        .iter()
        .filter(|test| !test.matched_concepts.is_empty())
        .count();
    let guardrail_test_count = guardrail_tests.len();
    let inferred_concept_count = inferred_concepts.len();
    let snapshot = state.cached_snapshot.clone();
    let project_shape = optional_project_shape_json(
        state,
        &root,
        snapshot.as_ref().map(|snapshot| snapshot.as_ref()),
        &config,
    );

    let mut response = json!({
        "kind": "concepts",
        "project": config.project,
        "project_shape": project_shape,
        "semantic_cache": semantic_cache_status_json(state),
        "rule_coverage": coverage,
        "concepts": graph.concepts,
        "contracts": graph.contracts,
        "state_models": graph.state_models,
        "guardrail_tests": guardrail_tests,
        "inferred_concepts": inferred_concepts,
        "suppressions": config.suppress,
        "summary": {
            "configured_concept_count": graph.concepts.len(),
            "contract_count": graph.contracts.len(),
            "state_model_count": graph.state_models.len(),
            "guardrail_test_count": guardrail_test_count,
            "matched_guardrail_test_count": matched_guardrail_tests,
            "inferred_concept_count": inferred_concept_count,
        }
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(object, rules_error, semantic_error, Vec::new());
    }
    Ok(response)
}

pub fn explain_concept_def() -> ToolDef {
    ToolDef {
        name: "explain_concept",
        description: "Show one configured concept with its rules, semantic reads/writes, findings, obligations, and related contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Concept id from .sentrux/rules.toml."
                }
            },
            "required": ["id"]
        }),
        min_tier: Tier::Free,
        handler: handle_explain_concept,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_explain_concept(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let concept_id = args
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or("Missing 'id' argument")?;
    let config = load_rules_config(&root)?;
    let concept = config
        .concept
        .iter()
        .find(|concept| concept.id == concept_id)
        .cloned()
        .ok_or_else(|| format!("Unknown concept: {concept_id}"))?;
    let graph = crate::analysis::concepts::extract_concept_graph(&config);
    let semantic = analyze_semantic_snapshot(state, &root).ok().flatten();
    let cached_snapshot = state.cached_snapshot.clone();
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let explain_findings = semantic_findings
        .into_iter()
        .filter(|finding| finding.concept_id == concept_id)
        .collect::<Vec<_>>();
    let (suppression_application, rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&explain_findings));
    let explain_obligations = obligations
        .into_iter()
        .filter(|obligation| obligation.concept_id.as_deref() == Some(concept_id))
        .collect::<Vec<_>>();
    let related_contracts = related_contract_ids_for_concept(&config, &concept);
    let parity = semantic
        .as_ref()
        .map(|semantic| build_explain_concept_parity(&config, semantic, &root, &related_contracts));
    let semantic_summary = semantic
        .as_ref()
        .map(|semantic| build_concept_semantic_summary(&concept, semantic));
    let related_tests = describe_concept_related_tests(&root, &concept);

    let mut response = json!({
        "kind": "explain_concept",
        "concept": graph.concepts.into_iter().find(|candidate| candidate.id == concept_id),
        "related_contract_ids": related_contracts.into_iter().collect::<Vec<_>>(),
        "related_tests": related_tests,
        "findings": suppression_application.visible_findings,
        "obligations": explain_obligations,
        "parity": parity,
        "semantic": semantic_summary,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(object, rules_error, semantic_error, Vec::new());
    }
    Ok(response)
}

fn related_contract_ids_for_concept(
    config: &crate::metrics::rules::RulesConfig,
    concept: &crate::metrics::rules::ConceptRule,
) -> BTreeSet<String> {
    config
        .contract
        .iter()
        .filter(|contract| contract_relates_to_concept(contract, concept))
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>()
}

fn build_explain_concept_parity(
    config: &crate::metrics::rules::RulesConfig,
    semantic: &crate::analysis::semantic::SemanticSnapshot,
    root: &Path,
    related_contracts: &BTreeSet<String>,
) -> Vec<crate::metrics::v2::ContractParityReport> {
    crate::metrics::v2::build_parity_reports(
        config,
        semantic,
        root,
        crate::metrics::v2::ParityScope::All,
        &BTreeSet::new(),
    )
    .reports
    .into_iter()
    .filter(|report| related_contracts.contains(&report.id))
    .collect::<Vec<_>>()
}

fn build_concept_semantic_summary(
    concept: &crate::metrics::rules::ConceptRule,
    semantic: &crate::analysis::semantic::SemanticSnapshot,
) -> Value {
    let writes = crate::metrics::v2::relevant_writes(concept, semantic)
        .into_iter()
        .map(|write| {
            json!({
                "path": write.path,
                "symbol_name": write.symbol_name,
                "write_kind": write.write_kind,
                "line": write.line,
            })
        })
        .collect::<Vec<_>>();
    let reads = crate::metrics::v2::relevant_reads(concept, semantic)
        .into_iter()
        .map(|read| {
            json!({
                "path": read.path,
                "symbol_name": read.symbol_name,
                "read_kind": read.read_kind,
                "line": read.line,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "writes": writes,
        "reads": reads,
    })
}

pub fn trace_symbol_def() -> ToolDef {
    ToolDef {
        name: "trace_symbol",
        description: "Trace a symbol to declarations, reads, writes, configured concepts, related obligations, and related contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name or scoped query like path::Symbol."
                }
            },
            "required": ["symbol"]
        }),
        min_tier: Tier::Free,
        handler: handle_trace_symbol,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_trace_symbol(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let trace_context = load_trace_symbol_context(args, state)?;
    let query = trace_context.query.as_str();
    let symbol_matches = trace_symbol_matches(&trace_context.semantic, query);
    let related_concepts = trace_related_concepts(&trace_context.config, query);
    let related_contracts = trace_related_contracts(&trace_context.config, query);
    let cached_snapshot = state.cached_snapshot.clone();
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &trace_context.root,
        cached_snapshot.as_deref(),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let semantic_error =
        semantic_error.filter(|error| Some(error) != trace_context.rules_error.as_ref());
    let findings = semantic_findings
        .into_iter()
        .filter(|finding| {
            related_concepts.contains(&finding.concept_id)
                || symbol_query_matches("", &finding.concept_id, query)
        })
        .collect::<Vec<_>>();
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &trace_context.root, serialized_values(&findings));
    let obligations = filter_trace_symbol_obligations(obligations, &related_concepts, query);
    let reference_ambiguity = trace_reference_ambiguity(query, &symbol_matches);
    let rules_error = merge_optional_errors(trace_context.rules_error, suppression_rules_error);
    let mut response = json!({
        "kind": "trace_symbol",
        "symbol": query,
        "declarations": symbol_matches.declarations,
        "reads": symbol_matches.reads,
        "writes": symbol_matches.writes,
        "related_concepts": related_concepts.into_iter().collect::<Vec<_>>(),
        "related_contracts": related_contracts.into_iter().collect::<Vec<_>>(),
        "findings": suppression_application.visible_findings,
        "obligations": obligations,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "reference_ambiguity": reference_ambiguity,
    });
    if let Some(object) = response.as_object_mut() {
        insert_rules_semantic_diagnostics(object, rules_error, semantic_error, Vec::new());
    }
    Ok(response)
}

struct TraceSymbolContext {
    root: PathBuf,
    query: String,
    config: crate::metrics::rules::RulesConfig,
    semantic: crate::analysis::semantic::SemanticSnapshot,
    rules_error: Option<String>,
}

struct TraceSymbolMatches {
    declarations: Vec<Value>,
    reads: Vec<Value>,
    writes: Vec<Value>,
    ambiguous_declarations: Vec<Value>,
    references_are_ambiguous: bool,
}

fn load_trace_symbol_context(
    args: &Value,
    state: &mut McpState,
) -> Result<TraceSymbolContext, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let query = args
        .get("symbol")
        .and_then(|value| value.as_str())
        .ok_or("Missing 'symbol' argument")?
        .to_string();
    let (config, rules_error) = load_v2_rules_config(state, &root);
    let semantic = analyze_semantic_snapshot(state, &root)
        .map_err(|error| {
            merge_optional_errors(rules_error.clone(), Some(error))
                .unwrap_or_else(|| "Semantic analysis unavailable".to_string())
        })?
        .ok_or_else(|| {
            merge_optional_errors(
                rules_error.clone(),
                Some(
                    "Symbol tracing requires TypeScript semantic analysis for this project"
                        .to_string(),
                ),
            )
            .unwrap()
        })?;
    Ok(TraceSymbolContext {
        root,
        query,
        config,
        semantic,
        rules_error,
    })
}

fn trace_symbol_matches(
    semantic: &crate::analysis::semantic::SemanticSnapshot,
    query: &str,
) -> TraceSymbolMatches {
    let (query_path, query_symbol) = split_symbol_query(query);
    let matched_declarations = semantic
        .symbols
        .iter()
        .filter(|symbol| symbol_query_matches(&symbol.path, &symbol.name, query))
        .collect::<Vec<_>>();
    let ambiguous_declarations = query_path
        .as_deref()
        .map(|scoped_path| {
            semantic
                .symbols
                .iter()
                .filter(|symbol| {
                    symbol.path != scoped_path
                        && symbol_query_matches("", &symbol.name, query_symbol)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let references_are_ambiguous = query_path.is_some() && !ambiguous_declarations.is_empty();

    TraceSymbolMatches {
        declarations: matched_declarations
            .iter()
            .map(|symbol| trace_symbol_declaration_json(symbol))
            .collect(),
        reads: trace_symbol_reference_jsons(
            &semantic.reads,
            query,
            references_are_ambiguous,
            |read| {
                json!({
                    "path": read.path,
                    "symbol_name": read.symbol_name,
                    "read_kind": read.read_kind,
                    "line": read.line,
                })
            },
            |read| &read.symbol_name,
        ),
        writes: trace_symbol_reference_jsons(
            &semantic.writes,
            query,
            references_are_ambiguous,
            |write| {
                json!({
                    "path": write.path,
                    "symbol_name": write.symbol_name,
                    "write_kind": write.write_kind,
                    "line": write.line,
                })
            },
            |write| &write.symbol_name,
        ),
        ambiguous_declarations: ambiguous_declarations
            .iter()
            .map(|symbol| trace_symbol_declaration_json(symbol))
            .collect(),
        references_are_ambiguous,
    }
}

fn trace_symbol_declaration_json(symbol: &crate::analysis::semantic::SymbolFact) -> Value {
    json!({
        "id": symbol.id,
        "path": symbol.path,
        "name": symbol.name,
        "kind": symbol.kind,
        "line": symbol.line,
    })
}

fn trace_symbol_reference_jsons<T, FValue, FName>(
    references: &[T],
    query: &str,
    references_are_ambiguous: bool,
    to_json: FValue,
    symbol_name: FName,
) -> Vec<Value>
where
    FValue: Fn(&T) -> Value,
    FName: Fn(&T) -> &str,
{
    references
        .iter()
        .filter(|reference| {
            !references_are_ambiguous && symbol_query_matches("", symbol_name(reference), query)
        })
        .map(to_json)
        .collect::<Vec<_>>()
}

fn trace_related_concepts(
    config: &crate::metrics::rules::RulesConfig,
    query: &str,
) -> BTreeSet<String> {
    config
        .concept
        .iter()
        .filter(|concept| concept_matches_symbol(concept, query))
        .map(|concept| concept.id.clone())
        .collect::<BTreeSet<_>>()
}

fn trace_related_contracts(
    config: &crate::metrics::rules::RulesConfig,
    query: &str,
) -> BTreeSet<String> {
    config
        .contract
        .iter()
        .filter(|contract| contract_matches_symbol(contract, query))
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>()
}

fn filter_trace_symbol_obligations(
    obligations: Vec<crate::metrics::v2::ObligationReport>,
    related_concepts: &BTreeSet<String>,
    query: &str,
) -> Vec<crate::metrics::v2::ObligationReport> {
    obligations
        .into_iter()
        .filter(|obligation| {
            obligation
                .domain_symbol_name
                .as_deref()
                .map(|symbol_name| symbol_query_matches("", symbol_name, query))
                .unwrap_or(false)
                || obligation
                    .concept_id
                    .as_deref()
                    .map(|concept_id| related_concepts.contains(concept_id))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>()
}

fn trace_reference_ambiguity(query: &str, symbol_matches: &TraceSymbolMatches) -> Option<Value> {
    if !symbol_matches.references_are_ambiguous {
        return None;
    }

    Some(json!({
        "message": format!(
            "Scoped query '{}' matches additional declarations in other files, so cross-file reads and writes are omitted to avoid false positives",
            query
        ),
        "conflicting_declarations": symbol_matches.ambiguous_declarations,
    }))
}

fn describe_concept_related_tests(
    root: &Path,
    concept: &crate::metrics::rules::ConceptRule,
) -> Vec<Value> {
    concept
        .related_tests
        .iter()
        .map(|pattern| {
            let matches = matching_project_paths(root, pattern);
            json!({
                "pattern": pattern,
                "matched_files": matches,
                "exists": !matches.is_empty(),
            })
        })
        .collect()
}

fn matching_project_paths(root: &Path, pattern: &str) -> Vec<String> {
    let has_glob = pattern.contains('*') || pattern.contains('?') || pattern.contains('[');
    if !has_glob {
        return if root.join(pattern).exists() {
            vec![pattern.to_string()]
        } else {
            Vec::new()
        };
    }

    let mut matches = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let relative_path = path.strip_prefix(root).ok()?;
            let relative_path = relative_path.to_string_lossy().replace('\\', "/");
            if crate::metrics::rules::glob_match(pattern, &relative_path) {
                Some(relative_path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

fn concept_rule_files(concept: &crate::metrics::rules::ConceptRule) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for scoped_path in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            files.insert(path.to_string());
        }
    }
    files.extend(concept.related_tests.iter().cloned());
    files
}

fn contract_rule_files(contract: &crate::metrics::rules::ContractRule) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for scoped_path in [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            files.insert(path.to_string());
        }
    }
    files.extend(contract.browser_entry.iter().cloned());
    files.extend(contract.electron_entry.iter().cloned());
    files
}

fn contract_relates_to_concept(
    contract: &crate::metrics::rules::ContractRule,
    concept: &crate::metrics::rules::ConceptRule,
) -> bool {
    let concept_files = concept_rule_files(concept);
    let contract_files = contract_rule_files(contract);
    if !concept_files.is_disjoint(&contract_files) {
        return true;
    }

    let concept_targets = crate::metrics::v2::concept_targets(concept);
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(crate::metrics::v2::symbol_from_scoped_path)
    .any(|symbol_name| crate::metrics::v2::symbol_matches_targets(&symbol_name, &concept_targets))
}

fn concept_matches_symbol(concept: &crate::metrics::rules::ConceptRule, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
        .any(|target| scoped_target_matches_query(target, query_path.as_deref(), query_symbol))
}

fn contract_matches_symbol(contract: &crate::metrics::rules::ContractRule, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|target| scoped_target_matches_query(target, query_path.as_deref(), query_symbol))
}

fn split_symbol_query(query: &str) -> (Option<String>, &str) {
    match query.split_once("::") {
        Some((path, symbol_name)) => (Some(path.replace('\\', "/")), symbol_name),
        None => (None, query),
    }
}

fn symbol_query_matches(path: &str, symbol_name: &str, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    if let Some(query_path) = query_path {
        if !path.is_empty() && path != query_path {
            return false;
        }
    }

    crate::metrics::v2::symbol_matches_targets(
        symbol_name,
        &HashSet::from([query_symbol.to_string()]),
    )
}

fn scoped_target_matches_query(target: &str, query_path: Option<&str>, query_symbol: &str) -> bool {
    let (path, symbol_name) = match target.split_once("::") {
        Some(parts) => parts,
        None => return false,
    };
    if let Some(query_path) = query_path {
        if path != query_path {
            return false;
        }
    }

    crate::metrics::v2::symbol_matches_targets(
        symbol_name,
        &HashSet::from([query_symbol.to_string()]),
    )
}

pub(crate) fn handle_check_rules(
    _args: &Value,
    tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .as_ref()
        .ok_or("No scan root. Call 'scan' first.")?;
    let h = state
        .cached_health
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let a = state
        .cached_arch
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snap = state
        .cached_snapshot
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;

    let mut config = load_rules_config(root)?;

    let total_rules = config.constraints.count_active()
        + config.layers.len()
        + config.boundaries.len()
        + config.module_contract.len();
    let truncated = if !tier.is_pro() && total_rules > 3 {
        let mut remaining = 3usize.saturating_sub(if config.constraints.count_active() > 0 {
            1
        } else {
            0
        });
        config.layers.truncate(remaining.min(config.layers.len()));
        remaining = remaining.saturating_sub(config.layers.len());
        config
            .boundaries
            .truncate(remaining.min(config.boundaries.len()));
        remaining = remaining.saturating_sub(config.boundaries.len());
        config
            .module_contract
            .truncate(remaining.min(config.module_contract.len()));
        true
    } else {
        false
    };

    let result = crate::metrics::rules::check_rules(&config, h, a, &snap.import_graph);
    let v2_rule_coverage = config.v2_rule_coverage();

    let mut response = json!({
        "kind": "check_rules",
        "pass": result.passed,
        "rules_checked": result.rules_checked,
        "violation_count": result.violations.len(),
        "v2_rule_coverage": v2_rule_coverage,
        "violations": result.violations.iter().map(|v| json!({
            "rule": v.rule,
            "severity": format!("{:?}", v.severity),
            "message": v.message,
            "files": v.files
        })).collect::<Vec<_>>(),
        "summary": if result.passed { "✓ All architectural rules pass" }
            else { "✗ Architectural rule violations detected" }
    });
    if truncated {
        response["truncated"] = json!({
            "total_rules_defined": total_rules,
            "rules_checked": result.rules_checked,
            "message": "Checking up to 3 rules. More available with sentrux Pro: https://github.com/yshaaban/sentrux"
        });
    }
    if let Some(object) = response.as_object_mut() {
        insert_diagnostics(object, Vec::new(), Vec::new(), false);
    }
    Ok(response)
}
