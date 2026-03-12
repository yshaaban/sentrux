//! Sentrux binary — GUI, CLI, and MCP entry points.
//!
//! All logic lives in `sentrux-core`. This crate is just the entry point
//! that wires together the three modes:
//! - GUI mode (default): interactive treemap/blueprint visualizer
//! - MCP mode (`--mcp`): Model Context Protocol server for AI agent integration
//! - Check mode (`check <path>`): CLI architectural rules enforcement

use sentrux_core::analysis;
use sentrux_core::app;
use sentrux_core::core;
use sentrux_core::metrics;

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
    let result = match analysis::scanner::scan_directory(
        path, None, None,
        &cli_scan_limits(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    let health = metrics::compute_health(&result.snapshot);
    let arch_report = metrics::arch::compute_arch(&result.snapshot);
    let check = metrics::rules::check_rules(&config, &health, &arch_report, &result.snapshot.import_graph);

    print_check_results(&check, &health, &arch_report)
}

/// Print check results and return exit code (0 = pass, 1 = violations).
fn print_check_results(
    check: &metrics::rules::RuleCheckResult,
    health: &metrics::HealthReport,
    arch_report: &metrics::arch::ArchReport,
) -> i32 {
    println!("sentrux check — {} rules checked\n", check.rules_checked);
    println!("Structure grade: {}  Architecture grade: {}\n",
        health.grade, arch_report.arch_grade);

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

/// Run structural regression gate from CLI. Returns exit code.
fn run_gate(args: &[String]) -> i32 {
    let save_mode = args.iter().any(|a| a == "--save");
    let path = args.iter()
        .skip(1)
        .rfind(|a| !a.starts_with('-') && *a != "gate")
        .map(|s| s.as_str())
        .unwrap_or(".");

    let root = std::path::Path::new(path);
    if !root.is_dir() {
        eprintln!("Error: not a directory: {path}");
        return 1;
    }

    let baseline_path = root.join(".sentrux").join("baseline.json");

    eprintln!("Scanning {path}...");
    let result = match analysis::scanner::scan_directory(
        path, None, None,
        &cli_scan_limits(),
    ) {
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

fn gate_save(
    baseline_path: &std::path::Path,
    health: &metrics::HealthReport,
    arch_report: &metrics::arch::ArchReport,
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
            println!("Baseline saved to {}", baseline_path.display());
            println!("Structure grade: {}  Architecture grade: {}",
                health.grade, arch_report.arch_grade);
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
            eprintln!("Failed to load baseline at {}: {e}", baseline_path.display());
            eprintln!("Run `sentrux gate --save` first to create one.");
            return 1;
        }
    };

    let diff = baseline.diff(health);

    println!("sentrux gate — structural regression check\n");
    println!("Structure:    {} → {}  Architecture: {}",
        diff.structure_grade_before, diff.structure_grade_after,
        arch_report.arch_grade);
    println!("Coupling:     {:.2} → {:.2}", diff.coupling_before, diff.coupling_after);
    println!("Cycles:       {} → {}", diff.cycles_before, diff.cycles_after);
    println!("God files:    {} → {}", diff.god_files_before, diff.god_files_after);

    if !arch_report.distance_metrics.is_empty() {
        println!("\nDistance from Main Sequence: {:.2} (grade {})",
            arch_report.avg_distance, arch_report.distance_grade);
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

fn cli_scan_limits() -> analysis::scanner::common::ScanLimits {
    let s = core::settings::Settings::default();
    analysis::scanner::common::ScanLimits {
        max_file_size_kb: s.max_file_size_kb,
        max_parse_size_kb: s.max_parse_size_kb,
        max_call_targets: s.max_call_targets,
    }
}

fn main() -> eframe::Result<()> {
    // Set license tier at startup (pro build validates key, free build stays Free)
    #[cfg(feature = "pro")]
    {
        let tier = sentrux_pro::license::load_and_validate()
            .unwrap_or(sentrux_core::license::Tier::Free);
        sentrux_core::license::set_tier(tier);
    }

    // --version: show version + edition (free or pro)
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        let edition = if cfg!(feature = "pro") { "Pro" } else { "Free" };
        println!("sentrux {} ({})", env!("CARGO_PKG_VERSION"), edition);
        return Ok(());
    }

    if std::env::args().any(|a| a == "--mcp") {
        #[cfg(feature = "pro")]
        {
            app::mcp_server::run_mcp_server(Some(&|reg| {
                sentrux_pro::register_pro_tools(reg);
            }));
        }
        #[cfg(not(feature = "pro"))]
        {
            app::mcp_server::run_mcp_server(None);
        }
        return Ok(());
    }

    if std::env::args().any(|a| a == "check") {
        let path = std::env::args()
            .skip_while(|a| a != "check")
            .nth(1)
            .unwrap_or_else(|| ".".to_string());
        std::process::exit(run_check(&path));
    }

    if std::env::args().any(|a| a == "gate") {
        let args: Vec<String> = std::env::args().collect();
        std::process::exit(run_gate(&args));
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title(if cfg!(feature = "pro") { "Sentrux Pro" } else { "sentrux" }),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Sentrux",
        options,
        Box::new(|cc| Ok(Box::new(app::SentruxApp::new(cc)))),
    )
}
