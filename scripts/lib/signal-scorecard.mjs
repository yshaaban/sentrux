import {
  applyReviewVerdicts,
  buildDefaultRolloutRecommendation,
  buildPromotionRecommendation,
  buildRankingQualitySummary,
  buildReviewMetrics,
  buildSignalReviewFields,
  ensureSignalEntry,
} from './signal-scorecard-review.mjs';
import { enrichReviewVerdictsFromPacket } from './review-verdict-enrichment.mjs';
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
import { buildSessionCorpus } from './session-corpus.mjs';
import { buildSignalMetadataLookup } from './signal-cohorts.mjs';
import {
  buildInterventionNetValueScore,
  buildSessionVerdictSummary,
} from './session-verdicts.mjs';

function buildScorecardContext({
  repoLabel = null,
  defectReport = null,
  reviewVerdicts = null,
  reviewPacket = null,
  remediationReport = null,
  benchmark = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
  sessionVerdicts = null,
  cohortManifest = null,
  cohortId = null,
}) {
  const hydratedReviewVerdicts = enrichReviewVerdictsFromPacket(reviewVerdicts, reviewPacket);
  const signalMap = buildSeededEntries(defectReport);
  applyReviewVerdicts(signalMap, hydratedReviewVerdicts);
  applyRemediationResults(signalMap, remediationReport, ensureSignalEntry);
  applySessionTelemetry(signalMap, sessionTelemetry, ensureSignalEntry);
  applyBatchSessionTrials(signalMap, codexBatch?.results, 'live', ensureSignalEntry);
  applyBatchSessionTrials(signalMap, replayBatch?.results, 'replay', ensureSignalEntry);

  const latencyMs = inferLatencyMs(benchmark);
  const repoLabelValue = inferScorecardRepoLabel({
    repoLabel,
    defectReport,
    reviewVerdicts: hydratedReviewVerdicts,
    remediationReport,
    sessionTelemetry,
    codexBatch,
    replayBatch,
    sessionVerdicts,
  });
  const liveSessionTrialCount = countExpectedSignalTrials(codexBatch?.results);
  const replaySessionTrialCount = countExpectedSignalTrials(replayBatch?.results);
  const totalSessionTrialCount = liveSessionTrialCount + replaySessionTrialCount;
  const sessionCount =
    sessionTelemetry?.summary?.session_count ?? sessionTelemetry?.sessions?.length ?? 0;
  const sessionCorpus = buildSessionCorpus({
    repoLabel: repoLabelValue,
    sessionTelemetry,
    codexBatch,
    replayBatch,
    sessionVerdicts,
  });
  applySignalSessionVerdicts(signalMap, sessionCorpus);
  applySignalTreatmentEvidence(signalMap, sessionCorpus);
  applySignalCohortMetadata(signalMap, cohortManifest, cohortId);

  return {
    signalMap,
    latencyMs,
    repoLabelValue,
    totalSessionTrialCount,
    sessionCount,
    sessionCorpus,
    hydratedReviewVerdicts,
  };
}

function applySignalCohortMetadata(signalMap, cohortManifest = null, cohortId = null) {
  const signalMetadata = buildSignalMetadataLookup(cohortManifest, cohortId);

  for (const [signalKind, entry] of signalMap.entries()) {
    const metadata = signalMetadata.get(signalKind);
    if (!metadata) {
      continue;
    }

    if (!entry.signal_family || entry.signal_family === 'unknown') {
      entry.signal_family = metadata.signal_family ?? entry.signal_family;
    }
    if (!entry.promotion_status || entry.promotion_status === 'unspecified') {
      entry.promotion_status = metadata.promotion_status ?? entry.promotion_status;
    }
    if (!entry.product_primary_lane) {
      entry.product_primary_lane = metadata.primary_lane ?? null;
    }
    if (!entry.default_surface_role) {
      entry.default_surface_role = metadata.default_surface_role ?? null;
    }
  }
}

