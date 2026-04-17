import {
  bestCutCandidate,
  cappedBonus,
  containsReason,
  hasRoleTag,
  metricValue,
} from './normalization.mjs';

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

export { rankingProfile };
