const FALLBACK_SEVERITY_SCORES = {
  high: 7000,
  medium: 5000,
  low: 3000,
  unknown: 1000,
};

// Score bands intentionally use coarse thresholds so summary candidates remain stable
// even when the underlying ranking model shifts slightly.
const SCORE_BAND_DEFINITIONS = [
  { minimumScore: 8500, label: 'very_high_signal' },
  { minimumScore: 6500, label: 'high_signal' },
  { minimumScore: 4000, label: 'moderate_signal' },
];

const ARCHITECTURE_SIGNAL_RANKING = {
  base: 2200,
  componentBarrelBonus: 2200,
  guardedBoundaryBonus: 1900,
  cyclePressureBonus: 1700,
  cutCandidateBonus: 900,
  cutReductionUnit: 170,
  cutReductionMax: 1200,
  containedRemainderUnit: 110,
  containedRemainderMax: 700,
  fanInUnit: 90,
  fanInMax: 1400,
  cycleSizeUnit: 110,
  cycleSizeMax: 1300,
  clusterSignalUnit: 220,
  clusterSignalMax: 900,
  hotspotOverlapBonus: 350,
  fanInReasonThreshold: 12,
};

const LOCAL_REFACTOR_RANKING = {
  base: 1800,
  extractedOwnerShellBonus: 1700,
  guardrailBackedBonus: 1400,
  containedSurfaceBonus: 1500,
  fanOutUnit: 95,
  fanOutMax: 1100,
  fanOutReasonThreshold: 10,
  clusterSignalUnit: 180,
  clusterSignalMax: 720,
  hotspotOverlapBonus: 500,
  maxComplexityUnit: 35,
  maxComplexityMax: 700,
  broadSurfacePenalty: 850,
  fanInPenaltyThreshold: 18,
  cyclePenaltyThreshold: 10,
};

const BOUNDARY_DISCIPLINE_RANKING = {
  base: 1900,
  facadeBoundaryBonus: 1800,
  fanInUnit: 110,
  fanInMax: 1800,
  fanOutUnit: 45,
  fanOutMax: 500,
  overlapBonus: 800,
};

const REGROWTH_WATCHPOINT_RANKING = {
  base: 1700,
  compositionRootBonus: 1800,
  entrySurfaceBonus: 900,
  fanOutUnit: 100,
  fanOutMax: 1500,
  fanOutReasonThreshold: 12,
  clusterSignalUnit: 170,
  clusterSignalMax: 700,
};

const SECONDARY_CLEANUP_RANKING = {
  base: 1500,
  facadeCleanupBonus: 900,
  hotspotOverlapBonus: 900,
  clusterSignalUnit: 220,
  clusterSignalMax: 900,
  fanInUnit: 50,
  fanInMax: 500,
  maxComplexityUnit: 30,
  maxComplexityMax: 500,
};

const CONTAINED_REFACTOR_SURFACE = {
  minimumFanOut: 3,
  maximumFanIn: 12,
  maximumCycleSize: 6,
};

const NEUTRAL_RANKING = {
  base: 1200,
  clusterSignalBonus: 300,
};

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
      return FALLBACK_SEVERITY_SCORES.high;
    case 'medium':
      return FALLBACK_SEVERITY_SCORES.medium;
    case 'low':
      return FALLBACK_SEVERITY_SCORES.low;
    default:
      return FALLBACK_SEVERITY_SCORES.unknown;
  }
}

export function scoreBandLabel(score) {
  for (const band of SCORE_BAND_DEFINITIONS) {
    if (score >= band.minimumScore) {
      return band.label;
    }
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
    fanOut >= CONTAINED_REFACTOR_SURFACE.minimumFanOut &&
    (fanIn === 0 || fanIn <= CONTAINED_REFACTOR_SURFACE.maximumFanIn) &&
    (cycleSize === 0 || cycleSize <= CONTAINED_REFACTOR_SURFACE.maximumCycleSize)
  );
}

