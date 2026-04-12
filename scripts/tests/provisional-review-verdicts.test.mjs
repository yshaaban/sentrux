import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildProvisionalReviewVerdictReport,
  buildVerdicts,
  parseArgs,
} from '../evals/build-provisional-review-verdicts.mjs';

test('parseArgs accepts packet, output, repo, and kind filters', function () {
  const args = parseArgs([
    'node',
    'script',
    '--packet',
    'packet.json',
    '--output-json',
    'verdicts.json',
    '--repo',
    'sentrux',
    '--kind',
    'large_file',
  ]);

  assert.equal(args.packetPath, 'packet.json');
  assert.equal(args.outputJsonPath, 'verdicts.json');
  assert.equal(args.repo, 'sentrux');
  assert.deepEqual(args.kinds, ['large_file']);
});

test('buildVerdicts applies known policies and kind filtering', function () {
  const packet = {
    samples: [
      {
        kind: 'large_file',
        scope: 'src/a.rs',
        report_bucket: 'actions',
        summary: 'Large file warning',
      },
      {
        kind: 'touched_clone_family',
        scope: 'src/b.rs',
        report_bucket: 'actions',
        summary: 'Clone context',
      },
    ],
  };

  const verdicts = buildVerdicts(packet, {
    kinds: ['large_file'],
  });

  assert.equal(verdicts.length, 1);
  assert.equal(verdicts[0].kind, 'large_file');
  assert.equal(verdicts[0].category, 'useful_watchpoint');
  assert.equal(verdicts[0].expected_trust_tier, 'watchpoint');
  assert.match(verdicts[0].engineer_note, /Large file warning/);
});

test('buildProvisionalReviewVerdictReport marks bootstrap verdicts as provisional', function () {
  const packet = {
    generated_at: '2026-04-11T00:00:00.000Z',
    repo_root: '/tmp/sentrux',
    samples: [
      {
        kind: 'forbidden_raw_read',
        scope: 'src/a.ts',
        report_bucket: 'actions',
        summary: 'Raw read crosses boundary',
      },
    ],
  };

  const report = buildProvisionalReviewVerdictReport(packet, {
    packetPath: '/tmp/check-review-packet.json',
    outputJsonPath: null,
    sourceReport: '/tmp/check-review-packet.md',
    sourceFeedback: null,
    repo: 'sentrux',
    kinds: [],
  });

  assert.equal(report.repo, 'sentrux');
  assert.equal(report.provisional, true);
  assert.equal(report.source_report, '/tmp/check-review-packet.md');
  assert.equal(report.verdicts.length, 1);
  assert.equal(report.verdicts[0].expected_presentation_class, 'boundary_discipline');
});
