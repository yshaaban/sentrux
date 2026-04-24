import { asArray } from './signal-summary-utils.mjs';
import { buildSessionVerdictSummary } from './session-verdicts.mjs';
import {
  buildExperimentArmSummaries,
  buildExperimentArmComparisons,
  buildFocusAreaSummaries,
  buildSignalExperimentSummaries,
  buildSignalExperimentComparisons,
  selectReviewQueue,
  buildTopActionFailureSummary,
} from './session-corpus.mjs';
import {
  comparisonQualifiesForDefaultRollout,
  SIGNAL_DEFAULT_ROLLOUT_POLICY,
  SIGNAL_PRIMARY_TARGET_POLICY,
  SIGNAL_PROMOTION_POLICY,
} from './signal-calibration-policy.mjs';

const SESSION_TRIAL_MISS_PROMOTION_MAX = 0.25;
const SESSION_TRIAL_MISS_DEMOTION_MAX = 0.4;
const DEMOTION_TOP1_ACTIONABLE_PRECISION_MIN = 0.4;
const DEMOTION_TOP3_ACTIONABLE_PRECISION_MIN = 0.5;

function numericOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function readOutcomeMetrics(signal) {
  return {
    topActionFollowRate: numericOrNull(signal.top_action_follow_rate),
    topActionHelpRate: numericOrNull(signal.top_action_help_rate),
    taskSuccessRate: numericOrNull(signal.task_success_rate),
    patchExpansionRate: numericOrNull(signal.patch_expansion_rate),
    interventionNetValueScore: numericOrNull(signal.intervention_net_value_score),
  };
}

function buildOutcomeEvidenceFields(signal) {
  const outcomeMetrics = readOutcomeMetrics(signal);

  return {
    session_verdict_count: signal.session_verdict_count ?? 0,
    top_action_follow_rate: outcomeMetrics.topActionFollowRate,
    top_action_help_rate: outcomeMetrics.topActionHelpRate,
    task_success_rate: outcomeMetrics.taskSuccessRate,
    patch_expansion_rate: outcomeMetrics.patchExpansionRate,
    intervention_net_value_score: outcomeMetrics.interventionNetValueScore,
  };
}

function hasVerdictEvidence(signal) {
  return (signal.session_verdict_count ?? 0) >= SIGNAL_PROMOTION_POLICY.sessionVerdictMinSamples;
}

function passesOutcomeThresholds(signal) {
  const outcomeMetrics = readOutcomeMetrics(signal);

  return (
    outcomeMetrics.topActionFollowRate !== null &&
    outcomeMetrics.topActionFollowRate >= SIGNAL_PROMOTION_POLICY.topActionFollowRateMin &&
    outcomeMetrics.topActionHelpRate !== null &&
    outcomeMetrics.topActionHelpRate >= SIGNAL_PROMOTION_POLICY.topActionHelpRateMin &&
    outcomeMetrics.taskSuccessRate !== null &&
    outcomeMetrics.taskSuccessRate >= SIGNAL_PROMOTION_POLICY.taskSuccessRateMin &&
    outcomeMetrics.patchExpansionRate !== null &&
    outcomeMetrics.patchExpansionRate <= SIGNAL_PROMOTION_POLICY.patchExpansionRateMax &&
    outcomeMetrics.interventionNetValueScore !== null &&
    outcomeMetrics.interventionNetValueScore >=
      SIGNAL_PROMOTION_POLICY.interventionNetValueScoreMin
  );
}

function violatesOutcomeThresholds(signal) {
  const outcomeMetrics = readOutcomeMetrics(signal);

  return (
    (outcomeMetrics.topActionFollowRate !== null &&
      outcomeMetrics.topActionFollowRate < SIGNAL_PROMOTION_POLICY.topActionFollowRateMin) ||
    (outcomeMetrics.topActionHelpRate !== null &&
      outcomeMetrics.topActionHelpRate < SIGNAL_PROMOTION_POLICY.topActionHelpRateMin) ||
    (outcomeMetrics.taskSuccessRate !== null &&
      outcomeMetrics.taskSuccessRate < SIGNAL_PROMOTION_POLICY.taskSuccessRateMin) ||
    (outcomeMetrics.patchExpansionRate !== null &&
      outcomeMetrics.patchExpansionRate > SIGNAL_PROMOTION_POLICY.patchExpansionRateMax) ||
    (outcomeMetrics.interventionNetValueScore !== null &&
      outcomeMetrics.interventionNetValueScore <
        SIGNAL_PROMOTION_POLICY.interventionNetValueScoreMin)
  );
}

