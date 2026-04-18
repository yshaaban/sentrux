import {
  candidateBooleanValue,
  candidateNumberValue,
  normalizeCandidate,
  signalMatchesScope,
} from './normalization.mjs';
import { rankingProfile } from './ranking.mjs';
import {
  compareCandidates,
  compareEvidenceMetrics,
  hasZeroActionWeight,
  sortCandidates,
  uniqueByScope,
} from './compare.mjs';
import { scoreBandLabel } from '../signal-policy.mjs';
import { buildDefaultAgentLeadSignalKindSet, buildSignalMetadataLookup } from '../signal-cohorts.mjs';

const SUMMARY_SLOT_LIMIT = 5;
const DEFAULT_LANE_SLOT_LIMIT = 3;
const DEFAULT_AGENT_LEAD_SIGNAL_KINDS = buildDefaultAgentLeadSignalKindSet();
const DEFAULT_SIGNAL_METADATA = buildSignalMetadataLookup();
const STRUCTURAL_PRESSURE_SIGNAL_KINDS = new Set([
  'cycle_cluster',
  'dependency_sprawl',
  'large_file',
  'missing_test_coverage',
  'unstable_hotspot',
]);

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

function addSummaryCandidate(summaryCandidates, selectedScopes, candidate) {
  if (!candidate || selectedScopes.has(candidate.scope) || summaryCandidates.length >= SUMMARY_SLOT_LIMIT) {
    return;
  }

  selectedScopes.add(candidate.scope);
  summaryCandidates.push(candidate);
}

function selectSummaryCandidates(summaryBuckets) {
  const bucketLeaders = uniqueByScope(summaryBuckets.map(firstCandidate).filter(Boolean));
  const summaryCandidates = [];
  const selectedScopes = new Set();
  const candidatePool = uniqueByScope(sortCandidates(summaryBuckets.flat()));
  const hasHigherPrioritySummarySignal = candidatePool.some(
    (candidate) => !hasZeroActionWeight(candidate),
  );

  if (!hasHigherPrioritySummarySignal) {
    for (const candidate of bucketLeaders) {
      addSummaryCandidate(summaryCandidates, selectedScopes, candidate);
    }

    return summaryCandidates;
  }

  for (const candidate of bucketLeaders) {
    if (hasZeroActionWeight(candidate)) {
      continue;
    }
    addSummaryCandidate(summaryCandidates, selectedScopes, candidate);
  }
  for (const candidate of candidatePool) {
    if (hasZeroActionWeight(candidate)) {
      continue;
    }
    addSummaryCandidate(summaryCandidates, selectedScopes, candidate);
  }

  return summaryCandidates;
}

