import {
  SIGNAL_PRIMARY_TARGET_POLICY,
  SIGNAL_PROMOTION_POLICY,
} from './signal-calibration-policy.mjs';
import { asArray, ensureMapEntry, safeRatio } from './signal-summary-utils.mjs';

const TOP_K_REVIEW_BUCKETS = Object.freeze([
  { fieldPrefix: 'top_1', limit: 1 },
  { fieldPrefix: 'top_3', limit: 3 },
  { fieldPrefix: 'top_10', limit: 10 },
]);

function normalizeReviewCategory(category) {
  switch (category) {
    case 'true_positive':
    case 'useful':
      return 'true_positive';
    case 'acceptable_warning':
    case 'useful_watchpoint':
      return 'acceptable_warning';
    case 'false_positive':
    case 'incorrect':
      return 'false_positive';
    case 'inconclusive':
    case 'low_value':
    case 'real_but_overstated':
      return 'inconclusive';
    default:
      return 'inconclusive';
  }
}

function isHelpfulReviewCategory(normalizedCategory) {
  return normalizedCategory === 'true_positive' || normalizedCategory === 'acceptable_warning';
}

function createEmptySignalEntry(signalKind, overrides = {}) {
  return {
    signal_kind: signalKind,
    signal_family: 'unknown',
    promotion_status: 'unspecified',
    blocking_intent: 'unspecified',
    seeded_total: 0,
    seeded_detected: 0,
    primary_lane: null,
    seeded_primary_supported: 0,
    seeded_primary_detected: 0,
    seeded_check_supported: 0,
    seeded_check_detected: 0,
    seeded_check_rules_supported: 0,
    seeded_check_rules_detected: 0,
    session_top_actions: 0,
    session_followups: 0,
    session_cleared: 0,
    session_regressions: 0,
    sessions_cleared: 0,
    session_clean: 0,
    session_total_checks_to_clear: 0,
    session_trial_count: 0,
    live_session_trial_count: 0,
    replay_session_trial_count: 0,
    session_expected_presentations: 0,
    session_expected_top_actions: 0,
    session_expectation_misses: 0,
    provisional_reviewed_total: 0,
    provisional_true_positive: 0,
    provisional_acceptable_warning: 0,
    provisional_false_positive: 0,
    provisional_inconclusive: 0,
    review_top_1_total: 0,
    review_top_1_actionable: 0,
    review_top_3_total: 0,
    review_top_3_actionable: 0,
    review_top_10_total: 0,
    review_top_10_actionable: 0,
    ranking_preference_total: 0,
    ranking_preference_satisfied: 0,
    ranking_preference_violated: 0,
    ranking_preference_unresolved: 0,
    provisional_review_top_1_total: 0,
    provisional_review_top_1_actionable: 0,
    provisional_review_top_3_total: 0,
    provisional_review_top_3_actionable: 0,
    provisional_review_top_10_total: 0,
    provisional_review_top_10_actionable: 0,
    provisional_ranking_preference_total: 0,
    provisional_ranking_preference_satisfied: 0,
    provisional_ranking_preference_violated: 0,
    provisional_ranking_preference_unresolved: 0,
    ...overrides,
  };
}

function ensureSignalEntry(signalMap, signalKind) {
  return ensureMapEntry(signalMap, signalKind, createEmptySignalEntry);
}

function buildReviewMetrics(reviewedTotal, truePositive, acceptableWarning, falsePositive, inconclusive) {
  const reviewedHelpfulCount = truePositive + acceptableWarning;
  const reviewNoiseCount = falsePositive + inconclusive;

  return {
    reviewed_helpful_count: reviewedHelpfulCount,
    reviewed_precision: safeRatio(reviewedHelpfulCount, reviewedTotal),
    true_positive_precision: safeRatio(truePositive, reviewedTotal),
    review_noise_count: reviewNoiseCount,
    review_noise_rate: safeRatio(reviewNoiseCount, reviewedTotal),
    false_positive_rate: safeRatio(falsePositive, reviewedTotal),
    inconclusive_rate: safeRatio(inconclusive, reviewedTotal),
  };
}

function recordTopKReviewCounts(entry, normalizedCategory, reviewIndex, prefix) {
  const actionable = isHelpfulReviewCategory(normalizedCategory);

  for (const bucket of TOP_K_REVIEW_BUCKETS) {
    if (reviewIndex >= bucket.limit) {
      continue;
    }

    entry[`${prefix}review_${bucket.fieldPrefix}_total`] += 1;
    if (actionable) {
      entry[`${prefix}review_${bucket.fieldPrefix}_actionable`] += 1;
    }
  }
}

function recordRankingPreference(entry, preferredIndex, verdictIndex, prefix) {
  entry[`${prefix}ranking_preference_total`] += 1;
  if (preferredIndex === null) {
    entry[`${prefix}ranking_preference_unresolved`] += 1;
    return;
  }
  if (verdictIndex < preferredIndex) {
    entry[`${prefix}ranking_preference_satisfied`] += 1;
    return;
  }

  entry[`${prefix}ranking_preference_violated`] += 1;
}

