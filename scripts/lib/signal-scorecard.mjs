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
    session_clean: 0,
    session_total_checks_to_clear: 0,
    ...overrides,
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
  for (const verdict of reviewVerdicts?.verdicts ?? []) {
    const signalKind = verdict.kind;
    if (!signalKind) {
      continue;
    }

    if (!signalMap.has(signalKind)) {
      signalMap.set(signalKind, createEmptySignalEntry(signalKind));
    }

    const entry = signalMap.get(signalKind);
    entry.reviewed_total = (entry.reviewed_total ?? 0) + 1;
    const normalizedCategory = normalizeReviewCategory(verdict.category);
    entry[normalizedCategory] = (entry[normalizedCategory] ?? 0) + 1;
  }
}

function applyRemediationResults(signalMap, remediationReport) {
  for (const result of remediationReport?.results ?? []) {
    const signalKind = result.signal_kind;
    if (!signalKind) {
      continue;
    }

    if (!signalMap.has(signalKind)) {
      signalMap.set(signalKind, createEmptySignalEntry(signalKind));
    }

    const entry = signalMap.get(signalKind);
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
  for (const signal of sessionTelemetry?.signals ?? []) {
    const signalKind = signal.signal_kind;
    if (!signalKind) {
      continue;
    }

    if (!signalMap.has(signalKind)) {
      signalMap.set(signalKind, createEmptySignalEntry(signalKind));
    }

    const entry = signalMap.get(signalKind);
    entry.session_top_actions += signal.top_action_presented ?? 0;
    entry.session_followups += signal.followup_checks ?? 0;
    entry.session_cleared += signal.target_cleared ?? 0;
    entry.session_regressions += signal.followup_regressions ?? 0;
    entry.session_clean += signal.sessions_clean ?? 0;
    entry.session_total_checks_to_clear += signal.total_checks_to_clear ?? 0;
  }
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
  const useful = (entry.true_positive ?? 0) + (entry.acceptable_warning ?? 0);
  const usefulPrecision = safeRatio(useful, reviewedTotal);
  const remediationSuccess = safeRatio(
    entry.remediation_success ?? 0,
    entry.remediation_total ?? 0,
  );
  const sessionResolution = safeRatio(
    entry.session_cleared ?? 0,
    entry.session_followups ?? 0,
  );
  const sessionCleanRate = safeRatio(entry.session_clean ?? 0, entry.session_top_actions ?? 0);

  if (reviewedTotal > 0 && falsePositives > 0) {
    return 'degrade_or_quarantine';
  }
  if (reviewedTotal > 0 && inconclusive / reviewedTotal > 0.2) {
    return 'needs_review';
  }
  if (seededRecall !== null && seededRecall < 0.95) {
    return 'improve_detection';
  }
  if (usefulPrecision !== null && usefulPrecision < 0.8) {
    return 'reduce_noise';
  }
  if (remediationSuccess !== null && remediationSuccess < 0.6) {
    return 'improve_fix_guidance';
  }
  if (sessionResolution !== null && sessionResolution < 0.6) {
    return 'improve_fix_guidance';
  }
  if (sessionCleanRate !== null && sessionCleanRate < 0.6) {
    return 'improve_fix_guidance';
  }
  return `keep_${entry.promotion_status ?? 'unspecified'}`;
}

export function buildSignalScorecard({
  repoLabel = null,
  defectReport,
  reviewVerdicts = null,
  remediationReport = null,
  benchmark = null,
  sessionTelemetry = null,
}) {
  const signalMap = buildSeededEntries(defectReport);
  applyReviewVerdicts(signalMap, reviewVerdicts);
  applyRemediationResults(signalMap, remediationReport);
  applySessionTelemetry(signalMap, sessionTelemetry);
  const latencyMs = inferLatencyMs(benchmark);

  const signals = [...signalMap.values()]
    .map((entry) => {
      const reviewedTotal = entry.reviewed_total ?? 0;
      const truePositive = entry.true_positive ?? 0;
      const acceptableWarning = entry.acceptable_warning ?? 0;
      const falsePositive = entry.false_positive ?? 0;
      const inconclusive = entry.inconclusive ?? 0;
      const remediationTotal = entry.remediation_total ?? 0;
      const remediationSuccess = entry.remediation_success ?? 0;
      const remediationRegressions = entry.remediation_regressions ?? 0;
      const sessionTopActions = entry.session_top_actions ?? 0;
      const sessionFollowups = entry.session_followups ?? 0;
      const sessionCleared = entry.session_cleared ?? 0;
      const sessionRegressions = entry.session_regressions ?? 0;
      const sessionClean = entry.session_clean ?? 0;
      const sessionTotalChecksToClear = entry.session_total_checks_to_clear ?? 0;
      const useful = truePositive + acceptableWarning;
      const latencyEligible = entry.seeded_check_supported > 0;

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
        true_positive: truePositive,
        acceptable_warning: acceptableWarning,
        false_positive: falsePositive,
        inconclusive,
        reviewed_precision: safeRatio(useful, reviewedTotal),
        useful_precision: safeRatio(truePositive, reviewedTotal),
        remediation_total: remediationTotal,
        remediation_success: remediationSuccess,
        remediation_regressions: remediationRegressions,
        remediation_success_rate: safeRatio(remediationSuccess, remediationTotal),
        session_top_actions: sessionTopActions,
        session_followups: sessionFollowups,
        session_cleared: sessionCleared,
        session_regressions: sessionRegressions,
        session_resolution_rate: safeRatio(sessionCleared, sessionFollowups),
        session_clean: sessionClean,
        session_clean_rate: safeRatio(sessionClean, sessionTopActions),
        average_checks_to_clear: safeRatio(
          sessionTotalChecksToClear,
          sessionCleared,
        ),
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
    repo_label:
      repoLabel ??
      defectReport.repo_label ??
      reviewVerdicts?.repo ??
      remediationReport?.repo_label ??
      sessionTelemetry?.repo_label ??
      null,
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
  lines.push('');
  lines.push('| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Useful Precision | Remediation Success | Session Resolution | Session Clean Rate | Avg Checks To Clear | Latency | Recommendation |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of scorecard.signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | \`${signal.signal_family}\` | \`${signal.promotion_status}\` | \`${signal.primary_lane ?? 'n/a'}\` | ${signal.seeded_recall ?? 'n/a'} | ${signal.primary_recall ?? 'n/a'} | ${signal.reviewed_precision ?? 'n/a'} | ${signal.useful_precision ?? 'n/a'} | ${signal.remediation_success_rate ?? 'n/a'} | ${signal.session_resolution_rate ?? 'n/a'} | ${signal.session_clean_rate ?? 'n/a'} | ${signal.average_checks_to_clear ?? 'n/a'} | ${signal.latency_ms ?? 'n/a'} | \`${signal.promotion_recommendation}\` |`,
    );
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
