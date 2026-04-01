import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

export function runChecked(command, args, { cwd, env = {} }) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    stdio: 'inherit',
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} exited with code ${result.status}`);
  }
}

export function runCheckedWithRetry(command, args, { cwd, env = {}, retries = 1 }) {
  let attempt = 0;
  while (true) {
    attempt += 1;
    try {
      runChecked(command, args, { cwd, env });
      return;
    } catch (error) {
      if (attempt > retries) {
        throw error;
      }
      console.error(
        `Retrying ${args[0] ?? command} after transient failure: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
}

export function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, 'utf8'));
}

function listJsonFiles(directoryPath) {
  return readdirSync(directoryPath)
    .filter((file) => file.endsWith('.json'))
    .sort();
}

function normalizeMetadata(metadata) {
  const copy = { ...metadata };
  delete copy.generated_at;
  return copy;
}

export function compareJsonFiles(expectedPath, actualPath, label) {
  const expected = readJson(expectedPath);
  const actual = readJson(actualPath);

  if (expectedPath.endsWith('metadata.json')) {
    if (!isDeepStrictEqual(normalizeMetadata(expected), normalizeMetadata(actual))) {
      throw new Error(`Metadata mismatch for ${label}`);
    }
    return;
  }

  if (!isDeepStrictEqual(expected, actual)) {
    throw new Error(`Golden mismatch for ${label}`);
  }
}

export function compareGoldenDirectories(expectedDir, actualDir) {
  const expectedFiles = listJsonFiles(expectedDir);
  const actualFiles = listJsonFiles(actualDir);

  if (!isDeepStrictEqual(expectedFiles, actualFiles)) {
    const expectedSet = new Set(expectedFiles);
    const actualSet = new Set(actualFiles);
    const missing = expectedFiles.filter((file) => !actualSet.has(file));
    const extra = actualFiles.filter((file) => !expectedSet.has(file));
    let message = 'Golden file set mismatch';
    if (missing.length > 0) {
      message += `; missing: ${missing.join(', ')}`;
    }
    if (extra.length > 0) {
      message += `; extra: ${extra.join(', ')}`;
    }
    throw new Error(message);
  }

  for (const file of expectedFiles) {
    compareJsonFiles(`${expectedDir}/${file}`, `${actualDir}/${file}`, file);
  }
}

export function assertBenchmarkFormatVersion({ expectedPath, actualPath, label }) {
  if (!existsSync(expectedPath)) {
    throw new Error(`Missing expected benchmark artifact: ${expectedPath}`);
  }
  if (!existsSync(actualPath)) {
    throw new Error(`Missing benchmark artifact: ${actualPath}`);
  }

  const expected = readJson(expectedPath);
  const actual = readJson(actualPath);
  if (expected.benchmark_format_version !== actual.benchmark_format_version) {
    throw new Error(
      `${label} benchmark format mismatch: expected ${expected.benchmark_format_version}, got ${actual.benchmark_format_version}`,
    );
  }
}

export async function runValidationSuite({
  repoLabel,
  repoEnvVar,
  repoRoot,
  sentruxBin,
  refreshScript,
  benchmarkScript,
  expectedGoldenDir,
  expectedBenchmarkPath,
  benchmarkTempFilename,
  tempRootPrefix,
  keepTemp,
  runGoldens,
  runBenchmark,
  skipGrammarDownload,
  repoWorkspaceRoot,
  nodeBin = process.execPath,
}) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), tempRootPrefix));
  const tempGoldenDir = path.join(tempRoot, 'goldens');
  const tempBenchmarkPath = path.join(tempRoot, benchmarkTempFilename);

  try {
    if (runGoldens) {
      runCheckedWithRetry('bash', [refreshScript], {
        cwd: repoWorkspaceRoot,
        env: {
          ...process.env,
          [repoEnvVar]: repoRoot,
          OUTPUT_DIR: tempGoldenDir,
          SENTRUX_BIN: sentruxBin,
          SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
        },
      });
      compareGoldenDirectories(expectedGoldenDir, tempGoldenDir);
      console.log(`Validated ${repoLabel} goldens against ${expectedGoldenDir}`);
    }

    if (runBenchmark) {
      runChecked(nodeBin, [benchmarkScript], {
        cwd: repoWorkspaceRoot,
        env: {
          ...process.env,
          [repoEnvVar]: repoRoot,
          SENTRUX_BIN: sentruxBin,
          OUTPUT_PATH: tempBenchmarkPath,
          COMPARE_TO: expectedBenchmarkPath,
          FAIL_ON_REGRESSION: '1',
          SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
        },
      });

      assertBenchmarkFormatVersion({
        expectedPath: expectedBenchmarkPath,
        actualPath: tempBenchmarkPath,
        label: repoLabel,
      });
      console.log(`Validated benchmark regression flow against ${expectedBenchmarkPath}`);
    }

    if (!runGoldens && !runBenchmark) {
      throw new Error('Nothing to do. Pass --goldens-only, --benchmark-only, or no flags to run both.');
    }

    console.log(`${repoLabel} v2 validation loop completed successfully.`);
  } finally {
    if (!keepTemp) {
      await rm(tempRoot, { recursive: true, force: true });
    } else {
      console.log(`Preserved temp validation output at ${tempRoot}`);
    }
  }
}
