//! Import resolution — resolves import specifiers to file paths.
//!
//! Tier 1: oxc_resolver for JS/TS (webpack-compatible).
//! Tier 2: suffix-index + file-path join for everything else.

pub mod helpers;
pub mod oxc;
pub mod suffix;


// Re-exports for backward compatibility
