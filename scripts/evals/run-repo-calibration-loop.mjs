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
    const loopState = await runLoopState({
      args,
      manifest,
      paths,
      repoRootPath,
      outputDir,
    });
    const stableArtifacts = buildLoopStableArtifacts(paths, outputDir, loopState);
    const warnings = await buildLoopWarnings(paths, loopState);
    const summary = buildLoopSummary({
      outputDir,
      repoId,
      repoRootPath,
      manifest,
      mergedTelemetry: loopState.stageResults.mergedTelemetry,
      reviewPacket: loopState.generatedArtifacts.reviewPacket,
      selectedReviewVerdicts: loopState.generatedArtifacts.selectedReviewVerdicts,
      scorecard: loopState.generatedArtifacts.scorecard,
      sessionCorpus: loopState.generatedArtifacts.sessionCorpus,
      backlog: loopState.generatedArtifacts.backlog,
      evidenceReview: loopState.generatedArtifacts.evidenceReview,
      previousArtifacts: loopState.previousArtifacts,
      stableArtifacts,
      batchResults: loopState.stageResults.batchResults,
      warnings,
      runs: loopState.stageResults.runs,
      nowIso,
    });

    await publishStableLoopArtifacts(paths, loopState.stageResults.selectedReviewVerdictsPath);
    await writeLoopOutputs(outputDir, summary);

    console.log(
      `Completed repo calibration loop for ${summary.repo_label}. Artifacts written to ${outputDir}`,
    );
  } finally {
    await releaseLoopLock();
  }
}

async function runLoopState({ args, manifest, paths, repoRootPath, outputDir }) {
  const manifests = await loadLoopBatchManifests(paths);
  const previousArtifacts = await capturePreviousArtifacts(outputDir, paths);
  const stageResults = await runLoopStages({
    args,
    manifest,
    paths,
    repoRootPath,
    repoRoot,
  });
  const generatedArtifacts = await loadGeneratedArtifacts(
    paths,
    stageResults.selectedReviewVerdictsPath,
  );

  return {
    manifests,
    previousArtifacts,
    stageResults,
    generatedArtifacts,
  };
}

function buildLoopStableArtifacts(paths, outputDir, loopState) {
  return buildSummaryArtifacts({
    outputDir,
    stableReviewPacketJsonPath: paths.stableReviewPacketJsonPath,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    reviewPacket: loopState.generatedArtifacts.reviewPacket,
    previousReviewPacketSnapshotPath:
      loopState.previousArtifacts.previousReviewPacketSnapshotPath,
    selectedReviewVerdictsPath: loopState.stageResults.selectedReviewVerdictsPath,
    stableReviewVerdictsOutputPath: paths.stableReviewVerdictsOutputPath,
    runReviewVerdictsOutputPath: paths.runReviewVerdictsOutputPath,
    stableScorecardJsonPath: paths.stableScorecardJsonPath,
    scorecardJsonPath: paths.scorecardJsonPath,
    previousScorecardSnapshotPath: loopState.previousArtifacts.previousScorecardSnapshotPath,
    stableSessionCorpusJsonPath: paths.stableSessionCorpusJsonPath,
    sessionCorpusJsonPath: paths.sessionCorpusJsonPath,
    previousSessionCorpusSnapshotPath:
      loopState.previousArtifacts.previousSessionCorpusSnapshotPath,
    stableBacklogJsonPath: paths.stableBacklogJsonPath,
    backlogJsonPath: paths.backlogJsonPath,
    previousBacklogSnapshotPath: loopState.previousArtifacts.previousBacklogSnapshotPath,
    stableEvidenceReviewJsonPath: paths.stableEvidenceReviewJsonPath,
    evidenceReviewJsonPath: paths.evidenceReviewJsonPath,
    previousEvidenceReviewSnapshotPath:
      loopState.previousArtifacts.previousEvidenceReviewSnapshotPath,
    mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
    codexBatchResult: loopState.stageResults.batchResults.codexBatchResult,
    codexBatchOutputDir: paths.codexBatchOutputDir,
    replayBatchResult: loopState.stageResults.batchResults.replayBatchResult,
    replayBatchOutputDir: paths.replayBatchOutputDir,
    selectedReviewVerdicts: loopState.generatedArtifacts.selectedReviewVerdicts,
    scorecard: loopState.generatedArtifacts.scorecard,
    sessionCorpus: loopState.generatedArtifacts.sessionCorpus,
    backlog: loopState.generatedArtifacts.backlog,
    evidenceReview: loopState.generatedArtifacts.evidenceReview,
  });
}

async function buildLoopWarnings(paths, loopState) {
  return buildLoopWarningSet({
    paths,
    reviewPacket: loopState.generatedArtifacts.reviewPacket,
    selectedReviewVerdicts: loopState.generatedArtifacts.selectedReviewVerdicts,
    selectedReviewVerdictsPath: loopState.stageResults.selectedReviewVerdictsPath,
    manifests: loopState.manifests,
    batchResults: loopState.stageResults.batchResults,
    existingPathOrNull,
  });
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
