import { asArray } from './signal-summary-utils.mjs';

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

function selectCorpusEntries(corpus, predicate) {
  return asArray(corpus?.sessions).filter(predicate).slice(0, 10);
}

function formatArmRate(numerator, denominator) {
  if (denominator <= 0) {
    return null;
  }

  return Number((numerator / denominator).toFixed(3));
}

function buildExperimentArms(corpus) {
  const arms = new Map();

  for (const session of asArray(corpus?.sessions)) {
    const arm = session.experiment_arm;
    if (!arm) {
      continue;
    }

    if (!arms.has(arm)) {
      arms.set(arm, {
        experiment_arm: arm,
        session_count: 0,
        clean_session_count: 0,
        regression_session_count: 0,
      });
    }

    const entry = arms.get(arm);
    entry.session_count += 1;
    if (session.outcome?.final_session_clean) {
      entry.clean_session_count += 1;
    }
    if (session.outcome?.followup_regression_introduced) {
      entry.regression_session_count += 1;
    }
  }

  return [...arms.values()]
    .map(function finalizeArm(entry) {
      return {
        ...entry,
        clean_rate: formatArmRate(entry.clean_session_count, entry.session_count),
        regression_rate: formatArmRate(entry.regression_session_count, entry.session_count),
      };
    })
    .sort(function compareArms(left, right) {
      return left.experiment_arm.localeCompare(right.experiment_arm);
    });
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
  const reviewQueue = asArray(sessionCorpus?.review_queue);
  const propagationExamples = selectCorpusEntries(sessionCorpus, function isPropagation(entry) {
    return entry.focus_areas.includes('propagation') && entry.outcome_bucket !== 'clean';
  });
  const cloneExamples = selectCorpusEntries(sessionCorpus, function isClone(entry) {
    return (
      entry.focus_areas.includes('clone_followthrough') && entry.outcome_bucket !== 'clean'
    );
  });
  const thrashingExamples = selectCorpusEntries(sessionCorpus, function isThrashing(entry) {
    return entry.outcome_bucket === 'thrashing' || entry.outcome_bucket === 'regressed';
  });
  const experimentArms = buildExperimentArms(sessionCorpus);
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
    },
    promotion_candidates: promotionCandidates,
    demotion_candidates: demotionCandidates,
    ranking_misses: rankingMisses,
    propagation_examples: propagationExamples,
    clone_examples: cloneExamples,
    thrashing_examples: thrashingExamples,
    experiment_arms: experimentArms,
  };
}

function appendEntrySection(lines, title, entries, formatter) {
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
  lines.push('');

  appendEntrySection(lines, 'Promotion Candidates', review.promotion_candidates, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: reviewed=${entry.reviewed_precision ?? 'n/a'}, top1=${entry.top_1_actionable_precision ?? 'n/a'}, top3=${entry.top_3_actionable_precision ?? 'n/a'}, remediation=${entry.remediation_success_rate ?? 'n/a'}, clean=${entry.session_clean_rate ?? 'n/a'}, miss=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendEntrySection(lines, 'Demotion Candidates', review.demotion_candidates, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: noise=${entry.review_noise_rate ?? 'n/a'}, top1=${entry.top_1_actionable_precision ?? 'n/a'}, top3=${entry.top_3_actionable_precision ?? 'n/a'}, clean=${entry.session_clean_rate ?? 'n/a'}, miss=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendEntrySection(lines, 'Ranking Misses', review.ranking_misses, function formatEntry(entry) {
    return `\`${entry.signal_kind}\`: missing=${entry.expected_missing_count}, present_not_top=${entry.expected_present_not_top_count}, crowded=${entry.crowded_out_expected_count}, unexpected_top=${entry.unexpected_top_action_count}, miss_rate=${entry.session_trial_miss_rate ?? 'n/a'}`;
  });
  appendEntrySection(lines, 'Propagation Examples', review.propagation_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}`;
  });
  appendEntrySection(lines, 'Clone Examples', review.clone_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}`;
  });
  appendEntrySection(lines, 'Thrashing Examples', review.thrashing_examples, function formatEntry(entry) {
    return `\`${entry.session_id}\` [${entry.lane}] bucket=${entry.outcome_bucket}, convergence=${entry.outcome.convergence_status ?? 'n/a'}, entropy=${entry.outcome.entropy_delta ?? 'n/a'}, top=${entry.outcome.initial_top_action_kind ?? 'none'}`;
  });
  appendEntrySection(lines, 'Experiment Arms', review.experiment_arms, function formatEntry(entry) {
    return `\`${entry.experiment_arm}\`: sessions=${entry.session_count}, clean=${entry.clean_rate ?? 'n/a'}, regressions=${entry.regression_rate ?? 'n/a'}`;
  });

  return `${lines.join('\n')}\n`;
}
