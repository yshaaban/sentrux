use super::{McpState, RulesCacheIdentity};
use crate::metrics::rules::{RulesConfig, SuppressionRule};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct SuppressionMatch {
    pub kind: String,
    pub concept: Option<String>,
    pub file: Option<String>,
    pub reason: String,
    pub expires: Option<String>,
    pub expired: bool,
    pub matched_finding_count: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SuppressionApplication {
    pub visible_findings: Vec<Value>,
    pub active_matches: Vec<SuppressionMatch>,
    pub expired_matches: Vec<SuppressionMatch>,
}

pub(crate) fn current_rules_cache_identity(root: &Path) -> RulesCacheIdentity {
    let rules_path = root.join(".sentrux").join("rules.toml");
    let metadata = std::fs::metadata(&rules_path).ok();

    RulesCacheIdentity {
        rules_path,
        exists: metadata.is_some(),
        len: metadata.as_ref().map(std::fs::Metadata::len),
        modified_unix_nanos: metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
    }
}

pub(crate) fn empty_rules_config() -> RulesConfig {
    RulesConfig {
        project: Default::default(),
        constraints: Default::default(),
        language: Default::default(),
        layers: Vec::new(),
        boundaries: Vec::new(),
        concept: Vec::new(),
        contract: Vec::new(),
        state_model: Vec::new(),
        module_contract: Vec::new(),
        suppress: Vec::new(),
    }
}

fn cached_rules_config_or_empty(state: &McpState) -> RulesConfig {
    state
        .cached_rules_config
        .clone()
        .unwrap_or_else(empty_rules_config)
}

pub(crate) fn load_v2_rules_config(
    state: &mut McpState,
    root: &Path,
) -> (RulesConfig, Option<String>) {
    let identity = current_rules_cache_identity(root);
    if state.cached_rules_identity.as_ref() == Some(&identity) {
        return (
            cached_rules_config_or_empty(state),
            state.cached_rules_error.clone(),
        );
    }

    let (config, error) = if !identity.exists {
        (empty_rules_config(), None)
    } else {
        match RulesConfig::load(&identity.rules_path) {
            Ok(config) => (config, None),
            Err(error) => (empty_rules_config(), Some(error)),
        }
    };

    state.cached_rules_identity = Some(identity);
    state.cached_rules_config = Some(config.clone());
    state.cached_rules_error = error.clone();

    (config, error)
}

pub(crate) fn invalidate_rules_cache(state: &mut McpState) {
    if state.cached_rules_identity.is_none()
        && state.cached_rules_config.is_none()
        && state.cached_rules_error.is_none()
    {
        return;
    }

    state.cached_rules_identity = None;
    state.cached_rules_config = None;
    state.cached_rules_error = None;
}

pub(crate) fn semantic_rules_loaded(config: &RulesConfig) -> bool {
    !config.concept.is_empty() || !config.contract.is_empty() || !config.state_model.is_empty()
}

pub(crate) fn semantic_cache_status_json(state: &McpState) -> Value {
    match (
        state.cached_semantic.as_ref(),
        state.cached_semantic_identity.as_ref(),
        state.cached_semantic_source,
    ) {
        (Some(snapshot), Some(identity), Some(source)) => json!({
            "available": true,
            "source": source.as_str(),
            "project_fingerprint": identity.project_fingerprint,
            "bridge_protocol_version": identity.bridge_protocol_version,
            "git_head": identity.git_head,
            "working_tree_path_count": identity.working_tree_paths.len(),
            "analyzed_files": snapshot.analyzed_files,
        }),
        _ => json!({
            "available": false,
        }),
    }
}

pub(crate) fn apply_root_suppressions(
    state: &mut McpState,
    root: &Path,
    findings: Vec<Value>,
) -> (SuppressionApplication, Option<String>) {
    let (config, rules_error) = load_v2_rules_config(state, root);
    (apply_suppressions(&config, findings), rules_error)
}

pub(crate) fn suppression_match_count(matches: &[SuppressionMatch]) -> usize {
    matches
        .iter()
        .map(|matched| matched.matched_finding_count)
        .sum()
}

pub(crate) fn apply_suppressions(
    config: &RulesConfig,
    findings: Vec<Value>,
) -> SuppressionApplication {
    let mut visible_findings = Vec::new();
    let mut active_matches = BTreeMap::<String, SuppressionMatch>::new();
    let mut expired_matches = BTreeMap::<String, SuppressionMatch>::new();

    for finding in findings {
        let mut suppressed = false;
        for suppression in &config.suppress {
            if !suppression_matches_finding(suppression, &finding) {
                continue;
            }

            let expired = suppression_is_expired(suppression);
            let entry = suppression_match_entry(suppression, expired);
            let key = stable_json_key(&serde_json::to_value(&entry).unwrap_or_else(|_| json!({})));
            let target_map = if entry.expired {
                &mut expired_matches
            } else {
                &mut active_matches
            };
            target_map
                .entry(key)
                .and_modify(|matched| matched.matched_finding_count += 1)
                .or_insert_with(|| {
                    let mut matched = entry;
                    matched.matched_finding_count = 1;
                    matched
                });
            suppressed |= !expired;
        }

        if !suppressed {
            visible_findings.push(finding);
        }
    }

    SuppressionApplication {
        visible_findings,
        active_matches: active_matches.into_values().collect(),
        expired_matches: expired_matches.into_values().collect(),
    }
}

fn suppression_match_entry(suppression: &SuppressionRule, expired: bool) -> SuppressionMatch {
    SuppressionMatch {
        kind: suppression.kind.clone(),
        concept: suppression.concept.clone(),
        file: suppression.file.clone(),
        reason: suppression.reason.clone(),
        expires: suppression.expires.clone(),
        expired,
        matched_finding_count: 0,
    }
}

fn suppression_matches_finding(suppression: &SuppressionRule, finding: &Value) -> bool {
    if !suppression_kind_matches(&suppression.kind, finding_kind(finding)) {
        return false;
    }
    if let Some(concept) = &suppression.concept {
        if finding_concept_id(finding) != Some(concept.as_str()) {
            return false;
        }
    }
    if let Some(file_pattern) = &suppression.file {
        if !finding_files(finding)
            .iter()
            .any(|file| crate::metrics::rules::glob_match(file_pattern, file))
        {
            return false;
        }
    }

    true
}

fn suppression_kind_matches(pattern: &str, finding_kind: &str) -> bool {
    pattern == "*" || pattern == finding_kind
}

fn suppression_is_expired(suppression: &SuppressionRule) -> bool {
    suppression
        .expires
        .as_deref()
        .and_then(parse_date)
        .map(|expires| expires < today_utc())
        .unwrap_or(false)
}

fn parse_date(value: &str) -> Option<time::Date> {
    time::Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()
}

fn today_utc() -> time::Date {
    time::OffsetDateTime::now_utc().date()
}

fn stable_json_key(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => {
            let mut items = values.iter().map(stable_json_key).collect::<Vec<_>>();
            items.sort();
            format!("[{}]", items.join(","))
        }
        Value::Object(map) => {
            let mut entries = map
                .iter()
                .map(|(key, value)| format!("{key}:{}", stable_json_key(value)))
                .collect::<Vec<_>>();
            entries.sort();
            format!("{{{}}}", entries.join(","))
        }
    }
}

fn finding_kind(finding: &Value) -> &str {
    finding.get("kind").and_then(Value::as_str).unwrap_or("")
}

fn finding_concept_id(finding: &Value) -> Option<&str> {
    finding.get("concept_id").and_then(Value::as_str)
}

fn finding_files(finding: &Value) -> Vec<String> {
    let Some(values) = finding.get("files").and_then(Value::as_array) else {
        return Vec::new();
    };

    values
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}
