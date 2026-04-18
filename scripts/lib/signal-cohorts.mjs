import { readFile } from 'node:fs/promises';

function buildAgentLoopCoreSignals() {
  return [
    {
      signal_kind: 'closed_domain_exhaustiveness',
      signal_family: 'obligation',
      promotion_status: 'trusted',
      rationale: 'High-value semantic completeness signal for TypeScript agent edits.',
    },
    {
      signal_kind: 'forbidden_raw_read',
      signal_family: 'rules',
      promotion_status: 'trusted',
      rationale: 'Strong explicit-rule regression that should block unsafe agent shortcuts.',
    },
    {
      signal_kind: 'missing_test_coverage',
      signal_family: 'structural',
      promotion_status: 'watchpoint',
      rationale: 'Common omission signal that should stay visible without over-blocking.',
    },
    {
      signal_kind: 'large_file',
      signal_family: 'structural',
      promotion_status: 'watchpoint',
      rationale: 'Regrowth warning that is valuable when it stays actionable.',
    },
    {
      signal_kind: 'session_introduced_clone',
      signal_family: 'clone',
      promotion_status: 'watchpoint',
      rationale:
        'Fresh duplication introduced in the current session is a high-ROI agent mistake signal when it stays session-scoped and concrete.',
    },
    {
      signal_kind: 'clone_propagation_drift',
      signal_family: 'clone',
      promotion_status: 'watchpoint',
      rationale:
        'Editing one side of an existing duplicate without syncing its sibling is a common agent followthrough miss and should stay visible in the fast loop.',
    },
    {
      signal_kind: 'incomplete_propagation',
      signal_family: 'obligation',
      promotion_status: 'watchpoint',
      rationale:
        'Explicit contract-surface propagation misses are sharp enough to calibrate as a conservative watchpoint before broader propagation heuristics.',
    },
    {
      signal_kind: 'zero_config_boundary_violation',
      signal_family: 'rules',
      promotion_status: 'watchpoint',
      rationale:
        'Direct zero-config boundary violations now have deterministic fixture-backed replay coverage plus seeded detection and remediation evidence, so they should stay visible in the fast loop as a maintained watchpoint.',
    },
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
          'Initial high-ROI signal cohort for fast coding-agent feedback calibration.',
        signals: buildAgentLoopCoreSignals(),
        next_candidates: [
          'multi_writer_concept',
          'forbidden_writer',
          'writer_outside_allowlist',
        ],
      },
    ],
  };
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