function buildSessionMetrics(
  sessionTopActions,
  sessionFollowups,
  sessionCleared,
  sessionRegressions,
  sessionsCleared,
  sessionClean,
  sessionTotalChecksToClear,
) {
  const topActionClearRate = safeRatio(sessionsCleared, sessionTopActions);

  return {
    session_resolution_rate: safeRatio(sessionCleared, sessionFollowups),
    session_clear_rate: topActionClearRate,
    top_action_clear_rate: topActionClearRate,
    followup_regression_rate: safeRatio(sessionRegressions, sessionFollowups),
    session_clean_rate: safeRatio(sessionClean, sessionTopActions),
    average_checks_to_clear: safeRatio(sessionTotalChecksToClear, sessionsCleared),
  };
}

function buildCoverageFlags(entry, reviewedTotal, remediationTotal, sessionTopActions) {
  const hasSeededEvidence = entry.seeded_total > 0;
  const hasReviewEvidence = reviewedTotal > 0;
  const hasProvisionalReviewEvidence = (entry.provisional_reviewed_total ?? 0) > 0;
  const hasRemediationEvidence = remediationTotal > 0;
  const hasSessionActionEvidence = sessionTopActions > 0;
  const hasSessionTrialEvidence = (entry.session_trial_count ?? 0) > 0;
  const hasSessionEvidence = hasSessionActionEvidence || hasSessionTrialEvidence;

  return {
    has_seeded_evidence: hasSeededEvidence,
    has_review_evidence: hasReviewEvidence,
    has_provisional_review_evidence: hasProvisionalReviewEvidence,
    has_remediation_evidence: hasRemediationEvidence,
    has_session_evidence: hasSessionEvidence,
    has_session_action_evidence: hasSessionActionEvidence,
    has_session_trial_evidence: hasSessionTrialEvidence,
    promotion_evidence_complete:
      hasSeededEvidence && hasReviewEvidence && hasRemediationEvidence && hasSessionEvidence,
  };
}

function buildSeededEntries(defectReport) {
  if (!defectReport) {
    return new Map();
  }

  const defectById = new Map((defectReport.defects ?? []).map((defect) => [defect.id, defect]));
  const signalMap = new Map();

  for (const result of defectReport.results ?? []) {
    const defect = defectById.get(result.defect_id);
    if (!defect?.signal_kind) {
      continue;
    }

    const key = defect.signal_kind;
    if (!signalMap.has(key)) {
      signalMap.set(
        key,
        createEmptySignalEntry(defect.signal_kind, {
          signal_family: defect.signal_family ?? 'unknown',
          promotion_status: defect.promotion_status ?? 'unspecified',
          blocking_intent: defect.blocking_intent ?? 'unspecified',
        }),
      );
    }

    const entry = signalMap.get(key);
    let primaryLane = null;
    if (result.check?.supported) {
      primaryLane = 'check';
    } else if (result.check_rules?.supported) {
      primaryLane = 'check_rules';
    }
    entry.seeded_total += 1;
    if (result.detected) {
      entry.seeded_detected += 1;
    }
    if (primaryLane) {
      entry.primary_lane =
        entry.primary_lane && entry.primary_lane !== primaryLane ? 'mixed' : primaryLane;
      entry.seeded_primary_supported += 1;
      if (
        (primaryLane === 'check' && result.check?.matched) ||
        (primaryLane === 'check_rules' && result.check_rules?.matched)
      ) {
        entry.seeded_primary_detected += 1;
      }
    }
    if (result.check?.supported) {
      entry.seeded_check_supported += 1;
    }
    if (result.check?.matched) {
      entry.seeded_check_detected += 1;
    }
    if (result.check_rules?.supported) {
      entry.seeded_check_rules_supported += 1;
    }
    if (result.check_rules?.matched) {
      entry.seeded_check_rules_detected += 1;
    }
  }

  return signalMap;
}

function applyReviewVerdicts(signalMap, reviewVerdicts) {
  const provisionalPrefix = reviewVerdicts?.provisional ? 'provisional_' : '';
  const verdicts = asArray(reviewVerdicts?.verdicts);
  const scopeToFirstIndex = new Map();

  verdicts.forEach(function recordScope(verdict, index) {
    if (typeof verdict.scope !== 'string' || scopeToFirstIndex.has(verdict.scope)) {
      return;
    }
    scopeToFirstIndex.set(verdict.scope, index);
  });

  verdicts.forEach(function applyVerdict(verdict, index) {
    const signalKind = verdict.kind;
    if (!signalKind) {
      return;
    }

    const entry = ensureSignalEntry(signalMap, signalKind);
    entry[`${provisionalPrefix}reviewed_total`] =
      (entry[`${provisionalPrefix}reviewed_total`] ?? 0) + 1;
    const normalizedCategory = normalizeReviewCategory(verdict.category);
    entry[`${provisionalPrefix}${normalizedCategory}`] =
      (entry[`${provisionalPrefix}${normalizedCategory}`] ?? 0) + 1;
    recordTopKReviewCounts(entry, normalizedCategory, index, provisionalPrefix);

    for (const preferredScope of asArray(verdict.preferred_over)) {
      if (typeof preferredScope !== 'string' || preferredScope.length === 0) {
        continue;
      }
      const preferredIndex = scopeToFirstIndex.has(preferredScope)
        ? scopeToFirstIndex.get(preferredScope)
        : null;
      recordRankingPreference(entry, preferredIndex, index, provisionalPrefix);
    }
  });
}

