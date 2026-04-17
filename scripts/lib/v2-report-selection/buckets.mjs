import { normalizeCandidate, signalMatchesScope } from './normalization.mjs';
import { rankingProfile } from './ranking.mjs';
import { sortCandidates, uniqueByScope } from './compare.mjs';
import { scoreBandLabel } from '../signal-policy.mjs';

function clustersForScope(debtClusters, scope) {
  return (debtClusters ?? []).filter((cluster) => (cluster.files ?? []).includes(scope));
}

function bestSignalForScope(debtSignals, scope) {
  return (debtSignals ?? [])
    .filter((signal) => signalMatchesScope(signal, scope))
    .sort((left, right) => (right.score_0_10000 ?? 0) - (left.score_0_10000 ?? 0))[0] ?? null;
}

function hasHotspotOverlap(matchingSignal, clusters) {
  if (matchingSignal && ['hotspot', 'unstable_hotspot'].includes(matchingSignal.kind)) {
    return true;
  }

  return clusters.some((cluster) =>
    (cluster.signal_kinds ?? []).some((kind) => ['hotspot', 'unstable_hotspot'].includes(kind)),
  );
}

function normalizeRankedCandidate(candidate, sourceType, debtSignals, debtClusters) {
  const matchingSignal = bestSignalForScope(debtSignals, candidate.scope);
  const matchingClusters = clustersForScope(debtClusters, candidate.scope);
  const normalizedCandidate = normalizeCandidate(
    candidate,
    sourceType,
    matchingSignal,
    matchingClusters,
  );
  const ranking = rankingProfile(normalizedCandidate);

  return {
    ...normalizedCandidate,
    score_band: scoreBandLabel(normalizedCandidate.score_0_10000),
    within_bucket_strength_0_10000: Math.min(ranking.strength, 10_000),
    ranking_reasons: [...new Set(ranking.reasons)].slice(0, 3),
    hotspot_overlap: hasHotspotOverlap(matchingSignal, matchingClusters),
  };
}

function collectCandidates(findingsPayload) {
  const debtSignals = findingsPayload.debt_signals ?? [];
  const debtClusters = findingsPayload.debt_clusters ?? [];
  const trustedDetails = (findingsPayload.finding_details ?? [])
    .filter((detail) => detail.trust_tier === 'trusted')
    .map((detail) => normalizeRankedCandidate(detail, 'finding_detail', debtSignals, debtClusters));
  const trustedWatchpoints = (findingsPayload.watchpoints ?? [])
    .filter((watchpoint) => watchpoint.trust_tier === 'watchpoint')
    .map((watchpoint) =>
      normalizeRankedCandidate(watchpoint, 'watchpoint', debtSignals, debtClusters),
    );

  return {
    trusted_details: uniqueByScope(sortCandidates(trustedDetails)),
    trusted_watchpoints: uniqueByScope(sortCandidates(trustedWatchpoints)),
  };
}

function bucketCandidates(candidates, leverageClass, limit) {
  return uniqueByScope(
    sortCandidates(candidates.filter((candidate) => candidate.leverage_class === leverageClass)),
  ).slice(0, limit);
}

function firstCandidate(candidates) {
  return candidates.length > 0 ? candidates[0] : null;
}

function excludeScopes(candidates, scopes) {
  return candidates.filter((candidate) => !scopes.has(candidate.scope));
}

function collectCoveredScopes(buckets) {
  const coveredScopes = new Set();

  for (const candidates of buckets) {
    for (const candidate of candidates) {
      coveredScopes.add(candidate.scope);
    }
  }

  return coveredScopes;
}

function selectLeverageBuckets(findingsPayload) {
  const candidateSets = collectCandidates(findingsPayload);
  const allCandidates = [...candidateSets.trusted_details, ...candidateSets.trusted_watchpoints];
  const architectureSignals = bucketCandidates(allCandidates, 'architecture_signal', 2);
  const localRefactorTargets = bucketCandidates(
    candidateSets.trusted_details,
    'local_refactor_target',
    2,
  );
  const boundaryDiscipline = bucketCandidates(
    candidateSets.trusted_details,
    'boundary_discipline',
    2,
  );
  const regrowthWatchpoints = bucketCandidates(allCandidates, 'regrowth_watchpoint', 2);
  const secondaryCleanup = bucketCandidates(allCandidates, 'secondary_cleanup', 3);
  const hardeningNotes = bucketCandidates(candidateSets.trusted_details, 'hardening_note', 3);
  const toolingDebt = bucketCandidates(candidateSets.trusted_details, 'tooling_debt', 3);
  const summaryCandidates = uniqueByScope(
    [
      firstCandidate(architectureSignals),
      firstCandidate(localRefactorTargets),
      firstCandidate(boundaryDiscipline),
      firstCandidate(regrowthWatchpoints),
      firstCandidate(secondaryCleanup),
    ].filter(Boolean),
  );
  const selectedScopes = new Set(summaryCandidates.map((candidate) => candidate.scope));
  const coveredScopes = collectCoveredScopes([
    architectureSignals,
    localRefactorTargets,
    boundaryDiscipline,
    regrowthWatchpoints,
    secondaryCleanup,
    hardeningNotes,
    toolingDebt,
  ]);
  const trustedWatchpoints = excludeScopes(candidateSets.trusted_watchpoints, coveredScopes).slice(
    0,
    4,
  );

  return {
    summary_candidates: summaryCandidates,
    architecture_signals: architectureSignals,
    local_refactor_targets: localRefactorTargets,
    boundary_discipline: boundaryDiscipline,
    regrowth_watchpoints: regrowthWatchpoints,
    secondary_cleanup: secondaryCleanup,
    hardening_notes: hardeningNotes,
    tooling_debt: toolingDebt,
    trusted_watchpoints: trustedWatchpoints,
    lead_candidates: summaryCandidates,
    secondary_hotspots: excludeScopes(secondaryCleanup, selectedScopes).slice(0, 2),
  };
}

function selectPresentationBuckets(findingsPayload) {
  return selectLeverageBuckets(findingsPayload);
}

export {
  bucketCandidates,
  collectCandidates,
  selectLeverageBuckets,
  selectPresentationBuckets,
};
