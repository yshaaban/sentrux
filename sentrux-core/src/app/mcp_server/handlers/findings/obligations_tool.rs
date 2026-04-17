use super::*;

pub fn obligations_def() -> ToolDef {
    ToolDef {
        name: "obligations",
        description: "Return required update sites for configured v2 concepts and conservative closed-domain exhaustiveness gaps.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all obligations or only obligations touched by working-tree changes (default all)."
                },
                "concept": {
                    "type": "string",
                    "description": "Optional concept id filter."
                },
                "file": {
                    "type": "string",
                    "description": "Optional file filter."
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional closed-domain symbol filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_obligations,
        invalidates_evolution: false,
    }
}

pub(crate) fn handle_obligations(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::ObligationScope::Changed,
        _ => crate::metrics::v2::ObligationScope::All,
    };
    let changed_files = if scope == crate::metrics::v2::ObligationScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };
    let concept_filter = args.get("concept").and_then(|value| value.as_str());
    let file_filter = args.get("file").and_then(|value| value.as_str());
    let symbol_filter = args.get("symbol").and_then(|value| value.as_str());
    let cached_snapshot = state.cached_snapshot.clone();

    let (_, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        scope,
        &changed_files,
    );
    let obligations = obligations
        .into_iter()
        .filter(|obligation| {
            concept_filter
                .map(|concept| obligation.concept_id.as_deref() == Some(concept))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            file_filter
                .map(|file| obligation.files.iter().any(|candidate| candidate == file))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            symbol_filter
                .map(|symbol| obligation.domain_symbol_name.as_deref() == Some(symbol))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let changed_concepts = crate::metrics::v2::changed_concepts_from_obligations(&obligations);
    let obligation_count = obligations.len();
    let missing_site_count: usize = obligations
        .iter()
        .map(|obligation| obligation.missing_sites.len())
        .sum();
    let context_burden: usize = obligations
        .iter()
        .map(|obligation| obligation.context_burden)
        .sum();
    let obligation_completeness_0_10000 =
        crate::metrics::v2::obligation_score_0_10000(&obligations);

    let mut response = json!({
        "kind": "obligations",
        "scope": if scope == crate::metrics::v2::ObligationScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "changed_concepts": changed_concepts,
        "obligation_count": obligation_count,
        "missing_site_count": missing_site_count,
        "context_burden": context_burden,
        "obligation_completeness_0_10000": obligation_completeness_0_10000,
        "obligations": obligations
    });
    if let Some(object) = response.as_object_mut() {
        insert_semantic_diagnostics(object, semantic_error);
    }
    Ok(response)
}
