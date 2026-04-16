//! Sentrux binary — GUI, CLI, and MCP entry points.
//!
//! All logic lives in `sentrux-core`. This crate is just the entry point
//! that wires together the three modes:
//! - GUI mode (default): interactive treemap/blueprint visualizer
//! - MCP mode (`sentrux mcp`): Model Context Protocol server for AI agent integration
//! - Check mode (`sentrux check [path]`): CLI architectural rules enforcement
//! - Gate mode (`sentrux gate [--save] [path]`): touched-concept or structural regression testing
//! - Brief mode (`sentrux brief --mode patch [path]`): structured v2 agent guidance JSON

use clap::{Parser, Subcommand, ValueEnum};
use sentrux_core::analysis;
use sentrux_core::app;
use sentrux_core::core;
use sentrux_core::metrics;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

fn edition_name() -> &'static str {
    let tier = sentrux_core::license::current_tier();
    if tier >= sentrux_core::license::Tier::Pro {
        "Pro"
    } else {
        "" // Don't show "Free" or "Community" — just "sentrux"
    }
}

fn version_string() -> &'static str {
    use std::sync::OnceLock;
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let edition = edition_name();
        let base = if edition.is_empty() {
            env!("CARGO_PKG_VERSION").to_string()
        } else {
            format!("{} ({})", env!("CARGO_PKG_VERSION"), edition)
        };
        if let Some(latest) = sentrux_core::app::update_check::available_update() {
            format!(
                "{}\n  Update available: v{} → {}",
                base,
                latest,
                sentrux_core::app::update_check::PUBLIC_UPDATE_HINT
            )
        } else {
            base
        }
    })
}

#[derive(Parser)]
#[command(
    name = "sentrux",
    about = "Live codebase visualization and patch-safety gate",
    version = version_string(),
    arg_required_else_help = false,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Directory to open in the GUI
    #[arg(global = false)]
    path: Option<String>,

    /// Start MCP server (hidden alias for `sentrux mcp`)
    #[arg(long = "mcp", hide = true)]
    mcp_flag: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Enforce architectural rules defined in .sentrux/rules.toml
    Check {
        /// Directory to check
        #[arg(default_value = ".")]
        path: String,
    },

    /// Touched-concept patch-safety gate with legacy structural fallback
    Gate {
        /// Save current metrics as the new baseline
        #[arg(long)]
        save: bool,

        /// Treat introduced medium-severity findings as blocking in v2 mode
        #[arg(long)]
        strict: bool,

        /// Directory to gate
        #[arg(default_value = ".")]
        path: String,
    },

    /// Emit a structured v2 agent guidance brief as JSON
    Brief {
        /// Brief mode to generate
        #[arg(long, value_enum, default_value_t = BriefModeArg::Patch)]
        mode: BriefModeArg,

        /// Treat introduced medium-severity findings as blocking in pre_merge mode
        #[arg(long)]
        strict: bool,

        /// Maximum number of primary targets to include
        #[arg(long, default_value_t = 3)]
        limit: usize,

        /// Directory to analyze
        #[arg(default_value = ".")]
        path: String,
    },

    /// Open the GUI with a pre-loaded directory
    Scan {
        /// Directory to visualize
        path: Option<String>,
    },

    /// Start the MCP (Model Context Protocol) server for AI agent integration
    Mcp,

    /// Manage language plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Control anonymous aggregate usage analytics
    Analytics {
        #[command(subcommand)]
        action: Option<AnalyticsAction>,
    },

    /// Upgrade to Sentrux Pro
    Login,
}

#[derive(Subcommand)]
enum AnalyticsAction {
    /// Turn analytics on
    On,
    /// Turn analytics off
    Off,
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List,

    /// Install all standard language plugins
    AddStandard,

    /// Install a single language plugin from the plugin registry
    Add {
        /// Plugin name (e.g. "python", "rust")
        name: String,
    },

    /// Remove an installed plugin
    Remove {
        /// Plugin name to remove
        name: String,
    },

    /// Create a new plugin template
    Init {
        /// Language name for the new plugin
        name: String,
    },

