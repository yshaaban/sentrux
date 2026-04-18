#!/usr/bin/env node

import path from 'node:path';
import { buildSessionCorpus, formatSessionCorpusMarkdown } from '../lib/session-corpus.mjs';
import { resolveLatestRepoCalibrationArtifacts } from '../lib/repo-calibration-artifacts.mjs';
import { readJsonFile, repoRootFromImportMeta, writeMaybe } from './build-artifact-support.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url);

function parseArgs(argv) {
  const result = {
    repoRootPath: null,
    repoLabel: null,
    sessionTelemetryPath: null,
    sessionVerdictsPath: null,
    codexBatchPath: null,
    replayBatchPath: null,
    latestCalibrationPath: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      index += 1;
      result.repoRootPath = argv[index];
      continue;
    }
    if (value === '--repo-label') {
      index += 1;
      result.repoLabel = argv[index];
      continue;
    }
    if (value === '--session-telemetry') {
      index += 1;
      result.sessionTelemetryPath = argv[index];
      continue;
    }
    if (value === '--session-verdicts') {
      index += 1;
      result.sessionVerdictsPath = argv[index];
      continue;
    }
    if (value === '--codex-batch') {
      index += 1;
      result.codexBatchPath = argv[index];
      continue;
    }
    if (value === '--replay-batch') {
      index += 1;
      result.replayBatchPath = argv[index];
      continue;
    }
    if (value === '--latest-calibration') {
      index += 1;
      result.latestCalibrationPath = argv[index];
      continue;
    }
    if (value === '--output-json') {
      index += 1;
      result.outputJsonPath = argv[index];
      continue;
    }
    if (value === '--output-md') {
      index += 1;
      result.outputMarkdownPath = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.codexBatchPath && !result.replayBatchPath && !result.sessionTelemetryPath) {
    throw new Error(
      'Provide at least one input: --codex-batch, --replay-batch, or --session-telemetry',
    );
  }

  return result;
}

function resolveRepoLabel(args, inputs) {
  return (
    args.repoLabel ??
    inputs.codexBatch?.repo_label ??
    inputs.replayBatch?.repo_label ??
    inputs.sessionVerdicts?.repo_label ??
    inputs.sessionVerdicts?.repo ??
    inputs.sessionTelemetry?.repo_label ??
    inputs.sessionTelemetry?.repo_root ??
    (args.repoRootPath ? path.basename(args.repoRootPath) : null) ??
    null
  );
}

async function resolveBatchInputs(args, resolvedRepoLabel) {
  const latestCalibration =
    (!args.codexBatchPath || !args.replayBatchPath) && resolvedRepoLabel
      ? await resolveLatestRepoCalibrationArtifacts({
          repoRootPath: args.repoRootPath,
          repoLabel: resolvedRepoLabel,
          latestCalibrationPath: args.latestCalibrationPath,
        })
      : null;
  const codexBatchPath = args.codexBatchPath ?? latestCalibration?.artifacts?.codex_batch_json ?? null;
  const replayBatchPath =
    args.replayBatchPath ?? latestCalibration?.artifacts?.replay_batch_json ?? null;

  return {
    codexBatch: codexBatchPath ? await readJsonFile(codexBatchPath) : null,
    replayBatch: replayBatchPath ? await readJsonFile(replayBatchPath) : null,
  };
}

async function main() {
  const args = parseArgs(process.argv);
  const sessionTelemetry = args.sessionTelemetryPath
    ? await readJsonFile(args.sessionTelemetryPath)
    : null;
  const sessionVerdicts = args.sessionVerdictsPath
    ? await readJsonFile(args.sessionVerdictsPath)
    : null;
  const inferredRepoLabel = resolveRepoLabel(args, {
    codexBatch: null,
    replayBatch: null,
    sessionVerdicts,
    sessionTelemetry,
  });
  const { codexBatch, replayBatch } = await resolveBatchInputs(args, inferredRepoLabel);
  const corpus = buildSessionCorpus({
    repoLabel: resolveRepoLabel(args, {
      codexBatch,
      replayBatch,
      sessionVerdicts,
      sessionTelemetry,
    }),
    repoRoot: args.repoRootPath,
    sessionTelemetry,
    sessionVerdicts,
    codexBatch,
    replayBatch,
  });
  const markdown = formatSessionCorpusMarkdown(corpus);

  await writeMaybe(
    args.outputJsonPath ?? path.join(repoRoot, 'docs/v2/examples', 'session-corpus.json'),
    `${JSON.stringify(corpus, null, 2)}\n`,
  );
  await writeMaybe(
    args.outputMarkdownPath ?? path.join(repoRoot, 'docs/v2/examples', 'session-corpus.md'),
    markdown,
  );

  console.log(
    `Built session corpus for ${corpus.repo_label ?? 'unknown'} with ${corpus.summary.session_count} session(s).`,
  );
}

main().catch(function handleError(error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
