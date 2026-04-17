function formatPrimaryTargetPolicy(policyState) {
  if (policyState === null) {
    return 'insufficient evidence';
  }
  if (policyState) {
    return 'pass';
  }

  return 'fail';
}

function formatMetricWithCoverage(metric, numerator, denominator) {
  if (metric === null || metric === undefined) {
    return 'n/a';
  }
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return String(metric);
  }

  return `${metric} (${numerator}/${denominator})`;
}

export function formatSignalScorecardMarkdown(scorecard) {
  const lines = [];
  lines.push('# Signal Quality Scorecard');
  lines.push('');
  appendOverviewLines(lines, scorecard);
  appendKpiLines(lines, scorecard.summary.kpis);
  appendRankingQualityLines(lines, scorecard.summary.ranking_quality);
  appendSessionHealthLines(lines, scorecard.summary.session_health);
  appendSignalTable(lines, scorecard.signals);

  lines.push('');
  return `${lines.join('\n')}\n`;
}

function appendOverviewLines(lines, scorecard) {
  lines.push(`- repo: \`${scorecard.repo_label ?? 'unknown'}\``);
  lines.push(`- generated at: \`${scorecard.generated_at}\``);
  lines.push(`- signals: ${scorecard.summary.total_signals}`);
  lines.push(`- trusted: ${scorecard.summary.trusted_count}`);
  lines.push(`- watchpoint: ${scorecard.summary.watchpoint_count}`);
  lines.push(`- needs review: ${scorecard.summary.needs_review_count}`);
  lines.push(`- degrade or quarantine: ${scorecard.summary.degrade_count}`);
  lines.push(
    `- complete promotion evidence: ${scorecard.summary.promotion_evidence_complete_count ?? 0}`,
  );
}

function appendKpiLines(lines, kpis) {
  if (!kpis) {
    return;
  }

  lines.push(`- seeded samples: ${kpis.defect_sample_count ?? 0}`);
  lines.push(`- reviewed samples: ${kpis.review_sample_count ?? 0}`);
  lines.push(`- provisional reviewed samples: ${kpis.provisional_review_sample_count ?? 0}`);
  lines.push(`- remediation samples: ${kpis.remediation_sample_count ?? 0}`);
  lines.push(`- sessions: ${kpis.session_count ?? 0}`);
  lines.push(`- actionable reviewed samples: ${kpis.actionable_review_sample_count ?? 0}`);
}

function appendRankingQualityLines(lines, rankingQuality) {
  if (!rankingQuality) {
    return;
  }

  lines.push(
    `- top-1 actionable precision: ${formatMetricWithCoverage(
      rankingQuality.top_1_actionable_precision,
      rankingQuality.top_1_actionable_count,
      rankingQuality.top_1_reviewed_count,
    )}`,
  );
  lines.push(
    `- top-3 actionable precision: ${formatMetricWithCoverage(
      rankingQuality.top_3_actionable_precision,
      rankingQuality.top_3_actionable_count,
      rankingQuality.top_3_reviewed_count,
    )}`,
  );
  lines.push(
    `- top-10 actionable precision: ${formatMetricWithCoverage(
      rankingQuality.top_10_actionable_precision,
      rankingQuality.top_10_actionable_count,
      rankingQuality.top_10_reviewed_count,
    )}`,
  );
  lines.push(
    `- ranking preference satisfaction: ${rankingQuality.ranking_preference_satisfaction_rate ?? 'n/a'}`,
  );
  lines.push(
    `- primary-target policy: ${formatPrimaryTargetPolicy(
      rankingQuality.meets_primary_target_policy,
    )}`,
  );
}

function appendSessionHealthLines(lines, sessionHealth) {
  if (!sessionHealth) {
    return;
  }

  lines.push(`- thrashing sessions: ${sessionHealth.thrashing_session_count ?? 0}`);
  lines.push(`- top-action sessions: ${sessionHealth.top_action_session_count ?? 0}`);
  lines.push(
    `- agent clear rate: ${formatMetricWithCoverage(
      sessionHealth.agent_clear_rate,
      sessionHealth.top_action_cleared_count,
      sessionHealth.top_action_session_count,
    )}`,
  );
  lines.push(
    `- follow-up regression session rate: ${formatMetricWithCoverage(
      sessionHealth.followup_regression_session_rate,
      sessionHealth.followup_regression_count,
      sessionHealth.top_action_session_count,
    )}`,
  );
  lines.push(
    `- regression-after-fix rate: ${formatMetricWithCoverage(
      sessionHealth.regression_after_fix_rate,
      sessionHealth.reopened_top_action_count,
      sessionHealth.top_action_session_count,
    )}`,
  );
  lines.push(
    `- session clean rate: ${formatMetricWithCoverage(
      sessionHealth.session_clean_rate,
      sessionHealth.session_clean_count,
      sessionHealth.top_action_session_count,
    )}`,
  );
  lines.push(`- session thrash rate: ${sessionHealth.session_thrash_rate ?? 'n/a'}`);
  lines.push(`- average checks to clear: ${sessionHealth.average_checks_to_clear ?? 'n/a'}`);
  lines.push(`- average entropy delta: ${sessionHealth.average_entropy_delta ?? 'n/a'}`);
}

function appendSignalTable(lines, signals) {
  lines.push('');
  lines.push('| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Noise Rate | Remediation Success | Trials | Top Action Sessions | Trial Miss Rate | Top Action Clear | Regression Rate | Session Clean Rate | Thrash Rate | Avg Entropy Delta | Avg Checks To Clear | Latency | Recommendation |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | \`${signal.signal_family}\` | \`${signal.promotion_status}\` | \`${signal.primary_lane ?? 'n/a'}\` | ${signal.seeded_recall ?? 'n/a'} | ${signal.primary_recall ?? 'n/a'} | ${signal.reviewed_precision ?? 'n/a'} | ${signal.review_noise_rate ?? 'n/a'} | ${signal.remediation_success_rate ?? 'n/a'} | ${signal.session_trial_count ?? 0} | ${signal.top_action_sessions ?? 0} | ${signal.session_trial_miss_rate ?? 'n/a'} | ${signal.top_action_clear_rate ?? 'n/a'} | ${signal.followup_regression_rate ?? 'n/a'} | ${signal.session_clean_rate ?? 'n/a'} | ${signal.session_thrash_rate ?? 'n/a'} | ${signal.average_entropy_delta ?? 'n/a'} | ${signal.average_checks_to_clear ?? 'n/a'} | ${signal.latency_ms ?? 'n/a'} | \`${signal.promotion_recommendation}\` |`,
    );
  }
}
