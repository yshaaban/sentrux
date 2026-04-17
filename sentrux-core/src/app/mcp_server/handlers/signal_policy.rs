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
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct SharedPolicyFixture {
        score_bands: Vec<ScoreBandCase>,
        action_kind_weights: Vec<WeightCase>,
        action_leverage_weights: Vec<WeightCase>,
        action_presentation_weights: Vec<WeightCase>,
        report_leverage_priority: Vec<PriorityCase>,
        report_presentation_priority: Vec<PriorityCase>,
    }

    #[derive(Debug, Deserialize)]
    struct ScoreBandCase {
        score: u32,
        label: String,
    }

    #[derive(Debug, Deserialize)]
    struct WeightCase {
        name: String,
        weight: u8,
    }

    #[derive(Debug, Deserialize)]
    struct PriorityCase {
        name: String,
        priority: usize,
    }

    fn shared_policy_fixture() -> SharedPolicyFixture {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/tests/fixtures/policy-parity/shared-policy.json"
        )))
        .expect("shared policy parity fixture should parse")
    }

    #[test]
    fn action_policy_matches_expected_weights() {
        let fixture = shared_policy_fixture();

        for case in fixture.action_kind_weights {
            assert_eq!(action_kind_weight(case.name.as_str()), case.weight);
        }
        for case in fixture.action_leverage_weights {
            assert_eq!(action_leverage_weight(case.name.as_str()), case.weight);
        }
        for case in fixture.action_presentation_weights {
            assert_eq!(action_presentation_weight(case.name.as_str()), case.weight);
        }
    }

    #[test]
    fn report_policy_matches_expected_order() {
        let fixture = shared_policy_fixture();

        for case in fixture.report_leverage_priority {
            assert_eq!(report_leverage_rank(case.name.as_str()), case.priority);
        }
        for case in fixture.report_presentation_priority {
            assert_eq!(report_presentation_rank(case.name.as_str()), case.priority);
        }
    }

    #[test]
    fn score_bands_match_shared_policy_thresholds() {
        let fixture = shared_policy_fixture();

        for case in fixture.score_bands {
            assert_eq!(score_band_label(case.score), case.label);
        }
    }
}
