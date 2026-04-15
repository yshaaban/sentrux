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
  assert.equal(result.source_lane_count, 1);
  assert.equal(result.canonical_candidate_count, 1);
  assert.equal(result.legacy_candidate_count, 1);
  assert.equal(result.overlapping_candidate_count, 0);
  assert.equal(result.legacy_only_candidate_count, 1);
  assert.equal(result.reviewer_lane_status, 'canonical_with_legacy_watchlist');
  assert.match(result.reviewer_lane_reason, /canonical experimental_debt_signals lane is the reviewer queue/);
  assert.deepEqual(result.candidates.map((candidate) => candidate.scope), ['src/canonical.ts']);
  assert.deepEqual(
    result.legacy_only_candidates.map((candidate) => candidate.scope),
    ['src/legacy.ts'],
  );
  assert.equal(result.considered_lanes[0].dead_private_candidate_count, 1);
  assert.equal(result.considered_lanes[0].selected_for_review, true);
  assert.equal(result.considered_lanes[1].dead_private_candidate_count, 1);
  assert.equal(result.considered_lanes[1].selected_for_review, false);
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
  assert.equal(result.canonical_candidate_count, 0);
  assert.equal(result.legacy_candidate_count, 2);
  assert.equal(result.overlapping_candidate_count, 0);
  assert.equal(result.legacy_only_candidate_count, 2);
  assert.equal(result.reviewer_lane_status, 'legacy_fallback');
  assert.match(result.reviewer_lane_reason, /falls back to experimental_findings/);
  assert.deepEqual(result.candidates.map((candidate) => candidate.scope), ['src/legacy.ts']);
  assert.equal(result.considered_lanes[0].selected_for_review, false);
  assert.equal(result.considered_lanes[1].selected_for_review, true);
});

test('selectDeadPrivateCandidatesFromPayload tracks overlapping legacy candidates separately', function () {
  const payload = {
    experimental_debt_signals: [
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/shared.ts',
      },
    ],
    experimental_findings: [
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/shared.ts',
      },
      {
        kind: 'dead_private_code_cluster',
        scope: 'src/watchlist.ts',
      },
    ],
  };

  const result = selectDeadPrivateCandidatesFromPayload(payload);

  assert.equal(result.canonical_candidate_count, 1);
  assert.equal(result.legacy_candidate_count, 2);
  assert.equal(result.overlapping_candidate_count, 1);
  assert.equal(result.legacy_only_candidate_count, 1);
  assert.deepEqual(
    result.legacy_only_candidates.map((candidate) => candidate.scope),
    ['src/watchlist.ts'],
  );
});
