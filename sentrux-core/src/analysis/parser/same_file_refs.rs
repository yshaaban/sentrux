use crate::core::types::FuncInfo;
use std::collections::HashMap;
use tree_sitter::Tree;

pub(super) fn annotate_same_file_reference_counts(
    tree: &Tree,
    content: &[u8],
    functions: &mut [FuncInfo],
) {
    let references = collect_same_file_reference_candidates(tree.root_node(), content);
    let mut references_by_name = HashMap::<String, Vec<u32>>::new();
    for (name, line) in references {
        references_by_name.entry(name).or_default().push(line);
    }

    let source = std::str::from_utf8(content).ok();
    let sanitized_lines = source.map(sanitize_source_lines);

    for function in functions {
        let ast_reference_count = references_by_name
            .get(&function.n)
            .map(|lines| {
                lines
                    .iter()
                    .filter(|line| **line < function.sl || **line > function.el)
                    .count()
            })
            .unwrap_or(0);
        let reference_count = if ast_reference_count > 0 {
            ast_reference_count
        } else {
            sanitized_lines
                .as_ref()
                .map(|lines| {
                    count_same_file_identifier_references(
                        lines,
                        &function.n,
                        function.sl as usize,
                        function.el as usize,
                    )
                })
                .unwrap_or(0)
        };
        if reference_count > 0 {
            function.same_file_ref_count = Some(reference_count as u32);
        }
    }
}

fn collect_same_file_reference_candidates(
    root: tree_sitter::Node<'_>,
    content: &[u8],
) -> Vec<(String, u32)> {
    let mut references = Vec::new();

    fn walk(node: tree_sitter::Node<'_>, content: &[u8], references: &mut Vec<(String, u32)>) {
        if is_same_file_reference_candidate(node) {
            if let Ok(name) = node.utf8_text(content) {
                if !name.is_empty() {
                    references.push((name.to_string(), node.start_position().row as u32 + 1));
                }
            }
        }

        for index in 0..node.child_count() {
            if let Some(child) = node.child(index) {
                walk(child, content, references);
            }
        }
    }

    walk(root, content, &mut references);
    references
}

#[derive(Default)]
struct SourceSanitizerState {
    block_comment_depth: usize,
    string_delimiter: Option<char>,
    escaped: bool,
}

fn sanitize_source_lines(source: &str) -> Vec<String> {
    let mut state = SourceSanitizerState::default();
    source
        .lines()
        .map(|line| sanitize_source_line(line, &mut state))
        .collect()
}

fn sanitize_source_line(line: &str, state: &mut SourceSanitizerState) -> String {
    let characters = line.chars().collect::<Vec<_>>();
    let mut sanitized = String::with_capacity(line.len());
    let mut index = 0usize;

    while index < characters.len() {
        let current = characters[index];
        let next = characters.get(index + 1).copied();

        if state.block_comment_depth > 0 {
            if current == '*' && next == Some('/') {
                state.block_comment_depth -= 1;
                sanitized.push(' ');
                sanitized.push(' ');
                index += 2;
            } else {
                sanitized.push(' ');
                index += 1;
            }
            continue;
        }

        if let Some(delimiter) = state.string_delimiter {
            sanitized.push(' ');
            if state.escaped {
                state.escaped = false;
            } else if current == '\\' {
                state.escaped = true;
            } else if current == delimiter {
                state.string_delimiter = None;
            }
            index += 1;
            continue;
        }

        if current == '/' && next == Some('/') {
            sanitized.extend(std::iter::repeat(' ').take(characters.len() - index));
            break;
        }

        if current == '/' && next == Some('*') {
            state.block_comment_depth += 1;
            sanitized.push(' ');
            sanitized.push(' ');
            index += 2;
            continue;
        }

        if matches!(current, '"' | '\'' | '`') {
            state.string_delimiter = Some(current);
            state.escaped = false;
            sanitized.push(' ');
            index += 1;
            continue;
        }

        sanitized.push(current);
        index += 1;
    }

    sanitized
}

fn count_same_file_identifier_references(
    lines: &[String],
    identifier: &str,
    start_line: usize,
    end_line: usize,
) -> usize {
    if identifier.is_empty() {
        return 0;
    }

    lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            let line_number = index + 1;
            (line_number < start_line || line_number > end_line)
                && !line.trim_start().starts_with("import ")
                && !line.trim_start().starts_with("export {")
        })
        .map(|(_, line)| count_identifier_occurrences(line, identifier))
        .sum()
}

fn count_identifier_occurrences(line: &str, identifier: &str) -> usize {
    let mut count = 0;
    let mut search_start = 0;

    while let Some(relative_index) = line[search_start..].find(identifier) {
        let start = search_start + relative_index;
        let end = start + identifier.len();
        if is_identifier_boundary(line[..start].chars().next_back())
            && is_identifier_boundary(line[end..].chars().next())
            && !is_member_access(line, start)
            && !is_object_property_key(line, end)
            && !is_declaration_context(line, start)
            && !is_type_only_context(line, identifier, start, end)
        {
            count += 1;
        }
        search_start = end;
    }

    count
}

