import path from 'node:path';

import { resolveManifestPath } from '../eval-batch.mjs';

export function resolveRepoArtifactPath(repoRootPath, relativePath) {
  if (!relativePath) {
    return null;
  }

  return path.resolve(repoRootPath, relativePath);
}

function deriveCompanionPath(targetPath, extension) {
  if (!targetPath) {
    return null;
  }

  const parsed = path.parse(targetPath);
  if (parsed.ext === extension) {
    return targetPath;
  }

  return path.join(parsed.dir, `${parsed.name}${extension}`);
}

function buildRunArtifactPath(outputDir, baseName, extension) {
  return path.join(outputDir, `${baseName}${extension}`);
}

function resolveManifestArtifactPaths(manifestPath, manifest, artifactConfig) {
  return {
    cohortManifestPath: resolveManifestPath(manifestPath, manifest.cohort_manifest),
    codexBatchManifestPath: resolveManifestPath(
      manifestPath,
      manifest.live_batch_manifest ?? manifest.codex_batch_manifest,
    ),
    replayBatchManifestPath: resolveManifestPath(
      manifestPath,
      manifest.replay_batch_manifest,
    ),
    reviewVerdictsPath: resolveManifestPath(
      manifestPath,
      artifactConfig.review_verdicts_input ?? manifest.review_verdicts,
    ),
    sessionVerdictsPath: resolveManifestPath(
      manifestPath,
      artifactConfig.session_verdicts_input,
    ),
    defectReportPath: resolveManifestPath(
      manifestPath,
      artifactConfig.seeded_defect_report ?? manifest.defect_report,
    ),
    remediationReportPath: resolveManifestPath(
      manifestPath,
      artifactConfig.remediation_report ?? manifest.remediation_report,
    ),
    benchmarkPath: resolveManifestPath(
      manifestPath,
      artifactConfig.benchmark_artifact ?? manifest.benchmark_artifact,
    ),
  };
}

function resolveStableArtifactPaths(repoRootPath, artifactConfig) {
  const configuredReviewPacketPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.review_packet_output,
  );
  const configuredScorecardPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.scorecard_output,
  );
  const configuredSessionCorpusPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.session_corpus_output,
  );
  const configuredBacklogPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.backlog_output,
  );
  const configuredEvidenceReviewPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.evidence_review_output,
  );

  return {
    stableReviewVerdictsOutputPath: resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.review_verdicts_output,
    ),
    stableSessionVerdictsOutputPath: resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.session_verdicts_output,
    ),
    stableReviewPacketJsonPath: deriveCompanionPath(
      configuredReviewPacketPath,
      '.json',
    ),
    stableReviewPacketMarkdownPath: deriveCompanionPath(
      configuredReviewPacketPath,
      '.md',
    ),
    stableScorecardJsonPath: deriveCompanionPath(configuredScorecardPath, '.json'),
    stableScorecardMarkdownPath: deriveCompanionPath(
      configuredScorecardPath,
      '.md',
    ),
    stableSessionCorpusJsonPath: deriveCompanionPath(
      configuredSessionCorpusPath,
      '.json',
    ),
    stableSessionCorpusMarkdownPath: deriveCompanionPath(
      configuredSessionCorpusPath,
      '.md',
    ),
    stableBacklogJsonPath: deriveCompanionPath(configuredBacklogPath, '.json'),
    stableBacklogMarkdownPath: deriveCompanionPath(configuredBacklogPath, '.md'),
    stableEvidenceReviewJsonPath: deriveCompanionPath(
      configuredEvidenceReviewPath,
      '.json',
    ),
    stableEvidenceReviewMarkdownPath: deriveCompanionPath(
      configuredEvidenceReviewPath,
      '.md',
    ),
  };
}

function resolveRunArtifactPaths(outputDir) {
  return {
    reviewPacketJsonPath: buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.json',
    ),
    reviewPacketMarkdownPath: buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.md',
    ),
    runReviewVerdictsOutputPath: buildRunArtifactPath(
      outputDir,
      'review-verdicts',
      '.json',
    ),
    runSessionVerdictsOutputPath: buildRunArtifactPath(
      outputDir,
      'session-verdicts',
      '.json',
    ),
    mergedTelemetryJsonPath: path.join(outputDir, 'session-telemetry-summary.json'),
    mergedTelemetryMarkdownPath: path.join(outputDir, 'session-telemetry-summary.md'),
    scorecardJsonPath: buildRunArtifactPath(outputDir, 'signal-scorecard', '.json'),
    scorecardMarkdownPath: buildRunArtifactPath(outputDir, 'signal-scorecard', '.md'),
    sessionCorpusJsonPath: buildRunArtifactPath(outputDir, 'session-corpus', '.json'),
    sessionCorpusMarkdownPath: buildRunArtifactPath(outputDir, 'session-corpus', '.md'),
    backlogJsonPath: buildRunArtifactPath(outputDir, 'signal-backlog', '.json'),
    backlogMarkdownPath: buildRunArtifactPath(outputDir, 'signal-backlog', '.md'),
    evidenceReviewJsonPath: buildRunArtifactPath(outputDir, 'evidence-review', '.json'),
    evidenceReviewMarkdownPath: buildRunArtifactPath(outputDir, 'evidence-review', '.md'),
  };
}

function resolveBatchArtifactPaths(outputDir) {
  const codexBatchOutputDir = path.join(outputDir, 'codex-batch');
  const replayBatchOutputDir = path.join(outputDir, 'replay-batch');

  return {
    codexBatchOutputDir,
    replayBatchOutputDir,
    codexBatchResultPath: path.join(
      codexBatchOutputDir,
      'codex-session-batch.json',
    ),
    replayBatchResultPath: path.join(
      replayBatchOutputDir,
      'diff-replay-batch.json',
    ),
  };
}

export function resolveLoopArtifactPaths({
  manifest,
  manifestPath,
  repoRootPath,
  outputDir,
}) {
  const artifactConfig = manifest.artifacts ?? {};
  return {
    artifactConfig,
    ...resolveManifestArtifactPaths(manifestPath, manifest, artifactConfig),
    ...resolveStableArtifactPaths(repoRootPath, artifactConfig),
    ...resolveRunArtifactPaths(outputDir),
    ...resolveBatchArtifactPaths(outputDir),
  };
}
