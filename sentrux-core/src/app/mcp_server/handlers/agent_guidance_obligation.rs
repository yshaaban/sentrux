use super::super::agent_format::{IssueConfidence, IssueOrigin};
use super::packets::{
    build_repair_packet, obligation_risk_statement, verify_after_for_kind, RepairPacket,
};
use super::shared::{
    describe_list, fallback_obligation_origin, guidance_kind_profile, looks_like_test_surface,
    obligation_family, ObligationFamily, ObligationGuidanceInput, ObligationSiteInput,
    INCOMPLETE_PROPAGATION_HINT, VERIFY_AFTER_GATE_MESSAGE,
};
use crate::metrics::v2::FindingSeverity;
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn repair_packet_for_obligation(obligation: &Value, kind: &str) -> RepairPacket {
    let input = ObligationGuidanceInput::from_value(obligation);
    let likely_fix_sites = obligation_likely_fix_sites(&input);
    let inspection_context = inspection_context_for_obligation(&input);
    let verify_after =
        obligation_verify_after(kind, &input, &likely_fix_sites, &inspection_context);

    build_repair_packet(
        kind,
        likely_fix_sites,
        inspection_context,
        obligation_fix_hint_from_input(&input, kind),
        obligation_risk_statement(kind),
        Some(verify_after),
    )
}

pub(crate) fn obligation_message(obligation: &Value, kind: &str) -> String {
    obligation_message_from_input(&ObligationGuidanceInput::from_value(obligation), kind)
}

pub(crate) fn obligation_fix_hint(obligation: &Value, kind: &str) -> Option<String> {
    obligation_fix_hint_from_input(&ObligationGuidanceInput::from_value(obligation), kind)
}

pub(crate) fn obligation_origin(obligation: &Value) -> IssueOrigin {
    let input = ObligationGuidanceInput::from_value(obligation);
    input
        .origin
        .unwrap_or_else(|| fallback_obligation_origin(&input))
}

pub(crate) fn obligation_confidence(obligation: &Value) -> IssueConfidence {
    let input = ObligationGuidanceInput::from_value(obligation);
    input.confidence.unwrap_or_else(|| {
        match input
            .origin
            .unwrap_or_else(|| fallback_obligation_origin(&input))
        {
            IssueOrigin::Explicit => IssueConfidence::High,
            IssueOrigin::ZeroConfig => IssueConfidence::Medium,
        }
    })
}

pub(crate) fn obligation_trust_tier(obligation: &Value) -> &'static str {
    let input = ObligationGuidanceInput::from_value(obligation);
    if let Some(trust_tier) = input.trust_tier.as_deref() {
        return match trust_tier {
            "trusted" => "trusted",
            "watchpoint" => "watchpoint",
            _ => "watchpoint",
        };
    }

    match input
        .origin
        .unwrap_or_else(|| fallback_obligation_origin(&input))
    {
        IssueOrigin::Explicit => "trusted",
        IssueOrigin::ZeroConfig => "watchpoint",
    }
}

pub(crate) fn obligation_severity(obligation: &Value) -> FindingSeverity {
    let input = ObligationGuidanceInput::from_value(obligation);
    input.severity.unwrap_or_else(|| {
        let exhaustiveness_gap = matches!(
            obligation_family(obligation_kind(obligation)),
            ObligationFamily::Exhaustiveness
        ) || !input.missing_variants.is_empty();
        let explicit_origin = input
            .origin
            .unwrap_or_else(|| fallback_obligation_origin(&input))
            == IssueOrigin::Explicit;

        if exhaustiveness_gap || explicit_origin {
            FindingSeverity::High
        } else {
            FindingSeverity::Medium
        }
    })
}

pub(crate) fn obligation_score_0_10000(obligation: &Value) -> u32 {
    let input = ObligationGuidanceInput::from_value(obligation);
    if let Some(score) = input.score_0_10000 {
        return score;
    }

    let severity_bonus = match obligation_severity(obligation) {
        FindingSeverity::High => 1800,
        FindingSeverity::Medium => 1000,
        FindingSeverity::Low => 200,
    };
    let origin_bonus = match obligation_origin(obligation) {
        IssueOrigin::Explicit => 1600,
        IssueOrigin::ZeroConfig => 600,
    };
    let site_bonus = obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| (sites.len().min(3) as u32) * 500)
        .unwrap_or_default();
    (6000 + severity_bonus + origin_bonus + site_bonus).min(10_000)
}

