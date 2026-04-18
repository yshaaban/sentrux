import {
  asArray,
} from './signal-summary-utils.mjs';
import { buildSessionVerdictSummary } from './session-verdicts.mjs';
import {
  buildExperimentArmSummaries,
  buildFocusAreaSummaries,
  selectReviewQueue,
  buildTopActionFailureSummary,
} from './session-corpus.mjs';

const PROMOTION_THRESHOLDS = Object.freeze({
  reviewed_precision: 0.75,
  top_1_actionable_precision: 0.5,
  top_3_actionable_precision: 0.66,
  remediation_success_rate: 0.5,
  session_clean_rate: 0.5,
  session_trial_miss_rate: 0.25,
});

const DEMOTION_THRESHOLDS = Object.freeze({
  review_noise_rate: 0.3,
  top_1_actionable_precision: 0.4,
  top_3_actionable_precision: 0.5,
  session_clean_rate: 0.4,
  session_trial_miss_rate: 0.4,
});

function numericOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function passesPromotionThresholds(signal) {
  const reviewedPrecision = numericOrNull(signal.reviewed_precision);
  const top1 = numericOrNull(signal.top_1_actionable_precision);
  const top3 = numericOrNull(signal.top_3_actionable_precision);
  const remediationSuccess = numericOrNull(signal.remediation_success_rate);
  const sessionClean = numericOrNull(signal.session_clean_rate);
  const sessionTrialMiss = numericOrNull(signal.session_trial_miss_rate);

  return (
    reviewedPrecision !== null &&
    reviewedPrecision >= PROMOTION_THRESHOLDS.reviewed_precision &&
    top1 !== null &&
    top1 >= PROMOTION_THRESHOLDS.top_1_actionable_precision &&
    top3 !== null &&
    top3 >= PROMOTION_THRESHOLDS.top_3_actionable_precision &&
    remediationSuccess !== null &&
    remediationSuccess >= PROMOTION_THRESHOLDS.remediation_success_rate &&
    sessionClean !== null &&
    sessionClean >= PROMOTION_THRESHOLDS.session_clean_rate &&
    sessionTrialMiss !== null &&
    sessionTrialMiss <= PROMOTION_THRESHOLDS.session_trial_miss_rate
  );
}

function violatesTrustedThresholds(signal) {
  const reviewNoise = numericOrNull(signal.review_noise_rate);
  const top1 = numericOrNull(signal.top_1_actionable_precision);
  const top3 = numericOrNull(signal.top_3_actionable_precision);
  const sessionClean = numericOrNull(signal.session_clean_rate);
  const sessionTrialMiss = numericOrNull(signal.session_trial_miss_rate);

  return (
    (reviewNoise !== null && reviewNoise > DEMOTION_THRESHOLDS.review_noise_rate) ||
    (top1 !== null && top1 < DEMOTION_THRESHOLDS.top_1_actionable_precision) ||
    (top3 !== null && top3 < DEMOTION_THRESHOLDS.top_3_actionable_precision) ||
    (sessionClean !== null && sessionClean < DEMOTION_THRESHOLDS.session_clean_rate) ||
    (sessionTrialMiss !== null && sessionTrialMiss > DEMOTION_THRESHOLDS.session_trial_miss_rate)
  );
}

function buildPromotionCandidates(scorecard) {
  return asArray(scorecard?.signals)
    .filter(function isNonTrusted(signal) {
      return signal.promotion_status !== 'trusted';
    })
    .filter(passesPromotionThresholds)
    .map(function toCandidate(signal) {
      return {
        signal_kind: signal.signal_kind,
        promotion_status: signal.promotion_status,
        reviewed_precision: signal.reviewed_precision ?? null,
        top_1_actionable_precision: signal.top_1_actionable_precision ?? null,
        top_3_actionable_precision: signal.top_3_actionable_precision ?? null,
        remediation_success_rate: signal.remediation_success_rate ?? null,
        session_clean_rate: signal.session_clean_rate ?? null,
        session_trial_miss_rate: signal.session_trial_miss_rate ?? null,
      };
    })
    .sort(function compareCandidates(left, right) {
      return (right.top_1_actionable_precision ?? 0) - (left.top_1_actionable_precision ?? 0);
    });
}

