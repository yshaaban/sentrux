use super::cli::{AnalyticsAction, BriefModeArg, ReportModeArg};
use super::output;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn run_login() {
    #[cfg(feature = "pro")]
    {
        println!("Opening browser for Sentrux Pro login...");
        println!("(Not yet implemented — coming soon)");
        return;
    }

    #[cfg(not(feature = "pro"))]
    {
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

pub(crate) fn run_analytics(action: Option<AnalyticsAction>) {
    let path = analytics_opt_out_path();
    match action {
        None => {
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
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(p, "1");
            }
            println!("Analytics are disabled.");
        }
    }
}

pub(crate) fn run_check(path: &str) -> i32 {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    let config = match sentrux_core::metrics::rules::RulesConfig::try_load(root) {
        Some(c) => c,
        None => {
            eprintln!("No .sentrux/rules.toml found in {path}");
            eprintln!("Create one to define architectural constraints:");
            eprintln!("  mkdir -p {path}/.sentrux");
            eprintln!("  $EDITOR {path}/.sentrux/rules.toml");
            eprintln!("Then run `sentrux check {path}` again.");
            return 1;
        }
    };

    eprintln!("Scanning {path}...");
    let result = match sentrux_core::analysis::scanner::scan_directory(
        path,
        None,
        None,
        &cli_scan_limits(),
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    let health = sentrux_core::metrics::compute_health(&result.snapshot);
    let arch_report = sentrux_core::metrics::arch::compute_arch(&result.snapshot);
    let check = sentrux_core::metrics::rules::check_rules(
        &config,
        &health,
        &arch_report,
        &result.snapshot.import_graph,
    );

    let has_v2_rules = config_has_v2_rules(&config);
    print_check_results(&check, &health, &arch_report, has_v2_rules)
}

pub(crate) fn run_gate(path: &str, save_mode: bool, strict: bool) -> i32 {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    if v2_rules_enabled(root) {
        return run_v2_gate(root, save_mode, strict);
    }

    let baseline_path = sentrux_core::metrics::arch::baseline_path(root);

    eprintln!("Scanning {path}...");
    let result = match sentrux_core::analysis::scanner::scan_directory(
        path,
        None,
        None,
        &cli_scan_limits(),
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    let health = sentrux_core::metrics::compute_health(&result.snapshot);
    let arch_report = sentrux_core::metrics::arch::compute_arch(&result.snapshot);

    if save_mode {
        gate_save(&baseline_path, &health, &arch_report)
    } else {
        gate_compare(&baseline_path, &health, &arch_report)
    }
}

pub(crate) fn run_brief(path: &str, mode: BriefModeArg, strict: bool, limit: usize) -> i32 {
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

pub(crate) struct ReportOptions<'a> {
    pub(crate) repo_root: &'a str,
    pub(crate) repo_label: Option<&'a str>,
    pub(crate) output_dir: Option<&'a str>,
    pub(crate) previous_analysis: Option<&'a str>,
    pub(crate) mode: ReportModeArg,
    pub(crate) rules_source: Option<&'a str>,
    pub(crate) no_apply_suggested_rules: bool,
    pub(crate) keep_workspace: bool,
    pub(crate) findings_limit: usize,
    pub(crate) dead_private_limit: usize,
}

pub(crate) fn run_report(options: ReportOptions<'_>) -> i32 {
    let root = Path::new(options.repo_root);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {}", options.repo_root);
        return 1;
    }

    let script_path = match repo_advisor_script_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            eprintln!(
                "Run from a source checkout, set SENTRUX_REPO_ROOT to the Sentrux repository root, or set SENTRUX_ADVISOR_SCRIPT to scripts/analyze-repo.mjs."
            );
            return 1;
        }
    };

    let args = repo_advisor_args(&script_path, &options);

    eprintln!(
        "Generating external repo report for {}...",
        options.repo_root
    );
    eprintln!(
        "Target repo mutation: {}",
        report_mutation_label(options.mode)
    );

    let mut command = Command::new("node");
    command.args(&args);
    if let Ok(current_exe) = std::env::current_exe() {
        command.env("SENTRUX_BIN", current_exe);
    }

    match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("Failed to run Node-backed report workflow: {error}");
            eprintln!(
                "Install Node.js 20+ or run `node scripts/analyze-repo.mjs --repo-root {}` from the Sentrux checkout.",
                options.repo_root
            );
            1
        }
    }
}

