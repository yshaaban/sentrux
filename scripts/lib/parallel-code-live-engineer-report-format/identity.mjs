export function isHeadCloneAnalysis(metadata) {
  return metadata?.analysis_mode === 'head_clone';
}

export function snapshotMatchesMetadata(snapshot, metadata) {
  const snapshotMetadata = snapshot?.generated_from?.metadata;
  if (!snapshotMetadata) {
    return false;
  }

  return JSON.stringify(snapshotMetadata) === JSON.stringify(metadata);
}

export function assertHeadCommitFresh(metadata, liveIdentity, allowStale) {
  const expectedCommit = metadata?.source_tree_identity?.commit ?? null;
  const actualCommit = liveIdentity?.commit ?? null;

  if (expectedCommit === actualCommit || allowStale) {
    return;
  }

  throw new Error(
    `parallel-code HEAD commit changed: expected ${expectedCommit ?? 'unknown'}, got ${actualCommit ?? 'unknown'}`,
  );
}
