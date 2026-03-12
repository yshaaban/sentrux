//! Tool registry builder — single point of registration for all MCP tools.
//!
//! Each tool's schema lives with its handler in `_def()` functions,
//! and this file collects them into a registry.
//!
//! Pro tools (gate, churn, coupling_history, bus_factor, whatif) live in a
//! separate private repository and are registered via the `pro` feature flag.

use super::handlers;
use super::handlers_evo;
use super::registry::ToolRegistry;

/// Build the core tool registry with free tools registered.
/// Called once at MCP server startup. Returns a mutable registry
/// so callers (e.g., sentrux-bin with pro feature) can register additional tools.
pub fn build_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();

    // ── Core scan/session tools ──
    reg.register(handlers::scan_def());
    reg.register(handlers::rescan_def());
    reg.register(handlers::session_start_def());
    reg.register(handlers::session_end_def());

    // ── Health & structure diagnostics ──
    reg.register(handlers::health_def());
    reg.register(handlers::coupling_def());
    reg.register(handlers::cycles_def());

    // ── Architecture diagnostics ──
    reg.register(handlers::architecture_def());
    reg.register(handlers::blast_radius_def());
    reg.register(handlers::hottest_def());
    reg.register(handlers::level_def());

    // ── Rules ──
    reg.register(handlers::check_rules_def());

    // ── Evolution (git history analysis) ──
    reg.register(handlers_evo::evolution_def());

    // ── DSM & Test Gaps ──
    reg.register(handlers_evo::dsm_def());
    reg.register(handlers_evo::test_gaps_def());

    reg
}
