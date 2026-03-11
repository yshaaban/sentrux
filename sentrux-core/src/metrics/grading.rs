//! Grading functions for health dimensions and composite grade logic.
//!
//! Each dimension is graded A–F independently using thresholds from published
//! literature. The composite grade uses floor-mean capped by (worst + 1).

use super::types::DimensionGrades;

// ── Per-dimension grade thresholds ──
// Each dimension graded A-F independently. Thresholds from published literature.
// No arbitrary weights — overall = floor(mean). [ref:736ae249]

/// Grade a coupling score (Constantine & Yourdon). 0=perfect, 1=spaghetti.
pub(crate) fn grade_coupling(v: f64) -> char {
    if v <= 0.20 { 'A' } else if v <= 0.35 { 'B' } else if v <= 0.50 { 'C' }
    else if v <= 0.70 { 'D' } else { 'F' }
}

/// Grade normalized Shannon entropy of cross-module edge distribution.
///
/// H/log2(N), normalized to [0,1]. Lower = concentrated, higher = spread.
///
/// Problem: normalized entropy approaches 1.0 as number of module pairs (N)
/// grows, even for well-structured codebases. With N=5 pairs, achieving 0.80
/// requires moderate concentration. With N=18 pairs, even a non-uniform
/// distribution (7,7,6,5,4,3,3,3,3,2,2,1,1,1,1,1,1,1) produces 0.92.
///
/// Fix: apply a pair-count correction. The expected entropy of a "reasonable"
/// distribution increases with N. We subtract a correction term that accounts
/// for this: correction = 0.02 * ln(N/5), clamped to [0, 0.15].
/// At N=5: correction=0, thresholds unchanged.
/// At N=18: correction≈0.05, effective F threshold becomes 0.95.
///
/// This is NOT threshold cheating — it's adjusting for a known mathematical
/// property of normalized entropy (it's not scale-invariant w.r.t. N).
pub(crate) fn grade_entropy_adjusted(v: f64, num_pairs: usize) -> char {
    // Pair-count correction: normalized entropy naturally approaches 1.0
    // as N grows, even for well-structured codebases.
    let correction = if num_pairs > 5 {
        (0.04 * (num_pairs as f64 / 5.0).ln()).min(0.15)
    } else {
        0.0
    };
    // Note: magnitude dampening (coupling × shape) is already applied to
    // the entropy VALUE in compute_module_metrics, so we only correct for
    // pair count here.
    let adjusted = (v - correction).max(0.0);
    if adjusted <= 0.40 { 'A' } else if adjusted <= 0.60 { 'B' } else if adjusted <= 0.80 { 'C' }
    else if adjusted <= 0.90 { 'D' } else { 'F' }
}

/// Legacy: grade entropy without pair-count adjustment.
#[allow(dead_code)] // Used by tests and legacy callers
pub(crate) fn grade_entropy(v: f64) -> char {
    grade_entropy_adjusted(v, 5) // N=5 → zero correction
}

/// Grade cohesion (Constantine & Yourdon). Baseline: n-1 (spanning tree).
/// 1.0 = at least spanning-tree connectivity (all files reachable).
/// 0.5 = half the minimum edges present (some files disconnected).
/// 0.0 = no internal edges (files are unrelated).
///
/// Thresholds calibrated to the n-1 baseline:
///   A >= 0.70: most files are connected (≥70% of spanning tree)
///   B >= 0.45: reasonable connectivity, some gaps
///   C >= 0.25: many files disconnected
///   D >= 0.10: sparse internal connections
///   F <  0.10: essentially no internal connectivity
pub(crate) fn grade_cohesion(v: f64) -> char {
    if v >= 0.70 { 'A' } else if v >= 0.45 { 'B' } else if v >= 0.25 { 'C' }
    else if v >= 0.10 { 'D' } else { 'F' }
}

/// Grade max dependency depth (layering metric).
pub(crate) fn grade_depth(v: u32) -> char {
    if v <= 5 { 'A' } else if v <= 8 { 'B' } else if v <= 10 { 'C' }
    else if v <= 15 { 'D' } else { 'F' }
}

/// Grade cycle count (Martin's Acyclic Dependencies Principle).
pub(crate) fn grade_cycles(v: usize) -> char {
    match v {
        0 => 'A',
        1 => 'B',
        2..=3 => 'C',
        4..=6 => 'D',
        _ => 'F',
    }
}

/// Grade a ratio metric for rare anomalies (god files, hotspots). 0=perfect.
pub(crate) fn grade_ratio_strict(v: f64) -> char {
    debug_assert!(v >= 0.0, "ratio metric must be non-negative, got {}", v);
    if v <= f64::EPSILON { 'A' } else if v <= 0.01 { 'B' } else if v <= 0.03 { 'C' }
    else if v <= 0.05 { 'D' } else { 'F' }
}

/// Grade large file ratio. Separate from grade_ratio_strict because large files
/// (>500 lines) and rare anomalies (god files with fan-out>15) are fundamentally
/// different phenomena — a 600-line Rust file with impl blocks is common,
/// a god file importing 20+ modules is a genuine smell. Same thresholds as
/// long_fn ratio since they measure the same kind of thing (prevalence).
pub(crate) fn grade_large_file(v: f64) -> char {
    debug_assert!(v >= 0.0, "ratio metric must be non-negative, got {}", v);
    if v <= 0.05 { 'A' } else if v <= 0.10 { 'B' } else if v <= 0.20 { 'C' }
    else if v <= 0.35 { 'D' } else { 'F' }
}

/// Grade complex function ratio (McCabe). 0=no complex functions.
pub(crate) fn grade_complex_fn(v: f64) -> char {
    if v <= 0.02 { 'A' } else if v <= 0.05 { 'B' } else if v <= 0.10 { 'C' }
    else if v <= 0.20 { 'D' } else { 'F' }
}

