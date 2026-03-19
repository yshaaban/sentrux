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

export function scoreBandLabel(score) {
  if (score >= 8500) {
    return 'very_high_signal';
  }
  if (score >= 6500) {
    return 'high_signal';
  }
  if (score >= 4000) {
    return 'moderate_signal';
  }
  return 'supporting_signal';
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

function normalizeMetricKey(key) {
  return String(key ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

function normalizeMetrics(metrics) {
  if (Array.isArray(metrics)) {
    return Object.fromEntries(
      metrics
        .filter((metric) => metric && metric.label)
        .map((metric) => [normalizeMetricKey(metric.label), metric.value]),
    );
  }

  if (metrics && typeof metrics === 'object') {
    return Object.fromEntries(
      Object.entries(metrics).map(([key, value]) => [normalizeMetricKey(key), value]),
    );
  }

  return {};
}

function metricValue(candidate, key) {
  const normalizedKey = normalizeMetricKey(key);
  const metric = candidate.metrics?.[normalizedKey];
  if (typeof metric === 'number') {
    return metric;
  }

  const directValue = candidate[normalizedKey];
  if (typeof directValue === 'number') {
    return directValue;
  }

  return 0;
}

function hasRoleTag(candidate, roleTag) {
  return (candidate.role_tags ?? []).includes(roleTag);
}

function bestCutCandidate(candidate) {
  return (candidate.cut_candidates ?? [])[0] ?? null;
}

function cappedBonus(value, unit, maxBonus) {
  return Math.min(value * unit, maxBonus);
}

function containsReason(candidate, reason) {
  return (candidate.leverage_reasons ?? []).includes(reason);
}

function isContainedRefactorSurface(candidate) {
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const fanOut = metricValue(candidate, 'fan_out');
  const cycleSize = metricValue(candidate, 'cycle_size');
  const guardrailCount = metricValue(candidate, 'guardrail_test_count');
  const hasExtractedOwnerShell =
    hasRoleTag(candidate, 'facade_with_extracted_owners') ||
    containsReason(candidate, 'extracted_owner_shell_pressure');

  return (
    (hasExtractedOwnerShell || guardrailCount > 0) &&
    fanOut >= 3 &&
    (fanIn === 0 || fanIn <= 12) &&
    (cycleSize === 0 || cycleSize <= 6)
  );
}

function architectureSignalRanking(candidate) {
  const reasons = [];
  let strength = 2200;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const cycleSize = metricValue(candidate, 'cycle_size');
  const cutCount = metricValue(candidate, 'cut_candidate_count');
  const bestCut = bestCutCandidate(candidate);

  if (hasRoleTag(candidate, 'component_barrel')) {
    strength += 2200;
    reasons.push('shared_barrel_boundary_hub');
  }
  if (
    hasRoleTag(candidate, 'guarded_boundary') ||
    containsReason(candidate, 'guardrail_backed_boundary_pressure')
  ) {
    strength += 1900;
    reasons.push('guardrail_backed_boundary_hub');
  }
  if (candidate.kind === 'cycle_cluster' || containsReason(candidate, 'mixed_cycle_pressure')) {
    strength += 1700;
    reasons.push('mixed_cycle_architecture_pressure');
  }
  if (cutCount > 0 || bestCut || containsReason(candidate, 'high_leverage_cycle_cut')) {
    strength += 900;
    reasons.push('high_leverage_cut_candidate');
  }
  if (bestCut) {
    strength += cappedBonus(bestCut.reduction_file_count ?? 0, 170, 1200);
    if ((bestCut.reduction_file_count ?? 0) > 0) {
      reasons.push('material_cycle_reduction');
    }
    if (typeof bestCut.remaining_cycle_size === 'number' && cycleSize > 0) {
      const containedRemainder = Math.max(0, cycleSize - bestCut.remaining_cycle_size);
      strength += cappedBonus(containedRemainder, 110, 700);
    }
  }
  if (fanIn > 0) {
    strength += cappedBonus(fanIn, 90, 1400);
    if (fanIn >= 12) {
      reasons.push('high_fan_in_boundary_pressure');
    }
  }
  if (cycleSize > 0) {
    strength += cappedBonus(cycleSize, 110, 1300);
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(candidate.cluster_signal_count, 220, 900);
  }
  if (candidate.hotspot_overlap) {
    strength += 350;
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function localRefactorRanking(candidate) {
  const reasons = [];
  let strength = 1800;
  const fanOut = metricValue(candidate, 'fan_out');
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const maxComplexity = metricValue(candidate, 'max_complexity');
  const cycleSize = metricValue(candidate, 'cycle_size');
  const guardrailCount = metricValue(candidate, 'guardrail_test_count');

  if (
    hasRoleTag(candidate, 'facade_with_extracted_owners') ||
    containsReason(candidate, 'extracted_owner_shell_pressure')
  ) {
    strength += 1700;
    reasons.push('extracted_owner_shell');
  }
  if (guardrailCount > 0 || containsReason(candidate, 'guardrail_backed_refactor_surface')) {
    strength += 1400;
    reasons.push('guardrail_backed_refactor_surface');
  }
  if (isContainedRefactorSurface(candidate) || containsReason(candidate, 'contained_refactor_surface')) {
    strength += 1500;
    reasons.push('contained_refactor_surface');
  }
  if (fanOut > 0) {
    strength += cappedBonus(fanOut, 95, 1100);
    if (fanOut >= 10) {
      reasons.push('broad_dependency_surface');
    }
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(candidate.cluster_signal_count, 180, 720);
  }
  if (candidate.hotspot_overlap) {
    strength += 500;
    reasons.push('coordination_overlap');
  }
  if (maxComplexity > 0) {
    strength += cappedBonus(maxComplexity, 35, 700);
  }
  if (fanIn >= 18 || cycleSize >= 10) {
    strength = Math.max(0, strength - 850);
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function boundaryDisciplineRanking(candidate) {
  const reasons = [];
  let strength = 1900;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const fanOut = metricValue(candidate, 'fan_out');

  if (
    hasRoleTag(candidate, 'transport_facade') ||
    candidate.presentation_class === 'guarded_facade' ||
    containsReason(candidate, 'guarded_or_transport_facade') ||
    containsReason(candidate, 'boundary_or_facade_seam_pressure')
  ) {
    strength += 1800;
    reasons.push('facade_boundary_surface');
  }
  if (fanIn > 0) {
    strength += cappedBonus(fanIn, 110, 1800);
    reasons.push('cross_surface_inbound_pressure');
  }
  if (fanOut > 0) {
    strength += cappedBonus(fanOut, 45, 500);
  }
  if (candidate.cluster_signal_count > 0 || candidate.hotspot_overlap) {
    strength += 800;
    reasons.push('boundary_overlap_pressure');
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function regrowthWatchpointRanking(candidate) {
  const reasons = [];
  let strength = 1700;
  const fanOut = metricValue(candidate, 'fan_out');

  if (hasRoleTag(candidate, 'composition_root') || containsReason(candidate, 'intentionally_central_surface')) {
    strength += 1800;
    reasons.push('composition_root_breadth');
  }
  if (hasRoleTag(candidate, 'entry_surface')) {
    strength += 900;
    reasons.push('entry_surface_pressure');
  }
  if (fanOut > 0) {
    strength += cappedBonus(fanOut, 100, 1500);
    if (fanOut >= 12) {
      reasons.push('growing_dependency_surface');
    }
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(candidate.cluster_signal_count, 170, 700);
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function secondaryCleanupRanking(candidate) {
  const reasons = [];
  let strength = 1500;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const maxComplexity = metricValue(candidate, 'max_complexity');

  if (
    hasRoleTag(candidate, 'facade_with_extracted_owners') ||
    containsReason(candidate, 'secondary_facade_cleanup')
  ) {
    strength += 900;
    reasons.push('secondary_facade_pressure');
  }
  if (candidate.hotspot_overlap) {
    strength += 900;
    reasons.push('hotspot_overlap');
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(candidate.cluster_signal_count, 220, 900);
    reasons.push('multi_signal_cleanup_overlap');
  }
  if (fanIn > 0) {
    strength += cappedBonus(fanIn, 50, 500);
  }
  if (maxComplexity > 0) {
    strength += cappedBonus(maxComplexity, 30, 500);
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function neutralRanking(candidate, reason) {
  return {
    strength: 1200 + (candidate.cluster_signal_count > 0 ? 300 : 0),
    reasons: reason ? [reason] : [],
  };
}

function rankingProfile(candidate) {
  switch (candidate.leverage_class) {
    case 'architecture_signal':
      return architectureSignalRanking(candidate);
    case 'local_refactor_target':
      return localRefactorRanking(candidate);
    case 'boundary_discipline':
      return boundaryDisciplineRanking(candidate);
    case 'regrowth_watchpoint':
      return regrowthWatchpointRanking(candidate);
    case 'secondary_cleanup':
      return secondaryCleanupRanking(candidate);
    case 'hardening_note':
      return neutralRanking(candidate, 'narrow_surface_hardening');
    case 'tooling_debt':
      return neutralRanking(candidate, 'tooling_maintenance_surface');
    case 'experimental':
      return neutralRanking(candidate, 'experimental_detector_surface');
    default:
      return neutralRanking(candidate, null);
  }
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
  const metrics = normalizeMetrics(candidate.metrics);
  const normalizedCandidate = {
    ...candidate,
    metrics,
    role_tags: candidate.role_tags ?? [],
    cut_candidates: candidate.cut_candidates ?? [],
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
  const ranking = rankingProfile(normalizedCandidate);

  return {
    ...normalizedCandidate,
    score_band: scoreBandLabel(normalizedCandidate.score_0_10000),
    within_bucket_strength_0_10000: Math.min(ranking.strength, 10_000),
    ranking_reasons: [...new Set(ranking.reasons)].slice(0, 3),
  };
}

function compareCandidates(left, right) {
  return (
    leverageClassPriority(left.leverage_class) - leverageClassPriority(right.leverage_class) ||
    right.within_bucket_strength_0_10000 - left.within_bucket_strength_0_10000 ||
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
    ranking_reasons: candidate.ranking_reasons ?? [],
    scope: candidate.scope ?? null,
    severity: candidate.severity ?? null,
    score_band: candidate.score_band ?? null,
    score_0_10000: candidate.score_0_10000 ?? null,
    within_bucket_strength_0_10000: candidate.within_bucket_strength_0_10000 ?? null,
    summary: candidate.summary ?? null,
    impact: candidate.impact ?? null,
    candidate_split_axes: candidate.candidate_split_axes ?? [],
    related_surfaces: candidate.related_surfaces ?? [],
  };
}
