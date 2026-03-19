function presentationClassPriority(presentationClass) {
  switch (presentationClass) {
    case 'structural_debt':
      return 0;
    case 'guarded_facade':
      return 1;
    case 'watchpoint':
      return 2;
    case 'hardening_note':
      return 3;
    case 'tooling_debt':
      return 4;
    case 'experimental':
      return 5;
    default:
      return 6;
  }
}

function leverageClassPriority(leverageClass) {
  switch (leverageClass) {
    case 'architecture_signal':
      return 0;
    case 'local_refactor_target':
      return 1;
    case 'boundary_discipline':
      return 2;
    case 'regrowth_watchpoint':
      return 3;
    case 'secondary_cleanup':
      return 4;
    case 'hardening_note':
      return 5;
    case 'tooling_debt':
      return 6;
    case 'experimental':
      return 7;
    default:
      return 8;
  }
}

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

function fallbackScore(severity) {
  switch (severity) {
    case 'high':
      return 7000;
    case 'medium':
      return 5000;
    case 'low':
      return 3000;
    default:
      return 1000;
  }
}

function defaultLeverageClass(presentationClass) {
  switch (presentationClass) {
    case 'guarded_facade':
      return 'boundary_discipline';
    case 'hardening_note':
      return 'hardening_note';
    case 'tooling_debt':
      return 'tooling_debt';
    case 'experimental':
      return 'experimental';
    default:
      return 'secondary_cleanup';
  }
}

function signalMatchesScope(signal, scope) {
  if (signal.scope === scope) {
    return true;
  }

  return (signal.files ?? []).includes(scope);
}

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

function normalizeCandidate(candidate, sourceType, debtSignals, debtClusters) {
  const matchingSignal = bestSignalForScope(debtSignals, candidate.scope);
  const matchingClusters = clustersForScope(debtClusters, candidate.scope);
  const clusterSignalCount = Math.max(
    0,
    ...matchingClusters.map((cluster) => cluster.metrics?.signal_count ?? 0),
  );
  const presentationClass = candidate.presentation_class ?? 'structural_debt';

  return {
    ...candidate,
    source_type: sourceType,
    presentation_class: presentationClass,
    leverage_class: candidate.leverage_class ?? defaultLeverageClass(presentationClass),
    leverage_reasons: candidate.leverage_reasons ?? [],
    score_0_10000:
      candidate.score_0_10000 ?? matchingSignal?.score_0_10000 ?? fallbackScore(candidate.severity),
    signal_families: candidate.signal_families ?? matchingSignal?.signal_families ?? [],
    cluster_overlap_count: matchingClusters.length,
    cluster_signal_count: clusterSignalCount,
    hotspot_overlap: hasHotspotOverlap(matchingSignal, matchingClusters),
  };
}

function compareCandidates(left, right) {
  return (
    leverageClassPriority(left.leverage_class) - leverageClassPriority(right.leverage_class) ||
    severityPriority(left.severity) - severityPriority(right.severity) ||
    presentationClassPriority(left.presentation_class) -
      presentationClassPriority(right.presentation_class) ||
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

function collectCandidates(findingsPayload) {
  const debtSignals = findingsPayload.debt_signals ?? [];
  const debtClusters = findingsPayload.debt_clusters ?? [];
  const trustedDetails = (findingsPayload.finding_details ?? [])
    .filter((detail) => detail.trust_tier === 'trusted')
    .map((detail) => normalizeCandidate(detail, 'finding_detail', debtSignals, debtClusters));
  const trustedWatchpoints = (findingsPayload.watchpoints ?? [])
    .filter((watchpoint) => watchpoint.trust_tier === 'watchpoint')
    .map((watchpoint) => normalizeCandidate(watchpoint, 'watchpoint', debtSignals, debtClusters));

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

export function selectLeverageBuckets(findingsPayload) {
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

export function selectPresentationBuckets(findingsPayload) {
  return selectLeverageBuckets(findingsPayload);
}

export function compactSelectedCandidate(candidate) {
  return {
    kind: candidate.kind ?? null,
    trust_tier: candidate.trust_tier ?? null,
    presentation_class: candidate.presentation_class ?? null,
    leverage_class: candidate.leverage_class ?? null,
    leverage_reasons: candidate.leverage_reasons ?? [],
    scope: candidate.scope ?? null,
    severity: candidate.severity ?? null,
    score_0_10000: candidate.score_0_10000 ?? null,
    summary: candidate.summary ?? null,
    impact: candidate.impact ?? null,
    candidate_split_axes: candidate.candidate_split_axes ?? [],
    related_surfaces: candidate.related_surfaces ?? [],
  };
}
