use super::super::FindingSeverity;
use super::path_roles::has_role_tag;
use super::CycleCutCandidate;

pub(super) fn large_file_score(line_count: usize, threshold: u32, max_complexity: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(line_count, threshold as usize, 3600);
    let complexity_bonus = max_complexity.saturating_sub(20).min(20) * 120;
    (2400 + over_threshold + complexity_bonus).min(10_000)
}

pub(super) fn dependency_sprawl_score(
    fan_out: usize,
    threshold: usize,
    instability_0_10000: u32,
) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_out, threshold, 3200);
    let instability_bonus = instability_0_10000 / 4;
    (2800 + over_threshold + instability_bonus).min(10_000)
}

pub(super) fn unstable_hotspot_score(
    fan_in: usize,
    threshold: usize,
    instability_0_10000: u32,
) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_in, threshold, 3000);
    let instability_bonus = instability_0_10000 / 3;
    (3200 + over_threshold + instability_bonus).min(10_000)
}

pub(super) fn cycle_cluster_score(
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
                ((candidate.remaining_cycle_size as f64 / file_count as f64) * 700.0).round() as u32
            };
            let contained_remainder_bonus = 700u32.saturating_sub(remainder_penalty);
            seam_bonus + reduction_bonus + reduction_ratio_bonus + contained_remainder_bonus
        })
        .unwrap_or(0);
    (2400 + size_bonus + line_bonus + role_bonus + cut_bonus).min(10_000)
}

pub(super) fn dead_private_cluster_score(dead_symbol_count: usize, dead_line_count: usize) -> u32 {
    let symbol_bonus = (dead_symbol_count as u32 * 900).min(3600);
    let line_bonus = (dead_line_count as u32 * 18).min(2800);
    (1500 + symbol_bonus + line_bonus).min(10_000)
}

pub(super) fn dead_island_score(
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

pub(super) fn instability_0_10000(fan_in: usize, fan_out: usize) -> u32 {
    let total = fan_in + fan_out;
    let instability = if total == 0 {
        0.5
    } else {
        fan_out as f64 / total as f64
    };
    (instability * 10_000.0).round() as u32
}

pub(super) fn signal_severity(score_0_10000: u32) -> FindingSeverity {
    match score_0_10000 {
        6500..=10_000 => FindingSeverity::High,
        3000..=6499 => FindingSeverity::Medium,
        _ => FindingSeverity::Low,
    }
}

pub(super) fn severity_priority(severity: FindingSeverity) -> u8 {
    severity.priority()
}

fn scaled_ratio_pressure(value: usize, threshold: usize, max_bonus: u32) -> u32 {
    if threshold == 0 || value <= threshold {
        return 0;
    }

    let pressure = ((value - threshold) as f64 / threshold as f64).min(1.0);
    (pressure * max_bonus as f64).round() as u32
}