pub(crate) fn obligation_files(obligation: &Value) -> Vec<String> {
    ObligationGuidanceInput::from_value(obligation).files
}

pub(crate) fn obligation_line(obligation: &Value) -> Option<u32> {
    prioritized_missing_sites(&ObligationGuidanceInput::from_value(obligation))
        .iter()
        .find_map(|site| site.line)
}

pub(crate) fn obligation_evidence(obligation: &Value) -> Vec<String> {
    prioritized_missing_sites(&ObligationGuidanceInput::from_value(obligation))
        .into_iter()
        .map(|site| format!("{} [{}]", display_site(&site), site.detail))
        .collect::<Vec<_>>()
}

fn obligation_message_from_input(obligation: &ObligationGuidanceInput, kind: &str) -> String {
    match obligation_family(kind) {
        ObligationFamily::Propagation => {
            let labeled_surfaces = fully_labeled_obligation_surfaces(obligation);
            if let Some(surface_phrase) = obligation_surface_phrase(&labeled_surfaces, true) {
                format!(
                    "Propagation is incomplete for '{}': update {} listed in the evidence.",
                    obligation.scope_label(),
                    surface_phrase
                )
            } else {
                format!(
                    "Propagation is incomplete for '{}': update the remaining sibling surfaces listed in the evidence.",
                    obligation.scope_label()
                )
            }
        }
        ObligationFamily::Exhaustiveness => {
            let domain = obligation_domain_label(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            let site_suffix = obligation_site_suffix(obligation);

            if !missing_variants.is_empty() {
                format!(
                    "Domain '{}' still needs explicit handling for variants [{}]{}.",
                    domain,
                    missing_variants.join(", "),
                    site_suffix
                )
            } else {
                format!(
                    "Domain '{}' still needs an explicit exhaustive branch{}.",
                    domain, site_suffix
                )
            }
        }
        ObligationFamily::Generic => obligation
            .summary
            .clone()
            .unwrap_or_else(|| "Changed concept still has missing update sites".to_string()),
    }
}

fn obligation_fix_hint_from_input(
    obligation: &ObligationGuidanceInput,
    kind: &str,
) -> Option<String> {
    match obligation_family(kind) {
        ObligationFamily::Propagation => {
            let labeled_surfaces = fully_labeled_obligation_surfaces(obligation);
            if let Some(surface_phrase) = obligation_surface_phrase(&labeled_surfaces, true) {
                return Some(format!(
                    "Update {} listed in the evidence before considering the change complete.",
                    surface_phrase
                ));
            }

            Some(INCOMPLETE_PROPAGATION_HINT.to_string())
        }
        ObligationFamily::Exhaustiveness => {
            let site_suffix = obligation_site_suffix(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            let test_doc_suffix = if obligation_has_surface_label(obligation, "test/doc") {
                " Then refresh the sibling test/doc surface so the new variant stays covered."
            } else {
                ""
            };
            if !missing_variants.is_empty() {
                return Some(format!(
                    "Handle the missing variants [{}] with an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch.{test_doc_suffix}",
                    missing_variants.join(", "),
                ));
            }

            Some(format!(
                "Add an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch.{test_doc_suffix}"
            ))
        }
        ObligationFamily::Generic => {
            let labeled_surfaces = fully_labeled_obligation_surfaces(obligation);
            if let Some(surface_phrase) = obligation_surface_phrase(&labeled_surfaces, false) {
                return Some(format!(
                    "Update {} tied to the changed concept before continuing.",
                    surface_phrase
                ));
            }

            Some(
                "Update the missing sites tied to the changed concept before continuing."
                    .to_string(),
            )
        }
    }
}

fn obligation_domain_label(obligation: &ObligationGuidanceInput) -> String {
    obligation
        .domain_symbol_name
        .as_deref()
        .or(obligation.concept_id.as_deref())
        .or(obligation.concept.as_deref())
        .unwrap_or("closed domain")
        .to_string()
}

fn obligation_missing_variants(obligation: &ObligationGuidanceInput) -> Vec<String> {
    obligation.missing_variants.clone()
}

fn obligation_missing_site(obligation: &ObligationGuidanceInput) -> Option<String> {
    prioritized_missing_sites(obligation)
        .first()
        .map(display_site)
}

fn obligation_site_suffix(obligation: &ObligationGuidanceInput) -> String {
    obligation_missing_site(obligation)
        .map(|site| format!(" at {site}"))
        .unwrap_or_default()
}

fn obligation_likely_fix_sites(obligation: &ObligationGuidanceInput) -> Vec<String> {
    prioritized_missing_sites(obligation)
        .into_iter()
        .map(|site| display_site(&site))
        .take(5)
        .collect::<Vec<_>>()
}

fn inspection_context_for_obligation(obligation: &ObligationGuidanceInput) -> Vec<String> {
    let mut context = prioritized_missing_sites(obligation)
        .into_iter()
        .map(|site| site.path)
        .collect::<Vec<_>>();
    context.extend(obligation.files.iter().cloned());
    dedupe_strings_preserve_order(context)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
}

fn obligation_verify_after(
    kind: &str,
    obligation: &ObligationGuidanceInput,
    likely_fix_sites: &[String],
    inspection_context: &[String],
) -> Vec<String> {
    let mut steps = verify_after_for_kind(kind, likely_fix_sites, inspection_context)
        .into_iter()
        .filter(|step| step != VERIFY_AFTER_GATE_MESSAGE)
        .collect::<Vec<_>>();
    let labels = obligation_surface_labels(obligation);
    let has_registry = labels_contain(&labels, "registry");
    let has_public_api = labels_contain(&labels, "public API");
    let has_dto = labels_contain(&labels, "DTO");
    let has_command_status = labels_contain(&labels, "command status");
    let has_config = labels_contain(&labels, "config");
    let has_test_doc = labels_contain(&labels, "test/doc");

    if has_registry {
        steps.push(
            "Verify the registry surface now routes the change consistently from each touched entry path."
                .to_string(),
        );
    }

    let mut exposed_surfaces = Vec::new();
    if has_public_api {
        exposed_surfaces.push("public API".to_string());
    }
    if has_dto {
        exposed_surfaces.push("response/DTO".to_string());
    }
    if has_command_status {
        exposed_surfaces.push("command-status".to_string());
    }
    if !exposed_surfaces.is_empty() {
        let surface_phrase = named_surface_phrase(&exposed_surfaces);
        let verb = if exposed_surfaces.len() == 1 {
            "stays"
        } else {
            "stay"
        };
        steps.push(format!(
            "Verify the emitted {surface_phrase} {verb} aligned anywhere this contract is exposed."
        ));
    }

    if has_config {
        steps.push(
            "Verify the config/default path exposes the same change without relying on stale fallback wiring."
                .to_string(),
        );
    }

    if has_test_doc {
        steps.push(
            "Run or refresh the targeted test/doc surface so the follow-through path is exercised."
                .to_string(),
        );
    }

    if guidance_kind_profile(kind).verify_after_gate {
        steps.push(VERIFY_AFTER_GATE_MESSAGE.to_string());
    }

    dedupe_strings_preserve_order(steps)
}

fn prioritized_missing_sites(obligation: &ObligationGuidanceInput) -> Vec<ObligationSiteInput> {
    let mut sites = obligation.missing_sites.clone();
    sites.sort_by(|left, right| {
        obligation_site_priority(left)
            .cmp(&obligation_site_priority(right))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.detail.cmp(&right.detail))
    });
    sites
}

fn obligation_site_priority(site: &ObligationSiteInput) -> u8 {
    match site.kind.as_str() {
        "browser_entry" | "electron_entry" => 0,
        "registry_symbol" => 1,
        "payload_map_symbol" | "categories_symbol" | "closed_domain" => 2,
        "related_test" => 9,
        _ => match obligation_surface_label(site) {
            Some("registry") => 1,
            Some("public API") => 3,
            Some("DTO") => 4,
            Some("command status") => 5,
            Some("config") => 6,
            Some("test/doc") => 9,
            _ => 7,
        },
    }
}

fn obligation_surface_labels(obligation: &ObligationGuidanceInput) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut labels = Vec::new();

    for site in prioritized_missing_sites(obligation) {
        let Some(label) = obligation_surface_label(&site) else {
            continue;
        };
        if seen.insert(label) {
            labels.push(label.to_string());
        }
    }

    labels
}

