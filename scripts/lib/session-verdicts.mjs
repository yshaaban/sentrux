import { asArray, safeRatio } from './signal-summary-utils.mjs';

const REVIEWER_CONFIDENCE_LEVELS = new Set(['low', 'medium', 'high']);

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function isBooleanValue(value) {
  return value === true || value === false;
}

function normalizeOptionalString(value) {
  if (!hasText(value)) {
    return null;
  }

  return value.trim();
}

function normalizeOptionalBoolean(value) {
  if (isBooleanValue(value)) {
    return value;
  }

  return null;
}

function normalizeOptionalInteger(value) {
  if (!Number.isInteger(value) || value < 0) {
    return null;
  }

  return value;
}

function normalizeReviewerConfidence(value) {
  const normalizedValue = normalizeOptionalString(value);
  if (!normalizedValue) {
    return null;
  }

  const normalizedConfidence = normalizedValue.toLowerCase();
  if (!REVIEWER_CONFIDENCE_LEVELS.has(normalizedConfidence)) {
    return null;
  }

  return normalizedConfidence;
}

function roundMetric(value) {
  if (!Number.isFinite(value)) {
    return null;
  }

  return Number(value.toFixed(3));
}

function averageNumbers(values) {
  if (values.length === 0) {
    return null;
  }

  return roundMetric(values.reduce(function sum(total, value) {
    return total + value;
  }, 0) / values.length);
}

function boundedPenalty(value, maxValue) {
  if (!Number.isFinite(value) || value <= 0 || maxValue <= 0) {
    return 0;
  }

  return Math.min(1, Number((value / maxValue).toFixed(3)));
}

function countBooleanField(verdicts, fieldName) {
  let sampleCount = 0;
  let trueCount = 0;

  for (const verdict of verdicts) {
    const value = verdict[fieldName];
    if (!isBooleanValue(value)) {
      continue;
    }

    sampleCount += 1;
    if (value) {
      trueCount += 1;
    }
  }

  return {
    sampleCount,
    trueCount,
    rate: safeRatio(trueCount, sampleCount),
  };
}

function collectIntegerField(verdicts, fieldName) {
  return verdicts
    .map(function selectIntegerValue(verdict) {
      return verdict[fieldName];
    })
    .filter(function isIntegerValue(value) {
      return Number.isInteger(value) && value >= 0;
    });
}

function buildConfidenceCounts(verdicts) {
  const counts = {
    high: 0,
    medium: 0,
    low: 0,
  };

  for (const verdict of verdicts) {
    if (verdict.reviewer_confidence === 'high') {
      counts.high += 1;
      continue;
    }
    if (verdict.reviewer_confidence === 'medium') {
      counts.medium += 1;
      continue;
    }
    if (verdict.reviewer_confidence === 'low') {
      counts.low += 1;
    }
  }

  return counts;
}

function selectSourceArtifact(sessionVerdicts) {
  const sourceArtifact = normalizeOptionalString(sessionVerdicts.source_artifact);
  if (sourceArtifact) {
    return sourceArtifact;
  }

  return normalizeOptionalString(sessionVerdicts.source_report);
}

function selectSourceReport(sessionVerdicts) {
  const sourceReport = normalizeOptionalString(sessionVerdicts.source_report);
  if (sourceReport) {
    return sourceReport;
  }

  return normalizeOptionalString(sessionVerdicts.source_artifact);
}

function buildInterventionNetValueScore({
  topActionFollowRate,
  topActionHelpRate,
  taskSuccessRate,
  patchExpansionRate,
  interventionCostChecksMean,
}) {
  const positiveSignals = [
    topActionFollowRate,
    topActionHelpRate,
    taskSuccessRate,
  ].filter(Number.isFinite);
  const penaltySignals = [];

  if (Number.isFinite(patchExpansionRate)) {
    penaltySignals.push(patchExpansionRate);
  }
  if (Number.isFinite(interventionCostChecksMean)) {
    const extraChecks = Math.max(interventionCostChecksMean - 1, 0);
    penaltySignals.push(boundedPenalty(extraChecks, 3));
  }

  const positiveMean = averageNumbers(positiveSignals);
  if (positiveMean === null) {
    return null;
  }

  const penaltyMean = averageNumbers(penaltySignals) ?? 0;
  return roundMetric(Math.max(-1, Math.min(1, positiveMean - penaltyMean)));
}

function buildSessionVerdictMap(sessionVerdicts) {
  const byLaneAndSessionId = new Map();
  const bySessionId = new Map();

  for (const verdict of asArray(sessionVerdicts?.verdicts)) {
    if (!verdict?.session_id) {
      continue;
    }

    if (verdict.lane) {
      byLaneAndSessionId.set(`${verdict.lane}:${verdict.session_id}`, verdict);
    }

    const existingVerdicts = bySessionId.get(verdict.session_id) ?? [];
    existingVerdicts.push(verdict);
    bySessionId.set(verdict.session_id, existingVerdicts);
  }

  return {
    byLaneAndSessionId,
    bySessionId,
  };
}