function applyRemediationResults(signalMap, remediationReport) {
  for (const result of remediationReport?.results ?? []) {
    const signalKind = result.signal_kind;
    if (!signalKind) {
      continue;
    }

    const entry = ensureSignalEntry(signalMap, signalKind);
    entry.remediation_total = (entry.remediation_total ?? 0) + 1;
    if (result.fixed) {
      entry.remediation_success = (entry.remediation_success ?? 0) + 1;
    }
    if (result.regression_free === false) {
      entry.remediation_regressions = (entry.remediation_regressions ?? 0) + 1;
    }
  }
}

function applySessionTelemetry(signalMap, sessionTelemetry) {
  for (const signal of asArray(sessionTelemetry?.signals)) {
    const signalKind = signal.signal_kind;
    if (!signalKind) {
      continue;
    }

    const entry = ensureSignalEntry(signalMap, signalKind);
    entry.session_top_actions += signal.top_action_presented ?? 0;
    entry.session_followups += signal.followup_checks ?? 0;
    entry.session_cleared += signal.target_cleared ?? 0;
    entry.session_regressions += signal.followup_regressions ?? 0;
    entry.sessions_cleared += signal.sessions_cleared ?? 0;
    entry.session_clean += signal.sessions_clean ?? 0;
    entry.session_total_checks_to_clear += signal.total_checks_to_clear ?? 0;
  }
}

function applyBatchSessionTrials(signalMap, results, lane) {
  for (const result of asArray(results)) {
    const expectedSignalKinds = asArray(result.expected_signal_kinds);
    if (expectedSignalKinds.length === 0) {
      continue;
    }

    const initialActionKinds = new Set(asArray(result.outcome?.initial_action_kinds));
    const initialTopActionKind = result.outcome?.initial_top_action_kind ?? null;

    for (const signalKind of expectedSignalKinds) {
      if (!signalKind) {
        continue;
      }

      const entry = ensureSignalEntry(signalMap, signalKind);
      entry.session_trial_count += 1;
      entry[`${lane}_session_trial_count`] += 1;

      if (initialActionKinds.has(signalKind)) {
        entry.session_expected_presentations += 1;
      } else {
        entry.session_expectation_misses += 1;
      }

      if (initialTopActionKind === signalKind) {
        entry.session_expected_top_actions += 1;
      }
    }
  }
}

function countExpectedSignalTrials(results) {
  return asArray(results).reduce(
    (total, result) => total + asArray(result.expected_signal_kinds).filter(Boolean).length,
    0,
  );
}

function inferScorecardRepoLabel({
  repoLabel = null,
  defectReport = null,
  reviewVerdicts = null,
  remediationReport = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
}) {
  return (
    repoLabel ??
    defectReport?.repo_label ??
    reviewVerdicts?.repo ??
    remediationReport?.repo_label ??
    sessionTelemetry?.repo_label ??
    sessionTelemetry?.repo_root ??
    codexBatch?.repo_label ??
    codexBatch?.repo_root ??
    replayBatch?.repo_label ??
    replayBatch?.repo_root ??
    null
  );
}

function inferLatencyMs(benchmark) {
  return (
    benchmark?.benchmark?.warm_patch_safety?.check?.elapsed_ms ??
    benchmark?.benchmark?.warm_cached?.check?.elapsed_ms ??
    benchmark?.benchmark?.warm_cached?.gate?.elapsed_ms ??
    null
  );
}

function buildPromotionRecommendation(entry) {
  const seededRecall = safeRatio(entry.seeded_detected, entry.seeded_total);
  const reviewedTotal = entry.reviewed_total ?? 0;
  const falsePositives = entry.false_positive ?? 0;
  const inconclusive = entry.inconclusive ?? 0;
  const reviewedPrecision = safeRatio(
    (entry.true_positive ?? 0) + (entry.acceptable_warning ?? 0),
    reviewedTotal,
  );
  const reviewNoiseRate = safeRatio(falsePositives + inconclusive, reviewedTotal);
  const remediationSuccess = safeRatio(
    entry.remediation_success ?? 0,
    entry.remediation_total ?? 0,
  );
  const top1ReviewedTotal = entry.review_top_1_total ?? 0;
  const top1ActionablePrecision = safeRatio(
    entry.review_top_1_actionable ?? 0,
    top1ReviewedTotal,
  );
  const top3ReviewedTotal = entry.review_top_3_total ?? 0;
  const top3ActionablePrecision = safeRatio(
    entry.review_top_3_actionable ?? 0,
    top3ReviewedTotal,
  );
  const rankingPreferenceTotal = entry.ranking_preference_total ?? 0;
  const rankingPreferenceSatisfactionRate = safeRatio(
    entry.ranking_preference_satisfied ?? 0,
    rankingPreferenceTotal,
  );
  const topActionClearRate = safeRatio(
    entry.sessions_cleared ?? 0,
    entry.session_top_actions ?? 0,
  );
  const followupRegressionRate = safeRatio(
    entry.session_regressions ?? 0,
    entry.session_followups ?? 0,
  );
  const sessionCleanRate = safeRatio(entry.session_clean ?? 0, entry.session_top_actions ?? 0);

  if (reviewedTotal > 0 && falsePositives > 0) {
    return 'degrade_or_quarantine';
  }
  if (
    reviewedTotal > 0 &&
    reviewNoiseRate !== null &&
    reviewNoiseRate > SIGNAL_PROMOTION_POLICY.reviewNoiseRateMax
  ) {
    return 'needs_review';
  }
  if (
    seededRecall !== null &&
    seededRecall < SIGNAL_PROMOTION_POLICY.seededRecallMin
  ) {
    return 'improve_detection';
  }
  if (
    reviewedPrecision !== null &&
    reviewedPrecision < SIGNAL_PROMOTION_POLICY.reviewedPrecisionMin
  ) {
    return 'reduce_noise';
  }
  if (
    top1ReviewedTotal >= SIGNAL_PRIMARY_TARGET_POLICY.top1MinReviewedSamples &&
    top1ActionablePrecision !== null &&
    top1ActionablePrecision < SIGNAL_PRIMARY_TARGET_POLICY.top1ActionablePrecisionMin
  ) {
    return 'reduce_noise';
  }
  if (
    top3ReviewedTotal >= SIGNAL_PRIMARY_TARGET_POLICY.top3MinReviewedSamples &&
    top3ActionablePrecision !== null &&
    top3ActionablePrecision < SIGNAL_PRIMARY_TARGET_POLICY.top3ActionablePrecisionMin
  ) {
    return 'reduce_noise';
  }
  if (
    rankingPreferenceTotal >=
      SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceMinComparisons &&
    rankingPreferenceSatisfactionRate !== null &&
    rankingPreferenceSatisfactionRate <
      SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceSatisfactionMin
  ) {
    return 'reduce_noise';
  }
  if (
    remediationSuccess !== null &&
    remediationSuccess < SIGNAL_PROMOTION_POLICY.remediationSuccessMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    topActionClearRate !== null &&
    topActionClearRate < SIGNAL_PROMOTION_POLICY.topActionClearRateMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    sessionCleanRate !== null &&
    sessionCleanRate < SIGNAL_PROMOTION_POLICY.sessionCleanRateMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    followupRegressionRate !== null &&
    followupRegressionRate > SIGNAL_PROMOTION_POLICY.followupRegressionRateMax
  ) {
    return 'improve_fix_guidance';
  }
  return `keep_${entry.promotion_status ?? 'unspecified'}`;
}

