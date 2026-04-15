#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertPathExists } from './lib/disposable-repo.mjs';
import { runValidationSuite } from './lib/v2-validation.mjs';
import { resolveWorkspaceRepoRoot, assertRepoRootExists } from './lib/path-roots.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const refreshScript = path.join(repoRoot, 'scripts/refresh_parallel_code_goldens.sh');
const benchmarkScript = path.join(repoRoot, 'scripts/benchmark_parallel_code_v2.mjs');
const expectedGoldenDir = path.join(repoRoot, 'docs/v2/examples/parallel-code-golden');
const expectedBenchmarkPath = path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const nodeBin = process.execPath;

const runGoldens = !process.argv.includes('--benchmark-only');
const runBenchmark = !process.argv.includes('--goldens-only');
const keepTemp = process.argv.includes('--keep-temp');
const benchmarkRepeats = process.env.BENCHMARK_REPEATS ?? '3';
const skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1';

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertRepoRootExists(parallelCodeRoot, 'parallel-code repo');
  assertPathExists(refreshScript, 'golden refresh script');
  assertPathExists(benchmarkScript, 'benchmark script');
  assertPathExists(expectedGoldenDir, 'parallel-code golden directory');
  assertPathExists(expectedBenchmarkPath, 'parallel-code benchmark artifact');

  await runValidationSuite({
    repoLabel: 'parallel-code',
    repoEnvVar: 'PARALLEL_CODE_ROOT',
    repoRoot: parallelCodeRoot,
    sentruxBin,
    refreshScript,
    benchmarkScript,
    expectedGoldenDir,
    expectedBenchmarkPath,
    benchmarkTempFilename: 'parallel-code-benchmark.json',
    tempRootPrefix: 'sentrux-v2-validate-',
    keepTemp,
    runGoldens,
    runBenchmark,
    benchmarkRepeats,
    skipGrammarDownload,
    repoWorkspaceRoot: repoRoot,
    nodeBin,
  });
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
