import assert from 'node:assert/strict';
import test from 'node:test';

const { selectDeadPrivateCandidatesFromPayload } = await import('../evals/review_dead_private.mjs');

test('selectDeadPrivateCandidatesFromPayload prefers the canonical debt lane', function () {
  const payload = {
    experimental_debt_signals: [
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/canonical.ts',
        summary: 'canonical',
      },
      {
        kind: 'other_kind',
        scope: 'src/ignored.ts',
        summary: 'ignored',
      },
    ],
    experimental_findings: [
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/legacy.ts',
        summary: 'legacy',
      },
    ],
  };

  const result = selectDeadPrivateCandidatesFromPayload(payload);

  assert.equal(result.source_lane, 'experimental_debt_signals');
  assert.equal(result.source_lane_count, 2);
  assert.deepEqual(result.candidates.map((candidate) => candidate.scope), ['src/canonical.ts']);
});

test('selectDeadPrivateCandidatesFromPayload falls back to the legacy lane when needed', function () {
  const payload = {
    experimental_findings: [
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/legacy.ts',
        summary: 'legacy',
      },
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/legacy.ts',
        summary: 'duplicate',
      },
    ],
  };

  const result = selectDeadPrivateCandidatesFromPayload(payload);

  assert.equal(result.source_lane, 'experimental_findings');
  assert.equal(result.source_lane_count, 2);
  assert.deepEqual(result.candidates.map((candidate) => candidate.scope), ['src/legacy.ts']);
});
