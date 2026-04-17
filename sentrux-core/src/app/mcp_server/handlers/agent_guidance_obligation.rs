use super::super::agent_format::{IssueConfidence, IssueOrigin};
use super::packets::{build_repair_packet, obligation_risk_statement, RepairPacket};
use super::shared::{
    fallback_obligation_origin, obligation_family, ObligationFamily, ObligationGuidanceInput,
};
use crate::metrics::v2::FindingSeverity;
use serde_json::Value;

pub(crate) fn repair_packet_for_obligation(obligation: &Value, kind: &str) -> RepairPacket {
    let input = ObligationGuidanceInput::from_value(obligation);
    build_repair_packet(
        kind,
        obligation_likely_fix_sites(&input),
        inspection_context_for_obligation(&input),
        obligation_fix_hint_from_input(&input, kind),
        obligation_risk_statement(kind),
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
        if matches!(
            obligation_family(obligation_kind(obligation)),
            ObligationFamily::Exhaustiveness
        ) || !input.missing_variants.is_empty()
        {
            FindingSeverity::High
        } else if input
            .origin
            .unwrap_or_else(|| fallback_obligation_origin(&input))
            == IssueOrigin::Explicit
        {
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
    ObligationGuidanceInput::from_value(obligation)
        .missing_sites
        .iter()
        .find_map(|site| site.line)
}

pub(crate) fn obligation_evidence(obligation: &Value) -> Vec<String> {
    ObligationGuidanceInput::from_value(obligation)
        .missing_sites
        .iter()
        .map(|site| {
            let line_suffix = site.line.map(|line| format!(":{line}")).unwrap_or_default();
            format!("{}{} [{}]", site.path, line_suffix, site.detail)
        })
        .collect::<Vec<_>>()
}

fn obligation_message_from_input(obligation: &ObligationGuidanceInput, kind: &str) -> String {
    match obligation_family(kind) {
        ObligationFamily::Propagation => {
            format!(
                "Propagation is incomplete for '{}': update the remaining sibling surfaces listed in the evidence.",
                obligation.scope_label()
            )
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
            Some(super::shared::INCOMPLETE_PROPAGATION_HINT.to_string())
        }
        ObligationFamily::Exhaustiveness => {
            let site_suffix = obligation_site_suffix(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            if !missing_variants.is_empty() {
                return Some(format!(
                    "Handle the missing variants [{}] with an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch.",
                    missing_variants.join(", "),
                ));
            }

            Some(format!(
                "Add an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch."
            ))
        }
        ObligationFamily::Generic => Some(
            "Update the missing sites tied to the changed concept before continuing.".to_string(),
        ),
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
    let site = obligation.missing_sites.first()?;
    let line = site.line.map(|line| format!(":{line}")).unwrap_or_default();

    Some(format!("{}{}", site.path, line))
}

fn obligation_site_suffix(obligation: &ObligationGuidanceInput) -> String {
    obligation_missing_site(obligation)
        .map(|site| format!(" at {site}"))
        .unwrap_or_default()
}

fn obligation_likely_fix_sites(obligation: &ObligationGuidanceInput) -> Vec<String> {
    obligation
        .missing_sites
        .iter()
        .take(5)
        .map(|site| {
            let line_suffix = site.line.map(|line| format!(":{line}")).unwrap_or_default();
            format!("{}{}", site.path, line_suffix)
        })
        .collect::<Vec<_>>()
}

fn inspection_context_for_obligation(obligation: &ObligationGuidanceInput) -> Vec<String> {
    obligation.files.iter().take(3).cloned().collect::<Vec<_>>()
}

fn obligation_kind(obligation: &Value) -> &str {
    obligation.get("kind").and_then(Value::as_str).unwrap_or("")
}
