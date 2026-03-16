//! Score-based grading with geometric mean quality signal.
//!
//! Design principles (from first-principle analysis):
//!   - 3 orthogonal categories: Blast Radius, Cognitive Load, Hidden Debt
//!   - Each raw metric normalized to [0,1] score (1 = best)
//!   - Category score = arithmetic mean of member scores (correlated within)
//!   - Quality signal = geometric mean of 3 categories (Nash Social Welfare optimal)
//!   - Grades derived FROM scores, not from raw ratios
//!   - Beta(1,1) uniform Bayesian prior everywhere (no assumptions)
//!
//! Normalization rules:
//!   - Bounded [0,1] "lower is better": score = 1 - ratio   (0%→1.0, 100%→0.0)
//!   - Bounded [0,1] "higher is better": score = ratio       (0%→0.0, 100%→1.0)
//!   - Unbounded [0,∞) counts:           score = 1/(1+count/m) (sigmoid)
//!   - Semi-bounded (comments):          score = v/(v+m)       (sigmoid, 100% is not the goal)

use super::types::{CategoryScores, DimensionGrades, DimensionScores};

// ══════════════════════════════════════════════════════════════════
//  LAYER 3: Normalize raw metrics to [0, 1] scores
// ══════════════════════════════════════════════════════════════════

/// Bounded [0,1] ratio, lower is better → score = 1 - ratio.
/// ratio=0.0 (perfect) → score=1.0.  ratio=1.0 (worst) → score=0.0.
/// No midpoints, no arbitrary parameters. Pure math.
fn score_bounded_lower(ratio: f64) -> f64 {
    (1.0 - ratio).clamp(0.0, 1.0)
}

/// Bounded [0,1] ratio, higher is better → score = ratio.
/// ratio=1.0 (perfect) → score=1.0.  ratio=0.0 (worst) → score=0.0.
fn score_bounded_higher(ratio: f64) -> f64 {
    ratio.clamp(0.0, 1.0)
}

/// Unbounded [0,∞) count, lower is better → sigmoid.
/// count=0 → score=1.0.  count=midpoint → score=0.5.  count→∞ → score→0.
/// Only used for truly unbounded metrics (cycles, depth).
fn score_unbounded_lower(count: f64, midpoint: f64) -> f64 {
    1.0 / (1.0 + count / midpoint)
}

// ══════════════════════════════════════════════════════════════════
//  LAYER 4: Category scores (3 orthogonal groups)
// ══════════════════════════════════════════════════════════════════

