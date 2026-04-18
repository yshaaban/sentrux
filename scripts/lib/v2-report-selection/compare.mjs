import {
  actionKindWeight,
  reportLeveragePriority,
  reportPresentationPriority,
} from '../signal-policy.mjs';
import {
  candidateBooleanValue,
  candidateFieldValue,
  candidateNumberValue,
} from './normalization.mjs';

const PATCH_WORSENED_KEYS = Object.freeze([
  'patch_directly_worsened',
  'patch_worsened',
  'current_patch_worsened',
  'introduced_by_patch',
  'session_introduced',
]);

const TREATMENT_NET_VALUE_DELTA_KEYS = Object.freeze([
  'signal_treatment_intervention_net_value_score_delta',
  'intervention_net_value_score_delta',
]);

const PATCH_EXPANSION_COST_KEYS = Object.freeze([
  'patch_expansion_cost',
  'intervention_cost_checks_mean',
]);

const REPAIR_PACKET_FIX_SURFACE_RATE_KEYS = Object.freeze([
  'repair_packet_fix_surface_clear_rate',
  'repair_packet_fix_surface_clarity_rate',
]);

function severityPriority(severity) {
  switch (severity) {
    case 'high':
      return 0;
    case 'medium':
      return 1;
    case 'low':
      return 2;
    default:
      return 3;
  }
}

function hasZeroActionWeight(candidate) {
  return actionKindWeight(candidate?.kind ?? '') === 0;
}

function promotionStatusPriority(status) {
  switch (status) {
    case 'trusted':
      return 0;
    case 'watchpoint':
      return 1;
    case 'experimental':
      return 2;
    default:
      return 3;
  }
}

function numericValue(value) {
  return Number.isFinite(value) ? value : null;
}

function compareOptionalNumberDesc(left, right) {
  const leftValue = numericValue(left);
  const rightValue = numericValue(right);
  if (leftValue === null && rightValue === null) {
    return 0;
  }
  if (leftValue === null) {
    return 1;
  }
  if (rightValue === null) {
    return -1;
  }

  return rightValue - leftValue;
}

function compareOptionalNumberAsc(left, right) {
  const leftValue = numericValue(left);
  const rightValue = numericValue(right);
  if (leftValue === null && rightValue === null) {
    return 0;
  }
  if (leftValue === null) {
    return 1;
  }
  if (rightValue === null) {
    return -1;
  }

  return leftValue - rightValue;
}

function compareBooleanTrueFirst(left, right) {
  return Number(Boolean(right)) - Number(Boolean(left));
}

function candidateNumber(candidate, keys) {
  for (const key of keys) {
    const value = candidateNumberValue(candidate, key);
    if (value !== null) {
      return value;
    }
  }

  return null;
}

function candidateBoolean(candidate, keys) {
  for (const key of keys) {
    const value = candidateBooleanValue(candidate, key);
    if (value !== null) {
      return value;
    }
  }

  return false;
}

function defaultRolloutReady(candidate) {
  return candidateFieldValue(candidate, 'default_rollout_recommendation') === 'ready_for_default_on';
}

function compareEvidenceMetrics(left, right) {
  return (
    compareBooleanTrueFirst(
      defaultRolloutReady(left),
      defaultRolloutReady(right),
    ) ||
    compareBooleanTrueFirst(
      candidateBoolean(left, ['signal_treatment_ready']),
      candidateBoolean(right, ['signal_treatment_ready']),
    ) ||
    compareBooleanTrueFirst(
      candidateBoolean(left, PATCH_WORSENED_KEYS),
      candidateBoolean(right, PATCH_WORSENED_KEYS),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, TREATMENT_NET_VALUE_DELTA_KEYS),
      candidateNumber(right, TREATMENT_NET_VALUE_DELTA_KEYS),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['top_action_help_rate']),
      candidateNumber(right, ['top_action_help_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['top_action_follow_rate']),
      candidateNumber(right, ['top_action_follow_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['reviewer_acceptance_rate']),
      candidateNumber(right, ['reviewer_acceptance_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['remediation_success_rate']),
      candidateNumber(right, ['remediation_success_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['task_success_rate']),
      candidateNumber(right, ['task_success_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['intervention_net_value_score']),
      candidateNumber(right, ['intervention_net_value_score']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['reviewed_precision']),
      candidateNumber(right, ['reviewed_precision']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['top_1_actionable_precision']),
      candidateNumber(right, ['top_1_actionable_precision']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['top_3_actionable_precision']),
      candidateNumber(right, ['top_3_actionable_precision']),
    ) ||
    compareOptionalNumberAsc(
      candidateNumber(left, ['reviewer_disagreement_rate']),
      candidateNumber(right, ['reviewer_disagreement_rate']),
    ) ||
    compareOptionalNumberAsc(
      candidateNumber(left, ['patch_expansion_rate']),
      candidateNumber(right, ['patch_expansion_rate']),
    ) ||
    compareOptionalNumberAsc(
      candidateNumber(left, PATCH_EXPANSION_COST_KEYS),
      candidateNumber(right, PATCH_EXPANSION_COST_KEYS),
    ) ||
    compareOptionalNumberAsc(
      candidateNumber(left, ['review_noise_rate']),
      candidateNumber(right, ['review_noise_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['repair_packet_complete_rate']),
      candidateNumber(right, ['repair_packet_complete_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, REPAIR_PACKET_FIX_SURFACE_RATE_KEYS),
      candidateNumber(right, REPAIR_PACKET_FIX_SURFACE_RATE_KEYS),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['repair_packet_verification_clear_rate']),
      candidateNumber(right, ['repair_packet_verification_clear_rate']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['session_verdict_count']),
      candidateNumber(right, ['session_verdict_count']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['reviewed_total']),
      candidateNumber(right, ['reviewed_total']),
    ) ||
    compareOptionalNumberDesc(
      candidateNumber(left, ['session_trial_count']),
      candidateNumber(right, ['session_trial_count']),
    ) ||
    promotionStatusPriority(left.promotion_status) -
      promotionStatusPriority(right.promotion_status)
  );
}

function compareCandidates(left, right) {
  return (
    Number(hasZeroActionWeight(left)) - Number(hasZeroActionWeight(right)) ||
    reportLeveragePriority(left.leverage_class) - reportLeveragePriority(right.leverage_class) ||
    right.within_bucket_strength_0_10000 - left.within_bucket_strength_0_10000 ||
    compareEvidenceMetrics(left, right) ||
    severityPriority(left.severity) - severityPriority(right.severity) ||
    reportPresentationPriority(left.presentation_class) -
      reportPresentationPriority(right.presentation_class) ||
    right.cluster_signal_count - left.cluster_signal_count ||
    Number(right.hotspot_overlap) - Number(left.hotspot_overlap) ||
    right.score_0_10000 - left.score_0_10000 ||
    left.scope.localeCompare(right.scope)
  );
}

function sortCandidates(candidates) {
  return [...candidates].sort(compareCandidates);
}

function uniqueByScope(candidates) {
  const seenScopes = new Set();
  const unique = [];

  for (const candidate of candidates) {
    if (seenScopes.has(candidate.scope)) {
      continue;
    }

    seenScopes.add(candidate.scope);
    unique.push(candidate);
  }

  return unique;
}

export {
  compareCandidates,
  compareEvidenceMetrics,
  hasZeroActionWeight,
  severityPriority,
  sortCandidates,
  uniqueByScope,
};
