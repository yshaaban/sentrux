#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runCommand } from '../lib/benchmark-harness.mjs';
import { mergeSessionTelemetrySummaries } from '../lib/session-telemetry.mjs';
import { getSignalCohort, loadSignalCohortManifest } from '../lib/signal-cohorts.mjs';
import {
  defaultBatchOutputDir,
  loadBatchManifest,
  normalizeExpectedSignalKinds,
  nowIso,
  parseTagList,
  resolveManifestPath,
  summarizeBundleOutcome,
  writeJson,
} from '../lib/eval-batch.mjs';
import { runDiffReplay } from './run-diff-replay.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    manifestPath: null,
    cohortManifestPath: path.join(repoRoot, 'docs/v2/evals', 'signal-cohorts.json'),
    cohortId: null,
    outputDir: null,
    maxCount: null,
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
    if (value === '--max-count') {
      index += 1;
      result.maxCount = Number(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

async function gitRead(repoRootPath, args) {
  const result = await runCommand('git', args, { cwd: repoRootPath });
  if (result.exit_code !== 0) {
    throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`);
  }

  return result.stdout.trim();
}

async function resolveReplayItems(manifest, sourceRoot, maxCount) {
  if (Array.isArray(manifest.replays) && manifest.replays.length > 0) {
    if (maxCount) {
      return manifest.replays.slice(0, maxCount);
    }

    return manifest.replays;
  }

  const count = maxCount ?? manifest.max_count ?? 10;
  const log = await gitRead(sourceRoot, [
    'log',
    '--format=%H %P',
    `--max-count=${count}`,
    ...(manifest.rev_range ? [manifest.rev_range] : []),
  ]);

  return log
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => line.trim().split(/\s+/))
    .filter((parts) => parts.length > 1)
    .map(([commit]) => ({
      replay_id: commit.slice(0, 12),
      commit,
      tags: manifest.default_tags ?? [],
      expected_signal_kinds: manifest.expected_signal_kinds ?? [],
    }));
}

function resolveReplayLabel(replay) {
  return replay.replay_id ?? replay.commit?.slice(0, 12) ?? replay.defect_id ?? 'replay';
}

function buildReplayOptions(replay, manifest, manifestDir, sourceRoot, outputDir) {
  const replayLabel = resolveReplayLabel(replay);

  return {
    sourceRoot,
    repoLabel: manifest.repo_label,
    replayId: replay.replay_id ?? replayLabel,
    commit: replay.commit,
    baseCommit: replay.base_commit ?? null,
    defectId: replay.defect_id ?? null,
    fixtureRepo: replay.fixture_repo ?? 'self',
    tags: parseTagList(replay.tags),
    expectedSignalKinds: normalizeExpectedSignalKinds(
      replay.expected_signal_kinds ?? manifest.expected_signal_kinds,
    ),
    expectedFixSurface: replay.expected_fix_surface ?? null,
    rulesSource: manifest.rules_source ? path.resolve(manifestDir, manifest.rules_source) : null,
    outputDir: path.join(outputDir, replayLabel),
  };
}

function buildReplayResult(bundle, replay) {
  return {
    replay_id: replay.replayId,
    replay_type: bundle.replay.replay_type ?? 'commit',
    commit: bundle.replay.commit ?? null,
    base_commit: bundle.replay.base_commit ?? null,
    defect_id: bundle.replay.defect_id ?? null,
    tags: replay.tags,
    expected_signal_kinds: replay.expectedSignalKinds,
    expected_fix_surface: replay.expectedFixSurface,
    telemetry_summary: bundle.telemetry_summary,
    output_dir: replay.outputDir,
    outcome: summarizeBundleOutcome(bundle),
  };
}

async function main() {
  const args = parseArgs(process.argv);
  const manifestPath = path.resolve(args.manifestPath);
  const manifest = await loadBatchManifest(manifestPath);
  const manifestDir = path.dirname(manifestPath);
  const cohortManifest = await loadSignalCohortManifest(args.cohortManifestPath);
  const cohort = getSignalCohort(cohortManifest, args.cohortId ?? manifest.cohort_id ?? null);
  const sourceRoot = resolveManifestPath(manifestPath, manifest.repo_root);
  const outputDir = path.resolve(
    args.outputDir ?? defaultBatchOutputDir(sourceRoot, 'replay-batch', manifest.batch_id ?? cohort.cohort_id),
  );
  const replayItems = await resolveReplayItems(manifest, sourceRoot, args.maxCount);

  const replayResults = [];
  for (const replay of replayItems) {
    const replayOptions = buildReplayOptions(
      replay,
      manifest,
      manifestDir,
      sourceRoot,
      outputDir,
    );
    const bundle = await runDiffReplay(replayOptions);
    replayResults.push(buildReplayResult(bundle, replayOptions));
  }

  const summaries = replayResults.map((result) => result.telemetry_summary);
  const mergedSummary = mergeSessionTelemetrySummaries(summaries, {
    repoRoot: sourceRoot,
    sourcePaths: replayResults.map((result) => path.join(result.output_dir, 'agent-session-events.jsonl')),
  });
  const batchResult = {
    schema_version: 1,
    generated_at: nowIso(),
    batch_id: manifest.batch_id ?? cohort.cohort_id,
    repo_label: manifest.repo_label ?? path.basename(sourceRoot),
    repo_root: sourceRoot,
    cohort_id: cohort.cohort_id,
    active_signal_kinds: cohort.signals.map((signal) => signal.signal_kind),
    replay_count: replayResults.length,
    results: replayResults.map(({ telemetry_summary, ...result }) => result),
    telemetry_summary: mergedSummary,
  };

  await writeJson(path.join(outputDir, 'diff-replay-batch.json'), batchResult);
  await writeJson(path.join(outputDir, 'session-telemetry-summary.json'), mergedSummary);

  console.log(
    `Replayed ${replayResults.length} replay(s) for cohort ${cohort.cohort_id}. Artifacts written to ${outputDir}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
