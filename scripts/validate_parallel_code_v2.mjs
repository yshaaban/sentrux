#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = process.env.PARALLEL_CODE_ROOT ?? '<parallel-code-root>';
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const refreshScript = path.join(repoRoot, 'scripts/refresh_parallel_code_goldens.sh');
const benchmarkScript = path.join(repoRoot, 'scripts/benchmark_parallel_code_v2.mjs');
const expectedGoldenDir = path.join(repoRoot, 'docs/v2/examples/parallel-code-golden');
const expectedBenchmarkPath = path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const nodeBin = process.execPath;

const runGoldens = !process.argv.includes('--benchmark-only');
const runBenchmark = !process.argv.includes('--goldens-only');
const keepTemp = process.argv.includes('--keep-temp');

function fail(message) {
  throw new Error(message);
}

function assertPathExists(targetPath, label) {
  if (!existsSync(targetPath)) {
    fail(`Missing ${label}: ${targetPath}`);
  }
}

function runChecked(command, args, extraEnv = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      ...extraEnv,
    },
    stdio: 'inherit',
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    const renderedArgs = args.join(' ');
    throw new Error(`${command} ${renderedArgs} exited with code ${result.status}`);
  }
}

function runCheckedWithRetry(command, args, extraEnv = {}, retries = 1) {
  let attempt = 0;
  // The refresh path occasionally hits a transient grammar archive extraction failure
  // on a cold environment. Retry once so validation stays usable instead of flaky.
  while (true) {
    attempt += 1;
    try {
      runChecked(command, args, extraEnv);
      return;
    } catch (error) {
      if (attempt > retries) {
        throw error;
      }

      console.error(
        `Retrying ${path.basename(args[0] ?? command)} after transient failure: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
}

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, 'utf8'));
}

function normalizeMetadata(metadata) {
  const copy = { ...metadata };
  delete copy.generated_at;
  return copy;
}

function compareJsonFiles(expectedPath, actualPath, label) {
  const expected = readJson(expectedPath);
  const actual = readJson(actualPath);

  if (path.basename(expectedPath) === 'metadata.json') {
    if (!isDeepStrictEqual(normalizeMetadata(expected), normalizeMetadata(actual))) {
      throw new Error(`Metadata mismatch for ${label}`);
    }
    return;
  }

  if (!isDeepStrictEqual(expected, actual)) {
    throw new Error(`Golden mismatch for ${label}`);
  }
}

function compareGoldenDirectories(expectedDir, actualDir) {
  const expectedFiles = readdirSync(expectedDir)
    .filter((file) => file.endsWith('.json'))
    .sort();
  const actualFiles = readdirSync(actualDir)
    .filter((file) => file.endsWith('.json'))
    .sort();

  if (!isDeepStrictEqual(expectedFiles, actualFiles)) {
    const expectedSet = new Set(expectedFiles);
    const actualSet = new Set(actualFiles);
    const missing = expectedFiles.filter((file) => !actualSet.has(file));
    const extra = actualFiles.filter((file) => !expectedSet.has(file));
    throw new Error(
      `Golden file set mismatch${missing.length ? `; missing: ${missing.join(', ')}` : ''}${
        extra.length ? `; extra: ${extra.join(', ')}` : ''
      }`,
    );
  }

  for (const file of expectedFiles) {
    compareJsonFiles(path.join(expectedDir, file), path.join(actualDir, file), file);
  }
}

async function main() {
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(parallelCodeRoot, 'parallel-code repo');
  assertPathExists(refreshScript, 'golden refresh script');
  assertPathExists(benchmarkScript, 'benchmark script');
  assertPathExists(expectedGoldenDir, 'parallel-code golden directory');
  assertPathExists(expectedBenchmarkPath, 'parallel-code benchmark artifact');

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-v2-validate-'));
  const tempGoldenDir = path.join(tempRoot, 'goldens');
  const tempBenchmarkPath = path.join(tempRoot, 'parallel-code-benchmark.json');

  try {
    if (runGoldens) {
      runCheckedWithRetry('bash', [refreshScript], {
        PARALLEL_CODE_ROOT: parallelCodeRoot,
        OUTPUT_DIR: tempGoldenDir,
        SENTRUX_BIN: sentruxBin,
      });
      compareGoldenDirectories(expectedGoldenDir, tempGoldenDir);
      console.log(`Validated parallel-code goldens against ${expectedGoldenDir}`);
    }

    if (runBenchmark) {
      runChecked(nodeBin, [benchmarkScript], {
        PARALLEL_CODE_ROOT: parallelCodeRoot,
        SENTRUX_BIN: sentruxBin,
        OUTPUT_PATH: tempBenchmarkPath,
        COMPARE_TO: expectedBenchmarkPath,
        FAIL_ON_REGRESSION: '1',
      });

      const benchmark = readJson(tempBenchmarkPath);
      const expectedBenchmark = readJson(expectedBenchmarkPath);
      if (benchmark.benchmark_format_version !== expectedBenchmark.benchmark_format_version) {
        throw new Error(
          `Benchmark format mismatch: expected ${expectedBenchmark.benchmark_format_version}, got ${benchmark.benchmark_format_version}`,
        );
      }
      console.log(`Validated benchmark regression flow against ${expectedBenchmarkPath}`);
    }

    if (!runGoldens && !runBenchmark) {
      fail('Nothing to do. Pass --goldens-only, --benchmark-only, or no flags to run both.');
    }

    console.log('V2 validation loop completed successfully.');
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