function passesPromotionThresholds(signal) {
  const reviewedPrecision = numericOrNull(signal.reviewed_precision);
  const top1 = numericOrNull(signal.top_1_actionable_precision);
  const top3 = numericOrNull(signal.top_3_actionable_precision);
  const remediationSuccess = numericOrNull(signal.remediation_success_rate);
  const sessionClean = numericOrNull(signal.session_clean_rate);
  const sessionTrialMiss = numericOrNull(signal.session_trial_miss_rate);
  const passesCoreThresholds =
    reviewedPrecision !== null &&
    reviewedPrecision >= SIGNAL_PROMOTION_POLICY.reviewedPrecisionMin &&
    top1 !== null &&
    top1 >= SIGNAL_PRIMARY_TARGET_POLICY.top1ActionablePrecisionMin &&
    top3 !== null &&
    top3 >= SIGNAL_PRIMARY_TARGET_POLICY.top3ActionablePrecisionMin &&
    remediationSuccess !== null &&
    remediationSuccess >= SIGNAL_PROMOTION_POLICY.remediationSuccessMin &&
    sessionClean !== null &&
    sessionClean >= SIGNAL_PROMOTION_POLICY.sessionCleanRateMin &&
    sessionTrialMiss !== null &&
    sessionTrialMiss <= SESSION_TRIAL_MISS_PROMOTION_MAX;

  if (!passesCoreThresholds) {
    return false;
  }

  if (!hasVerdictEvidence(signal)) {
    return true;
  }

  return passesOutcomeThresholds(signal);
}

function violatesTrustedThresholds(signal) {
  const reviewNoise = numericOrNull(signal.review_noise_rate);
  const top1 = numericOrNull(signal.top_1_actionable_precision);
  const top3 = numericOrNull(signal.top_3_actionable_precision);
  const sessionClean = numericOrNull(signal.session_clean_rate);
  const sessionTrialMiss = numericOrNull(signal.session_trial_miss_rate);
  const violatesCoreThresholds =
    (reviewNoise !== null && reviewNoise > SIGNAL_PROMOTION_POLICY.reviewNoiseRateMax) ||
    (top1 !== null && top1 < DEMOTION_TOP1_ACTIONABLE_PRECISION_MIN) ||
    (top3 !== null && top3 < DEMOTION_TOP3_ACTIONABLE_PRECISION_MIN) ||
    (sessionClean !== null && sessionClean < SIGNAL_PROMOTION_POLICY.sessionCleanRateMin) ||
    (sessionTrialMiss !== null && sessionTrialMiss > SESSION_TRIAL_MISS_DEMOTION_MAX);

  if (violatesCoreThresholds) {
    return true;
  }

  if (!hasVerdictEvidence(signal)) {
    return false;
  }

  return violatesOutcomeThresholds(signal);
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
        ...buildOutcomeEvidenceFields(signal),
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
        ...buildOutcomeEvidenceFields(signal),
      };
    })
    .sort(function compareCandidates(left, right) {
      return (right.review_noise_rate ?? 0) - (left.review_noise_rate ?? 0);
    });
}