function incrementBooleanSessionVerdict(entry, totalFieldName, countFieldName, value) {
  if (value !== true && value !== false) {
    return;
  }

  entry[totalFieldName] += 1;
  if (value) {
    entry[countFieldName] += 1;
  }
}

function applySignalSessionVerdicts(signalMap, sessionCorpus) {
  for (const session of sessionCorpus?.sessions ?? []) {
    const signalKind = session?.outcome?.initial_top_action_kind;
    const verdict = session?.session_verdict;
    if (typeof signalKind !== 'string' || signalKind.length === 0 || !verdict) {
      continue;
    }

    const entry = ensureSignalEntry(signalMap, signalKind);
    entry.session_verdict_count += 1;
    if (session.lane === 'live') {
      entry.live_session_verdict_count += 1;
    }
    if (session.lane === 'replay') {
      entry.replay_session_verdict_count += 1;
    }

    incrementBooleanSessionVerdict(
      entry,
      'top_action_follow_sample_count',
      'top_action_followed_count',
      verdict.top_action_followed,
    );
    incrementBooleanSessionVerdict(
      entry,
      'top_action_help_sample_count',
      'top_action_helped_count',
      verdict.top_action_helped,
    );
    incrementBooleanSessionVerdict(
      entry,
      'task_success_sample_count',
      'task_completed_successfully_count',
      verdict.task_completed_successfully,
    );
    incrementBooleanSessionVerdict(
      entry,
      'patch_expansion_sample_count',
      'patch_expanded_unnecessarily_count',
      verdict.patch_expanded_unnecessarily,
    );
    incrementBooleanSessionVerdict(
      entry,
      'reviewer_acceptance_sample_count',
      'reviewer_accepted_count',
      verdict.reviewer_accepts_top_action,
    );
    incrementBooleanSessionVerdict(
      entry,
      'reviewer_disagreement_sample_count',
      'reviewer_disagreed_count',
      verdict.reviewer_disagrees_with_top_action,
    );

    if (
      Number.isInteger(verdict.intervention_cost_checks) &&
      verdict.intervention_cost_checks >= 0
    ) {
      entry.intervention_cost_sample_count += 1;
      entry.intervention_cost_checks_total += verdict.intervention_cost_checks;
    }
  }
}

function compareSignalTreatmentComparison(left, right) {
  if (left.qualified_for_default_rollout !== right.qualified_for_default_rollout) {
    return Number(right.qualified_for_default_rollout) -
      Number(left.qualified_for_default_rollout);
  }
  if (left.intervention_net_value_score_delta !== right.intervention_net_value_score_delta) {
    return (
      (right.intervention_net_value_score_delta ?? Number.NEGATIVE_INFINITY) -
      (left.intervention_net_value_score_delta ?? Number.NEGATIVE_INFINITY)
    );
  }
  if (left.top_action_help_rate_delta !== right.top_action_help_rate_delta) {
    return (
      (right.top_action_help_rate_delta ?? Number.NEGATIVE_INFINITY) -
      (left.top_action_help_rate_delta ?? Number.NEGATIVE_INFINITY)
    );
  }
  if (left.task_success_rate_delta !== right.task_success_rate_delta) {
    return (
      (right.task_success_rate_delta ?? Number.NEGATIVE_INFINITY) -
      (left.task_success_rate_delta ?? Number.NEGATIVE_INFINITY)
    );
  }
  if (left.patch_expansion_rate_delta !== right.patch_expansion_rate_delta) {
    return (
      (left.patch_expansion_rate_delta ?? Number.POSITIVE_INFINITY) -
      (right.patch_expansion_rate_delta ?? Number.POSITIVE_INFINITY)
    );
  }
  if (left.session_count !== right.session_count) {
    return right.session_count - left.session_count;
  }
  if (left.baseline_session_count !== right.baseline_session_count) {
    return right.baseline_session_count - left.baseline_session_count;
  }

  return left.experiment_arm.localeCompare(right.experiment_arm);
}

