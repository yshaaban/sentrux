import { nowIso, runTool } from './common.mjs';
import { collectRepoIdentity } from './repo-identity.mjs';

export function buildSessionSnapshot(label, scanResult, checkResult, extra = {}) {
  return {
    label,
    recorded_at: nowIso(),
    scan_elapsed_ms: scanResult.elapsed_ms,
    check_elapsed_ms: checkResult.elapsed_ms,
    gate: checkResult.payload.gate ?? null,
    changed_files: checkResult.payload.changed_files ?? [],
    action_kinds: (checkResult.payload.actions ?? [])
      .map((action) => action.kind)
      .filter(Boolean),
    top_action_kind: checkResult.payload.actions?.[0]?.kind ?? null,
    check: checkResult.payload,
    ...extra,
  };
}

export async function recordSessionSnapshot(session, workRoot, label, options = {}) {
  const scanResult = await runTool(session, 'scan', { path: workRoot });
  const checkResult = await runTool(session, 'check', {});
  const snapshot = buildSessionSnapshot(label, scanResult, checkResult, options.extra ?? null);

  if (options.includeRepoIdentity) {
    snapshot.repo_identity = collectRepoIdentity(workRoot);
  }

  return snapshot;
}
