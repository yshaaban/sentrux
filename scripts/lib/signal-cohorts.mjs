import { readFile } from 'node:fs/promises';

function buildSignal({
  signalKind,
  signalFamily,
  promotionStatus,
  primaryLane,
  defaultSurfaceRole,
  rationale,
}) {
  return {
    signal_kind: signalKind,
    signal_family: signalFamily,
    promotion_status: promotionStatus,
    primary_lane: primaryLane,
    default_surface_role: defaultSurfaceRole,
    rationale,
  };
}

function buildAgentLoopCoreSignals() {
  return [
    buildSignal({
      signalKind: 'closed_domain_exhaustiveness',
      signalFamily: 'obligation',
      promotionStatus: 'trusted',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'supporting_note',
      rationale: 'High-value semantic completeness signal for TypeScript agent edits.',
    }),
    buildSignal({
      signalKind: 'forbidden_raw_read',
      signalFamily: 'rules',
      promotionStatus: 'trusted',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'lead',
      rationale: 'Strong explicit-rule regression that should block unsafe agent shortcuts.',
    }),
    buildSignal({
      signalKind: 'session_introduced_clone',
      signalFamily: 'clone',
      promotionStatus: 'watchpoint',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'lead',
      rationale:
        'Fresh duplication introduced in the current session is a high-ROI agent mistake signal when it stays session-scoped and concrete.',
    }),
    buildSignal({
      signalKind: 'clone_propagation_drift',
      signalFamily: 'clone',
      promotionStatus: 'watchpoint',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'lead',
      rationale:
        'Editing one side of an existing duplicate without syncing its sibling is a common agent followthrough miss and should stay visible in the fast loop.',
    }),
    buildSignal({
      signalKind: 'incomplete_propagation',
      signalFamily: 'obligation',
      promotionStatus: 'watchpoint',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'lead',
      rationale:
        'Explicit contract-surface propagation misses are sharp enough to calibrate as a conservative watchpoint before broader propagation heuristics.',
    }),
    buildSignal({
      signalKind: 'zero_config_boundary_violation',
      signalFamily: 'rules',
      promotionStatus: 'watchpoint',
      primaryLane: 'agent_default',
      defaultSurfaceRole: 'lead',
      rationale:
        'Direct zero-config boundary violations now have deterministic fixture-backed replay coverage plus seeded detection and remediation evidence, so they should stay visible in the fast loop as a maintained watchpoint.',
    }),
  ];
}

function buildSupportingStructuralWatchpoints() {
  return [
    buildSignal({
      signalKind: 'missing_test_coverage',
      signalFamily: 'structural',
      promotionStatus: 'watchpoint',
      primaryLane: 'maintainer_watchpoint',
      defaultSurfaceRole: 'supporting_watchpoint',
      rationale: 'Common omission signal that should stay visible without over-blocking.',
    }),
    buildSignal({
      signalKind: 'large_file',
      signalFamily: 'structural',
      promotionStatus: 'watchpoint',
      primaryLane: 'maintainer_watchpoint',
      defaultSurfaceRole: 'supporting_watchpoint',
      rationale: 'Regrowth warning that is valuable when it stays actionable.',
    }),
  ];
}

export function buildDefaultSignalCohorts() {
  return {
    schema_version: 1,
    default_cohort_id: 'agent-loop-core',
    cohorts: [
      {
        cohort_id: 'agent-loop-core',
        title: 'Agent Loop Core',
        description:
          'Intervention-grade default lane for fast coding-agent feedback calibration.',
        primary_lane: 'agent_default',
        signals: buildAgentLoopCoreSignals(),
        supporting_watchpoints: buildSupportingStructuralWatchpoints(),
        linked_supporting_cohort_ids: ['agent-structural-watchpoints'],
        next_candidates: [
          'multi_writer_concept',
          'forbidden_writer',
          'writer_outside_allowlist',
        ],
      },
      {
        cohort_id: 'agent-structural-watchpoints',
        title: 'Agent Structural Watchpoints',
        description:
          'Broader structural watchpoints that should stay inspectable without crowding the default agent lane.',
        primary_lane: 'maintainer_watchpoint',
        signals: buildSupportingStructuralWatchpoints(),
        next_candidates: [],
      },
    ],
  };
}

