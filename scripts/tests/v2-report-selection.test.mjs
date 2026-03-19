import test from 'node:test';
import assert from 'node:assert/strict';
import { selectLeverageBuckets } from '../lib/v2-report-selection.mjs';

function candidate({
  scope,
  kind,
  trustTier = 'trusted',
  presentationClass = 'structural_debt',
  leverageClass = 'secondary_cleanup',
  severity = 'medium',
  score = 6200,
  roleTags = [],
  metrics = {},
  cutCandidates = [],
}) {
  return {
    scope,
    kind,
    trust_tier: trustTier,
    presentation_class: presentationClass,
    leverage_class: leverageClass,
    severity,
    score_0_10000: score,
    summary: `${scope} ${kind}`,
    impact: `${scope} impact`,
    role_tags: roleTags,
    leverage_reasons: [`${leverageClass}_reason`],
    candidate_split_axes: [],
    related_surfaces: [],
    metrics,
    cut_candidates: cutCandidates,
  };
}

test('selectLeverageBuckets keeps architecture and local targets ahead of hardening and tooling', function () {
  const findingsPayload = {
    finding_details: [
      candidate({
        scope: 'src/store/store.ts',
        kind: 'dependency_sprawl',
        leverageClass: 'architecture_signal',
        severity: 'high',
      }),
      candidate({
        scope: 'src/components/TaskPanel.tsx',
        kind: 'dependency_sprawl',
        leverageClass: 'local_refactor_target',
        severity: 'high',
      }),
      candidate({
        scope: 'src/lib/ipc.ts',
        kind: 'unstable_hotspot',
        presentationClass: 'guarded_facade',
        leverageClass: 'boundary_discipline',
      }),
      candidate({
        scope: 'src/App.tsx',
        kind: 'dependency_sprawl',
        leverageClass: 'regrowth_watchpoint',
      }),
      candidate({
        scope: 'src/components/terminal-view/terminal-session.ts',
        kind: 'unstable_hotspot',
        leverageClass: 'secondary_cleanup',
      }),
      candidate({
        scope: 'ConnectionBannerState',
        kind: 'closed_domain_exhaustiveness',
        presentationClass: 'hardening_note',
        leverageClass: 'hardening_note',
      }),
      candidate({
        scope: 'scripts/session-stress.mjs',
        kind: 'large_file',
        presentationClass: 'tooling_debt',
        leverageClass: 'tooling_debt',
      }),
    ],
    watchpoints: [
      candidate({
        scope: 'cycle:store-app',
        kind: 'cycle_cluster',
        trustTier: 'watchpoint',
        presentationClass: 'watchpoint',
        leverageClass: 'architecture_signal',
        severity: 'high',
      }),
    ],
    debt_signals: [],
    debt_clusters: [],
  };

  const buckets = selectLeverageBuckets(findingsPayload);

  assert.deepEqual(
    buckets.summary_candidates.map((entry) => entry.leverage_class),
    [
      'architecture_signal',
      'local_refactor_target',
      'boundary_discipline',
      'regrowth_watchpoint',
      'secondary_cleanup',
    ],
  );
  assert.equal(buckets.architecture_signals[0].scope, 'cycle:store-app');
  assert(buckets.architecture_signals.some((entry) => entry.scope === 'src/store/store.ts'));
  assert.equal(buckets.local_refactor_targets[0].scope, 'src/components/TaskPanel.tsx');
  assert.equal(buckets.boundary_discipline[0].scope, 'src/lib/ipc.ts');
  assert.equal(buckets.regrowth_watchpoints[0].scope, 'src/App.tsx');
  assert.equal(
    buckets.secondary_cleanup[0].scope,
    'src/components/terminal-view/terminal-session.ts',
  );
  assert.equal(buckets.hardening_notes[0].scope, 'ConnectionBannerState');
  assert.equal(buckets.tooling_debt[0].scope, 'scripts/session-stress.mjs');
  assert(!buckets.summary_candidates.some((entry) => entry.scope === 'ConnectionBannerState'));
  assert(!buckets.summary_candidates.some((entry) => entry.scope === 'scripts/session-stress.mjs'));
  assert(buckets.architecture_signals.some((entry) => entry.scope === 'cycle:store-app'));
  assert(!buckets.trusted_watchpoints.some((entry) => entry.scope === 'cycle:store-app'));
});