function buildScorecardContext({
  repoLabel = null,
  defectReport = null,
  reviewVerdicts = null,
  remediationReport = null,
  benchmark = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
}) {
  const signalMap = buildSeededEntries(defectReport);
  applyReviewVerdicts(signalMap, reviewVerdicts);
  applyRemediationResults(signalMap, remediationReport);
  applySessionTelemetry(signalMap, sessionTelemetry);
  applyBatchSessionTrials(signalMap, codexBatch?.results, 'live');
  applyBatchSessionTrials(signalMap, replayBatch?.results, 'replay');

  const latencyMs = inferLatencyMs(benchmark);
  const repoLabelValue = inferScorecardRepoLabel({
    repoLabel,
    defectReport,
    reviewVerdicts,
    remediationReport,
    sessionTelemetry,
    codexBatch,
    replayBatch,
  });
  const liveSessionTrialCount = countExpectedSignalTrials(codexBatch?.results);
  const replaySessionTrialCount = countExpectedSignalTrials(replayBatch?.results);
  const totalSessionTrialCount = liveSessionTrialCount + replaySessionTrialCount;
  const sessionCount =
    sessionTelemetry?.summary?.session_count ?? sessionTelemetry?.sessions?.length ?? 0;

  return {
    signalMap,
    latencyMs,
    repoLabelValue,
    totalSessionTrialCount,
    sessionCount,
  };
}

function buildSignalCounts(entry) {
  return {
    review: {
      reviewedTotal: entry.reviewed_total ?? 0,
      truePositive: entry.true_positive ?? 0,
      acceptableWarning: entry.acceptable_warning ?? 0,
      falsePositive: entry.false_positive ?? 0,
      inconclusive: entry.inconclusive ?? 0,
    },
    provisionalReview: {
      reviewedTotal: entry.provisional_reviewed_total ?? 0,
      truePositive: entry.provisional_true_positive ?? 0,
      acceptableWarning: entry.provisional_acceptable_warning ?? 0,
      falsePositive: entry.provisional_false_positive ?? 0,
      inconclusive: entry.provisional_inconclusive ?? 0,
    },
    remediation: {
      remediationTotal: entry.remediation_total ?? 0,
      remediationSuccess: entry.remediation_success ?? 0,
      remediationRegressions: entry.remediation_regressions ?? 0,
    },
    session: {
      sessionTopActions: entry.session_top_actions ?? 0,
      sessionFollowups: entry.session_followups ?? 0,
      sessionCleared: entry.session_cleared ?? 0,
      sessionRegressions: entry.session_regressions ?? 0,
      sessionsCleared: entry.sessions_cleared ?? 0,
      sessionClean: entry.session_clean ?? 0,
      sessionTotalChecksToClear: entry.session_total_checks_to_clear ?? 0,
      sessionTrialCount: entry.session_trial_count ?? 0,
      liveSessionTrialCount: entry.live_session_trial_count ?? 0,
      replaySessionTrialCount: entry.replay_session_trial_count ?? 0,
      sessionExpectedPresentations: entry.session_expected_presentations ?? 0,
      sessionExpectedTopActions: entry.session_expected_top_actions ?? 0,
      sessionExpectationMisses: entry.session_expectation_misses ?? 0,
    },
  };
}