function buildDemotionCandidates(scorecard) {
  return asArray(scorecard?.signals)
    .filter(function isTrusted(signal) {
      return signal.promotion_status === 'trusted';
    })
    .filter(violatesTrustedThresholds)
    .map(function toCandidate(signal) {
      return {
        signal_kind: signal.signal_kind,
        review_noise_rate: signal.review_noise_rate ?? null,
        top_1_actionable_precision: signal.top_1_actionable_precision ?? null,
        top_3_actionable_precision: signal.top_3_actionable_precision ?? null,
        session_clean_rate: signal.session_clean_rate ?? null,
        session_trial_miss_rate: signal.session_trial_miss_rate ?? null,
      };
    })
    .sort(function compareCandidates(left, right) {
      return (right.review_noise_rate ?? 0) - (left.review_noise_rate ?? 0);
    });
}

function buildRankingMisses(backlog) {
  return asArray(backlog?.weak_signals)
    .map(function toRankingMiss(signal) {
      return {
        signal_kind: signal.signal_kind,
        recommendation: signal.recommendation ?? null,
        expected_missing_count: signal.expected_missing_count ?? 0,
        expected_present_not_top_count: signal.expected_present_not_top_count ?? 0,
        crowded_out_expected_count: signal.crowded_out_expected_count ?? 0,
        unexpected_top_action_count: signal.unexpected_top_action_count ?? 0,
        session_trial_miss_rate: signal.session_trial_miss_rate ?? null,
      };
    })
    .sort(function compareSignals(left, right) {
      if (right.expected_present_not_top_count !== left.expected_present_not_top_count) {
        return right.expected_present_not_top_count - left.expected_present_not_top_count;
      }
      if (right.expected_missing_count !== left.expected_missing_count) {
        return right.expected_missing_count - left.expected_missing_count;
      }

      return left.signal_kind.localeCompare(right.signal_kind);
    })
    .slice(0, 10);
}

function corpusSessions(sessionCorpus) {
  return asArray(sessionCorpus?.sessions);
}

function buildCorpusRollups(sessionCorpus) {
  const sessions = corpusSessions(sessionCorpus);
  if (sessions.length === 0) {
    return {
      reviewQueue: asArray(sessionCorpus?.review_queue),
      focusAreaSummaries: asArray(sessionCorpus?.focus_area_summaries),
      topActionFailureSummary: asArray(sessionCorpus?.top_action_failure_summary),
      experimentArms: asArray(sessionCorpus?.experiment_arm_summaries),
    };
  }

  return {
    reviewQueue: selectReviewQueue(sessions),
    focusAreaSummaries: buildFocusAreaSummaries(sessions),
    topActionFailureSummary: buildTopActionFailureSummary(sessions),
    experimentArms: buildExperimentArmSummaries(sessions),
  };
}

function buildProductValueSummary(sessionCorpus) {
  const summary =
    corpusSessions(sessionCorpus).length > 0
      ? buildSessionVerdictSummary(corpusSessions(sessionCorpus))
      : sessionCorpus?.summary ?? {};
  if ((summary.session_verdict_count ?? 0) === 0) {
    return null;
  }

  return {
    session_verdict_count: summary.session_verdict_count ?? 0,
    top_action_follow_rate: summary.top_action_follow_rate ?? null,
    top_action_help_rate: summary.top_action_help_rate ?? null,
    task_success_rate: summary.task_success_rate ?? null,
    patch_expansion_rate: summary.patch_expansion_rate ?? null,
    intervention_cost_checks_mean: summary.intervention_cost_checks_mean ?? null,
    intervention_net_value_score: summary.intervention_net_value_score ?? null,
  };
}

function selectCorpusEntries(sessionCorpus, predicate) {
  return corpusSessions(sessionCorpus).filter(predicate).slice(0, 10);
}