fn fully_labeled_obligation_surfaces(obligation: &ObligationGuidanceInput) -> Vec<String> {
    let sites = prioritized_missing_sites(obligation);
    if sites.is_empty()
        || sites
            .iter()
            .any(|site| obligation_surface_label(site).is_none())
    {
        return Vec::new();
    }

    let mut seen = BTreeSet::new();
    let mut labels = Vec::new();
    for site in sites {
        let label = obligation_surface_label(&site).expect("checked above");
        if seen.insert(label) {
            labels.push(label.to_string());
        }
    }

    labels
}

fn obligation_has_surface_label(obligation: &ObligationGuidanceInput, label: &str) -> bool {
    obligation_surface_labels(obligation)
        .iter()
        .any(|candidate| candidate == label)
}

fn labels_contain(labels: &[String], label: &str) -> bool {
    labels.iter().any(|candidate| candidate == label)
}

fn obligation_surface_label(site: &ObligationSiteInput) -> Option<&'static str> {
    if site.kind == "registry_symbol" {
        return Some("registry");
    }
    if site.kind == "related_test" || looks_like_test_or_doc_surface(&site.path) {
        return Some("test/doc");
    }

    let lowered_path = site.path.to_ascii_lowercase();
    if lowered_path.contains("registry") {
        return Some("registry");
    }
    if looks_like_public_api_surface(&lowered_path) {
        return Some("public API");
    }
    if looks_like_dto_surface(&lowered_path) {
        return Some("DTO");
    }
    if looks_like_command_status_surface(&lowered_path) {
        return Some("command status");
    }
    if looks_like_config_surface(&lowered_path) {
        return Some("config");
    }

    None
}