test('selectLeverageBuckets deduplicates summary scopes and keeps compatibility aliases', function () {
  const findingsPayload = {
    finding_details: [
      candidate({
        scope: 'src/store/store.ts',
        kind: 'dependency_sprawl',
        leverageClass: 'architecture_signal',
        severity: 'high',
      }),
      candidate({
        scope: 'src/store/store.ts',
        kind: 'large_file',
        leverageClass: 'secondary_cleanup',
        severity: 'medium',
      }),
    ],
    watchpoints: [],
    debt_signals: [],
    debt_clusters: [],
  };

  const buckets = selectLeverageBuckets(findingsPayload);

  assert.equal(buckets.summary_candidates.length, 1);
  assert.equal(buckets.summary_candidates[0].scope, 'src/store/store.ts');
  assert.deepEqual(
    buckets.lead_candidates.map((entry) => entry.scope),
    buckets.summary_candidates.map((entry) => entry.scope),
  );
  assert.deepEqual(
    buckets.secondary_hotspots.map((entry) => entry.scope),
    buckets.secondary_cleanup.map((entry) => entry.scope).slice(0, 2),
  );
});

test('selectLeverageBuckets prefers contained local refactor targets over broader peers', function () {
  const findingsPayload = {
    finding_details: [
      candidate({
        scope: 'src/components/TaskPanel.tsx',
        kind: 'dependency_sprawl',
        leverageClass: 'local_refactor_target',
        severity: 'medium',
        score: 6400,
        roleTags: ['facade_with_extracted_owners', 'guarded_seam'],
        metrics: {
          fan_in: 4,
          fan_out: 16,
          guardrail_test_count: 1,
          cycle_size: 0,
          max_complexity: 6,
        },
      }),
      candidate({
        scope: 'src/components/ReviewPanel.tsx',
        kind: 'dependency_sprawl',
        leverageClass: 'local_refactor_target',
        severity: 'high',
        score: 7900,
        roleTags: ['facade_with_extracted_owners', 'guarded_seam'],
        metrics: {
          fan_in: 20,
          fan_out: 12,
          guardrail_test_count: 1,
          cycle_size: 11,
          max_complexity: 9,
        },
      }),
    ],
    watchpoints: [],
    debt_signals: [],
    debt_clusters: [],
  };

  const buckets = selectLeverageBuckets(findingsPayload);

  assert.equal(buckets.local_refactor_targets[0].scope, 'src/components/TaskPanel.tsx');
  assert.equal(buckets.local_refactor_targets[1].scope, 'src/components/ReviewPanel.tsx');
  assert(
    buckets.local_refactor_targets[0].ranking_reasons.includes('contained_refactor_surface'),
  );
  assert.equal(buckets.local_refactor_targets[0].score_band, 'moderate_signal');
  assert.equal(buckets.local_refactor_targets[1].score_band, 'high_signal');
});

test('selectLeverageBuckets surfaces generic ranking reasons for cycle-heavy architecture signals', function () {
  const findingsPayload = {
    finding_details: [
      candidate({
        scope: 'src/store/store.ts',
        kind: 'unstable_hotspot',
        leverageClass: 'architecture_signal',
        severity: 'high',
        score: 9100,
        roleTags: ['component_barrel', 'guarded_boundary'],
        metrics: {
          fan_in: 28,
          cycle_size: 13,
          cut_candidate_count: 1,
        },
        cutCandidates: [
          {
            source: 'src/store/store.ts',
            target: 'src/app/task-workflows.ts',
            seam_kind: 'guarded_app_store_boundary',
            reduction_file_count: 9,
            remaining_cycle_size: 4,
          },
        ],
      }),
    ],
    watchpoints: [],
    debt_signals: [],
    debt_clusters: [],
  };

  const buckets = selectLeverageBuckets(findingsPayload);
  const candidateEntry = buckets.architecture_signals[0];

  assert.equal(candidateEntry.scope, 'src/store/store.ts');
  assert.equal(candidateEntry.score_band, 'very_high_signal');
  assert(candidateEntry.ranking_reasons.includes('shared_barrel_boundary_hub'));
  assert(candidateEntry.ranking_reasons.includes('high_leverage_cut_candidate'));
});
