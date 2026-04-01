use serde_json::{json, Map, Value};

pub(crate) struct DiagnosticEntry {
    key: &'static str,
    value: Option<String>,
}

impl DiagnosticEntry {
    pub(crate) fn new(key: &'static str, value: Option<String>) -> Self {
        Self { key, value }
    }
}

fn diagnostic_error_map(errors: Vec<DiagnosticEntry>) -> Map<String, Value> {
    let mut error_map = Map::new();
    for entry in errors {
        error_map.insert(entry.key.to_string(), json!(entry.value));
    }

    error_map
}

pub(crate) fn insert_diagnostics(
    object: &mut Map<String, Value>,
    errors: Vec<DiagnosticEntry>,
    warnings: Vec<String>,
    partial_results: bool,
) {
    object.insert(
        "diagnostics".to_string(),
        json!({
            "errors": diagnostic_error_map(errors),
            "warnings": warnings,
            "partial_results": partial_results,
        }),
    );
}

pub(crate) fn insert_error_diagnostics(
    object: &mut Map<String, Value>,
    errors: Vec<DiagnosticEntry>,
    warnings: Vec<String>,
) {
    let partial_results = errors.iter().any(|entry| entry.value.is_some());
    insert_diagnostics(object, errors, warnings, partial_results);
}

pub(crate) fn extend_diagnostics(object: &mut Map<String, Value>, details: Map<String, Value>) {
    if let Some(diagnostics) = object.get_mut("diagnostics").and_then(Value::as_object_mut) {
        diagnostics.extend(details);
    }
}

pub(crate) fn extend_diagnostics_availability(
    object: &mut Map<String, Value>,
    entries: Vec<(&'static str, bool)>,
) {
    let availability = entries
        .into_iter()
        .map(|(key, available)| (key.to_string(), Value::Bool(available)))
        .collect::<Map<_, _>>();
    extend_diagnostics(
        object,
        Map::from_iter([("availability".to_string(), Value::Object(availability))]),
    );
}

pub(crate) fn insert_semantic_diagnostics(
    object: &mut Map<String, Value>,
    semantic_error: Option<String>,
) {
    insert_error_diagnostics(
        object,
        vec![DiagnosticEntry::new("semantic", semantic_error)],
        Vec::new(),
    );
}

pub(crate) fn insert_rules_semantic_diagnostics(
    object: &mut Map<String, Value>,
    rules_error: Option<String>,
    semantic_error: Option<String>,
    warnings: Vec<String>,
) {
    insert_error_diagnostics(
        object,
        vec![
            DiagnosticEntry::new("rules", rules_error),
            DiagnosticEntry::new("semantic", semantic_error),
        ],
        warnings,
    );
}

pub(crate) fn insert_rules_semantic_context_diagnostics(
    object: &mut Map<String, Value>,
    rules_error: Option<String>,
    semantic_error: Option<String>,
    context_error: Option<String>,
) {
    insert_error_diagnostics(
        object,
        vec![
            DiagnosticEntry::new("rules", rules_error),
            DiagnosticEntry::new("semantic", semantic_error),
            DiagnosticEntry::new("context", context_error),
        ],
        Vec::new(),
    );
}

pub(crate) fn insert_rules_semantic_evolution_diagnostics(
    object: &mut Map<String, Value>,
    rules_error: Option<String>,
    semantic_error: Option<String>,
    evolution_error: Option<String>,
    warnings: Vec<String>,
) {
    insert_error_diagnostics(
        object,
        vec![
            DiagnosticEntry::new("rules", rules_error),
            DiagnosticEntry::new("semantic", semantic_error),
            DiagnosticEntry::new("evolution", evolution_error),
        ],
        warnings,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        extend_diagnostics, extend_diagnostics_availability, insert_error_diagnostics,
        DiagnosticEntry,
    };
    use serde_json::{json, Map, Value};

    #[test]
    fn insert_error_diagnostics_sets_partial_results_when_any_error_is_present() {
        let mut object = Map::new();
        insert_error_diagnostics(
            &mut object,
            vec![
                DiagnosticEntry::new("rules", None),
                DiagnosticEntry::new("semantic", Some("semantic unavailable".to_string())),
            ],
            vec!["used cached semantic snapshot".to_string()],
        );

        assert_eq!(
            object.get("diagnostics"),
            Some(&json!({
                "errors": {
                    "rules": null,
                    "semantic": "semantic unavailable",
                },
                "warnings": ["used cached semantic snapshot"],
                "partial_results": true,
            }))
        );
    }

    #[test]
    fn extend_diagnostics_merges_extra_sections_into_existing_diagnostics() {
        let mut object = Map::new();
        insert_error_diagnostics(
            &mut object,
            vec![DiagnosticEntry::new("rules", None)],
            Vec::new(),
        );

        extend_diagnostics(
            &mut object,
            Map::from_iter([(
                "root_cause".to_string(),
                Value::String("modularity".to_string()),
            )]),
        );

        assert_eq!(
            object.get("diagnostics"),
            Some(&json!({
                "errors": { "rules": null },
                "warnings": [],
                "partial_results": false,
                "root_cause": "modularity",
            }))
        );
    }

    #[test]
    fn extend_diagnostics_availability_adds_availability_flags() {
        let mut object = Map::new();
        insert_error_diagnostics(
            &mut object,
            vec![DiagnosticEntry::new("rules", None)],
            Vec::new(),
        );

        extend_diagnostics_availability(&mut object, vec![("evolution", false)]);

        assert_eq!(
            object.get("diagnostics"),
            Some(&json!({
                "errors": { "rules": null },
                "warnings": [],
                "partial_results": false,
                "availability": {
                    "evolution": false,
                },
            }))
        );
    }
}
