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
      rationale: 'Useful regrowth warning that should be tracked for actionability, not just correctness.',
    },
    {
      signal_kind: 'session_introduced_clone',
      signal_family: 'clone',
      promotion_status: 'watchpoint',
      rationale:
        'Fresh duplication introduced in the current session is a high-ROI agent mistake signal when it stays session-scoped and concrete.',
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
          'Initial high-ROI signal cohort for calibrating fast patch feedback in the coding loop.',
        signals: buildAgentLoopCoreSignals(),
        next_candidates: ['zero_config_boundary_violation', 'incomplete_propagation'],
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
