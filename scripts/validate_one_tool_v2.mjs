#!/usr/bin/env node

import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertPathExists } from './lib/disposable-repo.mjs';
import { resolveWorkspaceRepoRoot, assertRepoRootExists } from './lib/path-roots.mjs';
import {
  assertBenchmarkArtifact,
  runChecked,
} from './lib/v2-validation.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const oneToolRoot = resolveWorkspaceRepoRoot(process.env.ONE_TOOL_ROOT, 'one-tool', repoRoot);
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const benchmarkScript = path.join(repoRoot, 'scripts/benchmark_one_tool_v2.mjs');
const expectedBenchmarkPath = path.join(repoRoot, 'docs/v2/examples/one-tool-benchmark.json');
const nodeBin = process.execPath;
const benchmarkRepeats = process.env.BENCHMARK_REPEATS ?? '3';
const skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1';
const keepTemp = process.argv.includes('--keep-temp');
const shouldRunBenchmarkValidation = !process.argv.includes('--goldens-only');
const GOLDENS_ONLY_SKIP_MESSAGE =
  'Skipping one-tool benchmark validation for --goldens-only; no checked-in goldens exist for one-tool.';

async function main() {
  if (!shouldRunBenchmarkValidation) {
    console.log(GOLDENS_ONLY_SKIP_MESSAGE);
    return;
  }

  assertPathExists(sentruxBin, 'sentrux binary');
  assertRepoRootExists(oneToolRoot, 'one-tool repo');
  assertPathExists(benchmarkScript, 'one-tool benchmark script');
  assertPathExists(expectedBenchmarkPath, 'one-tool benchmark artifact');

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-one-tool-validate-'));
  const tempBenchmarkPath = path.join(tempRoot, 'one-tool-benchmark.json');
  try {
    runChecked(nodeBin, [benchmarkScript], {
      cwd: repoRoot,
      env: {
        ...process.env,
        ONE_TOOL_ROOT: oneToolRoot,
        SENTRUX_BIN: sentruxBin,
        SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
        OUTPUT_PATH: tempBenchmarkPath,
        COMPARE_TO: expectedBenchmarkPath,
        FAIL_ON_REGRESSION: '1',
        FAIL_ON_NONCOMPARABLE: '1',
        BENCHMARK_REPEATS: benchmarkRepeats,
      },
    });
    assertBenchmarkArtifact({
      expectedPath: expectedBenchmarkPath,
      actualPath: tempBenchmarkPath,
      label: 'one-tool',
    });
    console.log(`Validated benchmark regression flow against ${expectedBenchmarkPath}`);
    console.log('one-tool v2 validation loop completed successfully.');
  } finally {
    if (!keepTemp) {
      await rm(tempRoot, { recursive: true, force: true });
    } else {
      console.log(`Preserved temp validation output at ${tempRoot}`);
    }
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
