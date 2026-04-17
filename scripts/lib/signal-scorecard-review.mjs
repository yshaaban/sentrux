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

export function createEmptySignalEntry(signalKind, overrides = {}) {
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
    top_action_sessions: 0,
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
    sessions_thrashing: 0,
    sessions_stalled: 0,
    reopened_top_actions: 0,
    repeated_top_action_carries: 0,
    total_entropy_delta: 0,
    sessions_with_entropy_increase: 0,
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

export function ensureSignalEntry(signalMap, signalKind) {
  return ensureMapEntry(signalMap, signalKind, createEmptySignalEntry);
}

export function buildReviewMetrics(
  reviewedTotal,
  truePositive,
  acceptableWarning,
  falsePositive,
  inconclusive,
) {
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

export function applyReviewVerdicts(signalMap, reviewVerdicts) {
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

export function buildSignalReviewFields(
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
    provisional_ranking_preference_violated:
      entry.provisional_ranking_preference_violated ?? 0,
    provisional_ranking_preference_unresolved:
      entry.provisional_ranking_preference_unresolved ?? 0,
    provisional_ranking_preference_satisfaction_rate: safeRatio(
      entry.provisional_ranking_preference_satisfied ?? 0,
      entry.provisional_ranking_preference_total ?? 0,
    ),
  };
}

function buildPromotionInputs(entry) {
  const reviewedTotal = entry.reviewed_total ?? 0;
  const falsePositives = entry.false_positive ?? 0;
  const inconclusive = entry.inconclusive ?? 0;
  const top1ReviewedTotal = entry.review_top_1_total ?? 0;
  const top3ReviewedTotal = entry.review_top_3_total ?? 0;
  const rankingPreferenceTotal = entry.ranking_preference_total ?? 0;

  return {
    falsePositives,
    entropyIncreaseRate: safeRatio(
      entry.sessions_with_entropy_increase ?? 0,
      entry.top_action_sessions ?? 0,
    ),
    followupRegressionRate: safeRatio(
      entry.session_regressions ?? 0,
      entry.session_followups ?? 0,
    ),
    inconclusive,
    rankingPreferenceSatisfactionRate: safeRatio(
      entry.ranking_preference_satisfied ?? 0,
      rankingPreferenceTotal,
    ),
    rankingPreferenceTotal,
    remediationSuccess: safeRatio(
      entry.remediation_success ?? 0,
      entry.remediation_total ?? 0,
    ),
    reviewNoiseRate: safeRatio(falsePositives + inconclusive, reviewedTotal),
    reviewedPrecision: safeRatio(
      (entry.true_positive ?? 0) + (entry.acceptable_warning ?? 0),
      reviewedTotal,
    ),
    reviewedTotal,
    seededRecall: safeRatio(entry.seeded_detected, entry.seeded_total),
    sessionCleanRate: safeRatio(entry.session_clean ?? 0, entry.top_action_sessions ?? 0),
    sessionThrashRate: safeRatio(entry.sessions_thrashing ?? 0, entry.top_action_sessions ?? 0),
    top1ActionablePrecision: safeRatio(
      entry.review_top_1_actionable ?? 0,
      top1ReviewedTotal,
    ),
    top1ReviewedTotal,
    top3ActionablePrecision: safeRatio(
      entry.review_top_3_actionable ?? 0,
      top3ReviewedTotal,
    ),
    top3ReviewedTotal,
    topActionClearRate: safeRatio(
      entry.sessions_cleared ?? 0,
      entry.top_action_sessions ?? 0,
    ),
  };
}

function promotionNoiseDecision(metrics) {
  if (metrics.reviewedTotal > 0 && metrics.falsePositives > 0) {
    return 'degrade_or_quarantine';
  }
  if (
    metrics.reviewedTotal > 0 &&
    metrics.reviewNoiseRate !== null &&
    metrics.reviewNoiseRate > SIGNAL_PROMOTION_POLICY.reviewNoiseRateMax
  ) {
    return 'needs_review';
  }
  if (
    metrics.seededRecall !== null &&
    metrics.seededRecall < SIGNAL_PROMOTION_POLICY.seededRecallMin
  ) {
    return 'improve_detection';
  }
  if (
    metrics.reviewedPrecision !== null &&
    metrics.reviewedPrecision < SIGNAL_PROMOTION_POLICY.reviewedPrecisionMin
  ) {
    return 'reduce_noise';
  }
  return null;
}

function promotionPrimaryTargetDecision(metrics) {
  if (
    metrics.top1ReviewedTotal >= SIGNAL_PRIMARY_TARGET_POLICY.top1MinReviewedSamples &&
    metrics.top1ActionablePrecision !== null &&
    metrics.top1ActionablePrecision <
      SIGNAL_PRIMARY_TARGET_POLICY.top1ActionablePrecisionMin
  ) {
    return 'reduce_noise';
  }
  if (
    metrics.top3ReviewedTotal >= SIGNAL_PRIMARY_TARGET_POLICY.top3MinReviewedSamples &&
    metrics.top3ActionablePrecision !== null &&
    metrics.top3ActionablePrecision <
      SIGNAL_PRIMARY_TARGET_POLICY.top3ActionablePrecisionMin
  ) {
    return 'reduce_noise';
  }
  if (
    metrics.rankingPreferenceTotal >=
      SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceMinComparisons &&
    metrics.rankingPreferenceSatisfactionRate !== null &&
    metrics.rankingPreferenceSatisfactionRate <
      SIGNAL_PRIMARY_TARGET_POLICY.rankingPreferenceSatisfactionMin
  ) {
    return 'reduce_noise';
  }
  return null;
}

function promotionFixGuidanceDecision(metrics) {
  if (
    metrics.remediationSuccess !== null &&
    metrics.remediationSuccess < SIGNAL_PROMOTION_POLICY.remediationSuccessMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    metrics.topActionClearRate !== null &&
    metrics.topActionClearRate < SIGNAL_PROMOTION_POLICY.topActionClearRateMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    metrics.sessionCleanRate !== null &&
    metrics.sessionCleanRate < SIGNAL_PROMOTION_POLICY.sessionCleanRateMin
  ) {
    return 'improve_fix_guidance';
  }
  if (
    metrics.followupRegressionRate !== null &&
    metrics.followupRegressionRate > SIGNAL_PROMOTION_POLICY.followupRegressionRateMax
  ) {
    return 'improve_fix_guidance';
  }
  if (
    metrics.sessionThrashRate !== null &&
    metrics.sessionThrashRate > SIGNAL_PROMOTION_POLICY.sessionThrashRateMax
  ) {
    return 'improve_fix_guidance';
  }
  if (
    metrics.entropyIncreaseRate !== null &&
    metrics.entropyIncreaseRate > SIGNAL_PROMOTION_POLICY.entropyIncreaseRateMax
  ) {
    return 'improve_fix_guidance';
  }
  return null;
}

export function buildPromotionRecommendation(entry) {
  const metrics = buildPromotionInputs(entry);
  const noiseDecision = promotionNoiseDecision(metrics);
  if (noiseDecision) {
    return noiseDecision;
  }

  const primaryTargetDecision = promotionPrimaryTargetDecision(metrics);
  if (primaryTargetDecision) {
    return primaryTargetDecision;
  }

  const fixGuidanceDecision = promotionFixGuidanceDecision(metrics);
  if (fixGuidanceDecision) {
    return fixGuidanceDecision;
  }

  return `keep_${entry.promotion_status ?? 'unspecified'}`;
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

export function buildRankingQualitySummary(signals, prefix = '') {
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