fn repo_advisor_args(script_path: &Path, options: &ReportOptions<'_>) -> Vec<String> {
    let mut args = vec![
        script_path.to_string_lossy().to_string(),
        "--repo-root".to_string(),
        options.repo_root.to_string(),
        "--analysis-mode".to_string(),
        options.mode.as_str().to_string(),
        "--findings-limit".to_string(),
        options.findings_limit.to_string(),
        "--dead-private-limit".to_string(),
        options.dead_private_limit.to_string(),
    ];

    push_optional_arg(&mut args, "--repo-label", options.repo_label);
    push_optional_arg(&mut args, "--output-dir", options.output_dir);
    push_optional_arg(&mut args, "--previous-analysis", options.previous_analysis);
    push_optional_arg(&mut args, "--rules-source", options.rules_source);
    if options.no_apply_suggested_rules {
        args.push("--no-apply-suggested-rules".to_string());
    }
    if options.keep_workspace {
        args.push("--keep-workspace".to_string());
    }

    args
}

fn report_mutation_label(mode: ReportModeArg) -> &'static str {
    if mode == ReportModeArg::Live {
        "possible in live mode"
    } else {
        "none by default"
    }
}

fn push_optional_arg(args: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        args.push(flag.to_string());
        args.push(value.to_string());
    }
}

fn repo_advisor_script_path() -> Result<PathBuf, String> {
    if let Ok(script_path) = std::env::var("SENTRUX_ADVISOR_SCRIPT") {
        let candidate = PathBuf::from(script_path);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(format!(
            "SENTRUX_ADVISOR_SCRIPT does not point to a file: {}",
            candidate.display()
        ));
    }

    let candidate_roots = repo_advisor_candidate_roots();
    for root in candidate_roots {
        let candidate = root.join("scripts").join("analyze-repo.mjs");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Unable to locate scripts/analyze-repo.mjs for `sentrux report`.".to_string())
}

fn repo_advisor_candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(root) = std::env::var("SENTRUX_REPO_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir);
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            roots.push(ancestor.to_path_buf());
        }
    }
    roots
}

fn analytics_opt_out_path() -> Option<std::path::PathBuf> {
    sentrux_core::analysis::plugin::plugins_dir()
        .and_then(|d| d.parent().map(|parent| parent.join("telemetry_opt_out")))
}

fn cli_scan_limits() -> sentrux_core::analysis::scanner::common::ScanLimits {
    let s = sentrux_core::core::settings::Settings::default();
    sentrux_core::analysis::scanner::common::ScanLimits {
        max_file_size_kb: s.max_file_size_kb,
        max_parse_size_kb: s.max_parse_size_kb,
        max_call_targets: s.max_call_targets,
    }
}

fn config_has_v2_rules(config: &sentrux_core::metrics::rules::RulesConfig) -> bool {
    !config.concept.is_empty()
        || !config.contract.is_empty()
        || !config.state_model.is_empty()
        || !config.module_contract.is_empty()
}

fn print_check_results(
    check: &sentrux_core::metrics::rules::RuleCheckResult,
    health: &sentrux_core::metrics::HealthReport,
    _arch_report: &sentrux_core::metrics::arch::ArchReport,
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
                sentrux_core::metrics::rules::Severity::Error => "✗",
                sentrux_core::metrics::rules::Severity::Warning => "⚠",
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

fn v2_rules_enabled(root: &std::path::Path) -> bool {
    sentrux_core::metrics::rules::RulesConfig::try_load(root)
        .map(|config| config_has_v2_rules(&config))
        .unwrap_or(false)
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
                output::print_v2_gate_save(&payload);
                0
            } else {
                output::print_v2_gate_result(&payload)
            }
        }
        Err(error) => {
            eprintln!("v2 gate failed: {error}");
            1
        }
    }
}

fn gate_save(
    baseline_path: &std::path::Path,
    health: &sentrux_core::metrics::HealthReport,
    _arch_report: &sentrux_core::metrics::arch::ArchReport,
) -> i32 {
    if let Some(parent) = baseline_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Failed to create directory {}: {e}", parent.display());
            return 1;
        }
    }
    let baseline = sentrux_core::metrics::arch::ArchBaseline::from_health(health);
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
    health: &sentrux_core::metrics::HealthReport,
    arch_report: &sentrux_core::metrics::arch::ArchReport,
) -> i32 {
    let baseline = match sentrux_core::metrics::arch::ArchBaseline::load(baseline_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Failed to load baseline at {}: {e}",
                baseline_path.display()
            );
            eprintln!("Run `sentrux gate --save` first to create one.");
            eprintln!("For CI, commit .sentrux/baseline.json and run `sentrux gate` before merge.");
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
