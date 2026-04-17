import path from 'node:path';
import { readJson, writeJson, writeText } from '../eval-batch.mjs';
import {
  buildSummaryMarkdown,
  publishArtifacts,
  readExistingJson,
} from '../repo-calibration-loop-support.mjs';
import { pathExists } from '../repo-calibration-artifacts.mjs';

async function writeSnapshotIfPresent(targetPath, value) {
  if (!targetPath || !value) {
    return;
  }

  await writeJson(targetPath, value);
}

async function loadBatchManifestIfPresent(targetPath) {
  if (!targetPath || !(await pathExists(targetPath))) {
    return null;
  }

  return readJson(targetPath);
}

function buildPreviousSnapshotPath(outputDir, baseName, artifact) {
  return artifact ? path.join(outputDir, `${baseName}.json`) : null;
}

function buildLatestPointerPath(stableArtifacts, outputDir) {
  return path.join(
    path.dirname(
      stableArtifacts.scorecard_json ??
        stableArtifacts.session_corpus_json ??
        stableArtifacts.evidence_review_json ??
        stableArtifacts.backlog_json ??
        stableArtifacts.review_packet_json ??
        outputDir,
    ),
    'latest.json',
  );
}

export async function loadLoopBatchManifests(paths) {
  return {
    codexBatchManifest: await loadBatchManifestIfPresent(paths.codexBatchManifestPath),
    replayBatchManifest: await loadBatchManifestIfPresent(paths.replayBatchManifestPath),
  };
}

export async function capturePreviousArtifacts(outputDir, paths) {
  const previousReviewPacket = await readExistingJson(paths.stableReviewPacketJsonPath);
  const previousScorecard = await readExistingJson(paths.stableScorecardJsonPath);
  const previousSessionCorpus = await readExistingJson(paths.stableSessionCorpusJsonPath);
  const previousBacklog = await readExistingJson(paths.stableBacklogJsonPath);
  const previousEvidenceReview = await readExistingJson(paths.stableEvidenceReviewJsonPath);
  const previousReviewPacketSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-check-review-packet',
    previousReviewPacket,
  );
  const previousScorecardSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-signal-scorecard',
    previousScorecard,
  );
  const previousBacklogSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-signal-backlog',
    previousBacklog,
  );
  const previousSessionCorpusSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-session-corpus',
    previousSessionCorpus,
  );
  const previousEvidenceReviewSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-evidence-review',
    previousEvidenceReview,
  );

  await writeSnapshotIfPresent(previousReviewPacketSnapshotPath, previousReviewPacket);
  await writeSnapshotIfPresent(previousScorecardSnapshotPath, previousScorecard);
  await writeSnapshotIfPresent(previousSessionCorpusSnapshotPath, previousSessionCorpus);
  await writeSnapshotIfPresent(previousBacklogSnapshotPath, previousBacklog);
  await writeSnapshotIfPresent(previousEvidenceReviewSnapshotPath, previousEvidenceReview);

  return {
    previousReviewPacket,
    previousScorecard,
    previousSessionCorpus,
    previousBacklog,
    previousEvidenceReview,
    previousReviewPacketSnapshotPath,
    previousScorecardSnapshotPath,
    previousSessionCorpusSnapshotPath,
    previousBacklogSnapshotPath,
    previousEvidenceReviewSnapshotPath,
  };
}

export async function loadGeneratedArtifacts(paths, selectedReviewVerdictsPath) {
  const reviewPacket = await loadBatchManifestIfPresent(paths.reviewPacketJsonPath);
  const scorecard = await loadBatchManifestIfPresent(paths.scorecardJsonPath);
  const sessionCorpus = await loadBatchManifestIfPresent(paths.sessionCorpusJsonPath);
  const backlog = await loadBatchManifestIfPresent(paths.backlogJsonPath);
  const evidenceReview = await loadBatchManifestIfPresent(paths.evidenceReviewJsonPath);
  const selectedReviewVerdicts = selectedReviewVerdictsPath
    ? await readExistingJson(selectedReviewVerdictsPath)
    : null;

  return {
    reviewPacket,
    selectedReviewVerdicts,
    scorecard,
    sessionCorpus,
    backlog,
    evidenceReview,
  };
}

export async function publishStableLoopArtifacts(paths, selectedReviewVerdictsPath) {
  await publishArtifacts([
    {
      sourcePath: paths.reviewPacketJsonPath,
      targetPath: paths.stableReviewPacketJsonPath,
    },
    {
      sourcePath: paths.reviewPacketMarkdownPath,
      targetPath: paths.stableReviewPacketMarkdownPath,
    },
    {
      sourcePath: selectedReviewVerdictsPath,
      targetPath: paths.stableReviewVerdictsOutputPath,
    },
    {
      sourcePath: paths.scorecardJsonPath,
      targetPath: paths.stableScorecardJsonPath,
    },
    {
      sourcePath: paths.scorecardMarkdownPath,
      targetPath: paths.stableScorecardMarkdownPath,
    },
    {
      sourcePath: paths.sessionCorpusJsonPath,
      targetPath: paths.stableSessionCorpusJsonPath,
    },
    {
      sourcePath: paths.sessionCorpusMarkdownPath,
      targetPath: paths.stableSessionCorpusMarkdownPath,
    },
    {
      sourcePath: paths.backlogJsonPath,
      targetPath: paths.stableBacklogJsonPath,
    },
    {
      sourcePath: paths.backlogMarkdownPath,
      targetPath: paths.stableBacklogMarkdownPath,
    },
    {
      sourcePath: paths.evidenceReviewJsonPath,
      targetPath: paths.stableEvidenceReviewJsonPath,
    },
    {
      sourcePath: paths.evidenceReviewMarkdownPath,
      targetPath: paths.stableEvidenceReviewMarkdownPath,
    },
  ]);
}

export async function writeLoopOutputs(outputDir, summary) {
  const summaryJsonPath = path.join(outputDir, 'repo-calibration-loop.json');
  const latestPointerPath = buildLatestPointerPath(summary.artifacts, outputDir);

  await writeJson(summaryJsonPath, summary);
  await writeText(
    path.join(outputDir, 'repo-calibration-loop.md'),
    buildSummaryMarkdown(summary),
  );
  await writeJson(latestPointerPath, {
    repo_id: summary.repo_id,
    generated_at: summary.generated_at,
    latest_output_dir: outputDir,
    summary_json: summaryJsonPath,
    scorecard_json: summary.artifacts.scorecard_json,
    session_corpus_json: summary.artifacts.session_corpus_json,
    backlog_json: summary.artifacts.backlog_json,
    review_packet_json: summary.artifacts.review_packet_json,
    evidence_review_json: summary.artifacts.evidence_review_json,
  });
}
