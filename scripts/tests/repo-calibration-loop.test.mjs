import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildScorecardArgs,
  buildReviewArgs,
  selectReviewVerdictsPath,
} from '../evals/run-repo-calibration-loop.mjs';

test('buildReviewArgs fails fast when codex review source lacks a live batch', function () {
  assert.throws(
    () =>
      buildReviewArgs(
        {
          repo_root: '/tmp/sentrux',
          review_source: 'codex',
        },
        '/tmp/check-review-packet.json',
        '/tmp/check-review-packet.md',
        null,
        '/tmp/replay-batch.json',
      ),
    /review_source "codex" requires a live batch artifact/,
  );
});

test('buildReviewArgs fails fast when replay review source lacks a replay batch', function () {
  assert.throws(
    () =>
      buildReviewArgs(
        {
          repo_root: '/tmp/sentrux',
          review_source: 'replay',
        },
        '/tmp/check-review-packet.json',
        '/tmp/check-review-packet.md',
        '/tmp/codex-batch.json',
        null,
      ),
    /review_source "replay" requires a replay batch artifact/,
  );
});

test('buildReviewArgs keeps repo-root explicit and includes available sources', function () {
  const args = buildReviewArgs(
    {
      repo_root: '/tmp/sentrux',
      review_tool: 'check',
    },
    '/tmp/check-review-packet.json',
    '/tmp/check-review-packet.md',
    '/tmp/codex-batch.json',
    '/tmp/replay-batch.json',
  );

  assert.deepEqual(args.slice(0, 8), [
    '--repo-root',
    '/tmp/sentrux',
    '--tool',
    'check',
    '--output-json',
    '/tmp/check-review-packet.json',
    '--output-md',
    '/tmp/check-review-packet.md',
  ]);
  assert.match(args.join(' '), /--codex-batch \/tmp\/codex-batch\.json/);
  assert.match(args.join(' '), /--replay-batch \/tmp\/replay-batch\.json/);
});

test('selectReviewVerdictsPath prefers curated input over generated output', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-verdict-selection-'));
  try {
    const inputPath = path.join(tempRoot, 'review-verdicts.json');
    const outputPath = path.join(tempRoot, 'generated-review-verdicts.json');
    await writeFile(inputPath, '{}\n', 'utf8');
    await writeFile(outputPath, '{}\n', 'utf8');

    const selectedPath = await selectReviewVerdictsPath(outputPath, inputPath);

    assert.equal(selectedPath, inputPath);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildScorecardArgs includes live and replay batch artifacts when present', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-scorecard-args-'));
  try {
    const codexBatchPath = path.join(tempRoot, 'codex-session-batch.json');
    const replayBatchPath = path.join(tempRoot, 'diff-replay-batch.json');
    const defectReportPath = path.join(tempRoot, 'defect-report.json');
    const remediationReportPath = path.join(tempRoot, 'remediation-report.json');
    const benchmarkPath = path.join(tempRoot, 'benchmark.json');
    const reviewVerdictsPath = path.join(tempRoot, 'review-verdicts.json');

    await Promise.all([
      writeFile(codexBatchPath, '{}\n', 'utf8'),
      writeFile(replayBatchPath, '{}\n', 'utf8'),
      writeFile(defectReportPath, '{}\n', 'utf8'),
      writeFile(remediationReportPath, '{}\n', 'utf8'),
      writeFile(benchmarkPath, '{}\n', 'utf8'),
      writeFile(reviewVerdictsPath, '{}\n', 'utf8'),
    ]);

    const args = await buildScorecardArgs({
      manifest: {
        repo_root: '/tmp/sentrux',
        repo_label: 'sentrux',
      },
      repoRootPath: '/tmp/sentrux',
      mergedTelemetryJsonPath: '/tmp/session-telemetry-summary.json',
      scorecardJsonPath: '/tmp/signal-scorecard.json',
      scorecardMarkdownPath: '/tmp/signal-scorecard.md',
      codexBatchPath,
      replayBatchPath,
      defectReportPath,
      selectedReviewVerdictsPath: reviewVerdictsPath,
      remediationReportPath,
      benchmarkPath,
    });

    assert.match(args.join(' '), /--codex-batch .*codex-session-batch\.json/);
    assert.match(args.join(' '), /--replay-batch .*diff-replay-batch\.json/);
    assert.match(args.join(' '), /--review-verdicts .*review-verdicts\.json/);
    assert.match(args.join(' '), /--defect-report .*defect-report\.json/);
    assert.match(args.join(' '), /--remediation-report .*remediation-report\.json/);
    assert.match(args.join(' '), /--benchmark .*benchmark\.json/);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
