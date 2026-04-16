import { asArray, safeRatio } from './signal-summary-utils.mjs';
import { createEmptySignalEntry } from './signal-scorecard-review.mjs';

export function buildSessionMetrics(
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

export function buildCoverageFlags(entry, reviewedTotal, remediationTotal, sessionTopActions) {
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

export function buildSeededEntries(defectReport) {
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

export function applyRemediationResults(signalMap, remediationReport, ensureSignalEntry) {
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

export function applySessionTelemetry(signalMap, sessionTelemetry, ensureSignalEntry) {
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

export function applyBatchSessionTrials(signalMap, results, lane, ensureSignalEntry) {
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

export function countExpectedSignalTrials(results) {
  return asArray(results).reduce(
    (total, result) => total + asArray(result.expected_signal_kinds).filter(Boolean).length,
    0,
  );
}

export function inferScorecardRepoLabel({
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

export function inferLatencyMs(benchmark) {
  return (
    benchmark?.benchmark?.warm_patch_safety?.check?.elapsed_ms ??
    benchmark?.benchmark?.warm_cached?.check?.elapsed_ms ??
    benchmark?.benchmark?.warm_cached?.gate?.elapsed_ms ??
    null
  );
}

export function buildSignalCounts(entry) {
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

export function buildSignalRecallFields(entry) {
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

export function buildSignalRemediationFields(remediation) {
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

export function buildSignalSessionFields(session, sessionMetrics) {
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
