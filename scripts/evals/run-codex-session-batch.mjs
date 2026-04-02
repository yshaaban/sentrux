#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';
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

function buildTaskSessionOptions(task, manifest, manifestDir, sourceRoot, outputDir) {
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
    timeoutMs: manifest.timeout_ms ?? Number(process.env.EVAL_TIMEOUT_MS ?? '1800000'),
    pollMs: manifest.poll_ms ?? Number(process.env.EVAL_POLL_MS ?? '4000'),
    outputDir: path.join(outputDir, task.task_id ?? `task-${Date.now()}`),
    codexBin: manifest.codex_bin ?? process.env.CODEX_BIN ?? 'codex',
  };
}

function buildTaskResult(bundle) {
  return {
    task_id: bundle.task_id,
    task_label: bundle.task_label,
    tags: bundle.tags,
    expected_signal_kinds: bundle.expected_signal_kinds,
    telemetry_summary: bundle.telemetry_summary,
    output_dir: path.dirname(bundle.prompt_path),
    outcome: summarizeBundleOutcome(bundle),
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

  const taskResults = await runWithConcurrency(
    manifest.tasks ?? [],
    args.concurrency,
    async (task) => {
      const bundle = await runCodexSession(
        buildTaskSessionOptions(task, manifest, manifestDir, sourceRoot, outputDir),
      );
      return buildTaskResult(bundle);
    },
  );

  const summaries = taskResults.map((result) => result.telemetry_summary);
  const mergedSummary = mergeSessionTelemetrySummaries(summaries, {
    repoRoot: sourceRoot,
    sourcePaths: taskResults.map((result) => path.join(result.output_dir, 'agent-session-events.jsonl')),
  });
  const batchResult = {
    schema_version: 1,
    generated_at: nowIso(),
    batch_id: manifest.batch_id ?? cohort.cohort_id,
    repo_label: manifest.repo_label ?? path.basename(sourceRoot),
    repo_root: sourceRoot,
    cohort_id: cohort.cohort_id,
    active_signal_kinds: cohort.signals.map((signal) => signal.signal_kind),
    task_count: taskResults.length,
    results: taskResults.map(({ telemetry_summary, ...result }) => result),
    telemetry_summary: mergedSummary,
  };

  await writeJson(path.join(outputDir, 'codex-session-batch.json'), batchResult);
  await writeJson(path.join(outputDir, 'session-telemetry-summary.json'), mergedSummary);

  console.log(
    `Captured ${taskResults.length} Codex task session(s) for cohort ${cohort.cohort_id}. Artifacts written to ${outputDir}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
