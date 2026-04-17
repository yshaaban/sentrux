#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
  defaultBatchOutputDir,
  resolveManifestPath,
} from '../lib/eval-batch.mjs';
import {
  acquireLoopLock,
  buildReviewArgs,
  buildScorecardArgs,
  buildSummaryArtifacts,
  existingPathOrNull,
  resolveLoopArtifactPaths,
  selectReviewVerdictsPath,
} from '../lib/repo-calibration-loop-support.mjs';
import { parseArgs, nowIso } from '../lib/repo-calibration-loop/args.mjs';
import { loadRepoCalibrationManifest } from '../lib/repo-calibration-loop/manifest.mjs';
import {
  capturePreviousArtifacts,
  loadGeneratedArtifacts,
  loadLoopBatchManifests,
  publishStableLoopArtifacts,
  writeLoopOutputs,
} from '../lib/repo-calibration-loop/artifacts.mjs';
import { buildLoopSummary, buildLoopWarningSet } from '../lib/repo-calibration-loop/summary.mjs';
import { runLoopStages } from '../lib/repo-calibration-loop/stages.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

export {
  buildReviewArgs,
  buildScorecardArgs,
  parseArgs,
  selectReviewVerdictsPath,
};

function defaultLoopOutputDir(repoRootPath, manifest, requestedOutputDir) {
  return path.resolve(
    requestedOutputDir ??
      defaultBatchOutputDir(
        repoRootPath,
        'repo-calibration-loop',
        manifest.repo_id ?? path.basename(repoRootPath),
      ),
  );
}

function buildLoopLockPath(repoRootPath, repoId) {
  return path.join(
    repoRootPath,
    '.sentrux',
    'evals',
    `.repo-calibration-${repoId}.lock`,
  );
}

async function loadLoopContext(argv) {
  const args = parseArgs(argv);
  const manifestPath = path.resolve(args.manifestPath);
  const manifest = await loadRepoCalibrationManifest(manifestPath);
  const repoRootPath = resolveManifestPath(manifestPath, manifest.repo_root);
  const outputDir = defaultLoopOutputDir(repoRootPath, manifest, args.outputDir);
  const repoId = manifest.repo_id ?? path.basename(repoRootPath);

  return {
    args,
    manifestPath,
    manifest,
    repoRootPath,
    outputDir,
    repoId,
  };
}

async function runLoop(context) {
  const { args, manifestPath, manifest, repoRootPath, outputDir, repoId } = context;
  const releaseLoopLock = await acquireLoopLock(buildLoopLockPath(repoRootPath, repoId), {
    repo_id: repoId,
    repo_root: repoRootPath,
    output_dir: outputDir,
    pid: process.pid,
    started_at: nowIso(),
    manifest_path: manifestPath,
  });

  try {
    const paths = resolveLoopArtifactPaths({ manifest, manifestPath, repoRootPath, outputDir });
    const manifests = await loadLoopBatchManifests(paths);
    const previousArtifacts = await capturePreviousArtifacts(outputDir, paths);
    const {
      runs,
      batchResults,
      mergedTelemetry,
      selectedReviewVerdictsPath,
    } = await runLoopStages({
      args,
      manifest,
      paths,
      repoRootPath,
      repoRoot,
    });
    const generatedArtifacts = await loadGeneratedArtifacts(
      paths,
      selectedReviewVerdictsPath,
    );
    const { reviewPacket, selectedReviewVerdicts, scorecard, backlog } = generatedArtifacts;
    const stableArtifacts = buildSummaryArtifacts({
      outputDir,
      stableReviewPacketJsonPath: paths.stableReviewPacketJsonPath,
      reviewPacketJsonPath: paths.reviewPacketJsonPath,
      reviewPacket,
      previousReviewPacketSnapshotPath:
        previousArtifacts.previousReviewPacketSnapshotPath,
      selectedReviewVerdictsPath,
      stableReviewVerdictsOutputPath: paths.stableReviewVerdictsOutputPath,
      runReviewVerdictsOutputPath: paths.runReviewVerdictsOutputPath,
      stableScorecardJsonPath: paths.stableScorecardJsonPath,
      scorecardJsonPath: paths.scorecardJsonPath,
      previousScorecardSnapshotPath: previousArtifacts.previousScorecardSnapshotPath,
      stableBacklogJsonPath: paths.stableBacklogJsonPath,
      backlogJsonPath: paths.backlogJsonPath,
      previousBacklogSnapshotPath: previousArtifacts.previousBacklogSnapshotPath,
      mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
      codexBatchResult: batchResults.codexBatchResult,
      codexBatchOutputDir: paths.codexBatchOutputDir,
      replayBatchResult: batchResults.replayBatchResult,
      replayBatchOutputDir: paths.replayBatchOutputDir,
      selectedReviewVerdicts,
      scorecard,
      backlog,
    });
    const warnings = await buildLoopWarningSet({
      paths,
      reviewPacket,
      selectedReviewVerdicts,
      selectedReviewVerdictsPath,
      manifests,
      batchResults,
      existingPathOrNull,
    });
    const summary = buildLoopSummary({
      outputDir,
      repoId,
      repoRootPath,
      manifest,
      mergedTelemetry,
      reviewPacket,
      selectedReviewVerdicts,
      scorecard,
      backlog,
      previousArtifacts,
      stableArtifacts,
      batchResults,
      warnings,
      runs,
      nowIso,
    });

    await publishStableLoopArtifacts(paths, selectedReviewVerdictsPath);
    await writeLoopOutputs(outputDir, summary);

    console.log(
      `Completed repo calibration loop for ${summary.repo_label}. Artifacts written to ${outputDir}`,
    );
  } finally {
    await releaseLoopLock();
  }
}

async function main() {
  const context = await loadLoopContext(process.argv);
  await runLoop(context);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