/// Compute [0,1] scores for all individual dimensions and 3 category scores.
///
/// Categories (from exhaustive failure-mode analysis):
///   Blast Radius:   "Change one thing → how much else breaks?"
///   Cognitive Load:  "How hard is each unit to understand?"
///   Hidden Debt:     "How much invisible junk is accumulating?"
pub(crate) fn compute_scores(input: &GradeInput) -> (DimensionScores, CategoryScores) {
    // ── Blast Radius: all bounded [0,1] lower-is-better, except cycles/depth ──
    let coupling = score_bounded_lower(input.coupling);
    let cycles = score_unbounded_lower(input.cycles as f64, 1.0);     // unbounded count
    let god_files = score_bounded_lower(input.god_ratio);
    let hotspots = score_bounded_lower(input.hotspot_ratio);
    let levelization = score_bounded_lower(input.levelization_upward_ratio);
    let blast_radius = score_bounded_lower(input.blast_radius_ratio);
    let depth = score_unbounded_lower(input.depth as f64, 8.0);       // unbounded count
    let entropy = score_bounded_lower(input.entropy);

    // ── Cognitive Load: mix of lower-is-better and higher-is-better ──
    let complex_fn = score_bounded_lower(input.complex_fn_ratio);
    let cog_complex = score_bounded_lower(input.cog_complex_ratio);
    let long_fn = score_bounded_lower(input.long_fn_ratio);
    let large_files = score_bounded_lower(input.large_file_ratio);
    let high_params = score_bounded_lower(input.high_param_ratio);
    let cohesion = input.cohesion.map(score_bounded_higher);           // higher = better
    let distance = score_bounded_lower(input.distance);
    // Comments: NOT truly bounded — 100% comments is absurd.
    // Meaningful range is [0, ~0.30]. Use sigmoid: 8% → score 0.5, 20% → 0.71.
    let comments = input.comment_ratio.map(|v| v / (v + 0.08));       // sigmoid, midpoint 8%

    // ── Hidden Debt: mix ──
    let dead_code = score_bounded_lower(input.dead_code_ratio);
    let duplication = score_bounded_lower(input.duplication_ratio);
    let test_coverage = score_bounded_higher(input.test_coverage_ratio); // higher = better
    let attack_surface = score_bounded_lower(input.attack_surface_ratio);

    let dim_scores = DimensionScores {
        coupling, cycles, god_files, hotspots, levelization,
        blast_radius, depth, entropy,
        complex_fn, cog_complex, long_fn, large_files, high_params,
        cohesion, distance, comments,
        dead_code, duplication, test_coverage, attack_surface,
    };

    // ── Category scores (arithmetic mean within each) ──
    let blast_radius_cat = arithmetic_mean(&[
        coupling, cycles, god_files, hotspots,
        levelization, blast_radius, depth, entropy,
    ]);

    // Cognitive Load: include optional dimensions only if measured
    let mut cog_members = vec![complex_fn, cog_complex, long_fn, large_files, high_params, distance];
    if let Some(c) = cohesion { cog_members.push(c); }
    if let Some(c) = comments { cog_members.push(c); }
    let cognitive_load_cat = arithmetic_mean(&cog_members);

    let hidden_debt_cat = arithmetic_mean(&[
        dead_code, duplication, test_coverage, attack_surface,
    ]);

    let cats = CategoryScores {
        blast_radius: blast_radius_cat,
        cognitive_load: cognitive_load_cat,
        hidden_debt: hidden_debt_cat,
    };

    (dim_scores, cats)
}

// ══════════════════════════════════════════════════════════════════
//  LAYER 5: Quality Signal = geometric mean of 3 categories
// ══════════════════════════════════════════════════════════════════

/// Compute quality signal as geometric mean of 3 category scores.
///
/// Mathematical properties (Nash Social Welfare theorem):
///   - Gaming one category while tanking another → signal stays flat or drops
///   - Genuine improvement across all categories → signal rises
///   - Uniquely satisfies Pareto optimality + symmetry + independence axioms
///
/// For AI agent feedback loop: this is the ONE number to maximize.
pub(crate) fn compute_quality_signal(cats: &CategoryScores) -> f64 {
    // Geometric mean: (a × b × c)^(1/3)
    // Clamp inputs to avoid log(0) — minimum score 0.01
    let a = cats.blast_radius.max(0.01);
    let b = cats.cognitive_load.max(0.01);
    let c = cats.hidden_debt.max(0.01);
    (a * b * c).powf(1.0 / 3.0)
}

// ══════════════════════════════════════════════════════════════════
//  LAYER 6: Grades derived from scores
// ══════════════════════════════════════════════════════════════════

/// Convert a [0,1] score to a letter grade.
///   A > 0.80, B > 0.60, C > 0.40, D > 0.20, F ≤ 0.20
pub fn score_to_grade(score: f64) -> char {
    if score > 0.80 { 'A' }
    else if score > 0.60 { 'B' }
    else if score > 0.40 { 'C' }
    else if score > 0.20 { 'D' }
    else { 'F' }
}

/// Map letter grade to numeric value for backward compatibility.
pub(crate) fn grade_value(g: char) -> u32 {
    match g { 'A' => 4, 'B' => 3, 'C' => 2, 'D' => 1, _ => 0 }
}

/// Map numeric value back to letter grade.
pub(crate) fn value_grade(v: u32) -> char {
    match v { 4 => 'A', 3 => 'B', 2 => 'C', 1 => 'D', _ => 'F' }
}

