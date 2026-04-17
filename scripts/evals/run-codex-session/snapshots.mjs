import { runTool } from '../../lib/benchmark-harness.mjs';
import { collectRepoIdentity } from '../../lib/repo-identity.mjs';
import { nowIso } from './status.mjs';

function buildSnapshot(label, scanResult, checkResult, repoIdentity) {
  return {
    label,
    recorded_at: nowIso(),
    repo_identity: repoIdentity,
    scan_elapsed_ms: scanResult.elapsed_ms,
    check_elapsed_ms: checkResult.elapsed_ms,
    gate: checkResult.payload.gate ?? null,
    changed_files: checkResult.payload.changed_files ?? [],
    action_kinds: (checkResult.payload.actions ?? []).map((action) => action.kind).filter(Boolean),
    top_action_kind: checkResult.payload.actions?.[0]?.kind ?? null,
    check: checkResult.payload,
  };
}

export async function recordSnapshot(session, workRoot, label) {
  const scanResult = await runTool(session, 'scan', { path: workRoot });
  const checkResult = await runTool(session, 'check', {});
  const repoIdentity = collectRepoIdentity(workRoot);

  return buildSnapshot(label, scanResult, checkResult, repoIdentity);
}

export function shouldCaptureSnapshot(previousSnapshot, nextIdentity) {
  if (!previousSnapshot) {
    return true;
  }

  const previousIdentity = previousSnapshot.repo_identity ?? null;
  if (!previousIdentity) {
    return true;
  }

  return (
    previousIdentity.dirty_paths_fingerprint !== nextIdentity.dirty_paths_fingerprint ||
    previousIdentity.dirty_paths_count !== nextIdentity.dirty_paths_count ||
    previousIdentity.tree_fingerprint !== nextIdentity.tree_fingerprint
  );
}
