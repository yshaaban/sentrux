//! Sentrux binary library — allows private-integration-crate to reuse the entire CLI/GUI.
//!
//! Architecture: private-integration-crate depends on sentrux_bin and calls `sentrux_bin::run()`.
//! The only difference: private-integration-crate calls `license::set_tier(Pro)` before `run()`.

mod main_impl;
pub use main_impl::run;
