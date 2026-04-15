import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { runValidationSuite } from '../lib/v2-validation.mjs';

async function createValidationFixture({ benchmarkFormatVersion = 3 }) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sentrux-v2-validation-test-'));
  const repoRoot = path.join(root, 'sample-repo');
  const expectedGoldenDir = path.join(root, 'expected-goldens');
  const expectedBenchmarkPath = path.join(root, 'expected-benchmark.json');
  const refreshScript = path.join(root, 'refresh.sh');
  const benchmarkScript = path.join(root, 'benchmark.mjs');

  await mkdir(repoRoot, { recursive: true });
  await mkdir(expectedGoldenDir, { recursive: true });

  await writeFile(
    path.join(expectedGoldenDir, 'finding.json'),
    `${JSON.stringify({ id: 'finding-a', severity: 'high' }, null, 2)}\n`,
  );
  await writeFile(
    expectedBenchmarkPath,
    `${JSON.stringify(
      {
        benchmark_format_version: benchmarkFormatVersion,
        benchmark: {
          cold_process_total_ms: 42,
        },
      },
      null,
      2,
    )}\n`,
  );

  await writeFile(
    refreshScript,
    [
      '#!/usr/bin/env bash',
      'set -euo pipefail',
      'mkdir -p "$OUTPUT_DIR"',
      `cp "${path.join(expectedGoldenDir, 'finding.json')}" "$OUTPUT_DIR/finding.json"`,
      '',
    ].join('\n'),
  );
  await chmod(refreshScript, 0o755);

  await writeFile(
    benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const benchmarkFormatVersion = ${benchmarkFormatVersion};`,
      'await writeFile(',
      '  process.env.OUTPUT_PATH,',
      '  `${JSON.stringify({ benchmark_format_version: benchmarkFormatVersion, benchmark: { cold_process_total_ms: 40 } }, null, 2)}\\n`,',
      ');',
      '',
    ].join('\n'),
  );

  return {
    root,
    repoRoot,
    expectedGoldenDir,
    expectedBenchmarkPath,
    refreshScript,
    benchmarkScript,
  };
}

test('resolveWorkspaceRepoRoot prefers explicit env values and otherwise falls back to sibling repo roots', function () {
  assert.equal(
    resolveWorkspaceRepoRoot('/tmp/custom-root', 'one-tool', '/workspace/sentrux'),
    '/tmp/custom-root',
  );
  assert.equal(
    resolveWorkspaceRepoRoot('', 'one-tool', '/workspace/sentrux'),
    '/workspace/one-tool',
  );
});

test('runValidationSuite validates matching goldens and benchmark artifacts', async function () {
  const fixture = await createValidationFixture({});

  try {
    await runValidationSuite({
      repoLabel: 'sample-project',
      repoEnvVar: 'SAMPLE_PROJECT_ROOT',
      repoRoot: fixture.repoRoot,
      sentruxBin: '/tmp/sentrux',
      refreshScript: fixture.refreshScript,
      benchmarkScript: fixture.benchmarkScript,
      expectedGoldenDir: fixture.expectedGoldenDir,
      expectedBenchmarkPath: fixture.expectedBenchmarkPath,
      benchmarkTempFilename: 'benchmark.json',
      tempRootPrefix: 'sentrux-v2-validation-suite-',
      keepTemp: false,
      runGoldens: true,
      runBenchmark: true,
      skipGrammarDownload: '1',
      repoWorkspaceRoot: fixture.root,
      nodeBin: process.execPath,
    });

    const expectedBenchmark = JSON.parse(await readFile(fixture.expectedBenchmarkPath, 'utf8'));
    assert.equal(expectedBenchmark.benchmark_format_version, 3);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite rejects benchmark format mismatches', async function () {
  const fixture = await createValidationFixture({ benchmarkFormatVersion: 3 });

  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      'await writeFile(',
      '  process.env.OUTPUT_PATH,',
      '  `${JSON.stringify({ benchmark_format_version: 4, benchmark: { cold_process_total_ms: 40 } }, null, 2)}\\n`,',
      ');',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runValidationSuite({
        repoLabel: 'sample-project',
        repoEnvVar: 'SAMPLE_PROJECT_ROOT',
        repoRoot: fixture.repoRoot,
        sentruxBin: '/tmp/sentrux',
        refreshScript: fixture.refreshScript,
        benchmarkScript: fixture.benchmarkScript,
        expectedGoldenDir: fixture.expectedGoldenDir,
        expectedBenchmarkPath: fixture.expectedBenchmarkPath,
        benchmarkTempFilename: 'benchmark.json',
        tempRootPrefix: 'sentrux-v2-validation-suite-',
        keepTemp: false,
        runGoldens: false,
        runBenchmark: true,
        skipGrammarDownload: '1',
        repoWorkspaceRoot: fixture.root,
        nodeBin: process.execPath,
      }),
      /benchmark format mismatch/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite propagates benchmark runner failures', async function () {
  const fixture = await createValidationFixture({});

  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      'await writeFile(',
      '  process.env.OUTPUT_PATH,',
      '  `${JSON.stringify({ benchmark_format_version: 3, benchmark: { cold_process_total_ms: 40 } }, null, 2)}\\n`,',
      ');',
      'process.exitCode = 3;',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runValidationSuite({
        repoLabel: 'sample-project',
        repoEnvVar: 'SAMPLE_PROJECT_ROOT',
        repoRoot: fixture.repoRoot,
        sentruxBin: '/tmp/sentrux',
        refreshScript: fixture.refreshScript,
        benchmarkScript: fixture.benchmarkScript,
        expectedGoldenDir: fixture.expectedGoldenDir,
        expectedBenchmarkPath: fixture.expectedBenchmarkPath,
        benchmarkTempFilename: 'benchmark.json',
        tempRootPrefix: 'sentrux-v2-validation-suite-',
        keepTemp: false,
        runGoldens: false,
        runBenchmark: true,
        skipGrammarDownload: '1',
        repoWorkspaceRoot: fixture.root,
        nodeBin: process.execPath,
      }),
      /exited with code 3/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite passes benchmark gating env to the runner', async function () {
  const fixture = await createValidationFixture({});

  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      'if (process.env.FAIL_ON_REGRESSION !== "1") {',
      '  throw new Error(`Expected FAIL_ON_REGRESSION=1, got ${process.env.FAIL_ON_REGRESSION ?? "missing"}`);',
      '}',
      'if (process.env.FAIL_ON_NONCOMPARABLE !== "1") {',
      '  throw new Error(`Expected FAIL_ON_NONCOMPARABLE=1, got ${process.env.FAIL_ON_NONCOMPARABLE ?? "missing"}`);',
      '}',
      '',
      'await writeFile(',
      '  process.env.OUTPUT_PATH,',
      '  `${JSON.stringify({ benchmark_format_version: 3, benchmark: { cold_process_total_ms: 40 } }, null, 2)}\\n`,',
      ');',
      '',
    ].join('\n'),
  );

  try {
    await runValidationSuite({
      repoLabel: 'sample-project',
      repoEnvVar: 'SAMPLE_PROJECT_ROOT',
      repoRoot: fixture.repoRoot,
      sentruxBin: '/tmp/sentrux',
      refreshScript: fixture.refreshScript,
      benchmarkScript: fixture.benchmarkScript,
      expectedGoldenDir: fixture.expectedGoldenDir,
      expectedBenchmarkPath: fixture.expectedBenchmarkPath,
      benchmarkTempFilename: 'benchmark.json',
      tempRootPrefix: 'sentrux-v2-validation-suite-',
      keepTemp: false,
      runGoldens: false,
      runBenchmark: true,
      skipGrammarDownload: '1',
      repoWorkspaceRoot: fixture.root,
      nodeBin: process.execPath,
    });
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite accepts repeated benchmark artifacts with additive fields', async function () {
  const fixture = await createValidationFixture({});
  const repeatedBenchmark = {
    benchmark_format_version: 3,
    benchmark_repeat_count: 3,
    benchmark_aggregate_basis: 'median',
    benchmark_representative_sample_index: 1,
    benchmark_representative_sample_id: 'sample_2',
    benchmark_metric_statistics: {
      cold_process_total_ms: {
        sample_count: 3,
        values_ms: [39, 40, 41],
        min_ms: 39,
        max_ms: 41,
        median_ms: 40,
        mean_ms: 40,
        stddev_ms: 0.8,
        spread_ms: 2,
      },
    },
    benchmark_samples: [
      { sample_id: 'sample_1', generated_at: '2026-04-15T00:00:00.000Z', benchmark: { cold_process_total_ms: 39 } },
      { sample_id: 'sample_2', generated_at: '2026-04-15T00:01:00.000Z', benchmark: { cold_process_total_ms: 40 } },
      { sample_id: 'sample_3', generated_at: '2026-04-15T00:02:00.000Z', benchmark: { cold_process_total_ms: 41 } },
    ],
    benchmark: {
      cold_process_total_ms: 40,
    },
  };

  await writeFile(fixture.expectedBenchmarkPath, `${JSON.stringify(repeatedBenchmark, null, 2)}\n`);
  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const repeatedBenchmark = ${JSON.stringify(repeatedBenchmark)};`,
      'await writeFile(process.env.OUTPUT_PATH, `${JSON.stringify(repeatedBenchmark, null, 2)}\\n`);',
      '',
    ].join('\n'),
  );

  try {
    await runValidationSuite({
      repoLabel: 'sample-project',
      repoEnvVar: 'SAMPLE_PROJECT_ROOT',
      repoRoot: fixture.repoRoot,
      sentruxBin: '/tmp/sentrux',
      refreshScript: fixture.refreshScript,
      benchmarkScript: fixture.benchmarkScript,
      expectedGoldenDir: fixture.expectedGoldenDir,
      expectedBenchmarkPath: fixture.expectedBenchmarkPath,
      benchmarkTempFilename: 'benchmark.json',
      tempRootPrefix: 'sentrux-v2-validation-suite-',
      keepTemp: false,
      runGoldens: false,
      runBenchmark: true,
      skipGrammarDownload: '1',
      repoWorkspaceRoot: fixture.root,
      nodeBin: process.execPath,
    });
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});
