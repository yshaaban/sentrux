use clap::{Parser, Subcommand, ValueEnum};

pub(crate) fn edition_name() -> &'static str {
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
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    /// Directory to open in the GUI
    #[arg(global = false)]
    pub(crate) path: Option<String>,

    /// Start MCP server (hidden alias for `sentrux mcp`)
    #[arg(long = "mcp", hide = true)]
    pub(crate) mcp_flag: bool,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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

    /// Generate a standalone external repository engineering report
    Report {
        /// Repository to analyze
        #[arg(default_value = ".")]
        repo_root: String,

        /// Optional label used for report artifact names
        #[arg(long)]
        repo_label: Option<String>,

        /// Directory where report artifacts should be written
        #[arg(long)]
        output_dir: Option<String>,

        /// Previous raw-tool-analysis.json for before/after comparison
        #[arg(long)]
        previous_analysis: Option<String>,

        /// Analysis mode; defaults to isolated working-tree analysis
        #[arg(long, value_enum, default_value_t = ReportModeArg::WorkingTree)]
        mode: ReportModeArg,

        /// Existing rules.toml to apply inside the isolated analysis workspace
        #[arg(long)]
        rules_source: Option<String>,

        /// Do not apply generated rules inside the isolated analysis workspace
        #[arg(long)]
        no_apply_suggested_rules: bool,

        /// Keep the temporary analysis workspace for debugging
        #[arg(long)]
        keep_workspace: bool,

        /// Maximum findings to capture in evidence artifacts
        #[arg(long, default_value_t = 25)]
        findings_limit: usize,

        /// Maximum experimental dead-private candidates to review
        #[arg(long, default_value_t = 10)]
        dead_private_limit: usize,
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
pub(crate) enum AnalyticsAction {
    /// Turn analytics on
    On,
    /// Turn analytics off
    Off,
}

#[derive(Subcommand)]
pub(crate) enum PluginAction {
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
pub(crate) enum BriefModeArg {
    RepoOnboarding,
    Patch,
    PreMerge,
}

impl BriefModeArg {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RepoOnboarding => "repo_onboarding",
            Self::Patch => "patch",
            Self::PreMerge => "pre_merge",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ReportModeArg {
    WorkingTree,
    Head,
    Live,
}

impl ReportModeArg {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WorkingTree => "working-tree",
            Self::Head => "head",
            Self::Live => "live",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BriefModeArg, Cli, Command, PluginAction, ReportModeArg};
    use clap::Parser;

    fn parse_cli(args: &[&str]) -> Cli {
        Cli::try_parse_from(args.iter().copied()).expect("parse cli args")
    }

    #[test]
    fn hidden_mcp_flag_sets_compatibility_switch_without_command() {
        let cli = parse_cli(&["sentrux", "--mcp"]);

        assert!(cli.mcp_flag);
        assert!(cli.command.is_none());
        assert!(cli.path.is_none());
    }

    #[test]
    fn brief_args_preserve_mode_limit_and_path() {
        let cli = parse_cli(&[
            "sentrux",
            "brief",
            "--mode",
            "repo-onboarding",
            "--limit",
            "5",
            "fixtures/repo",
        ]);

        match cli.command {
            Some(Command::Brief {
                mode,
                strict,
                limit,
                path,
            }) => {
                assert_eq!(mode, BriefModeArg::RepoOnboarding);
                assert!(!strict);
                assert_eq!(limit, 5);
                assert_eq!(path, "fixtures/repo");
            }
            _ => panic!("expected brief command"),
        }
    }

    #[test]
    fn scan_command_keeps_optional_gui_path() {
        let cli = parse_cli(&["sentrux", "scan", "fixtures/repo"]);

        match cli.command {
            Some(Command::Scan { path }) => {
                assert_eq!(path.as_deref(), Some("fixtures/repo"));
            }
            _ => panic!("expected scan command"),
        }
    }

    #[test]
    fn plugin_subcommand_parses_nested_action() {
        let cli = parse_cli(&["sentrux", "plugin", "add", "typescript"]);

        match cli.command {
            Some(Command::Plugin {
                action: PluginAction::Add { name },
            }) => assert_eq!(name, "typescript"),
            _ => panic!("expected plugin add command"),
        }
    }

    #[test]
    fn report_command_parses_external_repo_options() {
        let cli = parse_cli(&[
            "sentrux",
            "report",
            "/tmp/mail-simulator",
            "--repo-label",
            "mail-simulator",
            "--output-dir",
            "/tmp/mail-report",
            "--mode",
            "head",
            "--previous-analysis",
            "/tmp/previous.json",
            "--rules-source",
            "/tmp/rules.toml",
            "--no-apply-suggested-rules",
            "--keep-workspace",
            "--findings-limit",
            "50",
            "--dead-private-limit",
            "12",
        ]);

        match cli.command {
            Some(Command::Report {
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
                assert_eq!(repo_root, "/tmp/mail-simulator");
                assert_eq!(repo_label.as_deref(), Some("mail-simulator"));
                assert_eq!(output_dir.as_deref(), Some("/tmp/mail-report"));
                assert_eq!(previous_analysis.as_deref(), Some("/tmp/previous.json"));
                assert_eq!(mode, ReportModeArg::Head);
                assert_eq!(rules_source.as_deref(), Some("/tmp/rules.toml"));
                assert!(no_apply_suggested_rules);
                assert!(keep_workspace);
                assert_eq!(findings_limit, 50);
                assert_eq!(dead_private_limit, 12);
            }
            _ => panic!("expected report command"),
        }
    }
}