fn arithmetic_mean(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.5; }
    values.iter().sum::<f64>() / values.len() as f64
}

// ══════════════════════════════════════════════════════════════════
//  UNIFIED GRADING: produces everything from one input
// ══════════════════════════════════════════════════════════════════

/// All raw metric values needed for grading. Includes health + arch + testgap.
pub(crate) struct GradeInput {
    // Blast Radius inputs (all bounded [0,1] except cycles/depth)
    pub coupling: f64,
    pub entropy: f64,
    pub depth: u32,                      // unbounded count
    pub cycles: usize,                   // unbounded count
    pub god_ratio: f64,
    pub hotspot_ratio: f64,
    pub levelization_upward_ratio: f64,
    pub blast_radius_ratio: f64,

    // Cognitive Load inputs
    pub complex_fn_ratio: f64,
    pub cog_complex_ratio: f64,
    pub long_fn_ratio: f64,
    pub large_file_ratio: f64,
    pub high_param_ratio: f64,
    pub cohesion: Option<f64>,           // higher is better
    pub distance: f64,
    pub comment_ratio: Option<f64>,      // higher is better

    // Hidden Debt inputs
    pub dead_code_ratio: f64,
    pub duplication_ratio: f64,
    pub test_coverage_ratio: f64,        // higher is better
    pub attack_surface_ratio: f64,
}

/// Compute everything: dimension scores, category scores, quality signal, grades.
pub(crate) fn compute_grades(input: &GradeInput) -> (DimensionScores, DimensionGrades, CategoryScores, f64, char) {
    let (dim_scores, cat_scores) = compute_scores(input);
    let quality_signal = compute_quality_signal(&cat_scores);
    let overall_grade = score_to_grade(quality_signal);

    // Per-dimension grades derived from scores
    let dims = DimensionGrades {
        // Blast Radius
        coupling: score_to_grade(dim_scores.coupling),
        cycles: score_to_grade(dim_scores.cycles),
        god_files: score_to_grade(dim_scores.god_files),
        hotspots: score_to_grade(dim_scores.hotspots),
        levelization: score_to_grade(dim_scores.levelization),
        blast_radius: score_to_grade(dim_scores.blast_radius),
        depth: score_to_grade(dim_scores.depth),
        entropy: score_to_grade(dim_scores.entropy),
        // Cognitive Load
        complex_fn: score_to_grade(dim_scores.complex_fn),
        cog_complex: score_to_grade(dim_scores.cog_complex),
        long_fn: score_to_grade(dim_scores.long_fn),
        file_size: score_to_grade(dim_scores.large_files),
        high_params: score_to_grade(dim_scores.high_params),
        cohesion: dim_scores.cohesion.map(score_to_grade),
        distance: score_to_grade(dim_scores.distance),
        comment: dim_scores.comments.map(score_to_grade),
        // Hidden Debt
        dead_code: score_to_grade(dim_scores.dead_code),
        duplication: score_to_grade(dim_scores.duplication),
        test_coverage: score_to_grade(dim_scores.test_coverage),
        attack_surface: score_to_grade(dim_scores.attack_surface),
    };

    (dim_scores, dims, cat_scores, quality_signal, overall_grade)
}

// ══════════════════════════════════════════════════════════════════
//  LEGACY COMPAT: individual grade functions kept for rules engine
// ══════════════════════════════════════════════════════════════════

/// Grade coupling score directly (used by rules/checks.rs).
pub(crate) fn grade_coupling(v: f64) -> char {
    score_to_grade(score_bounded_lower(v))
}

/// Grade entropy directly.
pub(crate) fn grade_entropy_adjusted(v: f64, _num_pairs: usize) -> char {
    score_to_grade(score_bounded_lower(v))
}

#[allow(dead_code)]
pub(crate) fn grade_entropy(v: f64) -> char {
    grade_entropy_adjusted(v, 5)
}