    /// Validate a plugin directory
    Validate {
        /// Path to the plugin directory
        dir: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BriefModeArg {
    RepoOnboarding,
    Patch,
    PreMerge,
}

impl BriefModeArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::RepoOnboarding => "repo_onboarding",
            Self::Patch => "patch",
            Self::PreMerge => "pre_merge",
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

pub fn run() -> eframe::Result<()> {
    // Pro initialization is handled by an optional integration crate externally
    // before calling run().

    // Step 1: Download missing grammar binaries (may overwrite configs with old versions)
    ensure_grammars_installed();

    // Step 2: Sync embedded plugin configs LAST — always wins over downloaded configs.
    // This ensures configs match the binary version even if the grammar tarball
    // included old plugin.toml/tags.scm files.
    sentrux_core::analysis::plugin::sync_embedded_plugins();

    // Non-blocking update check (once per day, background thread)
    app::update_check::check_for_updates_async(env!("CARGO_PKG_VERSION"));

    let cli = Cli::parse();

    // Hidden --mcp flag for backward compat with MCP client configs
    if cli.mcp_flag {
        app::mcp_server::run_mcp_server(None);
        return Ok(());
    }

    match cli.command {
        Some(Command::Check { path }) => {
            std::process::exit(run_check(&path));
        }
        Some(Command::Gate { save, strict, path }) => {
            std::process::exit(run_gate(&path, save, strict));
        }
        Some(Command::Brief {
            mode,
            strict,
            limit,
            path,
        }) => {
            std::process::exit(run_brief(&path, mode, strict, limit));
        }
        Some(Command::Mcp) => {
            app::mcp_server::run_mcp_server(None);
            Ok(())
        }
        Some(Command::Plugin { action }) => {
            run_plugin(action);
            Ok(())
        }
        Some(Command::Analytics { action }) => {
            run_analytics(action);
            Ok(())
        }
        Some(Command::Login) => {
            run_login();
            Ok(())
        }
        Some(Command::Scan { path }) => run_gui(path),
        None => run_gui(cli.path),
    }
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Analytics
// ---------------------------------------------------------------------------

fn analytics_opt_out_path() -> Option<std::path::PathBuf> {
    sentrux_core::analysis::plugin::plugins_dir()
        .map(|d| d.parent().unwrap().join("telemetry_opt_out"))
}

fn run_login() {
    // Check if this binary has Pro code compiled in
    #[cfg(feature = "pro")]
    {
        // Pro binary: do actual login flow
        // TODO: open browser → OAuth → save license key
        println!("Opening browser for Sentrux Pro login...");
        println!("(Not yet implemented — coming soon)");
        return;
    }

    #[cfg(not(feature = "pro"))]
    {
        // Free source build: tell user to get pre-built binary
        println!();
        println!("  Sentrux Pro requires the official binary.");
        println!();
        println!("  Install the public beta binary:");
        println!("    macOS/Linux: curl -fsSL https://raw.githubusercontent.com/yshaaban/sentrux/main/install.sh | sh");
        println!("    Source:      git clone https://github.com/yshaaban/sentrux.git && cd sentrux && cargo build --release -p sentrux");
        println!();
        println!("  Then run `sentrux login` to activate Pro.");
        println!();
    }
}

fn run_analytics(action: Option<AnalyticsAction>) {
    let path = analytics_opt_out_path();
    match action {
        None => {
            // No subcommand = show the current analytics state.
            let opted_out = path.as_ref().map_or(false, |p| p.exists());
            if opted_out {
                println!("Analytics are disabled.");
            } else {
                println!("Analytics are enabled.");
            }
        }
        Some(AnalyticsAction::On) => {
            if let Some(p) = &path {
                let _ = std::fs::remove_file(p);
            }
            println!("Analytics are enabled.");
        }
        Some(AnalyticsAction::Off) => {
            if let Some(p) = &path {
                let _ = std::fs::create_dir_all(p.parent().unwrap());
                let _ = std::fs::write(p, "1");
            }
            println!("Analytics are disabled.");
        }
    }
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

/// Run architectural rules check from CLI. Returns exit code.
fn run_check(path: &str) -> i32 {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    let config = match metrics::rules::RulesConfig::try_load(root) {
        Some(c) => c,
        None => {
            eprintln!("No .sentrux/rules.toml found in {path}");
            eprintln!("Create one to define architectural constraints.");
            return 1;
        }
    };

    eprintln!("Scanning {path}...");
    let result = match analysis::scanner::scan_directory(path, None, None, &cli_scan_limits(), None)
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    let health = metrics::compute_health(&result.snapshot);
    let arch_report = metrics::arch::compute_arch(&result.snapshot);
    let check = metrics::rules::check_rules(
        &config,
        &health,
        &arch_report,
        &result.snapshot.import_graph,
    );

    let has_v2_rules = config_has_v2_rules(&config);
    print_check_results(&check, &health, &arch_report, has_v2_rules)
}

/// Print check results and return exit code (0 = pass, 1 = violations).
fn print_check_results(
    check: &metrics::rules::RuleCheckResult,
    health: &metrics::HealthReport,
    _arch_report: &metrics::arch::ArchReport,
    has_v2_rules: bool,
) -> i32 {
    println!(
        "sentrux check — legacy structural rules check ({} rules checked)\n",
        check.rules_checked
    );
    if has_v2_rules {
        println!(
            "Primary v2 workflow: use `sentrux gate` for touched-concept regressions and MCP `findings` / `session_end` for actionable findings, obligations, and optimization priorities.\n"
        );
    }
    println!(
        "Supporting structural context: {}\n",
        (health.quality_signal * 10000.0).round() as u32
    );

    if check.violations.is_empty() {
        println!("✓ All rules pass");
        0
    } else {
        for v in &check.violations {
            let icon = match v.severity {
                metrics::rules::Severity::Error => "✗",
                metrics::rules::Severity::Warning => "⚠",
            };
            println!("{icon} [{:?}] {}: {}", v.severity, v.rule, v.message);
            for f in &v.files {
                println!("    {f}");
            }
        }
        println!("\n✗ {} violation(s) found", check.violations.len());
        1
    }
}

// ---------------------------------------------------------------------------
// Gate
// ---------------------------------------------------------------------------

/// Run the CLI gate. Uses the v2 touched-concept model when v2 rules are configured,
/// otherwise falls back to the legacy structural regression gate.
fn run_gate(path: &str, save_mode: bool, strict: bool) -> i32 {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    if v2_rules_enabled(root) {
        return run_v2_gate(root, save_mode, strict);
    }

    let baseline_path = metrics::arch::baseline_path(root);

    eprintln!("Scanning {path}...");
    let result = match analysis::scanner::scan_directory(path, None, None, &cli_scan_limits(), None)
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    let health = metrics::compute_health(&result.snapshot);
    let arch_report = metrics::arch::compute_arch(&result.snapshot);

    if save_mode {
        gate_save(&baseline_path, &health, &arch_report)
    } else {
        gate_compare(&baseline_path, &health, &arch_report)
    }
}

fn v2_rules_enabled(root: &std::path::Path) -> bool {
    sentrux_core::metrics::rules::RulesConfig::try_load(root)
        .map(|config| config_has_v2_rules(&config))
        .unwrap_or(false)
}

fn config_has_v2_rules(config: &metrics::rules::RulesConfig) -> bool {
    !config.concept.is_empty() || !config.contract.is_empty() || !config.state_model.is_empty()
}

fn run_v2_gate(root: &std::path::Path, save_mode: bool, strict: bool) -> i32 {
    let result = if save_mode {
        sentrux_core::app::mcp_server::handlers::cli_save_v2_session(root)
    } else {
        sentrux_core::app::mcp_server::handlers::cli_evaluate_v2_gate(root, strict)
    };

    match result {
        Ok(payload) => {
            if save_mode {
                print_v2_gate_save(&payload);
                0
            } else {
                print_v2_gate_result(&payload)
            }
        }
        Err(error) => {
            eprintln!("v2 gate failed: {error}");
            1
        }
    }
}

fn run_brief(path: &str, mode: BriefModeArg, strict: bool, limit: usize) -> i32 {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    match sentrux_core::app::mcp_server::handlers::cli_agent_brief(
        root,
        mode.as_str(),
        strict,
        limit,
    ) {
        Ok(payload) => match serde_json::to_string_pretty(&payload) {
            Ok(text) => {
                println!("{text}");
                0
            }
            Err(error) => {
                eprintln!("Failed to serialize brief JSON: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("agent brief failed: {error}");
            1
        }
    }
}

fn print_v2_gate_save(payload: &serde_json::Value) {
    println!("sentrux gate — v2 baseline saved\n");
    if let Some(path) = payload
        .get("session_v2_baseline_path")
        .and_then(|value| value.as_str())
    {
        println!("V2 session baseline: {path}");
    }
    if let Some(count) = payload
        .get("session_finding_count")
        .and_then(|value| value.as_u64())
    {
        println!("Tracked findings: {count}");
    }
    if let Some(count) = payload
        .get("suppressed_finding_count")
        .and_then(|value| value.as_u64())
    {
        println!("Suppressed findings: {count}");
    }
    if let Some(count) = payload
        .get("expired_suppression_match_count")
        .and_then(|value| value.as_u64())
    {
        println!("Expired suppression matches: {count}");
    }
    print_cli_confidence_summary(payload);
    if let Some(path) = payload
        .get("baseline_path")
        .and_then(|value| value.as_str())
    {
        println!("Legacy structural baseline: {path}");
    }
    if let Some(quality_signal) = payload
        .get("quality_signal")
        .and_then(|value| value.as_u64())
    {
        println!("Supporting structural context: {quality_signal}");
    }
    if let Some(error) = diagnostics_error(payload, "semantic") {
        println!("\nSemantic note: {error}");
    }
    if let Some(message) = payload.get("message").and_then(|value| value.as_str()) {
        println!("\n{message}");
    }
}

fn print_v2_gate_result(payload: &serde_json::Value) -> i32 {
    let decision = payload
        .get("decision")
        .and_then(|value| value.as_str())
        .unwrap_or("fail");
    let summary = payload
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("Touched-concept gate finished.");
    let changed_files = payload
        .get("changed_files")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let blocking_findings = payload
        .get("blocking_findings")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let missing_obligations = payload
        .get("missing_obligations")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let introduced_findings = payload
        .get("introduced_findings")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let introduced_nonblocking_findings = introduced_findings
        .iter()
        .filter(|finding| severity_of_value(finding) != "high")
        .cloned()
        .collect::<Vec<_>>();
    let suppression_hits = payload
        .get("suppression_hits")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let expired_suppressions = payload
        .get("expired_suppressions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let changed_concepts = string_array_from_value(payload.get("changed_concepts"));

    println!("sentrux gate — touched-concept regression check\n");
    print_v2_gate_summary(
        payload,
        decision,
        summary,
        &changed_files,
        &changed_concepts,
        &introduced_findings,
        &blocking_findings,
        &missing_obligations,
    );
    print_v2_gate_suppression_sections(&suppression_hits, &expired_suppressions);
    print_v2_gate_findings(&blocking_findings, &introduced_nonblocking_findings);
    print_v2_gate_obligations(&missing_obligations);
    print_v2_gate_notes(payload);
    exit_code_for_gate_decision(decision)
}

fn print_v2_gate_summary(
    payload: &serde_json::Value,
    decision: &str,
    summary: &str,
    changed_files: &[serde_json::Value],
    changed_concepts: &[String],
    introduced_findings: &[serde_json::Value],
    blocking_findings: &[serde_json::Value],
    missing_obligations: &[serde_json::Value],
) {
    println!("Decision:     {decision}");
    println!("Summary:      {summary}");
    println!("Changed files: {}", changed_files.len());
    if !changed_concepts.is_empty() {
        print_string_section("Changed concepts", changed_concepts, 10);
    }
    print_legacy_baseline_delta(payload);
    println!("Introduced findings: {}", introduced_findings.len());
    println!("Blocking findings:  {}", blocking_findings.len());
    println!("Missing obligations: {}", missing_obligations.len());
    if let Some(score) = payload
        .get("obligation_completeness_0_10000")
        .and_then(|value| value.as_u64())
    {
        println!("Obligation completeness: {score}/10000");
    }
    print_scan_trust_summary(payload);
    print_cli_confidence_summary(payload);
}

fn print_v2_gate_suppression_sections(
    suppression_hits: &[serde_json::Value],
    expired_suppressions: &[serde_json::Value],
) {
    println!("Suppression hits: {}", suppression_hits.len());
    println!("Expired suppressions: {}", expired_suppressions.len());
    print_gate_suppression_section("Suppression hits", suppression_hits);
    print_gate_suppression_section("Expired suppressions", expired_suppressions);
}

fn print_gate_suppression_section(title: &str, matches: &[serde_json::Value]) {
    if matches.is_empty() {
        return;
    }

    println!("\n{title}:");
    for matched in matches.iter().take(10) {
        print_cli_suppression_match(matched);
    }
}

fn print_v2_gate_findings(
    blocking_findings: &[serde_json::Value],
    introduced_nonblocking_findings: &[serde_json::Value],
) {
    print_gate_finding_section("Blocking findings", blocking_findings);
    print_gate_finding_section(
        "Introduced findings (non-blocking)",
        introduced_nonblocking_findings,
    );
}

fn print_gate_finding_section(title: &str, findings: &[serde_json::Value]) {
    if findings.is_empty() {
        return;
    }

    println!("\n{title}:");
    for finding in findings.iter().take(10) {
        print_cli_finding(finding);
    }
}

fn print_v2_gate_obligations(missing_obligations: &[serde_json::Value]) {
    if missing_obligations.is_empty() {
        return;
    }

    println!("\nMissing obligations:");
    for obligation in missing_obligations.iter().take(10) {
        print_cli_obligation(obligation);
    }
}

fn print_v2_gate_notes(payload: &serde_json::Value) {
    if let Some(error) = diagnostics_error(payload, "semantic") {
        println!("\nSemantic note: {error}");
    }
    if let Some(error) = diagnostics_error(payload, "baseline") {
        println!("Baseline note: {error}");
    }
}

fn exit_code_for_gate_decision(decision: &str) -> i32 {
    if decision == "pass" {
        0
    } else {
        1
    }
}

fn print_cli_finding(finding: &serde_json::Value) {
    let severity = finding
        .get("severity")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let summary = finding
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("finding");
    println!("  - [{severity}] {summary}");
}

fn print_cli_obligation(obligation: &serde_json::Value) {
    let summary = obligation
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("missing obligation");
    let missing_count = obligation
        .get("missing_sites")
        .and_then(|value| value.as_array())
        .map(|sites| sites.len())
        .unwrap_or(0);
    println!("  - {summary} ({missing_count} missing site(s))");
}

fn print_cli_suppression_match(matched: &serde_json::Value) {
    let kind = matched
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("*");
    let concept = matched
        .get("concept")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    let file = matched
        .get("file")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    let count = matched
        .get("matched_finding_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let reason = matched
        .get("reason")
        .and_then(|value| value.as_str())
        .unwrap_or("suppressed");
    println!("  - kind={kind} concept={concept} file={file} count={count} reason={reason}");
}

fn diagnostics_error<'a>(payload: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    payload
        .get("diagnostics")
        .and_then(|value| value.get("errors"))
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
}

fn severity_of_value(value: &serde_json::Value) -> &str {
    value
        .get("severity")
        .and_then(|severity| severity.as_str())
        .unwrap_or("low")
}

fn print_legacy_baseline_delta(payload: &serde_json::Value) {
    let Some(baseline_delta) = payload
        .get("baseline_delta")
        .and_then(|value| value.as_object())
    else {
        return;
    };
    if !baseline_delta
        .get("available")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return;
    }

    let signal_before = baseline_delta
        .get("signal_before")
        .and_then(|value| value.as_i64());
    let signal_after = baseline_delta
        .get("signal_after")
        .and_then(|value| value.as_i64());
    let signal_delta = baseline_delta
        .get("signal_delta")
        .and_then(|value| value.as_i64());
    if let (Some(before), Some(after), Some(delta)) = (signal_before, signal_after, signal_delta) {
        println!("Supporting structural delta: {before} -> {after} ({delta:+})");
    }

    let coupling_before = baseline_delta
        .get("coupling_before")
        .and_then(|value| value.as_f64());
    let coupling_after = baseline_delta
        .get("coupling_after")
        .and_then(|value| value.as_f64());
    if let (Some(before), Some(after)) = (coupling_before, coupling_after) {
        println!("Coupling:     {:.2} -> {:.2}", before, after);
    }

    let cycles_before = baseline_delta
        .get("cycles_before")
        .and_then(|value| value.as_i64());
    let cycles_after = baseline_delta
        .get("cycles_after")
        .and_then(|value| value.as_i64());
    if let (Some(before), Some(after)) = (cycles_before, cycles_after) {
        println!("Cycles:       {before} -> {after}");
    }
}

fn print_scan_trust_summary(payload: &serde_json::Value) {
    let Some(scan_trust) = payload
        .get("scan_trust")
        .and_then(|value| value.as_object())
    else {
        return;
    };

    let overall_confidence = scan_trust
        .get("overall_confidence_0_10000")
        .and_then(|value| value.as_u64());
    let scope_coverage = scan_trust
        .get("scope_coverage_0_10000")
        .and_then(|value| value.as_u64());
    let resolution = scan_trust
        .get("resolution")
        .and_then(|value| value.as_object());
    let resolved = resolution
        .and_then(|value| value.get("resolved"))
        .and_then(|value| value.as_u64());
    let unresolved_internal = resolution
        .and_then(|value| value.get("unresolved_internal"))
        .and_then(|value| value.as_u64());
    let unresolved_external = resolution
        .and_then(|value| value.get("unresolved_external"))
        .and_then(|value| value.as_u64());

    if let Some(overall_confidence) = overall_confidence {
        println!("Scan confidence: {overall_confidence}/10000");
    }
    if let Some(scope_coverage) = scope_coverage {
        println!("Scope coverage:  {scope_coverage}/10000");
    }
    if resolved.is_some() || unresolved_internal.is_some() || unresolved_external.is_some() {
        println!(
            "Resolution:      resolved {}, unresolved internal {}, unresolved external {}",
            resolved.unwrap_or(0),
            unresolved_internal.unwrap_or(0),
            unresolved_external.unwrap_or(0),
        );
    }

    if let Some(partial) = scan_trust.get("partial").and_then(|value| value.as_bool()) {
        if partial {
            println!("Scan note: partial coverage");
        }
    }
    if let Some(truncated) = scan_trust
        .get("truncated")
        .and_then(|value| value.as_bool())
    {
        if truncated {
            println!("Scan note: truncated results");
        }
    }
    if let Some(fallback_reason) = scan_trust
        .get("fallback_reason")
        .and_then(|value| value.as_str())
    {
        println!("Scan note: {fallback_reason}");
    }
}

fn print_cli_confidence_summary(payload: &serde_json::Value) {
    let Some(confidence) = payload
        .get("confidence")
        .and_then(|value| value.as_object())
    else {
        return;
    };

    if let Some(rule_coverage) = confidence
        .get("rule_coverage_0_10000")
        .and_then(|value| value.as_u64())
    {
        println!("Rule coverage:  {rule_coverage}/10000");
    }

    let session_baseline = confidence
        .get("session_baseline")
        .and_then(|value| value.as_object());
    if let Some(status) = session_baseline {
        let loaded = status
            .get("loaded")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let compatible = status
            .get("compatible")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let schema_version = status
            .get("schema_version")
            .and_then(|value| value.as_u64());
        let error = status.get("error").and_then(|value| value.as_str());

        if loaded {
            if let Some(version) = schema_version {
                let compatibility = if compatible {
                    "compatible"
                } else {
                    "incompatible"
                };
                println!("Session baseline: v{version} ({compatibility})");
            } else {
                println!("Session baseline: loaded");
            }
        } else {
            println!("Session baseline: unavailable");
        }

        if let Some(error) = error {
            println!("Session baseline note: {error}");
        }
    }
}

fn print_string_section(title: &str, items: &[String], limit: usize) {
    println!("\n{title}:");
    for item in items.iter().take(limit) {
        println!("  - {item}");
    }
    if items.len() > limit {
        println!("  - ... ({} more)", items.len() - limit);
    }
}

fn string_array_from_value(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|item| item.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn gate_save(
    baseline_path: &std::path::Path,
    health: &metrics::HealthReport,
    _arch_report: &metrics::arch::ArchReport,
) -> i32 {
    if let Some(parent) = baseline_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Failed to create directory {}: {e}", parent.display());
            return 1;
        }
    }
    let baseline = metrics::arch::ArchBaseline::from_health(health);
    match baseline.save(baseline_path) {
        Ok(()) => {
            println!(
                "Legacy structural baseline saved to {}",
                baseline_path.display()
            );
            println!(
                "Supporting structural context: {}",
                (health.quality_signal * 10000.0).round() as u32
            );
            println!("\nRun `sentrux gate` after making changes to compare.");
            0
        }
        Err(e) => {
            eprintln!("Failed to save baseline: {e}");
            1
        }
    }
}

fn gate_compare(
    baseline_path: &std::path::Path,
    health: &metrics::HealthReport,
    arch_report: &metrics::arch::ArchReport,
) -> i32 {
    let baseline = match metrics::arch::ArchBaseline::load(baseline_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Failed to load baseline at {}: {e}",
                baseline_path.display()
            );
            eprintln!("Run `sentrux gate --save` first to create one.");
            return 1;
        }
    };

    let diff = baseline.diff(health);

    println!("sentrux gate — legacy structural context check\n");
    println!(
        "Supporting structural context: {} -> {}",
        (diff.signal_before * 10000.0).round() as u32,
        (diff.signal_after * 10000.0).round() as u32
    );
    println!(
        "Coupling:     {:.2} → {:.2}",
        diff.coupling_before, diff.coupling_after
    );
    println!(
        "Cycles:       {} → {}",
        diff.cycles_before, diff.cycles_after
    );
    println!(
        "God files:    {} → {}",
        diff.god_files_before, diff.god_files_after
    );

    if !arch_report.distance_metrics.is_empty() {
        println!(
            "\nDistance from Main Sequence: {:.2}",
            arch_report.avg_distance
        );
    }

    if diff.degraded {
        println!("\n✗ DEGRADED");
        for v in &diff.violations {
            println!("  ✗ {v}");
        }
        1
    } else {
        println!("\n✓ No degradation detected");
        0
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

fn run_plugin(action: PluginAction) {
    match action {
        PluginAction::List => plugin_list(),
        PluginAction::Init { name } => plugin_init(&name),
        PluginAction::Validate { dir } => plugin_validate(&dir),
        PluginAction::AddStandard => plugin_add_standard(),
        PluginAction::Add { name } => plugin_add(&name),
        PluginAction::Remove { name } => plugin_remove(&name),
    }
}

fn plugin_list() {
    let dir = sentrux_core::analysis::plugin::plugins_dir();
    println!(
        "Plugin directory: {}",
        dir.as_ref()
            .map_or("(none)".into(), |d| d.display().to_string())
    );
    let (loaded, errors) = sentrux_core::analysis::plugin::load_all_plugins();
    if loaded.is_empty() && errors.is_empty() {
        println!("No plugins installed.");
        println!("\nInstall a plugin by placing it in ~/.sentrux/plugins/<name>/");
    } else {
        for p in &loaded {
            println!(
                "  {} v{} [{}] — {}",
                p.name,
                p.version,
                p.extensions.join(", "),
                p.display_name
            );
        }
        for e in &errors {
            println!("  (error) {} — {}", e.plugin_dir.display(), e.error);
        }
    }
}

fn plugin_init(name: &str) {
    let dir = sentrux_core::analysis::plugin::plugins_dir().unwrap_or_else(|| {
        eprintln!("Cannot determine home directory");
        std::process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if plugin_dir.exists() {
        eprintln!("Plugin directory already exists: {}", plugin_dir.display());
        std::process::exit(1);
    }
    std::fs::create_dir_all(plugin_dir.join("grammars")).unwrap();
    std::fs::create_dir_all(plugin_dir.join("queries")).unwrap();
    std::fs::create_dir_all(plugin_dir.join("tests")).unwrap();
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        format!(
            r#"[plugin]
name = "{name}"
display_name = "{name}"
version = "0.1.0"
extensions = ["TODO"]
min_sentrux_version = "0.1.3"

[plugin.metadata]
author = ""
description = ""

[grammar]
source = "https://github.com/TODO/tree-sitter-{name}"
ref = "main"
abi_version = 14

[queries]
capabilities = ["functions", "classes", "imports"]

[checksums]
"#
        ),
    )
    .unwrap();
    std::fs::write(plugin_dir.join("queries").join("tags.scm"),
        ";; TODO: Write tree-sitter queries for this language\n;;\n;; Required captures:\n;;   @func.def / @func.name — function definitions\n;;   @class.def / @class.name — class definitions\n;;   @import.path — import statements\n;;   @call.name — function calls (optional)\n"
    ).unwrap();
    println!("Created plugin template at {}", plugin_dir.display());
    println!("\nNext steps:");
    println!("  1. Edit plugin.toml — set extensions, grammar source");
    println!(
        "  2. Build the grammar: tree-sitter generate && cc -shared -o grammars/{} src/parser.c",
        sentrux_core::analysis::plugin::manifest::PluginManifest::grammar_filename()
    );
    println!("  3. Write queries/tags.scm");
    println!(
        "  4. Test: sentrux plugin validate {}",
        plugin_dir.display()
    );
}

fn plugin_validate(dir: &str) {
    let plugin_dir = std::path::Path::new(dir);
    print!("Validating {}... ", plugin_dir.display());
    match sentrux_core::analysis::plugin::manifest::PluginManifest::load(plugin_dir) {
        Ok(manifest) => {
            println!("plugin.toml OK");
            println!("  name: {}", manifest.plugin.name);
            println!("  version: {}", manifest.plugin.version);
            println!("  extensions: [{}]", manifest.plugin.extensions.join(", "));
            println!(
                "  capabilities: [{}]",
                manifest.queries.capabilities.join(", ")
            );
            let query_path = plugin_dir.join("queries").join("tags.scm");
            match std::fs::read_to_string(&query_path) {
                Ok(qs) => match manifest.validate_query_captures(&qs) {
                    Ok(()) => println!("  queries/tags.scm: OK (captures valid)"),
                    Err(e) => println!("  queries/tags.scm: FAIL — {}", e),
                },
                Err(e) => println!("  queries/tags.scm: MISSING — {}", e),
            }
            let gf = sentrux_core::analysis::plugin::manifest::PluginManifest::grammar_filename();
            let gp = plugin_dir.join("grammars").join(gf);
            if gp.exists() {
                println!("  grammars/{}: OK", gf);
            } else {
                println!("  grammars/{}: MISSING — build the grammar first", gf);
            }
        }
        Err(e) => {
            println!("FAIL — {}", e);
            std::process::exit(1);
        }
    }
}

fn plugin_add_standard() {
    sentrux_core::analysis::plugin::sync_embedded_plugins();
    ensure_grammars_installed();
    println!("Done. All plugins synced from embedded data.");
}

fn plugin_add(name: &str) {
    let dir = sentrux_core::analysis::plugin::plugins_dir().unwrap_or_else(|| {
        eprintln!("Cannot determine home directory");
        std::process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if plugin_dir.exists() {
        eprintln!(
            "Plugin '{}' already installed at {}",
            name,
            plugin_dir.display()
        );
        eprintln!("Remove it first: sentrux plugin remove {}", name);
        std::process::exit(1);
    }

    let platform = sentrux_core::analysis::plugin::manifest::PluginManifest::grammar_filename();
    let platform_key = platform.rsplit_once('.').map_or(platform, |(k, _)| k);

    let version = match sentrux_core::analysis::plugin::embedded::EMBEDDED_PLUGINS
        .iter()
        .find(|&&(n, _, _)| n == name)
        .and_then(|&(_, toml, _)| {
            toml.lines()
                .find(|l| l.starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
        }) {
        Some(v) => v,
        None => {
            eprintln!(
                "Plugin '{}' not found in embedded data. Is it a valid plugin name?",
                name
            );
            std::process::exit(1);
        }
    };
    let url = format!(
        "https://github.com/sentrux/plugins/releases/download/{name}-v{version}/{name}-{platform_key}.tar.gz"
    );
    println!("Downloading {name} plugin for {platform_key}...");
    println!("  {url}");

    std::fs::create_dir_all(&dir).unwrap();
    let tarball = dir.join(format!("{name}.tar.gz"));
    download_and_extract_plugin(&dir, name, &tarball, &url, &plugin_dir);
}

fn download_and_extract_plugin(
    dir: &std::path::Path,
    name: &str,
    tarball: &std::path::Path,
    url: &str,
    plugin_dir: &std::path::Path,
) {
    let output = std::process::Command::new("curl")
        .args(["-fsSL", url, "-o"])
        .arg(tarball)
        .status();

    match output {
        Ok(s) if s.success() => {
            let extract = std::process::Command::new("tar")
                .args(["xzf", &format!("{}.tar.gz", name)])
                .current_dir(dir)
                .status();
            let _ = std::fs::remove_file(tarball);
            match extract {
                Ok(s) if s.success() => {
                    println!("Installed {} to {}", name, plugin_dir.display());
                }
                _ => {
                    eprintln!("Failed to extract plugin archive");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            let _ = std::fs::remove_file(tarball);
            eprintln!("Failed to download plugin '{}'.", name);
            eprintln!("Check available plugins: https://github.com/sentrux/plugins/releases");
            std::process::exit(1);
        }
    }
}

fn plugin_remove(name: &str) {
    let dir = sentrux_core::analysis::plugin::plugins_dir().unwrap_or_else(|| {
        eprintln!("Cannot determine home directory");
        std::process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if !plugin_dir.exists() {
        eprintln!("Plugin '{}' not installed.", name);
        std::process::exit(1);
    }
    std::fs::remove_dir_all(&plugin_dir).unwrap();
    println!("Removed plugin '{}'", name);
}

// ---------------------------------------------------------------------------
// GUI
// ---------------------------------------------------------------------------

/// Probe which wgpu backends have usable GPU adapters on this system.
/// Returns only backends that actually have hardware support, avoiding
/// blind attempts that panic on unsupported drivers.
fn probe_available_backends() -> Vec<eframe::wgpu::Backends> {
    let candidates = [
        (
            "Primary+GL",
            eframe::wgpu::Backends::PRIMARY | eframe::wgpu::Backends::GL,
        ),
        ("GL-only", eframe::wgpu::Backends::GL),
        ("Primary", eframe::wgpu::Backends::PRIMARY),
    ];

    let mut available = Vec::new();
    for (label, backends) in &candidates {
        let instance = eframe::wgpu::Instance::new(&eframe::wgpu::InstanceDescriptor {
            backends: *backends,
            ..Default::default()
        });
        let adapters: Vec<_> = instance.enumerate_adapters(eframe::wgpu::Backends::all());
        if !adapters.is_empty() {
            sentrux_core::debug_log!("[gpu] probe {label}: {} adapter(s) found", adapters.len());
            available.push(*backends);
        } else {
            sentrux_core::debug_log!("[gpu] probe {label}: no adapters");
        }
    }
    available
}

fn run_gui(path: Option<String>) -> eframe::Result<()> {
    let initial_path = path
        .map(|p| {
            std::path::Path::new(&p)
                .canonicalize()
                .map(|c| c.to_string_lossy().to_string())
                .unwrap_or(p)
        })
        .filter(|p| std::path::Path::new(p).is_dir());

    // Determine backends: respect user override, otherwise probe hardware.
    let env_backends = eframe::wgpu::Backends::from_env();
    let backend_attempts: Vec<eframe::wgpu::Backends> = if let Some(user_choice) = env_backends {
        // User explicitly chose via WGPU_BACKEND — respect it, no fallback
        vec![user_choice]
    } else {
        let probed = probe_available_backends();
        if probed.is_empty() {
            // No hardware GPU — try software rendering via glow (OpenGL)
            return run_gui_glow(initial_path);
        }
        probed
    };

    let version = env!("CARGO_PKG_VERSION");
    let title = {
        let edition = edition_name();
        if edition.is_empty() {
            format!("sentrux v{}", version)
        } else {
            format!("Sentrux {} v{}", edition, version)
        }
    };
    let title = title.as_str();

    for (i, backends) in backend_attempts.iter().enumerate() {
        sentrux_core::debug_log!(
            "[gpu] attempt {}/{}: backends {:?}",
            i + 1,
            backend_attempts.len(),
            backends
        );

        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1600.0, 1000.0])
                .with_maximized(true)
                .with_title(title),
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                    eframe::egui_wgpu::WgpuSetupCreateNew {
                        instance_descriptor: eframe::wgpu::InstanceDescriptor {
                            backends: *backends,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
            ..Default::default()
        };

        let path_clone = initial_path.clone();
        // catch_unwind as safety net: wgpu can panic on surface creation
        // even when adapter enumeration succeeded (driver bugs, missing DRI3)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            eframe::run_native(
                "Sentrux",
                options,
                Box::new(move |cc| Ok(Box::new(app::SentruxApp::new(cc, path_clone)))),
            )
        }));

        match result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => {
                sentrux_core::debug_log!("[gpu] backend {:?} failed: {e}", backends);
            }
            Err(_panic) => {
                sentrux_core::debug_log!("[gpu] backend {:?} panicked (driver issue)", backends);
            }
        }

        if i + 1 == backend_attempts.len() {
            // All wgpu backends failed — fall back to glow (software OpenGL)
            return run_gui_glow(initial_path);
        }
    }
    Ok(())
}

/// Fallback GUI using glow (OpenGL) renderer — works on systems without
/// hardware GPU (VMs, RDP, headless servers with software OpenGL).
fn run_gui_glow(initial_path: Option<String>) -> eframe::Result<()> {
    sentrux_core::debug_log!("[gpu] falling back to glow (software OpenGL)");
    let version = env!("CARGO_PKG_VERSION");
    let title = {
        let edition = edition_name();
        if edition.is_empty() {
            format!("sentrux v{}", version)
        } else {
            format!("Sentrux {} v{}", edition, version)
        }
    };
    let title = title.as_str();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 1000.0])
            .with_maximized(true)
            .with_title(title),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Sentrux",
        options,
        Box::new(move |cc| Ok(Box::new(app::SentruxApp::new(cc, initial_path)))),
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cli_scan_limits() -> analysis::scanner::common::ScanLimits {
    let s = core::settings::Settings::default();
    analysis::scanner::common::ScanLimits {
        max_file_size_kb: s.max_file_size_kb,
        max_parse_size_kb: s.max_parse_size_kb,
        max_call_targets: s.max_call_targets,
    }
}

fn grammar_install_state_path(
    dir: &std::path::Path,
    version: &str,
    platform_key: &str,
) -> std::path::PathBuf {
    dir.join(format!(
        ".grammar-install-state-{version}-{platform_key}.json"
    ))
}

fn read_recorded_missing_grammars(path: &std::path::Path) -> Option<Vec<String>> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn write_recorded_missing_grammars(path: &std::path::Path, missing: &[String]) {
    if missing.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }

    if let Ok(serialized) = serde_json::to_string(missing) {
        let _ = std::fs::write(path, serialized);
    }
}

/// Ensure grammar binaries are installed for all embedded plugins.
/// Downloads ONE tarball with ALL grammars — not 49 individual downloads.
///
/// Architecture:
///   Each binary release on GitHub includes asset:
///     grammars-darwin-arm64.tar.gz (all grammars in one archive)
///   This function downloads that ONE file and extracts all grammars at once.
///
/// Handles: first launch, upgrade, accidental deletion.
fn ensure_grammars_installed() {
    // CI sets this to prevent overwriting already-installed grammars
    // with a 404 from a version tag that doesn't have grammar assets yet
    if std::env::var("SENTRUX_SKIP_GRAMMAR_DOWNLOAD").is_ok() {
        return;
    }

    let dir = match sentrux_core::analysis::plugin::plugins_dir() {
        Some(d) => d,
        None => return,
    };

    let platform = sentrux_core::analysis::plugin::manifest::PluginManifest::grammar_filename();
    let platform_key = platform.rsplit_once('.').map_or(platform, |(k, _)| k);

    let _ = std::fs::create_dir_all(&dir);

    let mut missing = sentrux_core::analysis::plugin::embedded::EMBEDDED_PLUGINS
        .iter()
        .filter_map(|&(name, toml, _)| {
            let grammar_path = dir.join(name).join("grammars").join(platform);
            (toml.contains("[grammar]") && !grammar_path.exists()).then(|| name.to_string())
        })
        .collect::<Vec<_>>();

    if missing.is_empty() {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");
    let state_path = grammar_install_state_path(&dir, version, platform_key);
    if read_recorded_missing_grammars(&state_path).as_deref() == Some(missing.as_slice()) {
        return;
    }

    let url = format!(
        "https://github.com/yshaaban/sentrux/releases/download/v{version}/grammars-{platform_key}.tar.gz"
    );
    let tarball = dir.join("grammars.tar.gz");

    eprintln!();
    eprintln!("  Downloading language grammars for v{version}...");
    eprintln!("  (one-time download, ~30MB)");
    eprint!("  [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]   0%");
    let _ = std::io::Write::flush(&mut std::io::stderr());

    let ok = std::process::Command::new("curl")
        .args(["-fsSL", "--progress-bar", &url, "-o"])
        .arg(&tarball)
        .stderr(std::process::Stdio::inherit()) // Show curl progress
        .stdout(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if ok {
        // Extract: tarball contains <lang>/grammars/<platform>.dylib for each language
        let extracted = std::process::Command::new("tar")
            .args(["xzf"])
            .arg(&tarball)
            .current_dir(&dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        let _ = std::fs::remove_file(&tarball);

        if extracted {
            missing = sentrux_core::analysis::plugin::embedded::EMBEDDED_PLUGINS
                .iter()
                .filter_map(|&(name, toml, _)| {
                    let grammar_path = dir.join(name).join("grammars").join(platform);
                    (toml.contains("[grammar]") && !grammar_path.exists()).then(|| name.to_string())
                })
                .collect();
            write_recorded_missing_grammars(&state_path, &missing);

            // Count how many grammars we now have
            let count = sentrux_core::analysis::plugin::embedded::EMBEDDED_PLUGINS
                .iter()
                .filter(|&&(name, _, _)| dir.join(name).join("grammars").join(platform).exists())
                .count();
            eprintln!("  {count} language grammars ready.");
            if !missing.is_empty() {
                eprintln!("  Still missing grammars for: {}", missing.join(", "));
            }
        } else {
            eprintln!("  Failed to extract grammars archive.");
        }
    } else {
        let _ = std::fs::remove_file(&tarball);
        eprintln!("  Download failed. Check your network and try again.");
        eprintln!("  URL: {url}");
    }
    eprintln!();
}
