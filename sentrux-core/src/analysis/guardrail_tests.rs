use ignore::WalkBuilder;
use std::path::Path;

pub(crate) fn walk_guardrail_test_sources(root: &Path) -> Vec<(String, String)> {
    let mut sources = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.into_path();
            if !path.is_file() {
                return None;
            }
            let relative_path = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if !is_guardrail_test_path(&relative_path) {
                return None;
            }
            let contents = std::fs::read_to_string(&path).ok()?;
            Some((relative_path, contents))
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

pub(crate) fn is_guardrail_test_path(path: &str) -> bool {
    path.ends_with(".architecture.test.ts")
        || path.ends_with(".architecture.test.tsx")
        || path.ends_with(".architecture.spec.ts")
        || path.ends_with(".architecture.spec.tsx")
}
