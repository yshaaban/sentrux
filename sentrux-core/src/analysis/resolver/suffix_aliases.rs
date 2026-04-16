use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A single path alias mapping: prefix → replacement paths.
pub(super) struct PathAlias {
    pub(super) prefix: String,
    pub(super) replacements: Vec<String>,
}

/// Apply path alias substitution to a specifier.
pub(super) fn apply_path_alias(specifier: &str, aliases: &[PathAlias]) -> Option<String> {
    for alias in aliases {
        if specifier.starts_with(&alias.prefix) {
            let remainder = &specifier[alias.prefix.len()..];
            if let Some(replacement) = alias.replacements.first() {
                return Some(format!("{}{}", replacement, remainder));
            }
        }

        let exact = alias.prefix.trim_end_matches('/');
        if specifier == exact {
            if let Some(replacement) = alias.replacements.first() {
                let dir = replacement.trim_end_matches('/');
                if dir.is_empty() {
                    continue;
                }
                return Some(dir.to_string());
            }
        }
    }
    None
}

/// Try loading path aliases for a single project directory from a resolver config.
fn try_load_project_aliases(
    project_dir: &str,
    scan_root: &Path,
    resolver: &crate::analysis::plugin::profile::ResolverConfig,
) -> Option<Vec<PathAlias>> {
    let config_path = if project_dir.is_empty() {
        scan_root.join(&resolver.path_alias_file)
    } else {
        scan_root.join(project_dir).join(&resolver.path_alias_file)
    };
    if !config_path.exists() {
        return None;
    }
    parse_path_alias_config(
        &config_path,
        &resolver.path_alias_field,
        &resolver.path_alias_base_url,
    )
}

pub(super) fn load_path_aliases(
    project_map: &HashMap<String, String>,
    scan_root: &Path,
) -> HashMap<String, Vec<PathAlias>> {
    let mut result: HashMap<String, Vec<PathAlias>> = HashMap::new();
    let unique_roots: HashSet<&str> = project_map.values().map(|s| s.as_str()).collect();

    for profile in crate::analysis::lang_registry::all_profiles() {
        let resolver = &profile.semantics.resolver;
        if resolver.path_alias_file.is_empty() || resolver.path_alias_field.is_empty() {
            continue;
        }
        for &project_dir in &unique_roots {
            if result.contains_key(project_dir) {
                continue;
            }
            if let Some(aliases) = try_load_project_aliases(project_dir, scan_root, resolver) {
                result
                    .entry(project_dir.to_string())
                    .or_default()
                    .extend(aliases);
            }
        }
    }
    result
}

/// Try to resolve a single project directory into a PathAlias from its manifest.
fn resolve_project_alias(
    project_dir: &str,
    scan_root: &Path,
    resolver: &crate::analysis::plugin::profile::ResolverConfig,
) -> Option<PathAlias> {
    let manifest_dir = if project_dir.is_empty() {
        scan_root.to_path_buf()
    } else {
        scan_root.join(project_dir)
    };
    let manifest_path = resolve_manifest_path(&manifest_dir, &resolver.alias_file)?;
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let name = extract_name_from_manifest(&content, &resolver.alias_field, &resolver.alias_file)
        .filter(|n| !n.is_empty())?;
    let transformed = apply_alias_transform(&name, &resolver.alias_transform);
    let dir_replacement = build_dir_replacement(project_dir, &resolver.source_root);
    Some(PathAlias {
        prefix: format!("{}/", transformed),
        replacements: vec![dir_replacement],
    })
}

/// Apply alias_transform (e.g., hyphen_to_underscore) to a manifest name.
pub(super) fn apply_alias_transform(name: &str, transform: &str) -> String {
    match transform {
        "hyphen_to_underscore" => name.replace('-', "_"),
        _ => name.to_string(),
    }
}

