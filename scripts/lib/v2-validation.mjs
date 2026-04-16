import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import {
  buildMetricStatistics,
  getBenchmarkMetric,
  listBenchmarkTimingMetricPaths,
  roundMs,
} from './benchmark-harness.mjs';

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
  if (copy.binary_identity && typeof copy.binary_identity === 'object') {
    copy.binary_identity = { ...copy.binary_identity };
    delete copy.binary_identity.sha256;
  }
  return copy;
}

function buildValidationEnv({ repoEnvVar, repoRoot, sentruxBin, skipGrammarDownload }) {
  return {
    ...process.env,
    [repoEnvVar]: repoRoot,
    SENTRUX_BIN: sentruxBin,
    SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload,
  };
}

function buildBenchmarkEnv({ baseEnv, tempBenchmarkPath, expectedBenchmarkPath, benchmarkRepeats }) {
  const env = {
    ...baseEnv,
    OUTPUT_PATH: tempBenchmarkPath,
    COMPARE_TO: expectedBenchmarkPath,
    FAIL_ON_REGRESSION: '1',
    FAIL_ON_NONCOMPARABLE: '1',
  };
  if (benchmarkRepeats) {
    env.BENCHMARK_REPEATS = benchmarkRepeats;
  }
  return env;
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

function assertBenchmarkFieldEqual(label, field, expected, actual) {
  if (expected[field] == null) {
    return;
  }
  if (expected[field] !== actual[field]) {
    throw new Error(
      `${label} benchmark ${field.replaceAll('_', ' ')} mismatch: expected ${expected[field]}, got ${actual[field] ?? 'missing'}`,
    );
  }
}

function assertBenchmarkSamples(label, actual) {
  if (!Array.isArray(actual.benchmark_samples)) {
    throw new Error(`${label} benchmark artifact is missing benchmark samples`);
  }

  for (const [sampleIndex, sample] of actual.benchmark_samples.entries()) {
    if (!sample || typeof sample !== 'object' || Array.isArray(sample)) {
      throw new Error(`${label} benchmark sample ${sampleIndex + 1} is malformed`);
    }
    if (typeof sample.sample_id !== 'string' || sample.sample_id.length === 0) {
      throw new Error(`${label} benchmark sample ${sampleIndex + 1} is missing a sample id`);
    }
    if (typeof sample.generated_at !== 'string' || sample.generated_at.length === 0) {
      throw new Error(`${label} benchmark sample ${sampleIndex + 1} is missing generated_at`);
    }
    if (!sample.benchmark || typeof sample.benchmark !== 'object' || Array.isArray(sample.benchmark)) {
      throw new Error(`${label} benchmark sample ${sampleIndex + 1} is missing the benchmark payload`);
    }
  }
}

function assertBenchmarkMedianAggregation(label, actual) {
  assertBenchmarkSamples(label, actual);

  const repeatCount = actual.benchmark_repeat_count;
  if (Number.isInteger(repeatCount) && actual.benchmark_samples.length !== repeatCount) {
    throw new Error(
      `${label} benchmark sample count mismatch: expected ${repeatCount} sample(s), got ${actual.benchmark_samples.length}`,
    );
  }

  const representativeSample = actual.benchmark_samples[actual.benchmark_representative_sample_index];
  if (!representativeSample) {
    throw new Error(`${label} benchmark artifact has an out-of-range representative sample index`);
  }
  if (representativeSample.sample_id !== actual.benchmark_representative_sample_id) {
    throw new Error(
      `${label} benchmark representative sample mismatch: expected ${representativeSample.sample_id}, got ${actual.benchmark_representative_sample_id}`,
    );
  }

  const timingMetricPaths = listBenchmarkTimingMetricPaths(actual.benchmark);
  const timingMetricPathSet = new Set(timingMetricPaths);
  for (const metricPath of timingMetricPaths) {
    const metricStatistics = actual.benchmark_metric_statistics?.[metricPath];
    if (!metricStatistics || typeof metricStatistics !== 'object' || Array.isArray(metricStatistics)) {
      throw new Error(`${label} benchmark artifact is missing benchmark metric statistics for ${metricPath}`);
    }

    const sampleValues = actual.benchmark_samples.map(function readSampleMetric(sample) {
      return getBenchmarkMetric(sample.benchmark, metricPath);
    });
    if (!sampleValues.every(Number.isFinite)) {
      throw new Error(`${label} benchmark samples are missing ${metricPath}`);
    }

    const expectedMetricStatistics = buildMetricStatistics(sampleValues);
    if (!expectedMetricStatistics || !isDeepStrictEqual(metricStatistics, expectedMetricStatistics)) {
      throw new Error(`${label} benchmark statistics mismatch for ${metricPath}`);
    }

    const benchmarkValue = getBenchmarkMetric(actual.benchmark, metricPath);
    if (!Number.isFinite(benchmarkValue)) {
      throw new Error(`${label} benchmark payload is missing ${metricPath}`);
    }
    if (roundMs(benchmarkValue) !== expectedMetricStatistics.median_ms) {
      throw new Error(`${label} benchmark median mismatch for ${metricPath}`);
    }

    if (
      Number.isInteger(repeatCount) &&
      expectedMetricStatistics.sample_count !== repeatCount
    ) {
      throw new Error(
        `${label} benchmark statistics sample count mismatch for ${metricPath}: expected ${repeatCount}, got ${expectedMetricStatistics.sample_count}`,
      );
    }
  }

  for (const metricPath of Object.keys(actual.benchmark_metric_statistics ?? {})) {
    if (!timingMetricPathSet.has(metricPath)) {
      throw new Error(`${label} benchmark metric statistics include an unknown metric path: ${metricPath}`);
    }
  }
}

function assertBenchmarkShape(label, expected, actual) {
  if (!actual.benchmark || typeof actual.benchmark !== 'object' || Array.isArray(actual.benchmark)) {
    throw new Error(`${label} benchmark artifact is missing the benchmark payload`);
  }

  assertBenchmarkFieldEqual(label, 'benchmark_repeat_count', expected, actual);
  assertBenchmarkFieldEqual(label, 'benchmark_aggregate_basis', expected, actual);

  if (expected.benchmark_metric_statistics != null) {
    if (
      !actual.benchmark_metric_statistics ||
      typeof actual.benchmark_metric_statistics !== 'object' ||
      Array.isArray(actual.benchmark_metric_statistics)
    ) {
      throw new Error(`${label} benchmark artifact is missing benchmark metric statistics`);
    }
    assertBenchmarkMedianAggregation(label, actual);
  }

  if (expected.benchmark_representative_sample_index != null) {
    if (!Number.isInteger(actual.benchmark_representative_sample_index)) {
      throw new Error(`${label} benchmark artifact is missing the representative sample index`);
    }
  }

  if (expected.benchmark_representative_sample_id != null) {
    if (
      typeof actual.benchmark_representative_sample_id !== 'string' ||
      actual.benchmark_representative_sample_id.length === 0
    ) {
      throw new Error(`${label} benchmark artifact is missing the representative sample id`);
    }
  }

  if (expected.benchmark_samples != null) {
    assertBenchmarkSamples(label, actual);
  }
}

export function assertBenchmarkArtifact({ expectedPath, actualPath, label }) {
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

  assertBenchmarkShape(label, expected, actual);
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
  benchmarkRepeats = null,
  skipGrammarDownload,
  repoWorkspaceRoot,
  nodeBin = process.execPath,
}) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), tempRootPrefix));
  const tempGoldenDir = path.join(tempRoot, 'goldens');
  const tempBenchmarkPath = path.join(tempRoot, benchmarkTempFilename);
  const baseEnv = buildValidationEnv({
    repoEnvVar,
    repoRoot,
    sentruxBin,
    skipGrammarDownload,
  });

  try {
    if (runGoldens) {
      runCheckedWithRetry('bash', [refreshScript], {
        cwd: repoWorkspaceRoot,
        env: {
          ...baseEnv,
          OUTPUT_DIR: tempGoldenDir,
        },
      });
      compareGoldenDirectories(expectedGoldenDir, tempGoldenDir);
      console.log(`Validated ${repoLabel} goldens against ${expectedGoldenDir}`);
    }

    if (runBenchmark) {
      runChecked(nodeBin, [benchmarkScript], {
        cwd: repoWorkspaceRoot,
        env: buildBenchmarkEnv({
          baseEnv,
          tempBenchmarkPath,
          expectedBenchmarkPath,
          benchmarkRepeats,
        }),
      });

      assertBenchmarkArtifact({
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
