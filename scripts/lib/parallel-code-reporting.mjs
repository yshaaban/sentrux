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

function normalizeCandidate(detail, debtSignals, debtClusters) {
  const matchingSignal = bestSignalForScope(debtSignals, detail.scope);
  const matchingClusters = clustersForScope(debtClusters, detail.scope);
  const clusterSignalCount = Math.max(
    0,
    ...matchingClusters.map((cluster) => cluster.metrics?.signal_count ?? 0),
  );

  return {
    ...detail,
    presentation_class: detail.presentation_class ?? 'structural_debt',
    score_0_10000: matchingSignal?.score_0_10000 ?? fallbackScore(detail.severity),
    signal_families: matchingSignal?.signal_families ?? [],
    cluster_overlap_count: matchingClusters.length,
    cluster_signal_count: clusterSignalCount,
    hotspot_overlap: hasHotspotOverlap(matchingSignal, matchingClusters),
  };
}

function hasRoleTag(candidate, roleTag) {
  return (candidate.role_tags ?? []).includes(roleTag);
}

function isExtractedOwnerFacadeHotspot(candidate) {
  return hasRoleTag(candidate, 'facade_with_extracted_owners') && candidate.hotspot_overlap;
}

function isLeadStructuralDebtCandidate(candidate) {
  return candidate.presentation_class === 'structural_debt' && !isExtractedOwnerFacadeHotspot(candidate);
}

function isSecondaryHotspotCandidate(candidate) {
  if (!['structural_debt', 'guarded_facade'].includes(candidate.presentation_class)) {
    return false;
  }
  if (!candidate.hotspot_overlap) {
    return false;
  }

  return (
    isExtractedOwnerFacadeHotspot(candidate) ||
    candidate.cluster_signal_count >= 3 ||
    candidate.score_0_10000 >= 6500
  );
}

function compareCandidates(left, right) {
  return (
    severityPriority(left.severity) - severityPriority(right.severity) ||
    presentationClassPriority(left.presentation_class) -
      presentationClassPriority(right.presentation_class) ||
    right.cluster_signal_count - left.cluster_signal_count ||
    Number(right.hotspot_overlap) - Number(left.hotspot_overlap) ||
    right.score_0_10000 - left.score_0_10000 ||
    left.scope.localeCompare(right.scope)
  );
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

function sortCandidates(candidates) {
  return [...candidates].sort(compareCandidates);
}

export function selectPresentationBuckets(findingsPayload) {
  const trustedDetails = (findingsPayload.finding_details ?? [])
    .filter((detail) => detail.trust_tier === 'trusted')
    .map((detail) =>
      normalizeCandidate(detail, findingsPayload.debt_signals ?? [], findingsPayload.debt_clusters ?? []),
    );
  const uniqueCandidates = uniqueByScope(sortCandidates(trustedDetails));
  const structuralDebt = uniqueCandidates.filter(
    (candidate) => candidate.presentation_class === 'structural_debt',
  );
  const guardedFacades = uniqueCandidates.filter(
    (candidate) => candidate.presentation_class === 'guarded_facade',
  );
  const hardeningNotes = uniqueCandidates.filter(
    (candidate) => candidate.presentation_class === 'hardening_note',
  );
  const toolingDebt = uniqueCandidates.filter(
    (candidate) => candidate.presentation_class === 'tooling_debt',
  );
  const primaryStructuralDebt = structuralDebt.filter(isLeadStructuralDebtCandidate);
  const secondaryHotspotCandidates = uniqueCandidates.filter(isSecondaryHotspotCandidate);

  const leadCandidates = [
    ...primaryStructuralDebt.slice(0, 3),
    ...guardedFacades.slice(0, 1),
  ];

  while (leadCandidates.length < 4) {
    const overflowCandidate = [
      ...primaryStructuralDebt.slice(3),
      ...guardedFacades.slice(1),
      ...structuralDebt.filter((candidate) => isExtractedOwnerFacadeHotspot(candidate)),
    ].find((candidate) => !leadCandidates.some((entry) => entry.scope === candidate.scope));
    if (!overflowCandidate) {
      break;
    }
    leadCandidates.push(overflowCandidate);
  }

  const secondaryHotspots = secondaryHotspotCandidates.filter(
    (candidate) => !leadCandidates.some((entry) => entry.scope === candidate.scope),
  );

  return {
    lead_candidates: leadCandidates,
    secondary_hotspots: secondaryHotspots.slice(0, 2),
    hardening_notes: hardeningNotes.slice(0, 3),
    tooling_debt: toolingDebt.slice(0, 3),
    trusted_watchpoints: (findingsPayload.watchpoints ?? [])
      .filter((watchpoint) => watchpoint.trust_tier === 'watchpoint')
      .slice(0, 4),
  };
}

export function compactSelectedCandidate(candidate) {
  return {
    kind: candidate.kind ?? null,
    trust_tier: candidate.trust_tier ?? null,
    presentation_class: candidate.presentation_class ?? null,
    scope: candidate.scope ?? null,
    severity: candidate.severity ?? null,
    score_0_10000: candidate.score_0_10000 ?? null,
    summary: candidate.summary ?? null,
    impact: candidate.impact ?? null,
    candidate_split_axes: candidate.candidate_split_axes ?? [],
    related_surfaces: candidate.related_surfaces ?? [],
  };
}