/// Build the directory replacement path from project_dir and source_root.
fn build_dir_replacement(project_dir: &str, source_root: &str) -> String {
    let base = if project_dir.is_empty() {
        String::new()
    } else {
        format!("{}/", project_dir)
    };
    if source_root.is_empty() {
        base
    } else {
        format!("{}{}/", base, source_root)
    }
}

pub(super) fn collect_manifest_path_aliases(
    project_map: &HashMap<String, String>,
    scan_root: &Path,
) -> Vec<PathAlias> {
    let mut aliases = Vec::new();
    let mut seen_dirs: HashSet<String> = HashSet::new();
    let unique_roots: HashSet<&str> = project_map.values().map(|s| s.as_str()).collect();

    for profile in crate::analysis::lang_registry::all_profiles() {
        let resolver = &profile.semantics.resolver;
        if resolver.alias_file.is_empty() || resolver.alias_field.is_empty() {
            continue;
        }
        for &project_dir in &unique_roots {
            if seen_dirs.contains(project_dir) {
                continue;
            }
            if let Some(alias) = resolve_project_alias(project_dir, scan_root, resolver) {
                aliases.push(alias);
                seen_dirs.insert(project_dir.to_string());
            }
        }
    }

    aliases.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));
    aliases
}

/// Resolve a manifest filename to an actual path.
/// Supports exact names ("Cargo.toml") and glob patterns ("*.csproj").
pub(super) fn resolve_manifest_path(dir: &Path, filename: &str) -> Option<PathBuf> {
    if filename.starts_with('*') {
        let ext = filename.trim_start_matches('*');
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(ext) && entry.path().is_file() {
                    return Some(entry.path());
                }
            }
        }
        None
    } else {
        let path = dir.join(filename);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }
}

/// Extract a name field from a manifest file.
/// Supports 5 strategies, auto-detected from filename extension.
pub(super) fn extract_name_from_manifest(
    content: &str,
    field: &str,
    filename: &str,
) -> Option<String> {
    if filename.ends_with(".json") {
        extract_json_field(content, field)
    } else if filename.ends_with(".toml") {
        extract_toml_field(content, field)
    } else if filename.ends_with(".xml") || filename.ends_with("proj") {
        extract_xml_field(content, field)
    } else if filename.ends_with(".yaml") || filename.ends_with(".yml") {
        extract_yaml_field(content, field)
    } else {
        extract_line_match(content, field)
    }
}

/// Extract a value by scanning lines for a prefix and extracting the string/symbol after it.
/// Handles: `prefix "value"`, `prefix 'value'`, `prefix :value` (Elixir atoms).
fn extract_line_match(content: &str, prefix: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let rest = rest
                .trim()
                .trim_start_matches(|c: char| c == '=' || c == ':')
                .trim();
            if rest.starts_with('"') {
                return rest[1..].find('"').map(|i| rest[1..1 + i].to_string());
            }
            if rest.starts_with('\'') {
                return rest[1..].find('\'').map(|i| rest[1..1 + i].to_string());
            }
            if rest.starts_with(':') {
                let atom = rest[1..]
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if !atom.is_empty() {
                    return Some(atom.to_string());
                }
            }
            let word = rest
                .split(|c: char| c.is_whitespace() || c == ',')
                .next()
                .unwrap_or("")
                .trim_matches('"');
            if !word.is_empty() {
                return Some(word.to_string());
            }
        }
    }
    None
}

/// Extract a dot-separated field from JSON (e.g., "name" or "compilerOptions.paths").
fn extract_json_field(content: &str, field: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    navigate_json(&json, field)?.as_str().map(|s| s.to_string())
}

/// Extract a dot-separated field from TOML (e.g., "package.name").
fn extract_toml_field(content: &str, field: &str) -> Option<String> {
    let val: toml::Value = content.parse().ok()?;
    let mut current = &val;
    for key in field.split('.') {
        current = current.get(key)?;
    }
    current.as_str().map(|s| s.to_string())
}

