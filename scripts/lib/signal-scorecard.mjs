import { SIGNAL_PROMOTION_POLICY } from './signal-calibration-policy.mjs';

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

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

function safeRatio(numerator, denominator) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return null;
  }

  return Number((numerator / denominator).toFixed(3));
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
    ...overrides,
  };
}

function ensureSignalEntry(signalMap, signalKind) {
  if (!signalMap.has(signalKind)) {
    signalMap.set(signalKind, createEmptySignalEntry(signalKind));
  }

  return signalMap.get(signalKind);
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

  for (const verdict of reviewVerdicts?.verdicts ?? []) {
    const signalKind = verdict.kind;
    if (!signalKind) {
      continue;
    }

    const entry = ensureSignalEntry(signalMap, signalKind);
    entry[`${provisionalPrefix}reviewed_total`] =
      (entry[`${provisionalPrefix}reviewed_total`] ?? 0) + 1;
    const normalizedCategory = normalizeReviewCategory(verdict.category);
    entry[`${provisionalPrefix}${normalizedCategory}`] =
      (entry[`${provisionalPrefix}${normalizedCategory}`] ?? 0) + 1;
  }
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
  const sessionCount = sessionTelemetry?.summary?.session_count ?? sessionTelemetry?.sessions?.length ?? 0;

  const signals = [...signalMap.values()]
    .map((entry) => {
      const reviewedTotal = entry.reviewed_total ?? 0;
      const truePositive = entry.true_positive ?? 0;
      const acceptableWarning = entry.acceptable_warning ?? 0;
      const falsePositive = entry.false_positive ?? 0;
      const inconclusive = entry.inconclusive ?? 0;
      const provisionalReviewedTotal = entry.provisional_reviewed_total ?? 0;
      const provisionalTruePositive = entry.provisional_true_positive ?? 0;
      const provisionalAcceptableWarning = entry.provisional_acceptable_warning ?? 0;
      const provisionalFalsePositive = entry.provisional_false_positive ?? 0;
      const provisionalInconclusive = entry.provisional_inconclusive ?? 0;
      const remediationTotal = entry.remediation_total ?? 0;
      const remediationSuccess = entry.remediation_success ?? 0;
      const remediationRegressions = entry.remediation_regressions ?? 0;
      const sessionTopActions = entry.session_top_actions ?? 0;
      const sessionFollowups = entry.session_followups ?? 0;
      const sessionCleared = entry.session_cleared ?? 0;
      const sessionRegressions = entry.session_regressions ?? 0;
      const sessionsCleared = entry.sessions_cleared ?? 0;
      const sessionClean = entry.session_clean ?? 0;
      const sessionTotalChecksToClear = entry.session_total_checks_to_clear ?? 0;
      const sessionTrialCount = entry.session_trial_count ?? 0;
      const liveSessionTrialCount = entry.live_session_trial_count ?? 0;
      const replaySessionTrialCount = entry.replay_session_trial_count ?? 0;
      const sessionExpectedPresentations = entry.session_expected_presentations ?? 0;
      const sessionExpectedTopActions = entry.session_expected_top_actions ?? 0;
      const sessionExpectationMisses = entry.session_expectation_misses ?? 0;
      const latencyEligible = entry.seeded_check_supported > 0;
      const reviewMetrics = buildReviewMetrics(
        reviewedTotal,
        truePositive,
        acceptableWarning,
        falsePositive,
        inconclusive,
      );
      const provisionalReviewMetrics = buildReviewMetrics(
        provisionalReviewedTotal,
        provisionalTruePositive,
        provisionalAcceptableWarning,
        provisionalFalsePositive,
        provisionalInconclusive,
      );
      const sessionMetrics = buildSessionMetrics(
        sessionTopActions,
        sessionFollowups,
        sessionCleared,
        sessionRegressions,
        sessionsCleared,
        sessionClean,
        sessionTotalChecksToClear,
      );
      const coverageFlags = buildCoverageFlags(
        entry,
        reviewedTotal,
        remediationTotal,
        sessionTopActions,
      );

      return {
        signal_kind: entry.signal_kind,
        signal_family: entry.signal_family,
        promotion_status: entry.promotion_status,
        blocking_intent: entry.blocking_intent,
        primary_lane: entry.primary_lane,
        seeded_total: entry.seeded_total,
        seeded_detected: entry.seeded_detected,
        seeded_recall: safeRatio(entry.seeded_detected, entry.seeded_total),
        seeded_primary_supported: entry.seeded_primary_supported,
        seeded_primary_detected: entry.seeded_primary_detected,
        primary_recall: safeRatio(
          entry.seeded_primary_detected,
          entry.seeded_primary_supported,
        ),
        seeded_check_supported: entry.seeded_check_supported,
        seeded_check_detected: entry.seeded_check_detected,
        check_recall: safeRatio(entry.seeded_check_detected, entry.seeded_check_supported),
        seeded_check_rules_supported: entry.seeded_check_rules_supported,
        seeded_check_rules_detected: entry.seeded_check_rules_detected,
        check_rules_recall: safeRatio(
          entry.seeded_check_rules_detected,
          entry.seeded_check_rules_supported,
        ),
        reviewed_total: reviewedTotal,
        has_seeded_evidence: coverageFlags.has_seeded_evidence,
        has_review_evidence: coverageFlags.has_review_evidence,
        has_provisional_review_evidence: coverageFlags.has_provisional_review_evidence,
        has_remediation_evidence: coverageFlags.has_remediation_evidence,
        has_session_evidence: coverageFlags.has_session_evidence,
        has_session_action_evidence: coverageFlags.has_session_action_evidence,
        has_session_trial_evidence: coverageFlags.has_session_trial_evidence,
        promotion_evidence_complete: coverageFlags.promotion_evidence_complete,
        true_positive: truePositive,
        acceptable_warning: acceptableWarning,
        false_positive: falsePositive,
        inconclusive,
        reviewed_helpful_count: reviewMetrics.reviewed_helpful_count,
        reviewed_precision: reviewMetrics.reviewed_precision,
        useful_precision: reviewMetrics.true_positive_precision,
        true_positive_precision: reviewMetrics.true_positive_precision,
        review_noise_count: reviewMetrics.review_noise_count,
        review_noise_rate: reviewMetrics.review_noise_rate,
        false_positive_rate: reviewMetrics.false_positive_rate,
        inconclusive_rate: reviewMetrics.inconclusive_rate,
        provisional_reviewed_total: provisionalReviewedTotal,
        provisional_reviewed_helpful_count: provisionalReviewMetrics.reviewed_helpful_count,
        provisional_reviewed_precision: provisionalReviewMetrics.reviewed_precision,
        provisional_review_noise_count: provisionalReviewMetrics.review_noise_count,
        provisional_review_noise_rate: provisionalReviewMetrics.review_noise_rate,
        remediation_total: remediationTotal,
        remediation_success: remediationSuccess,
        remediation_regressions: remediationRegressions,
        remediation_success_rate: safeRatio(remediationSuccess, remediationTotal),
        session_trial_count: sessionTrialCount,
        live_session_trial_count: liveSessionTrialCount,
        replay_session_trial_count: replaySessionTrialCount,
        session_expected_presentations: sessionExpectedPresentations,
        session_expected_top_actions: sessionExpectedTopActions,
        session_expectation_misses: sessionExpectationMisses,
        session_expectation_hit_rate: safeRatio(
          sessionExpectedPresentations,
          sessionTrialCount,
        ),
        session_expectation_top_action_rate: safeRatio(
          sessionExpectedTopActions,
          sessionTrialCount,
        ),
        session_trial_miss_rate: safeRatio(
          sessionExpectationMisses,
          sessionTrialCount,
        ),
        session_top_actions: sessionTopActions,
        session_followups: sessionFollowups,
        session_cleared: sessionCleared,
        session_regressions: sessionRegressions,
        sessions_cleared: sessionsCleared,
        session_resolution_rate: sessionMetrics.session_resolution_rate,
        session_clear_rate: sessionMetrics.session_clear_rate,
        top_action_clear_rate: sessionMetrics.top_action_clear_rate,
        followup_regression_rate: sessionMetrics.followup_regression_rate,
        session_clean: sessionClean,
        session_clean_rate: sessionMetrics.session_clean_rate,
        average_checks_to_clear: sessionMetrics.average_checks_to_clear,
        latency_ms: latencyEligible ? latencyMs : null,
        promotion_recommendation: buildPromotionRecommendation({
          ...entry,
          true_positive: truePositive,
          acceptable_warning: acceptableWarning,
          false_positive: falsePositive,
          inconclusive,
          remediation_total: remediationTotal,
          remediation_success: remediationSuccess,
        }),
      };
    })
    .sort((left, right) => left.signal_kind.localeCompare(right.signal_kind));

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label: repoLabelValue,
    signals,
    summary: {
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
    },
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
