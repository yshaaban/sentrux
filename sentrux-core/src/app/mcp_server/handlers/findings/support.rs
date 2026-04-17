use super::*;

pub(super) fn refresh_changed_scope(
    state: &mut McpState,
    root: &Path,
) -> Result<BTreeSet<String>, String> {
    let session_v2 = current_session_v2_baseline(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let changed_files = context.changed_files.clone();
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root.to_path_buf(),
            context.bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }
    Ok(changed_files)
}