function selectFocusAreaExamples(sessionCorpus, focusArea) {
  return selectCorpusEntries(sessionCorpus, function matchesFocusArea(entry) {
    return entry.focus_areas.includes(focusArea) && entry.outcome_bucket !== 'clean';
  });
}

function selectThrashingExamples(sessionCorpus) {
  return selectCorpusEntries(sessionCorpus, function isThrashing(entry) {
    return entry.outcome_bucket === 'thrashing' || entry.outcome_bucket === 'regressed';
  });
}

function appendSummarySection(lines, title, entries, formatter) {
  if (entries.length === 0) {
    return;
  }

  lines.push(`## ${title}`);
  lines.push('');
  for (const entry of entries) {
    lines.push(`- ${formatter(entry)}`);
  }
  lines.push('');
}

function focusAreaCountsToText(focusAreaCounts) {
  if (!Array.isArray(focusAreaCounts) || focusAreaCounts.length === 0) {
    return 'none';
  }

  return focusAreaCounts
    .map(function formatFocusAreaCount(entry) {
      return `${entry.focus_area}:${entry.session_count}`;
    })
    .join(', ');
}

export function buildEvidenceReview({
  scorecard = null,
  backlog = null,
  sessionCorpus = null,
  reviewPacket = null,
}) {
  const promotionCandidates = buildPromotionCandidates(scorecard);
  const demotionCandidates = buildDemotionCandidates(scorecard);
  const rankingMisses = buildRankingMisses(backlog);
  const {
    reviewQueue,
    focusAreaSummaries,
    topActionFailureSummary,
    experimentArms,
  } = buildCorpusRollups(sessionCorpus);
  const productValueSummary = buildProductValueSummary(sessionCorpus);
  const propagationExamples = selectFocusAreaExamples(sessionCorpus, 'propagation');
  const cloneExamples = selectFocusAreaExamples(sessionCorpus, 'clone_followthrough');
  const thrashingExamples = selectThrashingExamples(sessionCorpus);
  const reviewPacketSampleCount =
    reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0;

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label:
      scorecard?.repo_label ?? backlog?.repo_label ?? sessionCorpus?.repo_label ?? null,
    summary: {
      promotion_candidate_count: promotionCandidates.length,
      demotion_candidate_count: demotionCandidates.length,
      ranking_miss_count: rankingMisses.length,
      review_queue_count: reviewQueue.length,
      review_packet_sample_count: reviewPacketSampleCount,
      focus_area_count: focusAreaSummaries.length,
      top_action_failure_count: topActionFailureSummary.length,
      experiment_arm_count: experimentArms.length,
      session_verdict_count: productValueSummary?.session_verdict_count ?? 0,
    },
    promotion_candidates: promotionCandidates,
    demotion_candidates: demotionCandidates,
    ranking_misses: rankingMisses,
    focus_area_summaries: focusAreaSummaries,
    top_action_failure_summary: topActionFailureSummary,
    propagation_examples: propagationExamples,
    clone_examples: cloneExamples,
    thrashing_examples: thrashingExamples,
    experiment_arms: experimentArms,
    product_value: productValueSummary,
  };
}

