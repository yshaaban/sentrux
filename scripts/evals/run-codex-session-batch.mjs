#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { mergeSessionTelemetrySummaries } from '../lib/session-telemetry.mjs';
import { getSignalCohort, loadSignalCohortManifest } from '../lib/signal-cohorts.mjs';
import {
  defaultBatchOutputDir,
  loadBatchManifest,
  normalizeExpectedSignalKinds,
  nowIso,
  parseTagList,
  summarizeBundleOutcome,
  writeJson,
} from '../lib/eval-batch.mjs';
import { runCodexSession } from './run-codex-session.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    manifestPath: null,
    cohortManifestPath: path.join(repoRoot, 'docs/v2/evals', 'signal-cohorts.json'),
    cohortId: null,
    outputDir: null,
    concurrency: Number(process.env.EVAL_CONCURRENCY ?? '1'),
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--manifest') {
      index += 1;
      result.manifestPath = argv[index];
      continue;
    }
    if (value === '--cohort-manifest') {
      index += 1;
      result.cohortManifestPath = argv[index];
      continue;
    }
    if (value === '--cohort-id') {
      index += 1;
      result.cohortId = argv[index];
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--concurrency') {
      index += 1;
      result.concurrency = Number(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

async function runWithConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let nextIndex = 0;
  const workers = Array.from({ length: Math.min(concurrency, items.length || 1) }, async () => {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) {
        return;
      }

      results[index] = await worker(items[index], index);
    }
  });

  await Promise.all(workers);
  return results;
}

function resolveTaskTimeoutMs(task, manifest) {
  return task.timeout_ms ?? manifest.timeout_ms ?? Number(process.env.EVAL_TIMEOUT_MS ?? '1800000');
}

function resolveTaskIdleTimeoutMs(task, manifest) {
  return (
    task.idle_timeout_ms ??
    manifest.idle_timeout_ms ??
    Number(process.env.EVAL_IDLE_TIMEOUT_MS ?? '600000')
  );
}

export function buildTaskSessionOptions(task, manifest, manifestDir, sourceRoot, outputDir) {
  return {
    sourceRoot,
    repoLabel: manifest.repo_label,
    taskId: task.task_id,
    taskLabel: task.task_label,
    task: task.prompt ?? null,
    taskFile: task.prompt_file ? path.resolve(manifestDir, task.prompt_file) : null,
    tags: parseTagList(task.tags),
    expectedSignalKinds: normalizeExpectedSignalKinds(
      task.expected_signal_kinds ?? manifest.expected_signal_kinds,
    ),
    expectedFixSurface: task.expected_fix_surface ?? null,
    rulesSource: manifest.rules_source ? path.resolve(manifestDir, manifest.rules_source) : null,
    analysisMode: manifest.analysis_mode ?? 'working_tree',
    model: manifest.model ?? null,
    timeoutMs: resolveTaskTimeoutMs(task, manifest),
    idleTimeoutMs: resolveTaskIdleTimeoutMs(task, manifest),
    pollMs: manifest.poll_ms ?? Number(process.env.EVAL_POLL_MS ?? '4000'),
    outputDir: path.join(outputDir, task.task_id ?? `task-${Date.now()}`),
    codexBin: manifest.codex_bin ?? process.env.CODEX_BIN ?? 'codex',
  };
}

function buildTaskResult(bundle, taskOptions) {
  return {
    status: 'completed',
    task_id: taskOptions.taskId ?? bundle.task_id ?? null,
    task_label: taskOptions.taskLabel ?? bundle.task_label ?? null,
    tags: taskOptions.tags,
    expected_signal_kinds: taskOptions.expectedSignalKinds,
    expected_fix_surface: taskOptions.expectedFixSurface,
    telemetry_summary: bundle.telemetry_summary,
    output_dir: taskOptions.outputDir,
    outcome: summarizeBundleOutcome(bundle),
  };
}

function buildTaskFailure(taskOptions, error, bundle = null) {
  return {
    status: bundle?.status ?? 'failed',
    task_id: taskOptions.taskId ?? bundle?.task_id ?? null,
    task_label: taskOptions.taskLabel ?? bundle?.task_label ?? null,
    tags: taskOptions.tags,
    expected_signal_kinds: taskOptions.expectedSignalKinds,
    expected_fix_surface: taskOptions.expectedFixSurface,
    telemetry_summary: bundle?.telemetry_summary ?? null,
    output_dir: taskOptions.outputDir,
    outcome: bundle ? summarizeBundleOutcome(bundle) : null,
    provider_exit_code: bundle?.provider_run?.exit_code ?? null,
    provider_timed_out: bundle?.provider_run?.timed_out ?? false,
    provider_idle_timed_out: bundle?.provider_run?.idle_timed_out ?? false,
    provider_timeout_phase: bundle?.provider_timeout_phase ?? null,
    error_message: error instanceof Error ? error.message : String(error),
  };
}

