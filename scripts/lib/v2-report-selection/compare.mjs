import {
  actionKindWeight,
  reportLeveragePriority,
  reportPresentationPriority,
} from '../signal-policy.mjs';

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

function compareCandidates(left, right) {
  return (
    Number(hasZeroActionWeight(left)) - Number(hasZeroActionWeight(right)) ||
    reportLeveragePriority(left.leverage_class) - reportLeveragePriority(right.leverage_class) ||
    right.within_bucket_strength_0_10000 - left.within_bucket_strength_0_10000 ||
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

export { compareCandidates, hasZeroActionWeight, severityPriority, sortCandidates, uniqueByScope };
