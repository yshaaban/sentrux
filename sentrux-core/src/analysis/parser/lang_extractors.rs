//! Import extraction helpers and base-class extraction.
//!
//! Import extraction: most languages use AST-based extraction (ast_import_walker.rs)
//! or @import.module query captures. Only Elixir still needs a text extractor
//! for multi-alias expansion (alias Collect.{Listing, Offer}).
//!
//! Base class extraction: data-driven via base_class_node_kinds in plugin.toml.
//! Generic fallback for languages without configured node kinds.

// ── Import extractors ────────────────────────────────────────────────

/// Convert a dot-separated PascalCase module path to snake_case file path.
/// "Collect.Listing" → "collect/listing", "GenServer" → "gen_server"
pub(super) fn pascal_to_snake_path(module: &str) -> String {
    module.split('.').map(pascal_to_snake).collect::<Vec<_>>().join("/")
}

fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_lowercase() || prev.is_ascii_digit()
                    || (prev.is_uppercase()
                        && chars.get(i + 1).is_some_and(|ch| ch.is_lowercase()))
                {
                    result.push('_');
                }
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

fn elixir_module_to_path(module: &str) -> String {
    module.split('.').map(pascal_to_snake).collect::<Vec<_>>().join("/")
}

fn expand_elixir_multi_alias(text: &str) -> Vec<String> {
    let brace_start = match text.find('{') { Some(i) => i, None => return vec![] };
    let brace_end = match text.find('}') { Some(i) => i, None => return vec![] };
    let prefix = text[..brace_start].trim_end_matches('.');
    let items = &text[brace_start + 1..brace_end];
    items.split(',')
        .map(|item| {
            let name = item.trim();
            if name.is_empty() { String::new() }
            else { elixir_module_to_path(&format!("{}.{}", prefix, name)) }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Elixir: extract module paths from alias/import/use/require calls.
/// Handles PascalCase→snake_case conversion and multi-alias {A, B} expansion.
pub(super) fn extract_elixir(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    let rest = if let Some(r) = trimmed.strip_prefix("alias ") { r }
        else if let Some(r) = trimmed.strip_prefix("import ") { r }
        else if let Some(r) = trimmed.strip_prefix("use ") { r }
        else if let Some(r) = trimmed.strip_prefix("require ") { r }
        else { return vec![] };
    let rest = rest.trim_start();
    let rest = rest.split('\n').next().unwrap_or(rest);
    if rest.contains('{') { return expand_elixir_multi_alias(rest); }
    let module = rest.split(|c: char| c == ',' || c == ' ' || c == '\t')
        .next().unwrap_or("").trim();
    if module.is_empty() || !module.starts_with(|c: char| c.is_uppercase()) { return vec![]; }
    let path = elixir_module_to_path(module);
    if path.is_empty() { vec![] } else { vec![path] }
}

/// Generic fallback: search for standalone "from" keyword with word boundaries.
pub(super) fn extract_fallback(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut end = text.len();
    while let Some(rel) = text[..end].rfind("from") {
        let left_ok = rel == 0 || { let c = bytes[rel - 1]; !c.is_ascii_alphanumeric() && c != b'_' };
        let right_ok = rel + 4 >= bytes.len() || { let c = bytes[rel + 4]; !c.is_ascii_alphanumeric() && c != b'_' };
        if left_ok && right_ok {
            let s = text[rel + 4..].trim()
                .trim_matches(|c: char| c == '\'' || c == '"' || c == ';')
                .to_string();
            return if s.is_empty() { vec![] } else { vec![s] };
        }
        end = rel;
    }
    vec![]
}

// ── Base-class extraction helpers ────────────────────────────────────

/// Collect base classes by matching child node kinds against a set of patterns.
/// Used by the data-driven `base_class_node_kinds` profile field.
pub(super) fn extract_bases_by_kinds(node: tree_sitter::Node, content: &[u8], kinds: &[&str], bases: &mut Vec<String>) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if kinds.contains(&child.kind()) {
            collect_type_identifiers(child, content, bases);
        }
    }
}

/// Generic fallback: collect base classes from children whose kind contains inheritance keywords.
pub(super) fn extract_bases_generic(node: tree_sitter::Node, content: &[u8], bases: &mut Vec<String>) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        let k = child.kind();
        if k.contains("superclass") || k.contains("extends")
            || k.contains("base_class") || k.contains("heritage")
        {
            collect_type_identifiers(child, content, bases);
        }
    }
}

fn is_type_identifier_kind(kind: &str) -> bool {
    matches!(kind, "type_identifier" | "identifier" | "constant" | "scope_resolution")
}

fn is_leaf_type_node(node: tree_sitter::Node) -> bool {
    is_type_identifier_kind(node.kind())
        && (node.child_count() == 0 || node.kind() == "scope_resolution")
}

fn is_visibility_keyword(name: &str) -> bool {
    matches!(name, "public" | "private" | "protected")
}

const MAX_TYPE_COLLECT_DEPTH: usize = 64;

fn collect_type_identifiers(node: tree_sitter::Node, content: &[u8], out: &mut Vec<String>) {
    collect_type_identifiers_inner(node, content, out, 0);
}

fn collect_type_identifiers_inner(node: tree_sitter::Node, content: &[u8], out: &mut Vec<String>, depth: usize) {
    if depth >= MAX_TYPE_COLLECT_DEPTH { return; }
    if is_leaf_type_node(node) {
        if let Ok(text) = node.utf8_text(content) {
            let name = text.trim().to_string();
            if !name.is_empty() && !is_visibility_keyword(&name) {
                out.push(name);
                return;
            }
        }
    }
    for i in 0..node.child_count() {
        collect_type_identifiers_inner(node.child(i).unwrap(), content, out, depth + 1);
    }
}
