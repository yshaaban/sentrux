//! Shared test helpers for analysis test modules.
//!
//! Eliminates duplication of `make_file()` across graph/tests.rs,
//! graph/tests2.rs, and resolver/oxc.rs.

use crate::core::types::{FileNode, StructuralAnalysis};

/// Build a minimal `FileNode` for testing graph/resolver logic.
pub fn make_file(name: &str, path: &str, lang: &str, sa: Option<StructuralAnalysis>) -> FileNode {
    FileNode {
        name: name.to_string(),
        path: path.to_string(),
        lang: lang.to_string(),
        is_dir: false,
        lines: 10,
        logic: 8,
        comments: 1,
        blanks: 1,
        funcs: 0,
        mtime: 0.0,
        gs: String::new(),
        sa,
        children: None,
    }
}
