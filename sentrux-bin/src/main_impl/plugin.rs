use super::cli::PluginAction;
use std::path::Path;
use std::{fs, process};

pub(crate) fn run_plugin(action: PluginAction) {
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
        process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if plugin_dir.exists() {
        eprintln!("Plugin directory already exists: {}", plugin_dir.display());
        process::exit(1);
    }
    for (path, label) in [
        (
            plugin_dir.join("grammars"),
            "create plugin grammars directory",
        ),
        (
            plugin_dir.join("queries"),
            "create plugin queries directory",
        ),
        (plugin_dir.join("tests"), "create plugin tests directory"),
    ] {
        create_dir_all_or_exit(&path, label);
    }
    write_file_or_exit(
        &plugin_dir.join("plugin.toml"),
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
        "write plugin manifest",
    );
    write_file_or_exit(
        &plugin_dir.join("queries").join("tags.scm"),
        ";; TODO: Write tree-sitter queries for this language\n;;\n;; Required captures:\n;;   @func.def / @func.name — function definitions\n;;   @class.def / @class.name — class definitions\n;;   @import.path — import statements\n;;   @call.name — function calls (optional)\n",
        "write default query file",
    );
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
            process::exit(1);
        }
    }
}

fn plugin_add_standard() {
    sentrux_core::analysis::plugin::sync_embedded_plugins();
    crate::main_impl::gui::ensure_grammars_installed();
    println!("Done. All plugins synced from embedded data.");
}

fn plugin_add(name: &str) {
    let dir = sentrux_core::analysis::plugin::plugins_dir().unwrap_or_else(|| {
        eprintln!("Cannot determine home directory");
        process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if plugin_dir.exists() {
        eprintln!(
            "Plugin '{}' already installed at {}",
            name,
            plugin_dir.display()
        );
        eprintln!("Remove it first: sentrux plugin remove {}", name);
        process::exit(1);
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
            process::exit(1);
        }
    };
    let url = format!(
        "https://github.com/sentrux/plugins/releases/download/{name}-v{version}/{name}-{platform_key}.tar.gz"
    );
    println!("Downloading {name} plugin for {platform_key}...");
    println!("  {url}");

    create_dir_all_or_exit(&dir, "create plugin directory");
    let tarball = dir.join(format!("{name}.tar.gz"));
    download_and_extract_plugin(&dir, name, &tarball, &url, &plugin_dir);
}

fn download_and_extract_plugin(
    dir: &Path,
    name: &str,
    tarball: &Path,
    url: &str,
    plugin_dir: &Path,
) {
    let output = process::Command::new("curl")
        .args(["-fsSL", url, "-o"])
        .arg(tarball)
        .status();

    match output {
        Ok(s) if s.success() => {
            let extract = process::Command::new("tar")
                .args(["xzf", &format!("{}.tar.gz", name)])
                .current_dir(dir)
                .status();
            let _ = fs::remove_file(tarball);
            match extract {
                Ok(s) if s.success() => {
                    println!("Installed {} to {}", name, plugin_dir.display());
                }
                _ => {
                    eprintln!("Failed to extract plugin archive");
                    process::exit(1);
                }
            }
        }
        _ => {
            let _ = fs::remove_file(tarball);
            eprintln!("Failed to download plugin '{}'.", name);
            eprintln!("Check available plugins: https://github.com/sentrux/plugins/releases");
            process::exit(1);
        }
    }
}

fn plugin_remove(name: &str) {
    let dir = sentrux_core::analysis::plugin::plugins_dir().unwrap_or_else(|| {
        eprintln!("Cannot determine home directory");
        process::exit(1);
    });
    let plugin_dir = dir.join(name);
    if !plugin_dir.exists() {
        eprintln!("Plugin '{}' not installed.", name);
        process::exit(1);
    }
    remove_dir_all_or_exit(&plugin_dir, "remove plugin directory");
    println!("Removed plugin '{}'", name);
}

fn create_dir_all_or_exit(path: &Path, action: &str) {
    if let Err(error) = fs::create_dir_all(path) {
        exit_with_io_error(action, path, error);
    }
}

fn write_file_or_exit(path: &Path, contents: impl AsRef<[u8]>, action: &str) {
    if let Err(error) = fs::write(path, contents) {
        exit_with_io_error(action, path, error);
    }
}

fn remove_dir_all_or_exit(path: &Path, action: &str) {
    if let Err(error) = fs::remove_dir_all(path) {
        exit_with_io_error(action, path, error);
    }
}

fn exit_with_io_error(action: &str, path: &Path, error: std::io::Error) -> ! {
    eprintln!("Failed to {action} {}: {error}", path.display());
    process::exit(1);
}
