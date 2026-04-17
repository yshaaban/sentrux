use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static SIGNAL_POLICY: LazyLock<SignalPolicy> = LazyLock::new(load_signal_policy);

#[derive(Debug, Deserialize)]
struct SignalPolicy {
    action_ranking: ActionRankingPolicy,
    report_selection: ReportSelectionPolicy,
    score_bands: Vec<ScoreBandPolicy>,
}

#[derive(Debug, Deserialize)]
struct ActionRankingPolicy {
    kind_weights: BTreeMap<String, u8>,
    leverage_weights: BTreeMap<String, u8>,
    presentation_weights: BTreeMap<String, u8>,
}

#[derive(Debug, Deserialize)]
struct ReportSelectionPolicy {
    leverage_order: Vec<String>,
    presentation_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ScoreBandPolicy {
    minimum_score: u32,
    label: String,
}

fn load_signal_policy() -> SignalPolicy {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../.sentrux/signal-policy.json"
    )))
    .expect("embedded signal policy should parse")
}

pub(crate) fn action_kind_weight(kind: &str) -> u8 {
    SIGNAL_POLICY
        .action_ranking
        .kind_weights
        .get(kind)
        .copied()
        .unwrap_or(4)
}

pub(crate) fn action_leverage_weight(class_name: &str) -> u8 {
    SIGNAL_POLICY
        .action_ranking
        .leverage_weights
        .get(class_name)
        .copied()
        .unwrap_or_default()
}

pub(crate) fn action_presentation_weight(class_name: &str) -> u8 {
    SIGNAL_POLICY
        .action_ranking
        .presentation_weights
        .get(class_name)
        .copied()
        .unwrap_or_default()
}

pub(crate) fn report_leverage_rank(class_name: &str) -> usize {
    order_rank(&SIGNAL_POLICY.report_selection.leverage_order, class_name)
}

pub(crate) fn report_presentation_rank(class_name: &str) -> usize {
    order_rank(
        &SIGNAL_POLICY.report_selection.presentation_order,
        class_name,
    )
}

pub(crate) fn score_band_label(score_0_10000: u32) -> &'static str {
    SIGNAL_POLICY
        .score_bands
        .iter()
        .find(|band| score_0_10000 >= band.minimum_score)
        .map(|band| band.label.as_str())
        .unwrap_or("supporting_signal")
}

fn order_rank(order: &[String], class_name: &str) -> usize {
    order
        .iter()
        .position(|candidate| candidate == class_name)
        .unwrap_or(order.len())
}

#[cfg(test)]
mod tests {
    use super::{
        action_kind_weight, action_leverage_weight, action_presentation_weight,
        report_leverage_rank, report_presentation_rank, score_band_label,
    };

    #[test]
    fn action_policy_matches_expected_weights() {
        assert!(action_kind_weight("forbidden_raw_read") > action_kind_weight("large_file"));
        assert!(
            action_leverage_weight("boundary_discipline")
                > action_leverage_weight("secondary_cleanup")
        );
        assert!(
            action_presentation_weight("guarded_facade")
                > action_presentation_weight("tooling_debt")
        );
    }

    #[test]
    fn report_policy_matches_expected_order() {
        assert!(report_leverage_rank("architecture_signal") < report_leverage_rank("tooling_debt"));
        assert!(
            report_presentation_rank("structural_debt") < report_presentation_rank("experimental")
        );
    }

    #[test]
    fn score_bands_match_shared_policy_thresholds() {
        assert_eq!(score_band_label(0), "supporting_signal");
        assert_eq!(score_band_label(3999), "supporting_signal");
        assert_eq!(score_band_label(4000), "moderate_signal");
        assert_eq!(score_band_label(6499), "moderate_signal");
        assert_eq!(score_band_label(6500), "high_signal");
        assert_eq!(score_band_label(8499), "high_signal");
        assert_eq!(score_band_label(8500), "very_high_signal");
    }
}
