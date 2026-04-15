import assert from 'node:assert/strict';
import { execFile as execFileCallback } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execFile = promisify(execFileCallback);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

async function runNodeScript(scriptRelativePath, args) {
  await execFile(process.execPath, [path.join(repoRoot, scriptRelativePath), ...args], {
    cwd: repoRoot,
  });
}

test('build-signal-scorecard reuses latest calibration batch artifacts when batch args are omitted', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-latest-scorecard-'));
  try {
    const repoLabel = 'self-repo';
    const outputPath = path.join(tempRoot, 'signal-scorecard.json');
    const sessionTelemetryPath = path.join(tempRoot, 'session-telemetry.json');
    const codexBatchPath = path.join(tempRoot, 'latest-codex-batch.json');
    const latestPointerPath = path.join(tempRoot, '.sentrux', 'evals', repoLabel, 'latest.json');
    const latestSummaryPath = path.join(tempRoot, 'repo-calibration-loop.json');

    await writeJson(sessionTelemetryPath, {
      repo_label: repoLabel,
      summary: {
        session_count: 0,
      },
      signals: [],
    });
    await writeJson(codexBatchPath, {
      repo_label: repoLabel,
      results: [
        {
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: {
            initial_action_kinds: ['clone_propagation_drift'],
            initial_top_action_kind: 'clone_propagation_drift',
            final_session_clean: true,
          },
        },
      ],
    });
    await writeJson(latestSummaryPath, {
      cohort_id: 'agent-loop-core',
      artifacts: {
        codex_batch_json: codexBatchPath,
        replay_batch_json: null,
      },
    });
    await writeJson(latestPointerPath, {
      summary_json: latestSummaryPath,
      latest_output_dir: tempRoot,
    });

    await runNodeScript('scripts/evals/build-signal-scorecard.mjs', [
      '--repo-root',
      tempRoot,
      '--repo-label',
      repoLabel,
      '--session-telemetry',
      sessionTelemetryPath,
      '--output-json',
      outputPath,
    ]);

    const scorecard = JSON.parse(await readFile(outputPath, 'utf8'));

    assert.equal(scorecard.summary.kpis.session_trial_count, 1);
    assert.equal(scorecard.summary.coverage.has_session_trials, true);
    assert.equal(scorecard.signals[0].signal_kind, 'clone_propagation_drift');
    assert.equal(scorecard.signals[0].live_session_trial_count, 1);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('build-signal-scorecard infers repo label for latest calibration fallback when --repo-label is omitted', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-latest-scorecard-inferred-label-'));
  try {
    const repoLabel = 'self-repo';
    const outputPath = path.join(tempRoot, 'signal-scorecard.json');
    const sessionTelemetryPath = path.join(tempRoot, 'session-telemetry.json');
    const codexBatchPath = path.join(tempRoot, 'latest-codex-batch.json');
    const latestPointerPath = path.join(tempRoot, '.sentrux', 'evals', repoLabel, 'latest.json');
    const latestSummaryPath = path.join(tempRoot, 'repo-calibration-loop.json');

    await writeJson(sessionTelemetryPath, {
      repo_label: repoLabel,
      summary: {
        session_count: 0,
      },
      signals: [],
    });
    await writeJson(codexBatchPath, {
      repo_label: repoLabel,
      results: [
        {
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: {
            initial_action_kinds: ['clone_propagation_drift'],
            initial_top_action_kind: 'clone_propagation_drift',
            final_session_clean: true,
          },
        },
      ],
    });
    await writeJson(latestSummaryPath, {
      cohort_id: 'agent-loop-core',
      artifacts: {
        codex_batch_json: codexBatchPath,
        replay_batch_json: null,
      },
    });
    await writeJson(latestPointerPath, {
      summary_json: latestSummaryPath,
      latest_output_dir: tempRoot,
    });

    await runNodeScript('scripts/evals/build-signal-scorecard.mjs', [
      '--repo-root',
      tempRoot,
      '--session-telemetry',
      sessionTelemetryPath,
      '--output-json',
      outputPath,
    ]);

    const scorecard = JSON.parse(await readFile(outputPath, 'utf8'));

    assert.equal(scorecard.repo_label, repoLabel);
    assert.equal(scorecard.summary.kpis.session_trial_count, 1);
    assert.equal(scorecard.summary.coverage.has_session_trials, true);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('build-signal-backlog reuses latest calibration batch artifacts when batch args are omitted', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-latest-backlog-'));
  try {
    const repoLabel = 'self-repo';
    const outputPath = path.join(tempRoot, 'signal-backlog.json');
    const scorecardPath = path.join(tempRoot, 'signal-scorecard.json');
    const codexBatchPath = path.join(tempRoot, 'latest-codex-batch.json');
    const latestPointerPath = path.join(tempRoot, '.sentrux', 'evals', repoLabel, 'latest.json');
    const latestSummaryPath = path.join(tempRoot, 'repo-calibration-loop.json');

    await writeJson(scorecardPath, {
      repo_label: repoLabel,
      signals: [
        {
          signal_kind: 'clone_propagation_drift',
          promotion_status: 'watchpoint',
          promotion_recommendation: 'improve_fix_guidance',
          session_clean_rate: 0.5,
          session_trial_count: 1,
          session_trial_miss_rate: 0,
        },
      ],
      summary: {
        weak_signal_count: 1,
      },
    });
    await writeJson(codexBatchPath, {
      repo_label: repoLabel,
      results: [
        {
          task_id: 'clone-followthrough',
          task_label: 'clone followthrough',
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: {
            initial_action_kinds: ['clone_propagation_drift'],
            initial_top_action_kind: 'clone_propagation_drift',
            top_action_cleared: true,
            final_gate: 'pass',
            final_session_clean: true,
            followup_regression_introduced: false,
          },
        },
      ],
    });
    await writeJson(latestSummaryPath, {
      cohort_id: 'agent-loop-core',
      artifacts: {
        codex_batch_json: codexBatchPath,
        replay_batch_json: null,
      },
    });
    await writeJson(latestPointerPath, {
      summary_json: latestSummaryPath,
      latest_output_dir: tempRoot,
    });

    await runNodeScript('scripts/evals/build-signal-backlog.mjs', [
      '--repo-root',
      tempRoot,
      '--repo-label',
      repoLabel,
      '--scorecard',
      scorecardPath,
      '--cohort-manifest',
      path.join(repoRoot, 'docs/v2/evals/signal-cohorts.json'),
      '--cohort-id',
      'agent-loop-core',
      '--output-json',
      outputPath,
    ]);

    const backlog = JSON.parse(await readFile(outputPath, 'utf8'));

    assert.equal(backlog.summary.live_clean_rate, 1);
    assert.equal(backlog.summary.replay_clean_rate, null);
    assert.equal(backlog.summary.live_miss_count, 0);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('run-signal-calibration writes stable companion artifacts for repo-local output dirs', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-run-signal-calibration-'));
  try {
    const repoLabel = 'self-repo';
    const outputDir = path.join(tempRoot, '.sentrux', 'evals', repoLabel);
    const sessionEventsPath = path.join(tempRoot, 'agent-session-events.jsonl');
    const defectReportPath = path.join(tempRoot, 'defect-report.json');
    const codexBatchPath = path.join(tempRoot, 'latest-codex-batch.json');
    const latestPointerPath = path.join(outputDir, 'latest.json');
    const latestSummaryPath = path.join(tempRoot, 'repo-calibration-loop.json');

    await writeFile(sessionEventsPath, '', 'utf8');
    await writeJson(defectReportPath, {
      repo_label: repoLabel,
      defects: [
        {
          id: 'clone-drift',
          signal_kind: 'clone_propagation_drift',
          signal_family: 'clone',
          promotion_status: 'watchpoint',
          blocking_intent: 'watchpoint',
        },
      ],
      results: [
        {
          defect_id: 'clone-drift',
          detected: true,
          check: { supported: true, matched: true },
        },
      ],
    });
    await writeJson(codexBatchPath, {
      repo_label: repoLabel,
      results: [
        {
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: {
            initial_action_kinds: ['clone_propagation_drift'],
            initial_top_action_kind: 'clone_propagation_drift',
            final_session_clean: true,
          },
        },
      ],
    });
    await writeJson(latestSummaryPath, {
      cohort_id: 'agent-loop-core',
      artifacts: {
        codex_batch_json: codexBatchPath,
        replay_batch_json: null,
      },
    });
    await writeJson(latestPointerPath, {
      summary_json: latestSummaryPath,
      latest_output_dir: tempRoot,
    });

    await runNodeScript('scripts/evals/run-signal-calibration.mjs', [
      '--repo-root',
      tempRoot,
      '--repo-label',
      repoLabel,
      '--session-events',
      sessionEventsPath,
      '--defect-report',
      defectReportPath,
      '--output-dir',
      outputDir,
    ]);

    const stableScorecard = JSON.parse(
      await readFile(path.join(outputDir, 'signal-scorecard.json'), 'utf8'),
    );
    const prefixedScorecard = JSON.parse(
      await readFile(path.join(outputDir, `${repoLabel}-signal-scorecard.json`), 'utf8'),
    );

    assert.equal(stableScorecard.summary.kpis.session_trial_count, 1);
    assert.equal(prefixedScorecard.summary.kpis.session_trial_count, 1);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('run-signal-calibration infers repo label for latest calibration lookup and stable companion names', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-run-signal-calibration-inferred-label-'));
  try {
    const repoLabel = 'self-repo';
    const outputDir = path.join(tempRoot, '.sentrux', 'evals', repoLabel);
    const sessionEventsPath = path.join(tempRoot, 'agent-session-events.jsonl');
    const defectReportPath = path.join(tempRoot, 'defect-report.json');
    const codexBatchPath = path.join(tempRoot, 'latest-codex-batch.json');
    const latestPointerPath = path.join(outputDir, 'latest.json');
    const latestSummaryPath = path.join(tempRoot, 'repo-calibration-loop.json');

    await writeFile(sessionEventsPath, '', 'utf8');
    await writeJson(defectReportPath, {
      repo_label: repoLabel,
      defects: [
        {
          id: 'clone-drift',
          signal_kind: 'clone_propagation_drift',
          signal_family: 'clone',
          promotion_status: 'watchpoint',
          blocking_intent: 'watchpoint',
        },
      ],
      results: [
        {
          defect_id: 'clone-drift',
          detected: true,
          check: { supported: true, matched: true },
        },
      ],
    });
    await writeJson(codexBatchPath, {
      repo_label: repoLabel,
      results: [
        {
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: {
            initial_action_kinds: ['clone_propagation_drift'],
            initial_top_action_kind: 'clone_propagation_drift',
            final_session_clean: true,
          },
        },
      ],
    });
    await writeJson(latestSummaryPath, {
      cohort_id: 'agent-loop-core',
      artifacts: {
        codex_batch_json: codexBatchPath,
        replay_batch_json: null,
      },
    });
    await writeJson(latestPointerPath, {
      summary_json: latestSummaryPath,
      latest_output_dir: tempRoot,
    });

    await runNodeScript('scripts/evals/run-signal-calibration.mjs', [
      '--repo-root',
      tempRoot,
      '--session-events',
      sessionEventsPath,
      '--defect-report',
      defectReportPath,
      '--output-dir',
      outputDir,
    ]);

    const stableScorecard = JSON.parse(
      await readFile(path.join(outputDir, 'signal-scorecard.json'), 'utf8'),
    );
    const prefixedScorecard = JSON.parse(
      await readFile(path.join(outputDir, `${repoLabel}-signal-scorecard.json`), 'utf8'),
    );

    assert.equal(stableScorecard.repo_label, repoLabel);
    assert.equal(prefixedScorecard.repo_label, repoLabel);
    assert.equal(stableScorecard.summary.kpis.session_trial_count, 1);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