function bundleCompleted(bundle) {
  return (bundle?.status ?? 'completed') === 'completed';
}

function telemetrySummaryFromTaskRun(taskRun) {
  if (taskRun?.type === 'result') {
    return taskRun.result?.telemetry_summary ?? null;
  }
  if (taskRun?.type === 'failure') {
    return taskRun.failure?.telemetry_summary ?? null;
  }

  return null;
}

function telemetrySourcePathFromTaskRun(taskRun) {
  const telemetrySummary = telemetrySummaryFromTaskRun(taskRun);
  if (!telemetrySummary) {
    return null;
  }

  const outputDir =
    taskRun?.type === 'result' ? taskRun.result?.output_dir : taskRun.failure?.output_dir;
  if (!outputDir) {
    return null;
  }

  return path.join(outputDir, 'agent-session-events.jsonl');
}

function stripTaskTelemetry(entry) {
  const { telemetry_summary, ...rest } = entry;
  return rest;
}

export function summarizeTaskRuns(taskRuns, sourceRoot) {
  const taskResults = taskRuns
    .filter((entry) => entry?.type === 'result')
    .map((entry) => entry.result);
  const taskFailures = taskRuns
    .filter((entry) => entry?.type === 'failure')
    .map((entry) => entry.failure);
  const summaries = taskRuns.map(telemetrySummaryFromTaskRun).filter(Boolean);
  const sourcePaths = taskRuns.map(telemetrySourcePathFromTaskRun).filter(Boolean);
  const mergedSummary = mergeSessionTelemetrySummaries(summaries, {
    repoRoot: sourceRoot,
    sourcePaths,
  });

  return {
    taskResults,
    taskFailures,
    mergedSummary,
  };
}

async function main() {
  const args = parseArgs(process.argv);
  const manifest = await loadBatchManifest(args.manifestPath);
  const manifestDir = path.dirname(path.resolve(args.manifestPath));
  const cohortManifest = await loadSignalCohortManifest(args.cohortManifestPath);
  const cohort = getSignalCohort(cohortManifest, args.cohortId ?? manifest.cohort_id ?? null);
  const sourceRoot = path.resolve(manifest.repo_root);
  const outputDir = path.resolve(
    args.outputDir ?? defaultBatchOutputDir(sourceRoot, 'codex-batch', manifest.batch_id ?? cohort.cohort_id),
  );

  const taskRuns = await runWithConcurrency(
    manifest.tasks ?? [],
    args.concurrency,
    async (task) => {
      const taskOptions = buildTaskSessionOptions(
        task,
        manifest,
        manifestDir,
        sourceRoot,
        outputDir,
      );
      console.log(`Running Codex batch task ${taskOptions.taskId ?? taskOptions.taskLabel ?? 'unknown-task'}`);
      try {
        const bundle = await runCodexSession(taskOptions);
        if (!bundleCompleted(bundle)) {
          return {
            type: 'failure',
            failure: buildTaskFailure(
              taskOptions,
              new Error(`Provider run ended with status ${bundle.status}`),
              bundle,
            ),
          };
        }

        return {
          type: 'result',
          result: buildTaskResult(bundle, taskOptions),
        };
      } catch (error) {
        return {
          type: 'failure',
          failure: buildTaskFailure(taskOptions, error),
        };
      }
    },
  );
  const { taskResults, taskFailures, mergedSummary } = summarizeTaskRuns(taskRuns, sourceRoot);
  const batchResult = {
    schema_version: 1,
    generated_at: nowIso(),
    batch_id: manifest.batch_id ?? cohort.cohort_id,
    repo_label: manifest.repo_label ?? path.basename(sourceRoot),
    repo_root: sourceRoot,
    cohort_id: cohort.cohort_id,
    active_signal_kinds: cohort.signals.map((signal) => signal.signal_kind),
    task_count: (manifest.tasks ?? []).length,
    success_count: taskResults.length,
    failure_count: taskFailures.length,
    results: taskResults.map(stripTaskTelemetry),
    failures: taskFailures.map(stripTaskTelemetry),
    telemetry_summary: mergedSummary,
  };

  await writeJson(path.join(outputDir, 'codex-session-batch.json'), batchResult);
  await writeJson(path.join(outputDir, 'session-telemetry-summary.json'), mergedSummary);

  console.log(
    `Captured ${taskResults.length} successful Codex task session(s) with ${taskFailures.length} failure(s) for cohort ${cohort.cohort_id}. Artifacts written to ${outputDir}`,
  );
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