function signalMetadataForCandidate(candidate) {
  return DEFAULT_SIGNAL_METADATA.get(candidate?.kind);
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

function candidateNumber(candidate, keys) {
  for (const key of keys) {
    const value = candidateNumberValue(candidate, key);
    if (value !== null) {
      return value;
    }
  }

  return null;
}

function signalFamilyForCandidate(candidate, signalMetadata) {
  if (typeof candidate?.signal_family === 'string' && candidate.signal_family) {
    return candidate.signal_family;
  }
  if (Array.isArray(candidate?.signal_families) && candidate.signal_families.length > 0) {
    return candidate.signal_families[0];
  }

  return signalMetadata?.signal_family ?? null;
}

function isStructuralPressureCandidate(candidate, signalMetadata) {
  const signalFamily = signalFamilyForCandidate(candidate, signalMetadata);
  if (signalFamily === 'clone' || signalFamily === 'obligation' || signalFamily === 'rules') {
    return false;
  }

  return STRUCTURAL_PRESSURE_SIGNAL_KINDS.has(candidate.kind) || signalFamily === 'structural';
}

function candidatePatchDirectlyWorsened(candidate) {
  if (
    candidateBoolean(candidate, [
      'patch_directly_worsened',
      'patch_worsened',
      'current_patch_worsened',
      'introduced_by_patch',
      'session_introduced',
    ])
  ) {
    return true;
  }

  const directChangeCount = candidateNumber(candidate, [
    'patch_directly_worsened_count',
    'patch_worsened_count',
    'changed_scope_count',
  ]);
  return directChangeCount !== null && directChangeCount > 0;
}

function candidateHasConcreteRepairSurface(candidate) {
  if (
    candidateBoolean(candidate, [
      'repair_surface_clear',
      'fix_surface_clear',
      'repair_packet_complete',
      'repair_packet_fix_surface_clear',
    ])
  ) {
    return true;
  }

  const repairSurfaceRate = candidateNumber(candidate, [
    'repair_packet_complete_rate',
    'repair_packet_fix_surface_clear_rate',
    'repair_packet_fix_surface_clarity_rate',
  ]);
  if (repairSurfaceRate !== null && repairSurfaceRate > 0) {
    return true;
  }

  return (
    Array.isArray(candidate?.likely_fix_sites) && candidate.likely_fix_sites.length > 0
  ) || (
    Array.isArray(candidate?.related_surfaces) && candidate.related_surfaces.length > 0
  );
}

function isDefaultLaneCandidate(candidate) {
  if (!candidate) {
    return false;
  }

  const signalMetadata = signalMetadataForCandidate(candidate);
  const primaryLane = candidate.primary_lane ?? signalMetadata?.primary_lane ?? null;
  const defaultSurfaceRole =
    candidate.default_surface_role ?? signalMetadata?.default_surface_role ?? null;
  const structuralPressureCandidate = isStructuralPressureCandidate(candidate, signalMetadata);
  const patchWorsened = candidatePatchDirectlyWorsened(candidate);
  const concreteRepairSurface = candidateHasConcreteRepairSurface(candidate);

  if (
    hasZeroActionWeight(candidate) &&
    !(structuralPressureCandidate && patchWorsened && concreteRepairSurface)
  ) {
    return false;
  }

  if (defaultSurfaceRole === 'supporting_watchpoint') {
    return false;
  }

  if (
    primaryLane === 'maintainer_watchpoint' &&
    !concreteRepairSurface
  ) {
    return false;
  }

  if (
    structuralPressureCandidate &&
    (!patchWorsened || !concreteRepairSurface)
  ) {
    return false;
  }

  if (primaryLane === 'agent_default' && defaultSurfaceRole === 'lead') {
    return true;
  }

  return DEFAULT_AGENT_LEAD_SIGNAL_KINDS.has(candidate.kind);
}

function defaultLaneLeadPriority(candidate) {
  const signalMetadata = signalMetadataForCandidate(candidate);
  return candidate.lead_priority ?? signalMetadata?.lead_priority ?? Number.MAX_SAFE_INTEGER;
}

function defaultLaneCompressionKey(candidate) {
  if (!candidate) {
    return null;
  }

  const signalMetadata = signalMetadataForCandidate(candidate);
  const primaryLane = candidate.primary_lane ?? signalMetadata?.primary_lane ?? null;
  const defaultSurfaceRole =
    candidate.default_surface_role ?? signalMetadata?.default_surface_role ?? null;

  if (primaryLane === 'agent_default' && defaultSurfaceRole === 'lead' && candidate.kind) {
    return `kind:${candidate.kind}`;
  }

  return `scope:${candidate.scope ?? 'unknown'}`;
}

function uniqueByKey(candidates, keySelector) {
  const seenKeys = new Set();
  const unique = [];

  for (const candidate of candidates) {
    const key = keySelector(candidate);
    if (key === null || key === undefined || seenKeys.has(key)) {
      continue;
    }

    seenKeys.add(key);
    unique.push(candidate);
  }

  return unique;
}

function compareDefaultLaneCandidates(left, right) {
  return (
    defaultLaneLeadPriority(left) - defaultLaneLeadPriority(right) ||
    compareEvidenceMetrics(left, right) ||
    compareCandidates(left, right)
  );
}

function selectDefaultLaneCandidates(allCandidates) {
  return uniqueByKey(
    allCandidates
      .filter((candidate) => isDefaultLaneCandidate(candidate))
      .sort(compareDefaultLaneCandidates),
    defaultLaneCompressionKey,
  ).slice(0, DEFAULT_LANE_SLOT_LIMIT);
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
  const defaultLaneCandidates = selectDefaultLaneCandidates(allCandidates);
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
  let summaryCandidates = defaultLaneCandidates;

  if (summaryCandidates.length === 0) {
    summaryCandidates = selectSummaryCandidates([
      architectureSignals,
      localRefactorTargets,
      boundaryDiscipline,
      regrowthWatchpoints,
      secondaryCleanup,
    ]);
  }
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
