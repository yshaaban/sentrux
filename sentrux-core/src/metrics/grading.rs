//! Grade utility functions.
//!
//! The root_causes module is now the primary quality signal.
//! This module keeps only utility functions used by the evolution display,
//! rules engine, and what-if scenarios.

/// Convert a [0,1] score to a letter grade.
///   A > 0.80, B > 0.60, C > 0.40, D > 0.20, F <= 0.20
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

/// Bounded [0,1] ratio, lower is better -> score = 1 - ratio.
fn score_bounded_lower(ratio: f64) -> f64 {
    (1.0 - ratio).clamp(0.0, 1.0)
}

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
