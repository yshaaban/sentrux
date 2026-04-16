use super::cli::edition_name;

pub(crate) fn ensure_grammars_installed() {
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

pub(crate) fn run_gui(path: Option<String>) -> eframe::Result<()> {
    let initial_path = path
        .map(|p| {
            std::path::Path::new(&p)
                .canonicalize()
                .map(|c| c.to_string_lossy().to_string())
                .unwrap_or(p)
        })
        .filter(|p| std::path::Path::new(p).is_dir());

    let env_backends = eframe::wgpu::Backends::from_env();
    let backend_attempts: Vec<eframe::wgpu::Backends> = if let Some(user_choice) = env_backends {
        vec![user_choice]
    } else {
        let probed = probe_available_backends();
        if probed.is_empty() {
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            eframe::run_native(
                "Sentrux",
                options,
                Box::new(move |cc| {
                    Ok(Box::new(sentrux_core::app::SentruxApp::new(cc, path_clone)))
                }),
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
            return run_gui_glow(initial_path);
        }
    }
    Ok(())
}

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
        Box::new(move |cc| {
            Ok(Box::new(sentrux_core::app::SentruxApp::new(
                cc,
                initial_path,
            )))
        }),
    )
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