export function formatEvidenceReviewMarkdown(review) {
  const lines = [];
  lines.push('# Weekly Evidence Review');
  lines.push('');
  lines.push(`- repo: \`${review.repo_label ?? 'unknown'}\``);
  lines.push(`- generated at: \`${review.generated_at}\``);
  lines.push(
    `- promotion candidates: ${review.summary.promotion_candidate_count ?? 0}`,
  );
  lines.push(`- demotion candidates: ${review.summary.demotion_candidate_count ?? 0}`);
  lines.push(`- ranking misses: ${review.summary.ranking_miss_count ?? 0}`);
  lines.push(`- session review queue: ${review.summary.review_queue_count ?? 0}`);
  lines.push(`- focus areas: ${review.summary.focus_area_count ?? 0}`);
  lines.push(`- top action failures: ${review.summary.top_action_failure_count ?? 0}`);
  lines.push(`- experiment arms: ${review.summary.experiment_arm_count ?? 0}`);
  if (review.product_value) {
    lines.push(`- session verdicts: ${review.product_value.session_verdict_count ?? 0}`);
    lines.push(
      `- top-action follow rate: ${review.product_value.top_action_follow_rate ?? 'n/a'}`,
    );
    lines.push(
      `- top-action help rate: ${review.product_value.top_action_help_rate ?? 'n/a'}`,
    );
    lines.push(`- task success rate: ${review.product_value.task_success_rate ?? 'n/a'}`);
    lines.push(
      `- patch expansion rate: ${review.product_value.patch_expansion_rate ?? 'n/a'}`,
    );
    lines.push(
      `- intervention cost checks mean: ${review.product_value.intervention_cost_checks_mean ?? 'n/a'}`,
    );
    lines.push(
      `- intervention net value score: ${review.product_value.intervention_net_value_score ?? 'n/a'}`,
    );
  }
  lines.push('');

  appendSummarySection(lines, 'Promotion Candidates', review.promotion_candidates, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: reviewed=${entry.reviewed_precision ?? 'n/a'}, top1=${entry.top_1_actionable_precision ?? 'n/a'}, top3=${entry.top_3_actionable_precision ?? 'n/a'}, remediation=${entry.remediation_success_rate ?? 'n/a'}, clean=${entry.session_clean_rate ?? 'n/a'}, miss=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendSummarySection(lines, 'Demotion Candidates', review.demotion_candidates, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: noise=${entry.review_noise_rate ?? 'n/a'}, top1=${entry.top_1_actionable_precision ?? 'n/a'}, top3=${entry.top_3_actionable_precision ?? 'n/a'}, clean=${entry.session_clean_rate ?? 'n/a'}, miss=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendSummarySection(lines, 'Ranking Misses', review.ranking_misses, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: missing=${entry.expected_missing_count}, present_not_top=${entry.expected_present_not_top_count}, crowded=${entry.crowded_out_expected_count}, unexpected_top=${entry.unexpected_top_action_count}, miss_rate=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendSummarySection(lines, 'Focus Area Rollups', review.focus_area_summaries, function formatEntry(entry) {
    return `\`${entry.focus_area}\`: sessions=${entry.session_count}, review=${entry.review_queue_count}, clear=${entry.top_action_cleared_count}, miss=${entry.missed_expected_signal_count}, misrank=${entry.expected_signal_present_not_top_count}, escape=${entry.escape_rate ?? 'n/a'}`;
  });
  appendSummarySection(lines, 'Top Action Failures', review.top_action_failure_summary, function formatEntry(entry) {
    return `\`${entry.outcome_bucket}\`: sessions=${entry.session_count}, review=${entry.review_queue_count}, focus=[${focusAreaCountsToText(entry.focus_area_counts)}]`;
  });
  appendSummarySection(lines, 'Propagation Examples', review.propagation_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}`;
  });
  appendSummarySection(lines, 'Clone Examples', review.clone_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}`;
  });
  appendSummarySection(lines, 'Thrashing Examples', review.thrashing_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, convergence=${entry.outcome.convergence_status ?? 'n/a'}, entropy=${entry.outcome.entropy_delta ?? 'n/a'}, top=${entry.outcome.initial_top_action_kind ?? 'none'}`;
  });
  appendSummarySection(lines, 'Experiment Arms', review.experiment_arms, function formatEntry(entry) {
    return `\`${entry.experiment_arm}\`: sessions=${entry.session_count}, clear=${entry.agent_clear_rate ?? 'n/a'}, clean=${entry.clean_rate ?? 'n/a'}, regressions=${entry.regression_rate ?? 'n/a'}, review=${entry.review_queue_rate ?? 'n/a'}, follow=${entry.top_action_follow_rate ?? 'n/a'}, help=${entry.top_action_help_rate ?? 'n/a'}, success=${entry.task_success_rate ?? 'n/a'}, expand=${entry.patch_expansion_rate ?? 'n/a'}, value=${entry.intervention_net_value_score ?? 'n/a'}, focus=[${focusAreaCountsToText(entry.focus_area_counts)}]`;
  });

  return `${lines.join('\n')}\n`;
}
