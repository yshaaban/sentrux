function formatPrimaryTargetPolicy(policyState) {
  if (policyState === null) {
    return 'insufficient evidence';
  }
  if (policyState) {
    return 'pass';
  }

  return 'fail';
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
  lines.push(
    `- complete promotion evidence: ${scorecard.summary.promotion_evidence_complete_count ?? 0}`,
  );
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
      `- primary-target policy: ${formatPrimaryTargetPolicy(
        scorecard.summary.ranking_quality.meets_primary_target_policy,
      )}`,
    );
  }
  if (scorecard.summary.session_health) {
    lines.push(
      `- thrashing sessions: ${scorecard.summary.session_health.thrashing_session_count ?? 0}`,
    );
    lines.push(
      `- average entropy delta: ${scorecard.summary.session_health.average_entropy_delta ?? 'n/a'}`,
    );
  }
  lines.push('');
  lines.push('| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Noise Rate | Remediation Success | Trials | Top Action Sessions | Trial Miss Rate | Top Action Clear | Regression Rate | Session Clean Rate | Thrash Rate | Avg Entropy Delta | Avg Checks To Clear | Latency | Recommendation |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of scorecard.signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | \`${signal.signal_family}\` | \`${signal.promotion_status}\` | \`${signal.primary_lane ?? 'n/a'}\` | ${signal.seeded_recall ?? 'n/a'} | ${signal.primary_recall ?? 'n/a'} | ${signal.reviewed_precision ?? 'n/a'} | ${signal.review_noise_rate ?? 'n/a'} | ${signal.remediation_success_rate ?? 'n/a'} | ${signal.session_trial_count ?? 0} | ${signal.top_action_sessions ?? 0} | ${signal.session_trial_miss_rate ?? 'n/a'} | ${signal.top_action_clear_rate ?? 'n/a'} | ${signal.followup_regression_rate ?? 'n/a'} | ${signal.session_clean_rate ?? 'n/a'} | ${signal.session_thrash_rate ?? 'n/a'} | ${signal.average_entropy_delta ?? 'n/a'} | ${signal.average_checks_to_clear ?? 'n/a'} | ${signal.latency_ms ?? 'n/a'} | \`${signal.promotion_recommendation}\` |`,
    );
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