fn is_identifier_boundary(character: Option<char>) -> bool {
    match character {
        None => true,
        Some(value) => !matches!(value, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'),
    }
}

fn is_member_access(line: &str, start: usize) -> bool {
    previous_non_whitespace(line, start) == Some('.')
}

fn is_object_property_key(line: &str, end: usize) -> bool {
    next_non_whitespace(line, end) == Some(':')
}

fn is_declaration_context(line: &str, start: usize) -> bool {
    matches!(
        previous_token(line, start).as_deref(),
        Some(
            "function"
                | "class"
                | "const"
                | "let"
                | "var"
                | "type"
                | "interface"
                | "enum"
                | "import"
                | "export"
                | "typeof"
        )
    )
}

fn is_type_only_context(line: &str, identifier: &str, start: usize, end: usize) -> bool {
    let previous_significant = previous_non_whitespace(line, start);
    let next_significant = next_non_whitespace(line, end);

    if previous_significant == Some(':') {
        return true;
    }

    if previous_significant == Some('<') && !is_probable_jsx_tag_reference(line, identifier, start)
    {
        return true;
    }

    if matches!(previous_significant, Some('|' | '&')) {
        return true;
    }

    if matches!(next_significant, Some('|' | '&')) {
        return true;
    }

    matches!(
        previous_token(line, start).as_deref(),
        Some(
            "type"
                | "interface"
                | "extends"
                | "implements"
                | "as"
                | "satisfies"
                | "keyof"
                | "infer"
                | "readonly"
                | "is"
        )
    )
}

fn is_probable_jsx_tag_reference(line: &str, identifier: &str, start: usize) -> bool {
    if !identifier
        .chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
    {
        return false;
    }

    let Some(tag_start) = previous_non_whitespace_index(line, start) else {
        return false;
    };
    if line[tag_start..].chars().next() != Some('<') {
        return false;
    }

    match previous_non_whitespace(line, tag_start) {
        None => true,
        Some(character) if matches!(character, '(' | '{' | '[' | '=' | ',' | ':' | '?') => true,
        Some(_) => matches!(
            previous_token(line, tag_start).as_deref(),
            Some("return" | "yield")
        ),
    }
}

fn previous_non_whitespace(line: &str, start: usize) -> Option<char> {
    line[..start]
        .chars()
        .rev()
        .find(|character| !character.is_whitespace())
}

fn previous_non_whitespace_index(line: &str, start: usize) -> Option<usize> {
    line[..start]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, _)| index)
}

fn next_non_whitespace(line: &str, end: usize) -> Option<char> {
    line[end..]
        .chars()
        .find(|character| !character.is_whitespace())
}

fn previous_token(line: &str, start: usize) -> Option<String> {
    let prefix = &line[..start];
    let token_end = prefix
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())?;
    let token_start = prefix[..token_end]
        .char_indices()
        .rev()
        .find(|(_, character)| !matches!(character, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);

    Some(prefix[token_start..token_end].to_string())
}

fn is_same_file_reference_candidate(node: tree_sitter::Node<'_>) -> bool {
    if !matches!(
        node.kind(),
        "identifier" | "jsx_identifier" | "shorthand_property_identifier"
    ) {
        return false;
    }

    !is_declaration_name(node)
        && !is_type_only_reference(node)
        && !is_import_or_export_reference(node)
        && !is_member_property_reference(node)
        && !is_object_property_name(node)
}

fn is_declaration_name(node: tree_sitter::Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if !node_is_field(parent, node, "name") {
            current = parent.parent();
            continue;
        }

        return matches!(
            parent.kind(),
            "function_declaration"
                | "function_definition"
                | "generator_function_declaration"
                | "generator_function"
                | "method_definition"
                | "method_declaration"
                | "function_signature"
                | "class_declaration"
                | "class_definition"
                | "interface_declaration"
                | "type_alias_declaration"
                | "enum_declaration"
                | "variable_declarator"
                | "lexical_declaration"
                | "required_parameter"
                | "optional_parameter"
                | "formal_parameter"
        );
    }

    false
}

fn is_type_only_reference(node: tree_sitter::Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        let kind = parent.kind();
        if kind.contains("type")
            || matches!(
                kind,
                "interface_declaration"
                    | "type_alias_declaration"
                    | "type_annotation"
                    | "type_arguments"
                    | "type_parameters"
                    | "implements_clause"
                    | "extends_clause"
            )
        {
            return true;
        }
        current = parent.parent();
    }

    false
}

fn is_import_or_export_reference(node: tree_sitter::Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        let kind = parent.kind();
        if kind.contains("import") || kind.contains("export") {
            return true;
        }
        current = parent.parent();
    }

    false
}

fn is_member_property_reference(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    matches!(
        parent.kind(),
        "member_expression" | "optional_member_expression" | "field_expression"
    ) && node_is_field(parent, node, "property")
}

fn is_object_property_name(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    matches!(
        parent.kind(),
        "property_assignment" | "pair" | "pair_pattern" | "public_field_definition"
    ) && node_is_field(parent, node, "name")
}

fn node_is_field(
    parent: tree_sitter::Node<'_>,
    node: tree_sitter::Node<'_>,
    field_name: &str,
) -> bool {
    parent
        .child_by_field_name(field_name)
        .map(|field| field.id() == node.id())
        .unwrap_or(false)
}
