//! Sentrux binary library — allows an optional Pro integration crate to reuse the CLI/GUI.
//!
//! Architecture: an optional integration crate depends on `sentrux_bin` and calls
//! `sentrux_bin::run()`.
//! The only difference: the integration crate calls `license::set_tier(Pro)` before `run()`.

mod main_impl;
pub use main_impl::run;
