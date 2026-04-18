import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildScorecardArgs,
  buildReviewArgs,
  selectReviewVerdictsPath,
  selectSessionVerdictsPath,
} from '../evals/run-repo-calibration-loop.mjs';
import { acquireLoopLock } from '../lib/repo-calibration-loop-support/runtime.mjs';
import { buildSummaryMarkdown } from '../lib/repo-calibration-loop-support/summary.mjs';

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
    const reviewPacketPath = path.join(tempRoot, 'check-review-packet.json');
    const codexBatchPath = path.join(tempRoot, 'codex-session-batch.json');
    const replayBatchPath = path.join(tempRoot, 'diff-replay-batch.json');
    const defectReportPath = path.join(tempRoot, 'defect-report.json');
    const remediationReportPath = path.join(tempRoot, 'remediation-report.json');
    const benchmarkPath = path.join(tempRoot, 'benchmark.json');
    const reviewVerdictsPath = path.join(tempRoot, 'review-verdicts.json');
    const sessionVerdictsPath = path.join(tempRoot, 'session-verdicts.json');

    await Promise.all([
      writeFile(reviewPacketPath, '{}\n', 'utf8'),
      writeFile(codexBatchPath, '{}\n', 'utf8'),
      writeFile(replayBatchPath, '{}\n', 'utf8'),
      writeFile(defectReportPath, '{}\n', 'utf8'),
      writeFile(remediationReportPath, '{}\n', 'utf8'),
      writeFile(benchmarkPath, '{}\n', 'utf8'),
      writeFile(reviewVerdictsPath, '{}\n', 'utf8'),
      writeFile(sessionVerdictsPath, '{}\n', 'utf8'),
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
      reviewPacketJsonPath: reviewPacketPath,
      codexBatchPath,
      replayBatchPath,
      defectReportPath,
      selectedReviewVerdictsPath: reviewVerdictsPath,
      selectedSessionVerdictsPath: sessionVerdictsPath,
      remediationReportPath,
      benchmarkPath,
    });

    assert.match(args.join(' '), /--review-packet .*check-review-packet\.json/);
    assert.match(args.join(' '), /--codex-batch .*codex-session-batch\.json/);
    assert.match(args.join(' '), /--replay-batch .*diff-replay-batch\.json/);
    assert.match(args.join(' '), /--review-verdicts .*review-verdicts\.json/);
    assert.match(args.join(' '), /--session-verdicts .*session-verdicts\.json/);
    assert.match(args.join(' '), /--defect-report .*defect-report\.json/);
    assert.match(args.join(' '), /--remediation-report .*remediation-report\.json/);
    assert.match(args.join(' '), /--benchmark .*benchmark\.json/);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildScorecardArgs propagates cohort metadata arguments when provided', async function () {
  const args = await buildScorecardArgs({
    manifest: {
      repo_root: '/tmp/sentrux',
      repo_label: 'sentrux',
      cohort_id: 'custom-agent-loop',
    },
    repoRootPath: '/tmp/sentrux',
    mergedTelemetryJsonPath: '/tmp/session-telemetry-summary.json',
    scorecardJsonPath: '/tmp/signal-scorecard.json',
    scorecardMarkdownPath: '/tmp/signal-scorecard.md',
    cohortManifestPath: '/tmp/custom-signal-cohorts.json',
    cohortId: 'custom-agent-loop',
  });

  assert.match(args.join(' '), /--cohort-manifest \/tmp\/custom-signal-cohorts\.json/);
  assert.match(args.join(' '), /--cohort-id custom-agent-loop/);
});

test('selectSessionVerdictsPath prefers curated input over prior stable output', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-session-verdict-selection-'));
  try {
    const inputPath = path.join(tempRoot, 'session-verdicts.json');
    const outputPath = path.join(tempRoot, 'generated-session-verdicts.json');
    await writeFile(inputPath, '{}\n', 'utf8');
    await writeFile(outputPath, '{}\n', 'utf8');

    const selectedPath = await selectSessionVerdictsPath(outputPath, inputPath);

    assert.equal(selectedPath, inputPath);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('acquireLoopLock creates missing parent directories before taking the lock', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-loop-lock-'));
  const lockPath = path.join(tempRoot, '.sentrux', 'evals', '.repo-calibration-demo.lock');

  try {
    const release = await acquireLoopLock(lockPath, {
      repo_id: 'demo',
      output_dir: '/tmp/demo-output',
    });

    const ownerPath = path.join(lockPath, 'owner.json');
    const ownerSource = await readFile(ownerPath, 'utf8');
    const owner = JSON.parse(ownerSource);

    assert.equal(owner.repo_id, 'demo');
    await release();
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildSummaryMarkdown surfaces default-on evidence status explicitly', function () {
  const markdown = buildSummaryMarkdown({
    repo_id: 'demo',
    repo_root: '/tmp/demo',
    generated_at: '2026-04-18T00:00:00.000Z',
    output_dir: '/tmp/out',
    artifacts: {
      codex_batch_json: '/tmp/live.json',
      replay_batch_json: '/tmp/replay.json',
      session_telemetry_json: '/tmp/telemetry.json',
      review_packet_json: '/tmp/review-packet.json',
      session_verdicts_output: '/tmp/session-verdicts.json',
      scorecard_json: '/tmp/scorecard.json',
      session_corpus_json: '/tmp/session-corpus.json',
      backlog_json: '/tmp/backlog.json',
      evidence_review_json: '/tmp/evidence-review.json',
    },
    summary: {
      session_count: 4,
      corpus_session_count: 4,
      total_signals: 12,
      weak_signal_count: 2,
      review_sample_count: 8,
      session_verdict_count: 4,
      live_clean_rate: 0.5,
      replay_clean_rate: 0.25,
      agent_clear_rate: 0.5,
      top_action_follow_rate: 0.75,
      top_action_help_rate: 0.5,
      task_success_rate: 0.5,
      patch_expansion_rate: 0.25,
      intervention_net_value_score: 0.333,
      propagation_escape_rate: 0.25,
      clone_followthrough_escape_rate: 0.25,
      evidence_review_default_on_candidates: 2,
      default_on_ready: false,
      default_on_repo_treatment_ready: true,
      default_on_evidence_scope: 'repo_level',
      evidence_phase_id: 'phase_5_treatment_baseline',
      bounded_adjudication_status: 'scaffold_only',
      bounded_adjudication_decision_count: 3,
      bounded_adjudication_structured_evidence_only: true,
      bounded_adjudication_audit_logging_ready: true,
      bounded_adjudication_auto_apply_enabled: false,
      bounded_adjudication_phase_id: 'phase_3_bounded_llm_adjudication',
      recommended_next_signal: 'incomplete_propagation',
    },
    delta: null,
    warnings: [],
  });

  assert.match(markdown, /default-on candidates: 2/);
  assert.match(markdown, /default-on ready: false/);
  assert.match(markdown, /repo treatment ready: true/);
  assert.match(markdown, /default-on evidence scope: repo_level/);
  assert.match(markdown, /evidence phase: phase_5_treatment_baseline/);
  assert.match(markdown, /bounded adjudication status: scaffold_only/);
  assert.match(markdown, /bounded adjudication decisions: 3/);
  assert.match(markdown, /bounded adjudication structured-only: true/);
  assert.match(markdown, /bounded adjudication auto-apply enabled: false/);
  assert.match(markdown, /bounded adjudication phase: phase_3_bounded_llm_adjudication/);
});
