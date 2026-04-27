//! Sentrux binary — GUI, CLI, and MCP entry points.
//!
//! All logic lives in `sentrux-core`. This crate is just the entry point
//! that wires together the three modes:
//! - GUI mode (default): interactive treemap/blueprint visualizer
//! - MCP mode (`sentrux mcp`): Model Context Protocol server for AI agent integration
//! - Check mode (`sentrux check [path]`): CLI architectural rules enforcement
//! - Gate mode (`sentrux gate [--save] [path]`): touched-concept or structural regression testing
//! - Brief mode (`sentrux brief --mode patch [path]`): structured v2 agent guidance JSON
//! - Report mode (`sentrux report [path]`): standalone external repository report artifacts

mod cli;
mod commands;
mod gui;
mod output;
mod plugin;

use clap::Parser;

pub fn run() -> eframe::Result<()> {
    // Pro initialization is handled by an optional integration crate externally
    // before calling run().

    // Step 1: Download missing grammar binaries (may overwrite configs with old versions)
    gui::ensure_grammars_installed();

    // Step 2: Sync embedded plugin configs LAST — always wins over downloaded configs.
    // This ensures configs match the binary version even if the grammar tarball
    // included old plugin.toml/tags.scm files.
    sentrux_core::analysis::plugin::sync_embedded_plugins();

    // Non-blocking update check (once per day, background thread)
    sentrux_core::app::update_check::check_for_updates_async(env!("CARGO_PKG_VERSION"));

    let cli = cli::Cli::parse();

    // Hidden --mcp flag for backward compat with MCP client configs
    if cli.mcp_flag {
        sentrux_core::app::mcp_server::run_mcp_server(None);
        return Ok(());
    }

    match cli.command {
        Some(cli::Command::Check { path }) => {
            std::process::exit(commands::run_check(&path));
        }
        Some(cli::Command::Gate { save, strict, path }) => {
            std::process::exit(commands::run_gate(&path, save, strict));
        }
        Some(cli::Command::Brief {
            mode,
            strict,
            limit,
            path,
        }) => {
            std::process::exit(commands::run_brief(&path, mode, strict, limit));
        }
        Some(cli::Command::Report {
            repo_root,
            repo_label,
            output_dir,
            previous_analysis,
            mode,
            rules_source,
            no_apply_suggested_rules,
            keep_workspace,
            findings_limit,
            dead_private_limit,
        }) => {
            std::process::exit(commands::run_report(commands::ReportOptions {
                repo_root: &repo_root,
                repo_label: repo_label.as_deref(),
                output_dir: output_dir.as_deref(),
                previous_analysis: previous_analysis.as_deref(),
                mode,
                rules_source: rules_source.as_deref(),
                no_apply_suggested_rules,
                keep_workspace,
                findings_limit,
                dead_private_limit,
            }));
        }
        Some(cli::Command::Mcp) => {
            sentrux_core::app::mcp_server::run_mcp_server(None);
            Ok(())
        }
        Some(cli::Command::Plugin { action }) => {
            plugin::run_plugin(action);
            Ok(())
        }
        Some(cli::Command::Analytics { action }) => {
            commands::run_analytics(action);
            Ok(())
        }
        Some(cli::Command::Login) => {
            commands::run_login();
            Ok(())
        }
        Some(cli::Command::Scan { path }) => gui::run_gui(path),
        None => gui::run_gui(cli.path),
    }
}