fn looks_like_public_api_surface(lowered_path: &str) -> bool {
    lowered_path.ends_with("/mod.rs")
        || lowered_path.ends_with("/lib.rs")
        || lowered_path.ends_with("/index.ts")
        || lowered_path.ends_with("/index.tsx")
        || lowered_path.ends_with("/index.js")
        || lowered_path.ends_with("/index.jsx")
        || lowered_path.contains("public_api")
        || lowered_path.contains("public-api")
        || lowered_path.ends_with("/brief.rs")
}

fn looks_like_dto_surface(lowered_path: &str) -> bool {
    lowered_path.contains("/dto")
        || lowered_path.contains("_dto")
        || lowered_path.contains("-dto")
        || lowered_path.contains("response")
        || lowered_path.contains("request")
        || lowered_path.contains("payload")
        || lowered_path.contains("format")
}

fn looks_like_command_status_surface(lowered_path: &str) -> bool {
    lowered_path.contains("command_status")
        || lowered_path.contains("command-status")
        || lowered_path.contains("commandstatus")
        || lowered_path.contains("evaluation_signals")
        || (lowered_path.contains("command") && lowered_path.contains("status"))
}

fn looks_like_config_surface(lowered_path: &str) -> bool {
    lowered_path.contains("config")
        || lowered_path.contains("settings")
        || lowered_path.ends_with(".toml")
        || lowered_path.ends_with(".yaml")
        || lowered_path.ends_with(".yml")
        || lowered_path.ends_with(".json")
}

fn looks_like_test_or_doc_surface(path: &str) -> bool {
    let lowered_path = path.to_ascii_lowercase();
    looks_like_test_surface(&lowered_path)
        || lowered_path.contains("/docs/")
        || lowered_path.ends_with(".md")
        || lowered_path.ends_with(".mdx")
        || lowered_path.ends_with(".rst")
        || lowered_path.ends_with(".adoc")
}

fn obligation_surface_phrase(labels: &[String], remaining: bool) -> Option<String> {
    if labels.is_empty() {
        return None;
    }

    let prefix = if remaining { "remaining " } else { "" };
    Some(if labels.len() == 1 {
        format!("the {prefix}{} surface", labels[0])
    } else {
        format!("the {prefix}{} surfaces", describe_list(labels))
    })
}

fn named_surface_phrase(labels: &[String]) -> String {
    if labels.len() == 1 {
        format!("{} surface", labels[0])
    } else {
        format!("{} surfaces", describe_list(labels))
    }
}

fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();

    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }

    deduped
}

fn display_site(site: &ObligationSiteInput) -> String {
    let line_suffix = site.line.map(|line| format!(":{line}")).unwrap_or_default();
    format!("{}{}", site.path, line_suffix)
}

fn obligation_kind(obligation: &Value) -> &str {
    obligation.get("kind").and_then(Value::as_str).unwrap_or("")
}