function buildSignalRecallFields(entry) {
  return {
    seeded_total: entry.seeded_total,
    seeded_detected: entry.seeded_detected,
    seeded_recall: safeRatio(entry.seeded_detected, entry.seeded_total),
    seeded_primary_supported: entry.seeded_primary_supported,
    seeded_primary_detected: entry.seeded_primary_detected,
    primary_recall: safeRatio(entry.seeded_primary_detected, entry.seeded_primary_supported),
    seeded_check_supported: entry.seeded_check_supported,
    seeded_check_detected: entry.seeded_check_detected,
    check_recall: safeRatio(entry.seeded_check_detected, entry.seeded_check_supported),
    seeded_check_rules_supported: entry.seeded_check_rules_supported,
    seeded_check_rules_detected: entry.seeded_check_rules_detected,
    check_rules_recall: safeRatio(
      entry.seeded_check_rules_detected,
      entry.seeded_check_rules_supported,
    ),
  };
}

function buildSignalCoverageFields(coverageFlags) {
  return {
    has_seeded_evidence: coverageFlags.has_seeded_evidence,
    has_review_evidence: coverageFlags.has_review_evidence,
    has_provisional_review_evidence: coverageFlags.has_provisional_review_evidence,
    has_remediation_evidence: coverageFlags.has_remediation_evidence,
    has_session_evidence: coverageFlags.has_session_evidence,
    has_session_action_evidence: coverageFlags.has_session_action_evidence,
    has_session_trial_evidence: coverageFlags.has_session_trial_evidence,
    promotion_evidence_complete: coverageFlags.promotion_evidence_complete,
  };
}

function buildSignalReviewFields(
  review,
  reviewMetrics,
  provisionalReview,
  provisionalReviewMetrics,
  entry,
) {
  return {
    reviewed_total: review.reviewedTotal,
    true_positive: review.truePositive,
    acceptable_warning: review.acceptableWarning,
    false_positive: review.falsePositive,
    inconclusive: review.inconclusive,
    reviewed_helpful_count: reviewMetrics.reviewed_helpful_count,
    reviewed_precision: reviewMetrics.reviewed_precision,
    useful_precision: reviewMetrics.true_positive_precision,
    true_positive_precision: reviewMetrics.true_positive_precision,
    review_noise_count: reviewMetrics.review_noise_count,
    review_noise_rate: reviewMetrics.review_noise_rate,
    false_positive_rate: reviewMetrics.false_positive_rate,
    inconclusive_rate: reviewMetrics.inconclusive_rate,
    provisional_reviewed_total: provisionalReview.reviewedTotal,
    provisional_reviewed_helpful_count: provisionalReviewMetrics.reviewed_helpful_count,
    provisional_reviewed_precision: provisionalReviewMetrics.reviewed_precision,
    provisional_review_noise_count: provisionalReviewMetrics.review_noise_count,
    provisional_review_noise_rate: provisionalReviewMetrics.review_noise_rate,
    review_top_1_total: entry.review_top_1_total ?? 0,
    review_top_1_actionable: entry.review_top_1_actionable ?? 0,
    top_1_actionable_precision: safeRatio(
      entry.review_top_1_actionable ?? 0,
      entry.review_top_1_total ?? 0,
    ),
    review_top_3_total: entry.review_top_3_total ?? 0,
    review_top_3_actionable: entry.review_top_3_actionable ?? 0,
    top_3_actionable_precision: safeRatio(
      entry.review_top_3_actionable ?? 0,
      entry.review_top_3_total ?? 0,
    ),
    review_top_10_total: entry.review_top_10_total ?? 0,
    review_top_10_actionable: entry.review_top_10_actionable ?? 0,
    top_10_actionable_precision: safeRatio(
      entry.review_top_10_actionable ?? 0,
      entry.review_top_10_total ?? 0,
    ),
    ranking_preference_total: entry.ranking_preference_total ?? 0,
    ranking_preference_satisfied: entry.ranking_preference_satisfied ?? 0,
    ranking_preference_violated: entry.ranking_preference_violated ?? 0,
    ranking_preference_unresolved: entry.ranking_preference_unresolved ?? 0,
    ranking_preference_satisfaction_rate: safeRatio(
      entry.ranking_preference_satisfied ?? 0,
      entry.ranking_preference_total ?? 0,
    ),
    provisional_review_top_1_total: entry.provisional_review_top_1_total ?? 0,
    provisional_review_top_1_actionable: entry.provisional_review_top_1_actionable ?? 0,
    provisional_top_1_actionable_precision: safeRatio(
      entry.provisional_review_top_1_actionable ?? 0,
      entry.provisional_review_top_1_total ?? 0,
    ),
    provisional_review_top_3_total: entry.provisional_review_top_3_total ?? 0,
    provisional_review_top_3_actionable: entry.provisional_review_top_3_actionable ?? 0,
    provisional_top_3_actionable_precision: safeRatio(
      entry.provisional_review_top_3_actionable ?? 0,
      entry.provisional_review_top_3_total ?? 0,
    ),
    provisional_review_top_10_total: entry.provisional_review_top_10_total ?? 0,
    provisional_review_top_10_actionable: entry.provisional_review_top_10_actionable ?? 0,
    provisional_top_10_actionable_precision: safeRatio(
      entry.provisional_review_top_10_actionable ?? 0,
      entry.provisional_review_top_10_total ?? 0,
    ),
    provisional_ranking_preference_total: entry.provisional_ranking_preference_total ?? 0,
    provisional_ranking_preference_satisfied:
      entry.provisional_ranking_preference_satisfied ?? 0,
    provisional_ranking_preference_violated: entry.provisional_ranking_preference_violated ?? 0,
    provisional_ranking_preference_unresolved:
      entry.provisional_ranking_preference_unresolved ?? 0,
    provisional_ranking_preference_satisfaction_rate: safeRatio(
      entry.provisional_ranking_preference_satisfied ?? 0,
      entry.provisional_ranking_preference_total ?? 0,
    ),
  };
}