function applySignalTreatmentEvidence(signalMap, sessionCorpus) {
  const statsBySignalKind = new Map();

  for (const comparison of sessionCorpus?.signal_experiment_comparisons ?? []) {
    const current = statsBySignalKind.get(comparison.signal_kind) ?? {
      comparisonCount: 0,
      qualifiedComparisonCount: 0,
      bestComparison: null,
    };

    current.comparisonCount += 1;
    if (comparison.qualified_for_default_rollout === true) {
      current.qualifiedComparisonCount += 1;
    }
    if (
      !current.bestComparison ||
      compareSignalTreatmentComparison(comparison, current.bestComparison) < 0
    ) {
      current.bestComparison = comparison;
    }

    statsBySignalKind.set(comparison.signal_kind, current);
  }

  for (const [signalKind, stats] of statsBySignalKind.entries()) {
    const entry = ensureSignalEntry(signalMap, signalKind);

    entry.signal_treatment_comparison_count = stats.comparisonCount;
    entry.signal_treatment_qualified_comparison_count = stats.qualifiedComparisonCount;
    entry.signal_treatment_ready = stats.qualifiedComparisonCount > 0;
    entry.signal_treatment_best_arm = stats.bestComparison?.experiment_arm ?? null;
    entry.signal_treatment_top_action_help_rate_delta =
      stats.bestComparison?.top_action_help_rate_delta ?? null;
    entry.signal_treatment_task_success_rate_delta =
      stats.bestComparison?.task_success_rate_delta ?? null;
    entry.signal_treatment_patch_expansion_rate_delta =
      stats.bestComparison?.patch_expansion_rate_delta ?? null;
    entry.signal_treatment_intervention_net_value_score_delta =
      stats.bestComparison?.intervention_net_value_score_delta ?? null;
  }
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

function buildProductValueSummary(sessionCorpus) {
  const summary = sessionCorpus?.summary ?? {};
  const laneSummaries = buildLaneProductValueSummaries(sessionCorpus);

  return {
    session_verdict_count: summary.session_verdict_count ?? 0,
    top_action_follow_rate: summary.top_action_follow_rate ?? null,
    top_action_help_rate: summary.top_action_help_rate ?? null,
    task_success_rate: summary.task_success_rate ?? null,
    patch_expansion_rate: summary.patch_expansion_rate ?? null,
    reviewer_acceptance_rate: summary.reviewer_acceptance_rate ?? null,
    reviewer_disagreement_rate: summary.reviewer_disagreement_rate ?? null,
    intervention_cost_checks_mean: summary.intervention_cost_checks_mean ?? null,
    intervention_net_value_score: summary.intervention_net_value_score ?? null,
    lane_summaries: laneSummaries,
  };
}

function buildLaneProductValueSummary(entries, lane) {
  const sessionVerdictSummary = buildSessionVerdictSummary(entries);
  if ((sessionVerdictSummary.session_verdict_count ?? 0) === 0) {
    return null;
  }

  return {
    lane,
    session_count: entries.length,
    session_verdict_count: sessionVerdictSummary.session_verdict_count,
    top_action_follow_rate: sessionVerdictSummary.top_action_follow_rate,
    top_action_help_rate: sessionVerdictSummary.top_action_help_rate,
    task_success_rate: sessionVerdictSummary.task_success_rate,
    patch_expansion_rate: sessionVerdictSummary.patch_expansion_rate,
    reviewer_acceptance_rate: sessionVerdictSummary.reviewer_acceptance_rate,
    reviewer_disagreement_rate: sessionVerdictSummary.reviewer_disagreement_rate,
    intervention_cost_checks_mean: sessionVerdictSummary.intervention_cost_checks_mean,
    intervention_net_value_score: sessionVerdictSummary.intervention_net_value_score,
  };
}

function buildLaneProductValueSummaries(sessionCorpus) {
  const sessions = sessionCorpus?.sessions ?? [];
  const laneEntries = {
    live: sessions.filter(function isLiveSession(entry) {
      return entry.lane === 'live';
    }),
    replay: sessions.filter(function isReplaySession(entry) {
      return entry.lane === 'replay';
    }),
  };

  return ['live', 'replay']
    .map(function toLaneSummary(lane) {
      return buildLaneProductValueSummary(laneEntries[lane], lane);
    })
    .filter(Boolean);
}

function buildSignalProductValueFields(entry) {
  const topActionFollowRate = safeRatio(
    entry.top_action_followed_count ?? 0,
    entry.top_action_follow_sample_count ?? 0,
  );
  const topActionHelpRate = safeRatio(
    entry.top_action_helped_count ?? 0,
    entry.top_action_help_sample_count ?? 0,
  );
  const taskSuccessRate = safeRatio(
    entry.task_completed_successfully_count ?? 0,
    entry.task_success_sample_count ?? 0,
  );
  const patchExpansionRate = safeRatio(
    entry.patch_expanded_unnecessarily_count ?? 0,
    entry.patch_expansion_sample_count ?? 0,
  );
  const interventionCostChecksMean = safeRatio(
    entry.intervention_cost_checks_total ?? 0,
    entry.intervention_cost_sample_count ?? 0,
  );
  const reviewerAcceptanceRate = safeRatio(
    entry.reviewer_accepted_count ?? 0,
    entry.reviewer_acceptance_sample_count ?? 0,
  );
  const reviewerDisagreementRate = safeRatio(
    entry.reviewer_disagreed_count ?? 0,
    entry.reviewer_disagreement_sample_count ?? 0,
  );

  return {
    session_verdict_count: entry.session_verdict_count ?? 0,
    live_session_verdict_count: entry.live_session_verdict_count ?? 0,
    replay_session_verdict_count: entry.replay_session_verdict_count ?? 0,
    top_action_follow_sample_count: entry.top_action_follow_sample_count ?? 0,
    top_action_followed_count: entry.top_action_followed_count ?? 0,
    top_action_follow_rate: topActionFollowRate,
    top_action_help_sample_count: entry.top_action_help_sample_count ?? 0,
    top_action_helped_count: entry.top_action_helped_count ?? 0,
    top_action_help_rate: topActionHelpRate,
    task_success_sample_count: entry.task_success_sample_count ?? 0,
    task_completed_successfully_count: entry.task_completed_successfully_count ?? 0,
    task_success_rate: taskSuccessRate,
    patch_expansion_sample_count: entry.patch_expansion_sample_count ?? 0,
    patch_expanded_unnecessarily_count: entry.patch_expanded_unnecessarily_count ?? 0,
    patch_expansion_rate: patchExpansionRate,
    reviewer_acceptance_sample_count: entry.reviewer_acceptance_sample_count ?? 0,
    reviewer_accepted_count: entry.reviewer_accepted_count ?? 0,
    reviewer_acceptance_rate: reviewerAcceptanceRate,
    reviewer_disagreement_sample_count: entry.reviewer_disagreement_sample_count ?? 0,
    reviewer_disagreed_count: entry.reviewer_disagreed_count ?? 0,
    reviewer_disagreement_rate: reviewerDisagreementRate,
    intervention_cost_sample_count: entry.intervention_cost_sample_count ?? 0,
    intervention_cost_checks_total: entry.intervention_cost_checks_total ?? 0,
    intervention_cost_checks_mean: interventionCostChecksMean,
    signal_treatment_comparison_count: entry.signal_treatment_comparison_count ?? 0,
    signal_treatment_qualified_comparison_count:
      entry.signal_treatment_qualified_comparison_count ?? 0,
    signal_treatment_ready: entry.signal_treatment_ready === true,
    signal_treatment_best_arm: entry.signal_treatment_best_arm ?? null,
    signal_treatment_top_action_help_rate_delta:
      entry.signal_treatment_top_action_help_rate_delta ?? null,
    signal_treatment_task_success_rate_delta:
      entry.signal_treatment_task_success_rate_delta ?? null,
    signal_treatment_patch_expansion_rate_delta:
      entry.signal_treatment_patch_expansion_rate_delta ?? null,
    signal_treatment_intervention_net_value_score_delta:
      entry.signal_treatment_intervention_net_value_score_delta ?? null,
    intervention_net_value_score: buildInterventionNetValueScore({
      topActionFollowRate,
      topActionHelpRate,
      taskSuccessRate,
      patchExpansionRate,
      interventionCostChecksMean,
    }),
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
  const signalProductValue = buildSignalProductValueFields(entry);

  return {
    signal_kind: entry.signal_kind,
    signal_family: entry.signal_family,
    promotion_status: entry.promotion_status,
    blocking_intent: entry.blocking_intent,
    product_primary_lane: entry.product_primary_lane,
    default_surface_role: entry.default_surface_role,
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
    ...signalProductValue,
    latency_ms: entry.seeded_check_supported > 0 ? latencyMs : null,
    promotion_recommendation: buildPromotionRecommendation({
      ...promotionEntry,
      ...signalProductValue,
    }),
    default_rollout_recommendation: buildDefaultRolloutRecommendation({
      ...promotionEntry,
      ...signalProductValue,
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
  sessionTelemetry = null,
  sessionCorpus = null,
}) {
  const rankingQuality = buildRankingQualitySummary(signals);
  const provisionalRankingQuality = buildRankingQualitySummary(signals, 'provisional_');
  const productValue = buildProductValueSummary(sessionCorpus);

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
    default_rollout_candidate_count: signals.filter(
      (signal) =>
        signal.default_rollout_recommendation === 'await_treatment_proof' ||
        signal.default_rollout_recommendation === 'ready_for_default_on',
    ).length,
    default_rollout_ready_count: signals.filter(
      (signal) => signal.default_rollout_recommendation === 'ready_for_default_on',
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
      session_verdict_count: productValue.session_verdict_count,
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
      has_session_verdicts: productValue.session_verdict_count > 0,
      has_benchmark: latencyMs !== null,
    },
    ranking_quality: rankingQuality,
    provisional_ranking_quality: provisionalRankingQuality,
    session_health: summarizeSessionHealth(sessionTelemetry),
    product_value: productValue,
  };
}

export function buildSignalScorecard({
  repoLabel = null,
  defectReport = null,
  reviewVerdicts = null,
  reviewPacket = null,
  remediationReport = null,
  benchmark = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
  sessionVerdicts = null,
  cohortManifest = null,
  cohortId = null,
}) {
  const context = buildScorecardContext({
    repoLabel,
    defectReport,
    reviewVerdicts,
    reviewPacket,
    remediationReport,
    benchmark,
    sessionTelemetry,
    codexBatch,
    replayBatch,
    sessionVerdicts,
    cohortManifest,
    cohortId,
  });
  const signals = [...context.signalMap.values()]
    .map((entry) => buildSignalRecord(entry, context.latencyMs))
    .sort((left, right) => left.signal_kind.localeCompare(right.signal_kind));

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label: context.repoLabelValue,
    evidence_sources: context.sessionCorpus?.evidence_sources ?? { live: null, replay: null },
    signals,
    summary: buildScorecardSummary({
      signals,
      defectReport,
      reviewVerdicts: context.hydratedReviewVerdicts,
      remediationReport,
      sessionCount: context.sessionCount,
      totalSessionTrialCount: context.totalSessionTrialCount,
      latencyMs: context.latencyMs,
      sessionTelemetry,
      sessionCorpus: context.sessionCorpus,
    }),
  };
}

export { formatSignalScorecardMarkdown };
