#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  assertFileIdentityFresh,
  assertRepoIdentityFresh,
  collectFileIdentity,
  collectRepoIdentity,
} from './lib/repo-identity.mjs';
import { assertPathExists } from './lib/disposable-repo.mjs';
import { resolveWorkspaceRepoRoot } from './lib/path-roots.mjs';
import {
  assertHeadCommitFresh,
  buildLiveEngineerAppendix,
  buildLiveEngineerReport,
  isHeadCloneAnalysis,
  snapshotMatchesMetadata,
} from './lib/parallel-code-live-engineer-report-format.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);
const goldenDir =
  process.env.GOLDEN_DIR ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-golden');
const benchmarkPath =
  process.env.BENCHMARK_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const snapshotJsonPath =
  process.env.SNAPSHOT_JSON_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.json');
const reportMarkdownPath =
  process.env.OUTPUT_REPORT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-live-engineer-report.md');
const appendixMarkdownPath =
  process.env.OUTPUT_APPENDIX_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-live-engineer-report-appendix.md');
const snapshotMarkdownPath =
  process.env.OUTPUT_SNAPSHOT_MD_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.md');
const allowStaleGoldens =
  process.env.ALLOW_STALE_GOLDENS === '1' || process.argv.includes('--allow-stale-goldens');

function readJson(targetPath) {
  return JSON.parse(readFileSync(targetPath, 'utf8'));
}

async function main() {
  const metadataPath = path.join(goldenDir, 'metadata.json');
  const scanPath = path.join(goldenDir, 'scan.json');
  const findingsPath = path.join(goldenDir, 'findings-top12.json');
  const obligationsPath = path.join(goldenDir, 'obligations-task_presentation_status.json');

  assertPathExists(goldenDir, 'parallel-code golden directory');
  assertPathExists(snapshotJsonPath, 'parallel-code proof snapshot JSON');
  assertPathExists(benchmarkPath, 'parallel-code benchmark artifact');
  assertPathExists(metadataPath, 'parallel-code metadata snapshot');
  assertPathExists(scanPath, 'parallel-code scan snapshot');
  assertPathExists(findingsPath, 'parallel-code findings snapshot');
  assertPathExists(obligationsPath, 'parallel-code obligations snapshot');

  const snapshot = readJson(snapshotJsonPath);
  const findings = readJson(findingsPath);
  const metadata = readJson(metadataPath);
  const scan = readJson(scanPath);
  const benchmark = readJson(benchmarkPath);
  const liveIdentity = collectRepoIdentity(parallelCodeRoot);
  const liveRulesIdentity = collectFileIdentity(metadata.rules_source);
  const liveBinaryIdentity = collectFileIdentity(metadata.sentrux_binary);

  if (!snapshotMatchesMetadata(snapshot, metadata)) {
    throw new Error(
      'parallel-code proof snapshot JSON is stale relative to the current goldens; regenerate the proof snapshot first',
    );
  }
  if (!['working_tree', 'head_clone'].includes(metadata.analysis_mode)) {
    throw new Error(
      `parallel-code report requires working_tree or head_clone analysis metadata, got ${metadata.analysis_mode}`,
    );
  }

  if (metadata.analysis_mode === 'working_tree') {
    assertRepoIdentityFresh({
      expected: metadata.source_tree_identity,
      actual: { ...liveIdentity, analysis_mode: metadata.analysis_mode },
      label: 'parallel-code goldens',
      allowStale: allowStaleGoldens,
    });
    assertRepoIdentityFresh({
      expected: metadata.analyzed_tree_identity,
      actual: { ...liveIdentity, analysis_mode: metadata.analysis_mode },
      label: 'parallel-code analyzed tree',
      allowStale: allowStaleGoldens,
    });
  } else {
    assertHeadCommitFresh(metadata, liveIdentity, allowStaleGoldens);
  }
  assertFileIdentityFresh({
    expected: metadata.rules_identity,
    actual: liveRulesIdentity,
    label: 'parallel-code rules file',
    allowStale: allowStaleGoldens,
  });
  assertFileIdentityFresh({
    expected: metadata.binary_identity,
    actual: liveBinaryIdentity,
    label: 'parallel-code sentrux binary',
    allowStale: allowStaleGoldens,
  });

  await writeFile(
    reportMarkdownPath,
    buildLiveEngineerReport({
      snapshot,
      findings,
      scan,
      benchmark,
      metadata,
      liveIdentity,
      allowStale: allowStaleGoldens,
      snapshotMarkdownPath,
      goldenDir,
      benchmarkPath,
    }),
    'utf8',
  );
  await writeFile(
    appendixMarkdownPath,
    buildLiveEngineerAppendix({
      snapshot,
      findings,
      scan,
      metadata,
      reportMarkdownPath,
      repoRoot,
    }),
    'utf8',
  );

  console.log(`Wrote live engineer report to ${reportMarkdownPath}`);
  console.log(`Wrote live engineer appendix to ${appendixMarkdownPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
