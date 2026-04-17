import { deadPrivateCandidateKey, selectDeadPrivateCandidatesFromPayload } from '../../evals/review_dead_private.mjs';

export function sortByNumericField(values, fieldName) {
  return [...values].sort(function compare(left, right) {
    return (right?.[fieldName] ?? 0) - (left?.[fieldName] ?? 0);
  });
}

export function collectDeadPrivateCandidateSets(rawToolAnalysis) {
  const findingsPayload = rawToolAnalysis.findings ?? {};
  const selection = selectDeadPrivateCandidatesFromPayload(findingsPayload);
  const dedupedCandidates = new Map();
  const legacyOnlyCandidates = Array.isArray(selection.legacy_only_candidates)
    ? selection.legacy_only_candidates
    : [];

  for (const candidate of [...selection.candidates, ...legacyOnlyCandidates]) {
    const key = deadPrivateCandidateKey(candidate);
    if (!dedupedCandidates.has(key)) {
      dedupedCandidates.set(key, candidate);
    }
  }

  return {
    sourceLane: selection.source_lane,
    sourceLaneCount: selection.source_lane_count,
    selectedCandidates: selection.candidates,
    consideredLanes: selection.considered_lanes,
    reviewerLaneStatus: selection.reviewer_lane_status,
    reviewerLaneReason: selection.reviewer_lane_reason,
    canonicalCandidateCount: selection.canonical_candidate_count,
    legacyCandidateCount: selection.legacy_candidate_count,
    overlappingCandidateCount: selection.overlapping_candidate_count,
    legacyOnlyCandidates,
    combinedCandidates: [...dedupedCandidates.values()],
  };
}

export function deadPrivateSampleSymbols(finding) {
  const sampleEvidence = (finding?.evidence ?? []).find(function findSample(entry) {
    return typeof entry === 'string' && entry.startsWith('sample dead functions: ');
  });

  if (!sampleEvidence) {
    return [];
  }

  return sampleEvidence
    .replace('sample dead functions: ', '')
    .split(',')
    .map(function trimValue(value) {
      return value.trim();
    })
    .filter(Boolean);
}

export function hasSuspiciousDeadPrivateSymbols(symbols) {
  if (symbols.length === 0) {
    return false;
  }

  if (symbols.every(function isCell(symbol) {
    return symbol === 'cell';
  })) {
    return true;
  }

  return symbols.some(function isLifecycle(symbol) {
    return symbol === 'getDerivedStateFromError' || symbol === 'componentDidCatch';
  });
}

function topDeadPrivateExperimental(rawToolAnalysis, limit) {
  return sortByNumericField(
    collectDeadPrivateCandidateSets(rawToolAnalysis).combinedCandidates,
    'score_0_10000',
  ).slice(0, limit);
}

export function deadPrivateFalsePositiveCandidates(rawToolAnalysis) {
  return topDeadPrivateExperimental(rawToolAnalysis, 20).filter(function isSuspicious(finding) {
    const symbols = deadPrivateSampleSymbols(finding);
    return hasSuspiciousDeadPrivateSymbols(symbols);
  });
}

export function deadPrivatePlausibleCandidates(rawToolAnalysis) {
  return topDeadPrivateExperimental(rawToolAnalysis, 20).filter(function isPlausible(finding) {
    const symbols = deadPrivateSampleSymbols(finding);
    return symbols.length === 0 || !hasSuspiciousDeadPrivateSymbols(symbols);
  });
}

export function formatDeadPrivateCandidateDescriptor(finding, symbolLabel) {
  const label = symbolLabel ?? 'symbols';
  return `\`${finding.scope}\` with ${label} \`${deadPrivateSampleSymbols(finding).join(', ')}\``;
}

export function appendDeadPrivateCandidateSection(lines, findings, symbolLabel) {
  for (const finding of findings) {
    lines.push(`  - ${formatDeadPrivateCandidateDescriptor(finding, symbolLabel)}`);
  }
}