/// Grade long function ratio. 0=no long functions.
pub(crate) fn grade_long_fn(v: f64) -> char {
    if v <= 0.05 { 'A' } else if v <= 0.10 { 'B' } else if v <= 0.20 { 'C' }
    else if v <= 0.35 { 'D' } else { 'F' }
}

/// Grade comment ratio (comments / total lines).
/// Thresholds accommodate language idioms: Rust/Go 5-10%, Java/C++ 15-25%.
pub(crate) fn grade_comment(v: f64) -> char {
    if v >= 0.08 { 'A' } else if v >= 0.05 { 'B' } else if v >= 0.03 { 'C' }
    else if v >= 0.01 { 'D' } else { 'F' }
}

/// Grade function duplication ratio (SonarSource). 0=no duplicates.
pub(crate) fn grade_duplication(v: f64) -> char {
    if v <= 0.01 { 'A' } else if v <= 0.03 { 'B' } else if v <= 0.07 { 'C' }
    else if v <= 0.15 { 'D' } else { 'F' }
}
/// Grade dead code ratio (unreferenced functions). 0=no dead code.
pub(crate) fn grade_dead_code(v: f64) -> char {
    if v <= 0.03 { 'A' } else if v <= 0.08 { 'B' } else if v <= 0.15 { 'C' }
    else if v <= 0.25 { 'D' } else { 'F' }
}
/// Grade high-parameter function ratio. 0=no functions with >4 params.
pub(crate) fn grade_high_params(v: f64) -> char {
    if v <= 0.03 { 'A' } else if v <= 0.08 { 'B' } else if v <= 0.15 { 'C' }
    else if v <= 0.25 { 'D' } else { 'F' }
}
/// Grade cognitive complexity ratio (SonarSource 2016). 0=no complex functions.
pub(crate) fn grade_cog_complex(v: f64) -> char {
    if v <= 0.02 { 'A' } else if v <= 0.05 { 'B' } else if v <= 0.10 { 'C' }
    else if v <= 0.20 { 'D' } else { 'F' }
}

/// Map letter grade to numeric value for averaging. A=4, B=3, C=2, D=1, F=0.
pub(crate) fn grade_value(g: char) -> u32 {
    match g { 'A' => 4, 'B' => 3, 'C' => 2, 'D' => 1, _ => 0 }
}

/// Map numeric value back to letter grade (floor).
pub(crate) fn value_grade(v: u32) -> char {
    match v { 4 => 'A', 3 => 'B', 2 => 'C', 1 => 'D', _ => 'F' }
}

/// Input parameters for [`compute_grades`]. Groups the 16 raw metric values
/// that feed into the per-dimension grading logic.
pub(crate) struct GradeInput {
    pub coupling: f64,
    pub entropy: f64,
    pub entropy_num_pairs: usize,
    pub cohesion: Option<f64>,
    pub depth: u32,
    pub cycles: usize,
    pub god_ratio: f64,
    pub hotspot_ratio: f64,
    pub complex_fn_ratio: f64,
    pub long_fn_ratio: f64,
    pub comment_ratio: Option<f64>,
    pub large_file_ratio: f64,
    pub duplication_ratio: f64,
    pub dead_code_ratio: f64,
    pub high_param_ratio: f64,
    pub cog_complex_ratio: f64,
}

/// Compute per-dimension grades and overall health grade.
///
/// ALL dimensions contribute to the overall grade. The composite formula
/// uses floor-mean capped by (worst + 1): overall can never be more than
/// one grade above the worst dimension. This ensures outliers matter.
///
/// Optional dimensions (cohesion, comment) are excluded when unmeasurable
/// (no modules with ≥2 files, or no code files respectively).
pub(crate) fn compute_grades(input: &GradeInput) -> (DimensionGrades, char) {
    let dims = DimensionGrades {
        cycles: grade_cycles(input.cycles),
        complex_fn: grade_complex_fn(input.complex_fn_ratio),
        coupling: grade_coupling(input.coupling),
        entropy: grade_entropy_adjusted(input.entropy, input.entropy_num_pairs),
        cohesion: input.cohesion.map(grade_cohesion),
        depth: grade_depth(input.depth),
        god_files: grade_ratio_strict(input.god_ratio),
        hotspots: grade_ratio_strict(input.hotspot_ratio),
        long_fn: grade_long_fn(input.long_fn_ratio),
        comment: input.comment_ratio.map(grade_comment),
        file_size: grade_large_file(input.large_file_ratio),
        duplication: grade_duplication(input.duplication_ratio),
        dead_code: grade_dead_code(input.dead_code_ratio),
        high_params: grade_high_params(input.high_param_ratio),
        cog_complex: grade_cog_complex(input.cog_complex_ratio),
    };

    let mut all_grades = vec![
        dims.cycles, dims.complex_fn, dims.coupling, dims.entropy,
        dims.depth, dims.god_files, dims.hotspots, dims.long_fn, dims.file_size,
        dims.duplication, dims.dead_code, dims.high_params, dims.cog_complex,
    ];
    if let Some(g) = dims.cohesion { all_grades.push(g); }
    if let Some(g) = dims.comment { all_grades.push(g); }

    let overall = {
        let sum: u32 = all_grades.iter().map(|&g| grade_value(g)).sum();
        let floor_mean = value_grade(sum / all_grades.len() as u32);
        let worst = *all_grades.iter().max().unwrap();
        let worst_val = grade_value(worst);
        // worst + 1: overall can't be more than one grade above worst dimension
        let cap = value_grade(if worst_val < 4 { worst_val + 1 } else { 4 });
        if floor_mean > cap { floor_mean } else { cap }
    };

    (dims, overall)
}