function selectMatchingVerdict(entry, verdictMap) {
  const laneKey = `${entry.lane}:${entry.session_id}`;
  if (verdictMap.byLaneAndSessionId.has(laneKey)) {
    return verdictMap.byLaneAndSessionId.get(laneKey);
  }

  const sessionVerdicts = verdictMap.bySessionId.get(entry.session_id) ?? [];
  if (sessionVerdicts.length === 1) {
    return sessionVerdicts[0];
  }

  return null;
}

export function normalizeSessionVerdict(verdict) {
  if (!verdict || typeof verdict !== 'object') {
    return null;
  }

  const sessionId = normalizeOptionalString(verdict.session_id);
  if (!sessionId) {
    return null;
  }

  return {
    session_id: sessionId,
    repo_label: normalizeOptionalString(verdict.repo_label),
    batch_name: normalizeOptionalString(verdict.batch_name),
    lane: verdict.lane === 'live' || verdict.lane === 'replay' ? verdict.lane : null,
    session_label: normalizeOptionalString(verdict.session_label),
    task_id: normalizeOptionalString(verdict.task_id),
    experiment_arm: normalizeOptionalString(verdict.experiment_arm),
    top_action_followed: normalizeOptionalBoolean(verdict.top_action_followed),
    top_action_helped: normalizeOptionalBoolean(verdict.top_action_helped),
    task_completed_successfully: normalizeOptionalBoolean(
      verdict.task_completed_successfully,
    ),
    patch_expanded_unnecessarily: normalizeOptionalBoolean(
      verdict.patch_expanded_unnecessarily,
    ),
    intervention_cost_checks: normalizeOptionalInteger(verdict.intervention_cost_checks),
    intervention_cost_notes: normalizeOptionalString(verdict.intervention_cost_notes),
    reviewer_confidence: normalizeReviewerConfidence(verdict.reviewer_confidence),
    notes: normalizeOptionalString(verdict.notes),
  };
}

export function normalizeSessionVerdictReport(sessionVerdicts) {
  if (!sessionVerdicts || typeof sessionVerdicts !== 'object') {
    return null;
  }

  return {
    ...sessionVerdicts,
    repo: normalizeOptionalString(sessionVerdicts.repo),
    repo_label: normalizeOptionalString(sessionVerdicts.repo_label),
    source_artifact: selectSourceArtifact(sessionVerdicts),
    source_report: selectSourceReport(sessionVerdicts),
    source_feedback: normalizeOptionalString(sessionVerdicts.source_feedback),
    verdicts: asArray(sessionVerdicts.verdicts)
      .map(normalizeSessionVerdict)
      .filter(Boolean),
  };
}

export function applySessionVerdicts(entries, sessionVerdicts) {
  if (!sessionVerdicts) {
    return entries;
  }

  const verdictMap = buildSessionVerdictMap(normalizeSessionVerdictReport(sessionVerdicts));

  return entries.map(function attachSessionVerdict(entry) {
    const sessionVerdict = selectMatchingVerdict(entry, verdictMap);
    if (!sessionVerdict) {
      return entry;
    }

    return {
      ...entry,
      session_verdict: sessionVerdict,
    };
  });
}

export function buildSessionVerdictSummary(entries) {
  const verdicts = entries
    .map(function selectSessionVerdict(entry) {
      return entry.session_verdict ?? null;
    })
    .filter(Boolean);
  const topActionFollow = countBooleanField(verdicts, 'top_action_followed');
  const topActionHelp = countBooleanField(verdicts, 'top_action_helped');
  const taskSuccess = countBooleanField(verdicts, 'task_completed_successfully');
  const patchExpansion = countBooleanField(verdicts, 'patch_expanded_unnecessarily');
  const interventionCostChecks = collectIntegerField(verdicts, 'intervention_cost_checks');
  const interventionCostChecksTotal = interventionCostChecks.reduce(function sum(total, value) {
    return total + value;
  }, 0);
  const interventionCostChecksMean = averageNumbers(interventionCostChecks);
  const reviewerConfidenceCounts = buildConfidenceCounts(verdicts);

  return {
    session_verdict_count: verdicts.length,
    top_action_follow_sample_count: topActionFollow.sampleCount,
    top_action_followed_count: topActionFollow.trueCount,
    top_action_follow_rate: topActionFollow.rate,
    top_action_help_sample_count: topActionHelp.sampleCount,
    top_action_helped_count: topActionHelp.trueCount,
    top_action_help_rate: topActionHelp.rate,
    task_success_sample_count: taskSuccess.sampleCount,
    task_completed_successfully_count: taskSuccess.trueCount,
    task_success_rate: taskSuccess.rate,
    patch_expansion_sample_count: patchExpansion.sampleCount,
    patch_expanded_unnecessarily_count: patchExpansion.trueCount,
    patch_expansion_rate: patchExpansion.rate,
    intervention_cost_sample_count: interventionCostChecks.length,
    intervention_cost_checks_total: interventionCostChecksTotal,
    intervention_cost_checks_mean: interventionCostChecksMean,
    reviewer_confidence_counts: reviewerConfidenceCounts,
    intervention_net_value_score: buildInterventionNetValueScore({
      topActionFollowRate: topActionFollow.rate,
      topActionHelpRate: topActionHelp.rate,
      taskSuccessRate: taskSuccess.rate,
      patchExpansionRate: patchExpansion.rate,
      interventionCostChecksMean,
    }),
  };
}
