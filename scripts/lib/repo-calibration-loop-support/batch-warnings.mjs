function resolveBatchManifestKeys(idKey) {
  if (idKey === 'task_id') {
    return {
      manifestEntriesKey: 'tasks',
      manifestIdKey: 'task_id',
    };
  }

  return {
    manifestEntriesKey: 'replays',
    manifestIdKey: 'replay_id',
  };
}

function normalizeExpectedSignalKinds(expectedSignalKinds) {
  return [...new Set((expectedSignalKinds ?? []).filter(Boolean))].sort();
}

export function buildBatchExpectationWarnings(
  batchManifest,
  batchResult,
  idKey,
  laneLabel,
) {
  if (!batchManifest || !batchResult) {
    return [];
  }

  const { manifestEntriesKey, manifestIdKey } = resolveBatchManifestKeys(idKey);
  const manifestEntries = Array.isArray(batchManifest[manifestEntriesKey])
    ? batchManifest[manifestEntriesKey]
    : [];
  const manifestById = new Map(
    manifestEntries.map((entry) => [entry[manifestIdKey], entry]),
  );
  const warnings = [];

  for (const result of batchResult.results ?? []) {
    const resultId = result[idKey];
    if (!resultId) {
      warnings.push(`${laneLabel} batch result missing ${idKey}`);
      continue;
    }

    const manifestEntry = manifestById.get(resultId);
    if (!manifestEntry) {
      warnings.push(`${laneLabel} batch result ${resultId} is not present in the manifest`);
      continue;
    }

    const expectedFromManifest = normalizeExpectedSignalKinds(
      manifestEntry.expected_signal_kinds ?? batchManifest.expected_signal_kinds,
    );
    const expectedFromResult = normalizeExpectedSignalKinds(result.expected_signal_kinds);

    if (expectedFromManifest.join('|') !== expectedFromResult.join('|')) {
      warnings.push(
        `${laneLabel} batch result ${resultId} expected_signal_kinds drifted: manifest=[${expectedFromManifest.join(', ')}] result=[${expectedFromResult.join(', ')}]`,
      );
    }
  }

  return warnings;
}

export function buildBatchFailureWarnings(batchResult, laneLabel) {
  return (batchResult?.failures ?? []).map((failure) => {
    const failureId = failure.task_id ?? failure.replay_id ?? failure.task_label ?? 'unknown';
    return `${laneLabel} lane failure for ${failureId}: ${failure.error_message ?? failure.status ?? 'unknown error'}`;
  });
}