function buildSignalRemediationFields(remediation) {
  return {
    remediation_total: remediation.remediationTotal,
    remediation_success: remediation.remediationSuccess,
    remediation_regressions: remediation.remediationRegressions,
    remediation_success_rate: safeRatio(
      remediation.remediationSuccess,
      remediation.remediationTotal,
    ),
  };
}

function buildSignalSessionFields(session, sessionMetrics) {
  return {
    session_trial_count: session.sessionTrialCount,
    live_session_trial_count: session.liveSessionTrialCount,
    replay_session_trial_count: session.replaySessionTrialCount,
    session_expected_presentations: session.sessionExpectedPresentations,
    session_expected_top_actions: session.sessionExpectedTopActions,
    session_expectation_misses: session.sessionExpectationMisses,
    session_expectation_hit_rate: safeRatio(
      session.sessionExpectedPresentations,
      session.sessionTrialCount,
    ),
    session_expectation_top_action_rate: safeRatio(
      session.sessionExpectedTopActions,
      session.sessionTrialCount,
    ),
    session_trial_miss_rate: safeRatio(
      session.sessionExpectationMisses,
      session.sessionTrialCount,
    ),
    session_top_actions: session.sessionTopActions,
    session_followups: session.sessionFollowups,
    session_cleared: session.sessionCleared,
    session_regressions: session.sessionRegressions,
    sessions_cleared: session.sessionsCleared,
    session_resolution_rate: sessionMetrics.session_resolution_rate,
    session_clear_rate: sessionMetrics.session_clear_rate,
    top_action_clear_rate: sessionMetrics.top_action_clear_rate,
    followup_regression_rate: sessionMetrics.followup_regression_rate,
    session_clean: session.sessionClean,
    session_clean_rate: sessionMetrics.session_clean_rate,
    average_checks_to_clear: sessionMetrics.average_checks_to_clear,
  };
}

function buildSignalRecord(entry, latencyMs) {
  const counts = buildSignalCounts(entry);
  const reviewMetrics = buildReviewMetrics(
    counts.review.reviewedTotal,
    counts.review.truePositive,
    counts.review.acceptableWarning,
    counts.review.falsePositive,
    counts.review.inconclusive,
  );
  const provisionalReviewMetrics = buildReviewMetrics(
    counts.provisionalReview.reviewedTotal,
    counts.provisionalReview.truePositive,
    counts.provisionalReview.acceptableWarning,
    counts.provisionalReview.falsePositive,
    counts.provisionalReview.inconclusive,
  );
  const sessionMetrics = buildSessionMetrics(
    counts.session.sessionTopActions,
    counts.session.sessionFollowups,
    counts.session.sessionCleared,
    counts.session.sessionRegressions,
    counts.session.sessionsCleared,
    counts.session.sessionClean,
    counts.session.sessionTotalChecksToClear,
  );
  const coverageFlags = buildCoverageFlags(
    entry,
    counts.review.reviewedTotal,
    counts.remediation.remediationTotal,
    counts.session.sessionTopActions,
  );
  const promotionEntry = {
    ...entry,
    true_positive: counts.review.truePositive,
    acceptable_warning: counts.review.acceptableWarning,
    false_positive: counts.review.falsePositive,
    inconclusive: counts.review.inconclusive,
    remediation_total: counts.remediation.remediationTotal,
    remediation_success: counts.remediation.remediationSuccess,
  };

  return {
    signal_kind: entry.signal_kind,
    signal_family: entry.signal_family,
    promotion_status: entry.promotion_status,
    blocking_intent: entry.blocking_intent,
    primary_lane: entry.primary_lane,
    ...buildSignalRecallFields(entry),
    ...buildSignalCoverageFields(coverageFlags),
    ...buildSignalReviewFields(
      counts.review,
      reviewMetrics,
      counts.provisionalReview,
      provisionalReviewMetrics,
      entry,
    ),
    ...buildSignalRemediationFields(counts.remediation),
    ...buildSignalSessionFields(counts.session, sessionMetrics),
    latency_ms: entry.seeded_check_supported > 0 ? latencyMs : null,
    promotion_recommendation: buildPromotionRecommendation(promotionEntry),
  };
}

function sumSignalField(signals, fieldName) {
  return signals.reduce((total, signal) => total + (signal[fieldName] ?? 0), 0);
}