function buildDefaultOnCandidates(scorecard) {
  return asArray(scorecard?.signals)
    .filter(function isDefaultRolloutCandidate(signal) {
      return (
        signal.default_rollout_recommendation === 'await_treatment_proof' ||
        signal.default_rollout_recommendation === 'ready_for_default_on'
      );
    })
    .map(function toDefaultOnCandidate(signal) {
      return {
        signal_kind: signal.signal_kind,
        signal_family: signal.signal_family ?? null,
        promotion_status: signal.promotion_status ?? null,
        product_primary_lane: signal.product_primary_lane ?? null,
        default_surface_role: signal.default_surface_role ?? null,
        session_verdict_count: signal.session_verdict_count ?? 0,
        top_action_follow_rate: signal.top_action_follow_rate ?? null,
        top_action_help_rate: signal.top_action_help_rate ?? null,
        task_success_rate: signal.task_success_rate ?? null,
        patch_expansion_rate: signal.patch_expansion_rate ?? null,
        reviewer_acceptance_rate: signal.reviewer_acceptance_rate ?? null,
        reviewer_disagreement_rate: signal.reviewer_disagreement_rate ?? null,
        intervention_net_value_score: signal.intervention_net_value_score ?? null,
        promotion_recommendation: signal.promotion_recommendation ?? null,
        default_rollout_recommendation: signal.default_rollout_recommendation ?? null,
        signal_treatment_ready: signal.signal_treatment_ready === true,
        signal_treatment_comparison_count: signal.signal_treatment_comparison_count ?? 0,
        signal_treatment_qualified_comparison_count:
          signal.signal_treatment_qualified_comparison_count ?? 0,
        signal_treatment_best_arm: signal.signal_treatment_best_arm ?? null,
      };
    })
    .sort(function compareCandidates(left, right) {
      return (
        Number(right.signal_treatment_ready) - Number(left.signal_treatment_ready) ||
        (right.top_action_help_rate ?? 0) - (left.top_action_help_rate ?? 0)
      );
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
      experimentArmComparisons: asArray(sessionCorpus?.experiment_arm_comparisons),
      signalExperimentComparisons: asArray(sessionCorpus?.signal_experiment_comparisons),
    };
  }

  const experimentArms = buildExperimentArmSummaries(sessions);
  const signalExperimentSummaries = buildSignalExperimentSummaries(sessions);

  return {
    reviewQueue: selectReviewQueue(sessions),
    focusAreaSummaries: buildFocusAreaSummaries(sessions),
    topActionFailureSummary: buildTopActionFailureSummary(sessions),
    experimentArms,
    experimentArmComparisons: buildExperimentArmComparisons(experimentArms),
    signalExperimentComparisons: buildSignalExperimentComparisons(signalExperimentSummaries),
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
    reviewer_acceptance_rate: summary.reviewer_acceptance_rate ?? null,
    reviewer_disagreement_rate: summary.reviewer_disagreement_rate ?? null,
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

function isQualifiedDefaultOnComparison(entry) {
  return comparisonQualifiesForDefaultRollout(entry);
}

function isQualifiedSignalMatchedComparison(entry) {
  if (!entry) {
    return false;
  }

  return (
    isQualifiedDefaultOnComparison(entry) &&
    entry.session_count >= SIGNAL_DEFAULT_ROLLOUT_POLICY.experimentArmMinComparisons &&
    entry.baseline_session_count >= SIGNAL_DEFAULT_ROLLOUT_POLICY.experimentArmMinComparisons
  );
}

function sortSignalMatchedComparisons(left, right) {
  return (
    (right.top_action_help_rate_delta ?? Number.NEGATIVE_INFINITY) -
      (left.top_action_help_rate_delta ?? Number.NEGATIVE_INFINITY) ||
    (right.task_success_rate_delta ?? Number.NEGATIVE_INFINITY) -
      (left.task_success_rate_delta ?? Number.NEGATIVE_INFINITY) ||
    (right.intervention_net_value_score_delta ?? Number.NEGATIVE_INFINITY) -
      (left.intervention_net_value_score_delta ?? Number.NEGATIVE_INFINITY) ||
    (left.patch_expansion_rate_delta ?? Number.POSITIVE_INFINITY) -
      (right.patch_expansion_rate_delta ?? Number.POSITIVE_INFINITY)
  );
}

function buildSignalMatchedCandidateEvidence(defaultOnCandidates, signalExperimentComparisons) {
  return defaultOnCandidates.map(function toSignalCandidateEvidence(candidate) {
    const comparisons = asArray(signalExperimentComparisons)
      .filter(function matchesSignal(entry) {
        return entry.signal_kind === candidate.signal_kind;
      })
      .sort(sortSignalMatchedComparisons);
    const qualifiedComparisons = comparisons.filter(isQualifiedSignalMatchedComparison);
    const bestComparison = qualifiedComparisons[0] ?? comparisons[0] ?? null;

    return {
      signal_kind: candidate.signal_kind,
      qualified: qualifiedComparisons.length > 0,
      comparison_count: comparisons.length,
      qualified_comparison_count: qualifiedComparisons.length,
      best_treatment_arm: bestComparison?.experiment_arm ?? null,
      deltas: {
        top_action_help_rate: bestComparison?.top_action_help_rate_delta ?? null,
        task_success_rate: bestComparison?.task_success_rate_delta ?? null,
        patch_expansion_rate: bestComparison?.patch_expansion_rate_delta ?? null,
        intervention_net_value_score:
          bestComparison?.intervention_net_value_score_delta ?? null,
      },
      effect_size: bestComparison?.effect_size ?? null,
    };
  });
}

function buildDefaultOnPromotionSummary(
  defaultOnCandidates,
  experimentArmComparisons,
  signalExperimentComparisons,
  scorecard,
) {
  const allComparisons = asArray(experimentArmComparisons);
  const qualifiedComparisons = allComparisons.filter(isQualifiedDefaultOnComparison);
  const bestComparison = qualifiedComparisons[0] ?? allComparisons[0] ?? null;
  const pairedVerdictSampleCount = scorecard?.summary?.kpis?.session_verdict_count ?? 0;
  const hasSignalCandidates = defaultOnCandidates.length > 0;
  const hasVerdictEvidence =
    pairedVerdictSampleCount >= SIGNAL_DEFAULT_ROLLOUT_POLICY.sessionVerdictMinSamples;
  const hasPairedBaseline = allComparisons.length > 0;
  const hasPositiveRepoTreatmentEvidence =
    qualifiedComparisons.length >= SIGNAL_DEFAULT_ROLLOUT_POLICY.experimentArmMinComparisons;
  const candidateSignalEvidence = buildSignalMatchedCandidateEvidence(
    defaultOnCandidates,
    signalExperimentComparisons,
  );
  const signalTreatmentReadyCount = candidateSignalEvidence.filter(function isQualified(entry) {
    return entry.qualified;
  }).length;
  const hasSignalMatchedTreatmentEvidence =
    hasSignalCandidates &&
    candidateSignalEvidence.length > 0 &&
    candidateSignalEvidence.every(function allCandidatesQualified(entry) {
      return entry.qualified;
    });
  const blockers = [];

  if (!hasSignalCandidates) {
    blockers.push('no_signal_candidates');
  }
  if (!hasVerdictEvidence) {
    blockers.push('missing_session_verdicts');
  }
  if (!hasPairedBaseline) {
    blockers.push('missing_paired_baseline');
  } else if (!hasPositiveRepoTreatmentEvidence) {
    blockers.push('missing_positive_treatment_delta');
  }
  if (
    hasSignalCandidates &&
    hasVerdictEvidence &&
    hasPositiveRepoTreatmentEvidence &&
    !hasSignalMatchedTreatmentEvidence
  ) {
    blockers.push('missing_signal_matched_treatment_evidence');
  }

  const evidenceComplete =
    hasSignalCandidates &&
    hasVerdictEvidence &&
    hasPairedBaseline &&
    hasPositiveRepoTreatmentEvidence &&
    hasSignalMatchedTreatmentEvidence;

  return {
    ready: evidenceComplete,
    evidence_complete: evidenceComplete,
    evidence_scope: defaultOnEvidenceScope(signalTreatmentReadyCount, candidateSignalEvidence),
    repo_treatment_ready: hasVerdictEvidence && hasPositiveRepoTreatmentEvidence,
    signal_matched_treatment_evidence: hasSignalMatchedTreatmentEvidence,
    paired_baseline_present: hasPairedBaseline,
    paired_verdict_sample_count: pairedVerdictSampleCount,
    best_treatment_arm: bestComparison?.experiment_arm ?? null,
    qualified_comparison_count: qualifiedComparisons.length,
    signal_matched_comparison_count: asArray(signalExperimentComparisons).length,
    qualified_signal_matched_comparison_count: candidateSignalEvidence.reduce(
      function sumQualifiedComparisons(total, entry) {
        return total + entry.qualified_comparison_count;
      },
      0,
    ),
    signal_treatment_ready_count: signalTreatmentReadyCount,
    candidate_signal_count: defaultOnCandidates.length,
    candidate_signals: defaultOnCandidates.map(function toSignalKind(entry) {
      return entry.signal_kind;
    }),
    candidate_signal_evidence: candidateSignalEvidence,
    deltas: {
      top_action_help_rate: bestComparison?.top_action_help_rate_delta ?? null,
      task_success_rate: bestComparison?.task_success_rate_delta ?? null,
      patch_expansion_rate: bestComparison?.patch_expansion_rate_delta ?? null,
      intervention_net_value_score:
        bestComparison?.intervention_net_value_score_delta ?? null,
    },
    blockers,
  };
}

function defaultOnEvidenceScope(signalTreatmentReadyCount, candidateSignalEvidence) {
  if (signalTreatmentReadyCount === 0) {
    return 'repo_level';
  }

  if (signalTreatmentReadyCount === candidateSignalEvidence.length) {
    return 'signal_level';
  }

  return 'mixed';
}

function nullableBooleanText(value) {
  if (value === null || value === undefined) {
    return 'n/a';
  }

  return value ? 'true' : 'false';
}

function metricField(name, value) {
  return `${name}=${value ?? 'n/a'}`;
}

function formatSignalMetricSummary(signalKind, fields) {
  return `\`${signalKind}\`: ${fields.join(', ')}`;
}

function formatDefaultOnDeltas(deltas) {
  return [
    metricField('help', deltas.top_action_help_rate),
    metricField('success', deltas.task_success_rate),
    metricField('expand', deltas.patch_expansion_rate),
    metricField('value', deltas.intervention_net_value_score),
  ].join(', ');
}

function formatExperimentArmSummary(entry) {
  return `\`${entry.experiment_arm}\`: ${[
    `sessions=${entry.session_count}`,
    metricField('clear', entry.agent_clear_rate),
    metricField('clean', entry.clean_rate),
    metricField('regressions', entry.regression_rate),
    metricField('review', entry.review_queue_rate),
    metricField('follow', entry.top_action_follow_rate),
    metricField('help', entry.top_action_help_rate),
    metricField('success', entry.task_success_rate),
    metricField('expand', entry.patch_expansion_rate),
    metricField('value', entry.intervention_net_value_score),
    `focus=[${focusAreaCountsToText(entry.focus_area_counts)}]`,
  ].join(', ')}`;
}

function formatExperimentArmComparison(entry) {
  return `\`${entry.experiment_arm}\` vs \`${entry.baseline_experiment_arm}\`: ${[
    metricField('clear_delta', entry.agent_clear_rate_delta),
    metricField('help_delta', entry.top_action_help_rate_delta),
    metricField('success_delta', entry.task_success_rate_delta),
    metricField('expand_delta', entry.patch_expansion_rate_delta),
    metricField('value_delta', entry.intervention_net_value_score_delta),
  ].join(', ')}`;
}

function formatSignalExperimentComparison(entry) {
  return `\`${entry.signal_kind}\` / \`${entry.experiment_arm}\` vs \`${entry.baseline_experiment_arm}\`: ${[
    metricField('top_match_delta', entry.expected_top_action_rate_delta),
    metricField('help_delta', entry.top_action_help_rate_delta),
    metricField('success_delta', entry.task_success_rate_delta),
    metricField('expand_delta', entry.patch_expansion_rate_delta),
    metricField('value_delta', entry.intervention_net_value_score_delta),
  ].join(', ')}`;
}

function formatCandidateSignalEvidence(entry) {
  return formatSignalMetricSummary(entry.signal_kind, [
    `qualified=${entry.qualified ? 'true' : 'false'}`,
    `comparisons=${entry.comparison_count}`,
    `qualified_comparisons=${entry.qualified_comparison_count}`,
    metricField('best_arm', entry.best_treatment_arm),
    metricField('help_delta', entry.deltas?.top_action_help_rate),
    metricField('success_delta', entry.deltas?.task_success_rate),
    metricField('expand_delta', entry.deltas?.patch_expansion_rate),
    metricField('value_delta', entry.deltas?.intervention_net_value_score),
  ]);
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
  const defaultOnCandidates = buildDefaultOnCandidates(scorecard);
  const rankingMisses = buildRankingMisses(backlog);
  const {
    reviewQueue,
    focusAreaSummaries,
    topActionFailureSummary,
    experimentArms,
    experimentArmComparisons,
    signalExperimentComparisons,
  } = buildCorpusRollups(sessionCorpus);
  const productValueSummary = buildProductValueSummary(sessionCorpus);
  const propagationExamples = selectFocusAreaExamples(sessionCorpus, 'propagation');
  const cloneExamples = selectFocusAreaExamples(sessionCorpus, 'clone_followthrough');
  const thrashingExamples = selectThrashingExamples(sessionCorpus);
  const defaultOnPromotion = buildDefaultOnPromotionSummary(
    defaultOnCandidates,
    experimentArmComparisons,
    signalExperimentComparisons,
    scorecard,
  );
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
      default_on_candidate_count: defaultOnCandidates.length,
      ranking_miss_count: rankingMisses.length,
      review_queue_count: reviewQueue.length,
      review_packet_sample_count: reviewPacketSampleCount,
      focus_area_count: focusAreaSummaries.length,
      top_action_failure_count: topActionFailureSummary.length,
      experiment_arm_count: experimentArms.length,
      experiment_arm_comparison_count: experimentArmComparisons.length,
      signal_experiment_comparison_count: signalExperimentComparisons.length,
      default_on_ready_signal_count: defaultOnCandidates.filter(function isReady(signal) {
        return signal.default_rollout_recommendation === 'ready_for_default_on';
      }).length,
      session_verdict_count: productValueSummary?.session_verdict_count ?? 0,
      bounded_adjudication_task_count: sessionCorpus?.adjudication_summary?.task_count ?? 0,
      bounded_adjudication_decision_count:
        sessionCorpus?.adjudication_summary?.decision_count ?? 0,
    },
    evidence_sources: sessionCorpus?.evidence_sources ?? { live: null, replay: null },
    phase_tracking: sessionCorpus?.phase_tracking ?? null,
    adjudication_summary: sessionCorpus?.adjudication_summary ?? null,
    promotion_candidates: promotionCandidates,
    demotion_candidates: demotionCandidates,
    default_on_candidates: defaultOnCandidates,
    default_on_promotion: defaultOnPromotion,
    ranking_misses: rankingMisses,
    focus_area_summaries: focusAreaSummaries,
    top_action_failure_summary: topActionFailureSummary,
    propagation_examples: propagationExamples,
    clone_examples: cloneExamples,
    thrashing_examples: thrashingExamples,
    experiment_arms: experimentArms,
    experiment_arm_comparisons: experimentArmComparisons,
    signal_experiment_comparisons: signalExperimentComparisons,
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
  lines.push(
    `- experiment arm comparisons: ${review.summary.experiment_arm_comparison_count ?? 0}`,
  );
  lines.push(
    `- signal-matched comparisons: ${review.summary.signal_experiment_comparison_count ?? 0}`,
  );
  lines.push(`- default-on candidates: ${review.summary.default_on_candidate_count ?? 0}`);
  lines.push(
    `- default-on ready signals: ${review.summary.default_on_ready_signal_count ?? 0}`,
  );
  if (review.adjudication_summary) {
    lines.push(
      `- bounded adjudication status: ${review.adjudication_summary.status ?? 'n/a'}`,
    );
    lines.push(
      `- bounded adjudication decisions: ${review.adjudication_summary.decision_count ?? 0}`,
    );
  }
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
      `- reviewer acceptance rate: ${review.product_value.reviewer_acceptance_rate ?? 'n/a'}`,
    );
    lines.push(
      `- reviewer disagreement rate: ${review.product_value.reviewer_disagreement_rate ?? 'n/a'}`,
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
    return formatSignalMetricSummary(entry.signal_kind, [
      metricField('reviewed', entry.reviewed_precision),
      metricField('top1', entry.top_1_actionable_precision),
      metricField('top3', entry.top_3_actionable_precision),
      metricField('remediation', entry.remediation_success_rate),
      metricField('clean', entry.session_clean_rate),
      metricField('follow', entry.top_action_follow_rate),
      metricField('help', entry.top_action_help_rate),
      metricField('success', entry.task_success_rate),
      metricField('expand', entry.patch_expansion_rate),
      metricField('value', entry.intervention_net_value_score),
      metricField('miss', entry.session_trial_miss_rate),
    ]);
  });
  appendSummarySection(lines, 'Demotion Candidates', review.demotion_candidates, function formatEntry(entry) {
    return formatSignalMetricSummary(entry.signal_kind, [
      metricField('noise', entry.review_noise_rate),
      metricField('top1', entry.top_1_actionable_precision),
      metricField('top3', entry.top_3_actionable_precision),
      metricField('clean', entry.session_clean_rate),
      metricField('follow', entry.top_action_follow_rate),
      metricField('help', entry.top_action_help_rate),
      metricField('success', entry.task_success_rate),
      metricField('expand', entry.patch_expansion_rate),
      metricField('value', entry.intervention_net_value_score),
      metricField('miss', entry.session_trial_miss_rate),
    ]);
  });
  appendSummarySection(lines, 'Default-On Candidates', review.default_on_candidates, function formatEntry(entry) {
    return formatSignalMetricSummary(entry.signal_kind, [
      metricField('lane', entry.product_primary_lane),
      metricField('role', entry.default_surface_role),
      `verdicts=${entry.session_verdict_count ?? 0}`,
      metricField('follow', entry.top_action_follow_rate),
      metricField('help', entry.top_action_help_rate),
      metricField('success', entry.task_success_rate),
      metricField('expand', entry.patch_expansion_rate),
      metricField('accept', entry.reviewer_acceptance_rate),
      metricField('disagree', entry.reviewer_disagreement_rate),
      metricField('value', entry.intervention_net_value_score),
      `treatment=${entry.signal_treatment_ready ? 'ready' : 'pending'}`,
    ]);
  });
  lines.push('## Default-On Promotion');
  lines.push('');
  lines.push(`- ready for default-on: ${review.default_on_promotion.ready ? 'true' : 'false'}`);
  lines.push(
    `- evidence complete: ${review.default_on_promotion.evidence_complete ? 'true' : 'false'}`,
  );
  lines.push(`- evidence scope: ${review.default_on_promotion.evidence_scope ?? 'unknown'}`);
  lines.push(
    `- repo treatment ready: ${review.default_on_promotion.repo_treatment_ready ? 'true' : 'false'}`,
  );
  lines.push(
    `- signal-matched treatment evidence: ${review.default_on_promotion.signal_matched_treatment_evidence ? 'true' : 'false'}`,
  );
  lines.push(
    `- paired baseline present: ${review.default_on_promotion.paired_baseline_present ? 'true' : 'false'}`,
  );
  lines.push(
    `- paired verdict sample count: ${review.default_on_promotion.paired_verdict_sample_count ?? 0}`,
  );
  lines.push(
    `- qualified comparisons: ${review.default_on_promotion.qualified_comparison_count ?? 0}`,
  );
  lines.push(
    `- qualified signal-matched comparisons: ${
      review.default_on_promotion.qualified_signal_matched_comparison_count ?? 0
    }`,
  );
  lines.push(
    `- signal treatment ready count: ${review.default_on_promotion.signal_treatment_ready_count ?? 0}`,
  );
  lines.push(
    `- best treatment arm: ${review.default_on_promotion.best_treatment_arm ?? 'none'}`,
  );
  lines.push(`- deltas: ${formatDefaultOnDeltas(review.default_on_promotion.deltas)}`);
  lines.push(
    `- blockers: ${
      review.default_on_promotion.blockers.length > 0
        ? review.default_on_promotion.blockers.join(', ')
        : 'none'
    }`,
  );
  lines.push('');
  if (review.adjudication_summary) {
    lines.push('## Bounded Adjudication');
    lines.push('');
    lines.push(`- status: ${review.adjudication_summary.status ?? 'n/a'}`);
    lines.push(`- task count: ${review.adjudication_summary.task_count ?? 0}`);
    lines.push(`- decision count: ${review.adjudication_summary.decision_count ?? 0}`);
    lines.push(
      `- structured evidence only: ${nullableBooleanText(
        review.adjudication_summary.structured_evidence_only,
      )}`,
    );
    lines.push(
      `- audit logging ready: ${nullableBooleanText(
        review.adjudication_summary.audit_logging_ready,
      )}`,
    );
    lines.push(
      `- auto-apply enabled: ${nullableBooleanText(
        review.adjudication_summary.auto_apply_enabled,
      )}`,
    );
    lines.push(
      `- recommended model: ${review.adjudication_summary.recommended_model ?? 'n/a'}`,
    );
    lines.push('');
  }
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
    return formatExperimentArmSummary(entry);
  });
  appendSummarySection(
    lines,
    'Experiment Arm Comparisons',
    review.experiment_arm_comparisons,
    function formatEntry(entry) {
      return formatExperimentArmComparison(entry);
    },
  );
  appendSummarySection(
    lines,
    'Signal-Matched Comparisons',
    review.signal_experiment_comparisons ?? [],
    function formatEntry(entry) {
      return formatSignalExperimentComparison(entry);
    },
  );
  appendSummarySection(
    lines,
    'Signal-Matched Default-On Evidence',
    review.default_on_promotion.candidate_signal_evidence ?? [],
    function formatEntry(entry) {
      return formatCandidateSignalEvidence(entry);
    },
  );

  return `${lines.join('\n')}\n`;
}
