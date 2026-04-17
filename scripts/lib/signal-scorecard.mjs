import {
  applyReviewVerdicts,
  buildPromotionRecommendation,
  buildRankingQualitySummary,
  buildReviewMetrics,
  buildSignalReviewFields,
  ensureSignalEntry,
} from './signal-scorecard-review.mjs';
import {
  applyBatchSessionTrials,
  applyRemediationResults,
  applySessionTelemetry,
  buildCoverageFlags,
  buildSeededEntries,
  buildSessionMetrics,
  buildSignalCounts,
  buildSignalRecallFields,
  buildSignalRemediationFields,
  buildSignalSessionFields,
  countExpectedSignalTrials,
  inferLatencyMs,
  inferScorecardRepoLabel,
} from './signal-scorecard-evidence.mjs';
import { formatSignalScorecardMarkdown } from './signal-scorecard-format.mjs';
import { safeRatio } from './signal-summary-utils.mjs';

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
  applyRemediationResults(signalMap, remediationReport, ensureSignalEntry);
  applySessionTelemetry(signalMap, sessionTelemetry, ensureSignalEntry);
  applyBatchSessionTrials(signalMap, codexBatch?.results, 'live', ensureSignalEntry);
  applyBatchSessionTrials(signalMap, replayBatch?.results, 'replay', ensureSignalEntry);

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

function totalSessionSignalField(sessionTelemetry, fieldName) {
  return (sessionTelemetry?.signals ?? []).reduce(function sumSignalField(total, signal) {
    return total + (signal?.[fieldName] ?? 0);
  }, 0);
}

function summarizeSessionHealth(sessionTelemetry) {
  const summary = sessionTelemetry?.summary ?? {};
  const topActionSessionCount =
    summary.top_action_session_count ?? totalSessionSignalField(sessionTelemetry, 'top_action_sessions');
  const topActionClearedCount =
    summary.top_action_cleared_count ?? totalSessionSignalField(sessionTelemetry, 'sessions_cleared');
  const followupRegressionCount =
    summary.followup_regression_count ?? totalSessionSignalField(sessionTelemetry, 'followup_regressions');
  const reopenedTopActionCount =
    summary.reopened_top_action_count ?? totalSessionSignalField(sessionTelemetry, 'reopened_top_actions');
  const sessionCleanCount =
    summary.session_clean_count ?? totalSessionSignalField(sessionTelemetry, 'sessions_clean');
  const sessionThrashingCount =
    summary.thrashing_session_count ?? totalSessionSignalField(sessionTelemetry, 'sessions_thrashing');
  const sessionStalledCount =
    summary.stalled_session_count ?? totalSessionSignalField(sessionTelemetry, 'sessions_stalled');
  const entropyIncreaseSessionCount =
    summary.entropy_increase_session_count ??
    totalSessionSignalField(sessionTelemetry, 'sessions_with_entropy_increase');
  const totalChecksToClear =
    summary.average_checks_to_clear !== undefined &&
    summary.average_checks_to_clear !== null &&
    summary.top_action_cleared_count !== undefined
      ? summary.average_checks_to_clear * summary.top_action_cleared_count
      : totalSessionSignalField(sessionTelemetry, 'total_checks_to_clear');
  const totalEntropyDelta = totalSessionSignalField(sessionTelemetry, 'total_entropy_delta');
  const fallbackAverageEntropyDelta = safeRatio(totalEntropyDelta, topActionSessionCount);

  return {
    converged_session_count: summary.converged_session_count ?? 0,
    converging_session_count: summary.converging_session_count ?? 0,
    stalled_session_count: sessionStalledCount,
    thrashing_session_count: sessionThrashingCount,
    top_action_session_count: topActionSessionCount,
    top_action_cleared_count: topActionClearedCount,
    followup_regression_count: followupRegressionCount,
    reopened_top_action_count: reopenedTopActionCount,
    session_clean_count: sessionCleanCount,
    entropy_increase_session_count: entropyIncreaseSessionCount,
    top_action_clear_rate:
      summary.top_action_clear_rate ?? safeRatio(topActionClearedCount, topActionSessionCount),
    agent_clear_rate:
      summary.agent_clear_rate ?? safeRatio(topActionClearedCount, topActionSessionCount),
    followup_regression_session_rate:
      summary.followup_regression_session_rate ??
      safeRatio(followupRegressionCount, topActionSessionCount),
    regression_after_fix_rate:
      summary.regression_after_fix_rate ??
      safeRatio(reopenedTopActionCount, topActionSessionCount),
    session_clean_rate:
      summary.session_clean_rate ?? safeRatio(sessionCleanCount, topActionSessionCount),
    session_thrash_rate:
      summary.session_thrash_rate ?? safeRatio(sessionThrashingCount, topActionSessionCount),
    session_stall_rate:
      summary.session_stall_rate ?? safeRatio(sessionStalledCount, topActionSessionCount),
    entropy_increase_rate:
      summary.entropy_increase_rate ??
      safeRatio(entropyIncreaseSessionCount, topActionSessionCount),
    average_checks_to_clear:
      summary.average_checks_to_clear ?? safeRatio(totalChecksToClear, topActionClearedCount),
    average_entropy_delta: summary.average_entropy_delta ?? fallbackAverageEntropyDelta,
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
    counts.session.topActionSessions,
    counts.session.sessionFollowups,
    counts.session.sessionCleared,
    counts.session.sessionRegressions,
    counts.session.sessionsCleared,
    counts.session.sessionClean,
    counts.session.sessionTotalChecksToClear,
    counts.session.sessionsThrashing,
    counts.session.sessionsStalled,
    counts.session.reopenedTopActions,
    counts.session.repeatedTopActionCarries,
    counts.session.totalEntropyDelta,
    counts.session.sessionsWithEntropyIncrease,
  );
  const coverageFlags = buildCoverageFlags(
    entry,
    counts.review.reviewedTotal,
    counts.remediation.remediationTotal,
    counts.session.topActionSessions,
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

function buildScorecardSummary({
  signals,
  defectReport,
  reviewVerdicts,
  remediationReport,
  sessionCount,
  totalSessionTrialCount,
  latencyMs,
  sessionTelemetry = null,
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
      has_review_verdicts:
        !reviewVerdicts?.provisional && (reviewVerdicts?.verdicts?.length ?? 0) > 0,
      has_provisional_review_verdicts:
        Boolean(reviewVerdicts?.provisional) && (reviewVerdicts?.verdicts?.length ?? 0) > 0,
      has_remediation_results: (remediationReport?.results?.length ?? 0) > 0,
      has_session_telemetry: sessionCount > 0,
      has_session_trials: totalSessionTrialCount > 0,
      has_benchmark: latencyMs !== null,
    },
    ranking_quality: rankingQuality,
    provisional_ranking_quality: provisionalRankingQuality,
    session_health: summarizeSessionHealth(sessionTelemetry),
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
      sessionTelemetry,
    }),
  };
}

export { formatSignalScorecardMarkdown };