function architectureSignalRanking(candidate) {
  const reasons = [];
  let strength = ARCHITECTURE_SIGNAL_RANKING.base;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const cycleSize = metricValue(candidate, 'cycle_size');
  const cutCount = metricValue(candidate, 'cut_candidate_count');
  const bestCut = bestCutCandidate(candidate);

  if (hasRoleTag(candidate, 'component_barrel')) {
    strength += ARCHITECTURE_SIGNAL_RANKING.componentBarrelBonus;
    reasons.push('shared_barrel_boundary_hub');
  }
  if (
    hasRoleTag(candidate, 'guarded_boundary') ||
    containsReason(candidate, 'guardrail_backed_boundary_pressure')
  ) {
    strength += ARCHITECTURE_SIGNAL_RANKING.guardedBoundaryBonus;
    reasons.push('guardrail_backed_boundary_hub');
  }
  if (candidate.kind === 'cycle_cluster' || containsReason(candidate, 'mixed_cycle_pressure')) {
    strength += ARCHITECTURE_SIGNAL_RANKING.cyclePressureBonus;
    reasons.push('mixed_cycle_architecture_pressure');
  }
  if (cutCount > 0 || bestCut || containsReason(candidate, 'high_leverage_cycle_cut')) {
    strength += ARCHITECTURE_SIGNAL_RANKING.cutCandidateBonus;
    reasons.push('high_leverage_cut_candidate');
  }
  if (bestCut) {
    strength += cappedBonus(
      bestCut.reduction_file_count ?? 0,
      ARCHITECTURE_SIGNAL_RANKING.cutReductionUnit,
      ARCHITECTURE_SIGNAL_RANKING.cutReductionMax,
    );
    if ((bestCut.reduction_file_count ?? 0) > 0) {
      reasons.push('material_cycle_reduction');
    }
    if (typeof bestCut.remaining_cycle_size === 'number' && cycleSize > 0) {
      const containedRemainder = Math.max(0, cycleSize - bestCut.remaining_cycle_size);
      strength += cappedBonus(
        containedRemainder,
        ARCHITECTURE_SIGNAL_RANKING.containedRemainderUnit,
        ARCHITECTURE_SIGNAL_RANKING.containedRemainderMax,
      );
    }
  }
  if (fanIn > 0) {
    strength += cappedBonus(
      fanIn,
      ARCHITECTURE_SIGNAL_RANKING.fanInUnit,
      ARCHITECTURE_SIGNAL_RANKING.fanInMax,
    );
    if (fanIn >= ARCHITECTURE_SIGNAL_RANKING.fanInReasonThreshold) {
      reasons.push('high_fan_in_boundary_pressure');
    }
  }
  if (cycleSize > 0) {
    strength += cappedBonus(
      cycleSize,
      ARCHITECTURE_SIGNAL_RANKING.cycleSizeUnit,
      ARCHITECTURE_SIGNAL_RANKING.cycleSizeMax,
    );
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(
      candidate.cluster_signal_count,
      ARCHITECTURE_SIGNAL_RANKING.clusterSignalUnit,
      ARCHITECTURE_SIGNAL_RANKING.clusterSignalMax,
    );
  }
  if (candidate.hotspot_overlap) {
    strength += ARCHITECTURE_SIGNAL_RANKING.hotspotOverlapBonus;
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function localRefactorRanking(candidate) {
  const reasons = [];
  let strength = LOCAL_REFACTOR_RANKING.base;
  const fanOut = metricValue(candidate, 'fan_out');
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const maxComplexity = metricValue(candidate, 'max_complexity');
  const cycleSize = metricValue(candidate, 'cycle_size');
  const guardrailCount = metricValue(candidate, 'guardrail_test_count');

  if (
    hasRoleTag(candidate, 'facade_with_extracted_owners') ||
    containsReason(candidate, 'extracted_owner_shell_pressure')
  ) {
    strength += LOCAL_REFACTOR_RANKING.extractedOwnerShellBonus;
    reasons.push('extracted_owner_shell');
  }
  if (guardrailCount > 0 || containsReason(candidate, 'guardrail_backed_refactor_surface')) {
    strength += LOCAL_REFACTOR_RANKING.guardrailBackedBonus;
    reasons.push('guardrail_backed_refactor_surface');
  }
  if (isContainedRefactorSurface(candidate) || containsReason(candidate, 'contained_refactor_surface')) {
    strength += LOCAL_REFACTOR_RANKING.containedSurfaceBonus;
    reasons.push('contained_refactor_surface');
  }
  if (fanOut > 0) {
    strength += cappedBonus(
      fanOut,
      LOCAL_REFACTOR_RANKING.fanOutUnit,
      LOCAL_REFACTOR_RANKING.fanOutMax,
    );
    if (fanOut >= LOCAL_REFACTOR_RANKING.fanOutReasonThreshold) {
      reasons.push('broad_dependency_surface');
    }
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(
      candidate.cluster_signal_count,
      LOCAL_REFACTOR_RANKING.clusterSignalUnit,
      LOCAL_REFACTOR_RANKING.clusterSignalMax,
    );
  }
  if (candidate.hotspot_overlap) {
    strength += LOCAL_REFACTOR_RANKING.hotspotOverlapBonus;
    reasons.push('coordination_overlap');
  }
  if (maxComplexity > 0) {
    strength += cappedBonus(
      maxComplexity,
      LOCAL_REFACTOR_RANKING.maxComplexityUnit,
      LOCAL_REFACTOR_RANKING.maxComplexityMax,
    );
  }
  if (
    fanIn >= LOCAL_REFACTOR_RANKING.fanInPenaltyThreshold ||
    cycleSize >= LOCAL_REFACTOR_RANKING.cyclePenaltyThreshold
  ) {
    strength = Math.max(0, strength - LOCAL_REFACTOR_RANKING.broadSurfacePenalty);
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function boundaryDisciplineRanking(candidate) {
  const reasons = [];
  let strength = BOUNDARY_DISCIPLINE_RANKING.base;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const fanOut = metricValue(candidate, 'fan_out');

  if (
    hasRoleTag(candidate, 'transport_facade') ||
    candidate.presentation_class === 'guarded_facade' ||
    containsReason(candidate, 'guarded_or_transport_facade') ||
    containsReason(candidate, 'boundary_or_facade_seam_pressure')
  ) {
    strength += BOUNDARY_DISCIPLINE_RANKING.facadeBoundaryBonus;
    reasons.push('facade_boundary_surface');
  }
  if (fanIn > 0) {
    strength += cappedBonus(
      fanIn,
      BOUNDARY_DISCIPLINE_RANKING.fanInUnit,
      BOUNDARY_DISCIPLINE_RANKING.fanInMax,
    );
    reasons.push('cross_surface_inbound_pressure');
  }
  if (fanOut > 0) {
    strength += cappedBonus(
      fanOut,
      BOUNDARY_DISCIPLINE_RANKING.fanOutUnit,
      BOUNDARY_DISCIPLINE_RANKING.fanOutMax,
    );
  }
  if (candidate.cluster_signal_count > 0 || candidate.hotspot_overlap) {
    strength += BOUNDARY_DISCIPLINE_RANKING.overlapBonus;
    reasons.push('boundary_overlap_pressure');
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function regrowthWatchpointRanking(candidate) {
  const reasons = [];
  let strength = REGROWTH_WATCHPOINT_RANKING.base;
  const fanOut = metricValue(candidate, 'fan_out');

  if (hasRoleTag(candidate, 'composition_root') || containsReason(candidate, 'intentionally_central_surface')) {
    strength += REGROWTH_WATCHPOINT_RANKING.compositionRootBonus;
    reasons.push('composition_root_breadth');
  }
  if (hasRoleTag(candidate, 'entry_surface')) {
    strength += REGROWTH_WATCHPOINT_RANKING.entrySurfaceBonus;
    reasons.push('entry_surface_pressure');
  }
  if (fanOut > 0) {
    strength += cappedBonus(
      fanOut,
      REGROWTH_WATCHPOINT_RANKING.fanOutUnit,
      REGROWTH_WATCHPOINT_RANKING.fanOutMax,
    );
    if (fanOut >= REGROWTH_WATCHPOINT_RANKING.fanOutReasonThreshold) {
      reasons.push('growing_dependency_surface');
    }
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(
      candidate.cluster_signal_count,
      REGROWTH_WATCHPOINT_RANKING.clusterSignalUnit,
      REGROWTH_WATCHPOINT_RANKING.clusterSignalMax,
    );
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function secondaryCleanupRanking(candidate) {
  const reasons = [];
  let strength = SECONDARY_CLEANUP_RANKING.base;
  const fanIn = metricValue(candidate, 'fan_in') || metricValue(candidate, 'inbound_reference_count');
  const maxComplexity = metricValue(candidate, 'max_complexity');

  if (
    hasRoleTag(candidate, 'facade_with_extracted_owners') ||
    containsReason(candidate, 'secondary_facade_cleanup')
  ) {
    strength += SECONDARY_CLEANUP_RANKING.facadeCleanupBonus;
    reasons.push('secondary_facade_pressure');
  }
  if (candidate.hotspot_overlap) {
    strength += SECONDARY_CLEANUP_RANKING.hotspotOverlapBonus;
    reasons.push('hotspot_overlap');
  }
  if (candidate.cluster_signal_count > 0) {
    strength += cappedBonus(
      candidate.cluster_signal_count,
      SECONDARY_CLEANUP_RANKING.clusterSignalUnit,
      SECONDARY_CLEANUP_RANKING.clusterSignalMax,
    );
    reasons.push('multi_signal_cleanup_overlap');
  }
  if (fanIn > 0) {
    strength += cappedBonus(
      fanIn,
      SECONDARY_CLEANUP_RANKING.fanInUnit,
      SECONDARY_CLEANUP_RANKING.fanInMax,
    );
  }
  if (maxComplexity > 0) {
    strength += cappedBonus(
      maxComplexity,
      SECONDARY_CLEANUP_RANKING.maxComplexityUnit,
      SECONDARY_CLEANUP_RANKING.maxComplexityMax,
    );
  }

  return {
    strength,
    reasons: reasons.slice(0, 3),
  };
}

function neutralRanking(candidate, reason) {
  return {
    strength:
      NEUTRAL_RANKING.base +
      (candidate.cluster_signal_count > 0 ? NEUTRAL_RANKING.clusterSignalBonus : 0),
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
  const current = candidate ?? {};
  return {
    kind: current.kind ?? null,
    trust_tier: current.trust_tier ?? null,
    presentation_class: current.presentation_class ?? null,
    leverage_class: current.leverage_class ?? null,
    leverage_reasons: current.leverage_reasons ?? [],
    ranking_reasons: current.ranking_reasons ?? [],
    scope: current.scope ?? null,
    severity: current.severity ?? null,
    score_band: current.score_band ?? null,
    score_0_10000: current.score_0_10000 ?? null,
    within_bucket_strength_0_10000: current.within_bucket_strength_0_10000 ?? null,
    summary: current.summary ?? null,
    impact: current.impact ?? null,
    candidate_split_axes: current.candidate_split_axes ?? [],
    related_surfaces: current.related_surfaces ?? [],
  };
}
