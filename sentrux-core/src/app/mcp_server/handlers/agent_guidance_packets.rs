use super::shared::{describe_list, guidance_kind_profile, VERIFY_AFTER_GATE_MESSAGE};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepairPacketRequiredFields {
    pub(crate) risk_statement: bool,
    pub(crate) repair_surface: bool,
    pub(crate) first_cut: bool,
    pub(crate) verification: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepairPacket {
    pub(crate) risk_statement: String,
    pub(crate) likely_fix_sites: Vec<String>,
    pub(crate) inspection_context: Vec<String>,
    pub(crate) smallest_safe_first_cut: Option<String>,
    pub(crate) verify_after: Vec<String>,
    pub(crate) do_not_touch_yet: Vec<String>,
    pub(crate) completeness_0_10000: u32,
    pub(crate) complete: bool,
    pub(crate) required_fields: RepairPacketRequiredFields,
    pub(crate) missing_fields: Vec<String>,
}

pub(crate) fn build_repair_packet(
    kind: &str,
    likely_fix_sites: Vec<String>,
    inspection_context: Vec<String>,
    smallest_safe_first_cut: Option<String>,
    risk_statement: String,
    verify_after_override: Option<Vec<String>>,
) -> RepairPacket {
    let verify_after = verify_after_override
        .unwrap_or_else(|| verify_after_for_kind(kind, &likely_fix_sites, &inspection_context));
    let do_not_touch_yet = do_not_touch_yet_for_kind(kind);
    let required_fields = repair_packet_required_fields(
        !risk_statement.trim().is_empty(),
        !likely_fix_sites.is_empty(),
        smallest_safe_first_cut.is_some(),
        !verify_after.is_empty(),
    );
    let missing_fields = repair_packet_missing_fields(&required_fields);
    let completeness_0_10000 =
        repair_packet_completeness_0_10000(&required_fields, !do_not_touch_yet.is_empty());
    RepairPacket {
        risk_statement,
        likely_fix_sites,
        inspection_context,
        smallest_safe_first_cut,
        verify_after,
        do_not_touch_yet,
        completeness_0_10000,
        complete: missing_fields.is_empty(),
        required_fields,
        missing_fields,
    }
}

fn repair_packet_required_fields(
    has_risk_statement: bool,
    has_fix_sites: bool,
    has_first_cut: bool,
    has_verify_after: bool,
) -> RepairPacketRequiredFields {
    RepairPacketRequiredFields {
        risk_statement: has_risk_statement,
        repair_surface: has_fix_sites,
        first_cut: has_first_cut,
        verification: has_verify_after,
    }
}

fn repair_packet_missing_fields(required_fields: &RepairPacketRequiredFields) -> Vec<String> {
    let mut missing_fields = Vec::new();
    if !required_fields.risk_statement {
        missing_fields.push("risk_statement".to_string());
    }
    if !required_fields.repair_surface {
        missing_fields.push("repair_surface".to_string());
    }
    if !required_fields.first_cut {
        missing_fields.push("first_cut".to_string());
    }
    if !required_fields.verification {
        missing_fields.push("verification".to_string());
    }
    missing_fields
}

fn repair_packet_completeness_0_10000(
    required_fields: &RepairPacketRequiredFields,
    has_do_not_touch_yet: bool,
) -> u32 {
    let mut score = 0;
    if required_fields.risk_statement {
        score += 2500;
    }
    if required_fields.repair_surface {
        score += 2500;
    }
    if required_fields.first_cut {
        score += 2500;
    }
    if required_fields.verification {
        score += 2000;
    }
    if has_do_not_touch_yet {
        score += 500;
    }
    score.min(10_000)
}

pub(crate) fn verify_after_for_kind(
    kind: &str,
    likely_fix_sites: &[String],
    inspection_context: &[String],
) -> Vec<String> {
    let mut steps = Vec::new();
    if !likely_fix_sites.is_empty() {
        steps.push(format!(
            "Re-run `sentrux check` and confirm the lead issue clears for {}.",
            describe_list(likely_fix_sites)
        ));
    } else if !inspection_context.is_empty() {
        steps.push(format!(
            "Re-run `sentrux check` and confirm the lead issue clears around {}.",
            describe_list(inspection_context)
        ));
    } else {
        steps.push(
            "Re-run `sentrux check` and confirm this issue no longer appears in the lead actions."
                .to_string(),
        );
    }

    if guidance_kind_profile(kind).verify_after_gate {
        steps.push(VERIFY_AFTER_GATE_MESSAGE.to_string());
    }

    steps
}

pub(crate) fn do_not_touch_yet_for_kind(kind: &str) -> Vec<String> {
    guidance_kind_profile(kind)
        .do_not_touch_yet
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>()
}

pub(crate) fn finding_risk_statement(finding: &serde_json::Value) -> String {
    super::super::build_finding_details(std::slice::from_ref(finding), 1)
        .into_iter()
        .next()
        .map(|detail| detail.impact)
        .unwrap_or_else(|| super::shared::DEFAULT_FINDING_RISK_STATEMENT.to_string())
}

pub(crate) fn obligation_risk_statement(kind: &str) -> String {
    match super::shared::obligation_family(kind) {
        super::shared::ObligationFamily::Propagation => {
            super::shared::INCOMPLETE_PROPAGATION_RISK_STATEMENT.to_string()
        }
        super::shared::ObligationFamily::Exhaustiveness => {
            super::shared::CLOSED_DOMAIN_EXHAUSTIVENESS_RISK_STATEMENT.to_string()
        }
        super::shared::ObligationFamily::Generic => {
            super::shared::DEFAULT_OBLIGATION_RISK_STATEMENT.to_string()
        }
    }
}