function evaluatePrimaryTargetPolicy({
  top1ReviewedCount,
  top1ActionablePrecision,
  top3ReviewedCount,
  top3ActionablePrecision,
  top10ReviewedCount,
  top10ActionablePrecision,
  rankingPreferenceTotal,
  rankingPreferenceSatisfactionRate,
}) {
  const checks = [];

  if (top1ReviewedCount >= SIGNAL_PRIMARY_TARGET_POLICY.top1MinReviewedSamples) {
    checks.push(
      top1ActionablePrecision !== null &&
        top1ActionablePrecision >= SIGNAL_PRIMARY_TARGET_POLICY.top1ActionablePrecisionMin,
    );
  }
  if (top3ReviewedCount >= SIGNAL_PRIMARY_TARGET_POLICY.top3MinReviewedSamples) {
    checks.push(
      top3ActionablePrecision !== null &&
        top3ActionablePrecision >= SIGNAL_PRIMARY_TARGET_POLICY.top3ActionablePrecisionMin,
    );
  }
  if (top10ReviewedCount >= SIGNAL_PRIMARY_TARGET_POLICY.top10MinReviewedSamples) {
    checks.push(
      top10ActionablePrecision !== null &&
        top10ActionablePrecision >= SIGNAL_PRIMARY_TARGET_POLICY.top10ActionablePrecisionMin,
    );
  }
  if (
    rankingPreferenceTotal >=
    SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceMinComparisons
  ) {
    checks.push(
      rankingPreferenceSatisfactionRate !== null &&
        rankingPreferenceSatisfactionRate >=
          SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceSatisfactionMin,
    );
  }

  if (checks.length === 0) {
    return null;
  }

  return checks.every(Boolean);
}

function buildRankingQualitySummary(signals, prefix = '') {
  const top1ReviewedCount = sumSignalField(signals, `${prefix}review_top_1_total`);
  const top1ActionableCount = sumSignalField(signals, `${prefix}review_top_1_actionable`);
  const top3ReviewedCount = sumSignalField(signals, `${prefix}review_top_3_total`);
  const top3ActionableCount = sumSignalField(signals, `${prefix}review_top_3_actionable`);
  const top10ReviewedCount = sumSignalField(signals, `${prefix}review_top_10_total`);
  const top10ActionableCount = sumSignalField(signals, `${prefix}review_top_10_actionable`);
  const rankingPreferenceTotal = sumSignalField(signals, `${prefix}ranking_preference_total`);
  const rankingPreferenceSatisfied = sumSignalField(
    signals,
    `${prefix}ranking_preference_satisfied`,
  );
  const rankingPreferenceViolated = sumSignalField(
    signals,
    `${prefix}ranking_preference_violated`,
  );
  const rankingPreferenceUnresolved = sumSignalField(
    signals,
    `${prefix}ranking_preference_unresolved`,
  );
  const top1ActionablePrecision = safeRatio(top1ActionableCount, top1ReviewedCount);
  const top3ActionablePrecision = safeRatio(top3ActionableCount, top3ReviewedCount);
  const top10ActionablePrecision = safeRatio(top10ActionableCount, top10ReviewedCount);
  const rankingPreferenceSatisfactionRate = safeRatio(
    rankingPreferenceSatisfied,
    rankingPreferenceTotal,
  );

  return {
    ranking_supported: top1ReviewedCount > 0,
    actionable_review_sample_count: sumSignalField(signals, `${prefix}reviewed_helpful_count`),
    top_1_reviewed_count: top1ReviewedCount,
    top_1_actionable_count: top1ActionableCount,
    top_1_actionable_precision: top1ActionablePrecision,
    top_3_reviewed_count: top3ReviewedCount,
    top_3_actionable_count: top3ActionableCount,
    top_3_actionable_precision: top3ActionablePrecision,
    top_10_reviewed_count: top10ReviewedCount,
    top_10_actionable_count: top10ActionableCount,
    top_10_actionable_precision: top10ActionablePrecision,
    ranking_preference_total: rankingPreferenceTotal,
    ranking_preference_satisfied: rankingPreferenceSatisfied,
    ranking_preference_violated: rankingPreferenceViolated,
    ranking_preference_unresolved: rankingPreferenceUnresolved,
    ranking_preference_satisfaction_rate: rankingPreferenceSatisfactionRate,
    meets_primary_target_policy: evaluatePrimaryTargetPolicy({
      top1ReviewedCount,
      top1ActionablePrecision,
      top3ReviewedCount,
      top3ActionablePrecision,
      top10ReviewedCount,
      top10ActionablePrecision,
      rankingPreferenceTotal,
      rankingPreferenceSatisfactionRate,
    }),
  };
}

function buildScorecardSummary({
  signals,
  defectReport,
  reviewVerdicts,
  remediationReport,
  sessionCount,
  totalSessionTrialCount,
  latencyMs,
}) {
  const rankingQuality = buildRankingQualitySummary(signals);
  const provisionalRankingQuality = buildRankingQualitySummary(signals, 'provisional_');

  return {
    total_signals: signals.length,
    trusted_count: signals.filter((signal) => signal.promotion_status === 'trusted').length,
    watchpoint_count: signals.filter((signal) => signal.promotion_status === 'watchpoint').length,
    needs_review_count: signals.filter(
      (signal) => signal.promotion_recommendation === 'needs_review',
    ).length,
    degrade_count: signals.filter(
      (signal) => signal.promotion_recommendation === 'degrade_or_quarantine',
    ).length,
    promotion_evidence_complete_count: signals.filter(
      (signal) => signal.promotion_evidence_complete,
    ).length,
    kpis: {
      defect_sample_count: defectReport?.results?.length ?? 0,
      review_sample_count: reviewVerdicts?.provisional ? 0 : reviewVerdicts?.verdicts?.length ?? 0,
      provisional_review_sample_count: reviewVerdicts?.provisional
        ? reviewVerdicts?.verdicts?.length ?? 0
        : 0,
      remediation_sample_count: remediationReport?.results?.length ?? 0,
      session_trial_count: totalSessionTrialCount,
      session_count: sessionCount,
      actionable_review_sample_count: rankingQuality.actionable_review_sample_count,
    },
    coverage: {
      has_seeded_defects: (defectReport?.results?.length ?? 0) > 0,
      has_review_verdicts: !reviewVerdicts?.provisional && (reviewVerdicts?.verdicts?.length ?? 0) > 0,
      has_provisional_review_verdicts:
        Boolean(reviewVerdicts?.provisional) && (reviewVerdicts?.verdicts?.length ?? 0) > 0,
      has_remediation_results: (remediationReport?.results?.length ?? 0) > 0,
      has_session_telemetry: sessionCount > 0,
      has_session_trials: totalSessionTrialCount > 0,
      has_benchmark: latencyMs !== null,
    },
    ranking_quality: rankingQuality,
    provisional_ranking_quality: provisionalRankingQuality,
  };
}