/// Extract a dot-separated field from XML (e.g., "project.artifactId").
fn extract_xml_field(content: &str, field: &str) -> Option<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let path_parts: Vec<&str> = field.split('.').collect();
    let mut reader = Reader::from_str(content);
    let mut depth_matched = 0usize;
    let mut capture_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if depth_matched < path_parts.len() && tag == path_parts[depth_matched] {
                    depth_matched += 1;
                    if depth_matched == path_parts.len() {
                        capture_text = true;
                    }
                }
            }
            Ok(Event::Text(e)) if capture_text => {
                let text = e.unescape().ok()?.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
            Ok(Event::End(_)) if capture_text => {
                return None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

/// Extract a dot-separated field from YAML (e.g., "name" or "project.name").
fn extract_yaml_field(content: &str, field: &str) -> Option<String> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    let mut current = &yaml;
    for key in field.split('.') {
        current = current.get(key)?;
    }
    current.as_str().map(|s| s.to_string())
}

/// Parse a JSON config file and extract path alias mappings.
fn parse_path_alias_config(
    config_path: &Path,
    field_path: &str,
    base_url_path: &str,
) -> Option<Vec<PathAlias>> {
    let content = std::fs::read_to_string(config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let base_url = if !base_url_path.is_empty() {
        navigate_json(&json, base_url_path)
            .and_then(|v| v.as_str())
            .unwrap_or(".")
    } else {
        "."
    };

    let paths_obj = navigate_json(&json, field_path)?.as_object()?;
    let mut aliases = Vec::new();

    for (pattern, mapped) in paths_obj {
        let prefix = pattern.trim_end_matches('*');
        if prefix.is_empty() {
            continue;
        }
        let replacements: Vec<String> = match mapped {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| {
                    let stripped = s.trim_end_matches('*');
                    if base_url == "." {
                        stripped.to_string()
                    } else {
                        format!(
                            "{}/{}",
                            base_url.trim_end_matches('/'),
                            stripped.trim_start_matches("./")
                        )
                    }
                })
                .collect(),
            _ => continue,
        };
        if !replacements.is_empty() {
            aliases.push(PathAlias {
                prefix: prefix.to_string(),
                replacements,
            });
        }
    }

    aliases.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));
    if aliases.is_empty() {
        None
    } else {
        Some(aliases)
    }
}

/// Navigate a JSON value by dot-separated path.
fn navigate_json<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::{apply_path_alias, collect_manifest_path_aliases, load_path_aliases};
    use crate::test_support::temp_root;
    use std::collections::HashMap;

    #[test]
    fn load_path_aliases_reads_project_tsconfig_paths() {
        let root = temp_root("sentrux", "suffix-aliases", &["web"]);
        std::fs::write(
            root.join("web/tsconfig.json"),
            r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@app/*": ["src/*"]
    }
  }
}
"#,
        )
        .expect("write tsconfig");

        let project_map = HashMap::from([("web/src/main.ts".to_string(), "web".to_string())]);
        let aliases = load_path_aliases(&project_map, &root);
        let web_aliases = aliases.get("web").expect("web aliases");

        assert!(web_aliases
            .iter()
            .any(|alias| alias.prefix == "@app/" && alias.replacements == vec!["src/"]));
    }

    #[test]
    fn collect_manifest_path_aliases_maps_package_name_to_source_root() {
        let root = temp_root("sentrux", "manifest-aliases", &["web"]);
        std::fs::write(
            root.join("web/package.json"),
            r#"{
  "name": "web-app"
}
"#,
        )
        .expect("write package.json");

        let project_map = HashMap::from([("web/src/main.ts".to_string(), "web".to_string())]);
        let aliases = collect_manifest_path_aliases(&project_map, &root);

        assert_eq!(
            apply_path_alias("web-app/components/button", &aliases).as_deref(),
            Some("web/src/components/button")
        );
    }
}
