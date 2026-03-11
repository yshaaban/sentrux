//! Sentrux core library — structural quality analysis engine.
//!
//! This crate contains all analysis, metrics, visualization, and MCP server logic.
//! It is consumed by:
//! - `sentrux-bin` (the main binary — GUI, CLI, MCP entry points)
//! - `private-integration-crate` (private crate — pro tool handlers, license validation)
//!
//! All modules are `pub` so that external crates can access types like
//! `ToolDef`, `McpState`, `Tier`, `Snapshot`, etc.

pub mod analysis;
pub mod app;
pub mod core;
pub mod layout;
pub mod license;
pub mod metrics;
pub mod renderer;