export function buildSignalScorecard({
  repoLabel = null,
  defectReport = null,
  reviewVerdicts = null,
  remediationReport = null,
  benchmark = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
}) {
  const context = buildScorecardContext({
    repoLabel,
    defectReport,
    reviewVerdicts,
    remediationReport,
    benchmark,
    sessionTelemetry,
    codexBatch,
    replayBatch,
  });
  const signals = [...context.signalMap.values()]
    .map((entry) => buildSignalRecord(entry, context.latencyMs))
    .sort((left, right) => left.signal_kind.localeCompare(right.signal_kind));

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label: context.repoLabelValue,
    signals,
    summary: buildScorecardSummary({
      signals,
      defectReport,
      reviewVerdicts,
      remediationReport,
      sessionCount: context.sessionCount,
      totalSessionTrialCount: context.totalSessionTrialCount,
      latencyMs: context.latencyMs,
    }),
  };
}

export function formatSignalScorecardMarkdown(scorecard) {
  const lines = [];
  lines.push('# Signal Quality Scorecard');
  lines.push('');
  lines.push(`- repo: \`${scorecard.repo_label ?? 'unknown'}\``);
  lines.push(`- generated at: \`${scorecard.generated_at}\``);
  lines.push(`- signals: ${scorecard.summary.total_signals}`);
  lines.push(`- trusted: ${scorecard.summary.trusted_count}`);
  lines.push(`- watchpoint: ${scorecard.summary.watchpoint_count}`);
  lines.push(`- needs review: ${scorecard.summary.needs_review_count}`);
  lines.push(`- degrade or quarantine: ${scorecard.summary.degrade_count}`);
  lines.push(`- complete promotion evidence: ${scorecard.summary.promotion_evidence_complete_count ?? 0}`);
  if (scorecard.summary.kpis) {
    lines.push(`- seeded samples: ${scorecard.summary.kpis.defect_sample_count ?? 0}`);
    lines.push(`- reviewed samples: ${scorecard.summary.kpis.review_sample_count ?? 0}`);
    lines.push(
      `- provisional reviewed samples: ${scorecard.summary.kpis.provisional_review_sample_count ?? 0}`,
    );
    lines.push(`- remediation samples: ${scorecard.summary.kpis.remediation_sample_count ?? 0}`);
    lines.push(`- sessions: ${scorecard.summary.kpis.session_count ?? 0}`);
    lines.push(
      `- actionable reviewed samples: ${scorecard.summary.kpis.actionable_review_sample_count ?? 0}`,
    );
  }
  if (scorecard.summary.ranking_quality) {
    lines.push(
      `- top-1 actionable precision: ${scorecard.summary.ranking_quality.top_1_actionable_precision ?? 'n/a'}`,
    );
    lines.push(
      `- top-3 actionable precision: ${scorecard.summary.ranking_quality.top_3_actionable_precision ?? 'n/a'}`,
    );
    lines.push(
      `- top-10 actionable precision: ${scorecard.summary.ranking_quality.top_10_actionable_precision ?? 'n/a'}`,
    );
    lines.push(
      `- ranking preference satisfaction: ${scorecard.summary.ranking_quality.ranking_preference_satisfaction_rate ?? 'n/a'}`,
    );
    lines.push(
      `- primary-target policy: ${
        scorecard.summary.ranking_quality.meets_primary_target_policy === null
          ? 'insufficient evidence'
          : scorecard.summary.ranking_quality.meets_primary_target_policy
            ? 'pass'
            : 'fail'
      }`,
    );
  }
  lines.push('');
  lines.push('| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Noise Rate | Remediation Success | Trials | Trial Miss Rate | Top Action Clear | Regression Rate | Session Clean Rate | Avg Checks To Clear | Latency | Recommendation |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of scorecard.signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | \`${signal.signal_family}\` | \`${signal.promotion_status}\` | \`${signal.primary_lane ?? 'n/a'}\` | ${signal.seeded_recall ?? 'n/a'} | ${signal.primary_recall ?? 'n/a'} | ${signal.reviewed_precision ?? 'n/a'} | ${signal.review_noise_rate ?? 'n/a'} | ${signal.remediation_success_rate ?? 'n/a'} | ${signal.session_trial_count ?? 0} | ${signal.session_trial_miss_rate ?? 'n/a'} | ${signal.top_action_clear_rate ?? 'n/a'} | ${signal.followup_regression_rate ?? 'n/a'} | ${signal.session_clean_rate ?? 'n/a'} | ${signal.average_checks_to_clear ?? 'n/a'} | ${signal.latency_ms ?? 'n/a'} | \`${signal.promotion_recommendation}\` |`,
    );
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