function collectCohortMetadataEntries(manifest, cohortId, seenCohortIds = new Set()) {
  const cohort = getSignalCohort(manifest, cohortId);
  if (seenCohortIds.has(cohort.cohort_id)) {
    return [];
  }

  seenCohortIds.add(cohort.cohort_id);
  const metadataEntries = [];

  for (const signal of [...(cohort.signals ?? []), ...(cohort.supporting_watchpoints ?? [])]) {
    metadataEntries.push({
      cohort_id: cohort.cohort_id,
      ...signal,
    });
  }
  for (const linkedCohortId of cohort.linked_supporting_cohort_ids ?? []) {
    metadataEntries.push(
      ...collectCohortMetadataEntries(manifest, linkedCohortId, seenCohortIds),
    );
  }

  return metadataEntries;
}

export function buildSignalMetadataLookup(manifest = null, cohortId = null) {
  const resolvedManifest = manifest ?? buildDefaultSignalCohorts();
  const metadataEntries = collectCohortMetadataEntries(
    resolvedManifest,
    cohortId ?? resolvedManifest.default_cohort_id,
  );
  const metadataBySignalKind = new Map();

  for (const entry of metadataEntries) {
    if (!entry?.signal_kind || metadataBySignalKind.has(entry.signal_kind)) {
      continue;
    }

    metadataBySignalKind.set(entry.signal_kind, {
      cohort_id: entry.cohort_id,
      signal_kind: entry.signal_kind,
      signal_family: entry.signal_family ?? 'unknown',
      promotion_status: entry.promotion_status ?? 'unspecified',
      primary_lane: entry.primary_lane ?? null,
      default_surface_role: entry.default_surface_role ?? null,
      rationale: entry.rationale ?? null,
    });
  }

  return metadataBySignalKind;
}

export function buildDefaultAgentLeadSignalKindSet(manifest = null, cohortId = null) {
  const metadataBySignalKind = buildSignalMetadataLookup(manifest, cohortId);

  return new Set(
    Array.from(metadataBySignalKind.values())
      .filter(function isAgentLead(entry) {
        return (
          entry.primary_lane === 'agent_default' &&
          entry.default_surface_role === 'lead'
        );
      })
      .map(function toSignalKind(entry) {
        return entry.signal_kind;
      }),
  );
}

export function getSignalCohort(manifest, cohortId = null) {
  const resolvedManifest = manifest ?? buildDefaultSignalCohorts();
  const resolvedCohortId = cohortId ?? resolvedManifest.default_cohort_id;
  const cohort = resolvedManifest.cohorts.find(
    (entry) => entry.cohort_id === resolvedCohortId,
  );

  if (!cohort) {
    throw new Error(`Unknown signal cohort: ${resolvedCohortId}`);
  }

  return cohort;
}

export async function loadSignalCohortManifest(targetPath) {
  if (!targetPath) {
    return buildDefaultSignalCohorts();
  }

  const source = await readFile(targetPath, 'utf8');
  const manifest = JSON.parse(source);
  if (manifest?.schema_version !== 1 || !Array.isArray(manifest?.cohorts)) {
    throw new Error(`Unsupported signal cohort manifest: ${targetPath}`);
  }

  return manifest;
}

export function resolveSignalCohortId({
  cohortId = null,
  fallbackCohortId = null,
  codexBatch = null,
  replayBatch = null,
}) {
  return (
    cohortId ??
    codexBatch?.cohort_id ??
    replayBatch?.cohort_id ??
    fallbackCohortId ??
    null
  );
}

export async function loadSignalCohortContext({
  cohortManifestPath,
  cohortId = null,
  fallbackCohortId = null,
  codexBatch = null,
  replayBatch = null,
}) {
  const resolvedCohortId = resolveSignalCohortId({
    cohortId,
    fallbackCohortId,
    codexBatch,
    replayBatch,
  });
  const cohortManifest = resolvedCohortId
    ? await loadSignalCohortManifest(cohortManifestPath)
    : null;

  return {
    cohortId: resolvedCohortId,
    cohortManifest,
  };
}
