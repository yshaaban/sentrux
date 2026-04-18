import { normalizeExperimentArm } from '../experiment-arms.mjs';

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

function buildExperimentArmWarnings(batchManifest, manifestEntries, laneLabel) {
  const warnings = [];
  const normalizedArms = manifestEntries
    .map(function selectExperimentArm(entry) {
      return normalizeExperimentArm(entry.experiment_arm);
    })
    .filter(Boolean);

  if (normalizedArms.length === 0) {
    return warnings;
  }

  if (normalizedArms.length !== manifestEntries.length) {
    warnings.push(`${laneLabel} batch manifest mixes experiment-arm coverage with missing experiment_arm fields`);
  }

  const armCounts = new Map();
  for (const arm of normalizedArms) {
    armCounts.set(arm, (armCounts.get(arm) ?? 0) + 1);
  }

  if (armCounts.size < 2) {
    warnings.push(`${laneLabel} batch manifest has only one experiment arm; cross-arm comparison will be weak`);
    return warnings;
  }

  const counts = [...armCounts.values()];
  const maxCount = Math.max(...counts);
  const minCount = Math.min(...counts);

  if (maxCount - minCount > 1) {
    const countSummary = [...armCounts.entries()]
      .map(function formatArmCount([arm, count]) {
        return `${arm}:${count}`;
      })
      .join(', ');
    warnings.push(`${laneLabel} batch manifest experiment arms are imbalanced: [${countSummary}]`);
  }

  return warnings;
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

  return [...warnings, ...buildExperimentArmWarnings(batchManifest, manifestEntries, laneLabel)];
}

export function buildBatchFailureWarnings(batchResult, laneLabel) {
  return (batchResult?.failures ?? []).map((failure) => {
    const failureId = failure.task_id ?? failure.replay_id ?? failure.task_label ?? 'unknown';
    return `${laneLabel} lane failure for ${failureId}: ${failure.error_message ?? failure.status ?? 'unknown error'}`;
  });
}
