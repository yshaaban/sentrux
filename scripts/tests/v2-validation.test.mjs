import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { buildMetricStatistics } from '../lib/benchmark-harness.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { compareJsonFiles, runValidationSuite } from '../lib/v2-validation.mjs';

async function createValidationFixture({ benchmarkFormatVersion = 3 } = {}) {
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

function runFixtureValidation(fixture, overrides = {}) {
  return runValidationSuite({
    repoLabel: 'one-tool',
    repoEnvVar: 'ONE_TOOL_DIR',
    repoRoot: fixture.repoRoot,
    sentruxBin: '/tmp/sentrux',
    refreshScript: fixture.refreshScript,
    benchmarkScript: fixture.benchmarkScript,
    expectedGoldenDir: fixture.expectedGoldenDir,
    expectedBenchmarkPath: fixture.expectedBenchmarkPath,
    benchmarkTempFilename: 'benchmark.json',
    tempRootPrefix: 'sentrux-v2-validation-suite-',
    keepTemp: false,
    skipGrammarDownload: '1',
    repoWorkspaceRoot: fixture.root,
    nodeBin: process.execPath,
    ...overrides,
  });
}

function buildRepeatedBenchmarkArtifact({
  coldProcessValues = [39, 40, 41],
  coldFindingsValues = null,
  representativeSampleIndex = Math.floor(coldProcessValues.length / 2),
} = {}) {
  const benchmarkSamples = coldProcessValues.map(function buildSample(coldProcessTotalMs, index) {
    const benchmark = {
      cold_process_total_ms: coldProcessTotalMs,
    };

    if (Array.isArray(coldFindingsValues)) {
      benchmark.cold = {
        findings: {
          elapsed_ms: coldFindingsValues[index],
          summary: `summary-${index + 1}`,
        },
      };
    }

    return {
      sample_id: `sample_${index + 1}`,
      generated_at: `2026-04-15T00:0${index}:00.000Z`,
      benchmark,
    };
  });

  const representativeSample = benchmarkSamples[representativeSampleIndex];
  const benchmark = structuredClone(representativeSample.benchmark);
  const metricStatistics = {
    cold_process_total_ms: buildMetricStatistics(coldProcessValues),
  };

  benchmark.cold_process_total_ms = metricStatistics.cold_process_total_ms.median_ms;

  if (Array.isArray(coldFindingsValues)) {
    metricStatistics['cold.findings.elapsed_ms'] = buildMetricStatistics(coldFindingsValues);
    benchmark.cold.findings.elapsed_ms = metricStatistics['cold.findings.elapsed_ms'].median_ms;
  }

  return {
    benchmark_format_version: 3,
    benchmark_repeat_count: benchmarkSamples.length,
    benchmark_aggregate_basis: 'median',
    benchmark_representative_sample_index: representativeSampleIndex,
    benchmark_representative_sample_id: representativeSample.sample_id,
    benchmark_metric_statistics: metricStatistics,
    benchmark_samples: benchmarkSamples,
    benchmark,
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
  const fixture = await createValidationFixture();

  try {
    await runFixtureValidation(fixture, {
      runGoldens: true,
      runBenchmark: true,
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
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /benchmark format mismatch/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('compareJsonFiles ignores metadata timestamps and binary hashes', async function () {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sentrux-v2-validation-metadata-'));
  const expectedPath = path.join(root, 'expected-metadata.json');
  const actualPath = path.join(root, 'actual-metadata.json');

  try {
    const basePayload = {
      repo_root: '<public-repo-root>',
      analysis_mode: 'head_clone',
      binary_identity: {
        path: '<sentrux-root>/target/debug/sentrux',
        exists: true,
      },
    };
    await writeFile(
      expectedPath,
      `${JSON.stringify(
        {
          ...basePayload,
          generated_at: '2026-04-16T11:40:41Z',
          binary_identity: {
            ...basePayload.binary_identity,
            sha256: 'old-hash',
          },
        },
        null,
        2,
      )}\n`,
    );
    await writeFile(
      actualPath,
      `${JSON.stringify(
        {
          ...basePayload,
          generated_at: '2026-04-16T12:21:44Z',
          binary_identity: {
            ...basePayload.binary_identity,
            sha256: 'new-hash',
          },
        },
        null,
        2,
      )}\n`,
    );

    assert.doesNotThrow(function compareMetadata() {
      compareJsonFiles(expectedPath, actualPath, 'metadata.json');
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('runValidationSuite propagates benchmark runner failures', async function () {
  const fixture = await createValidationFixture();

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
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /exited with code 3/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite passes benchmark gating env to the runner', async function () {
  const fixture = await createValidationFixture();

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
      'if (process.env.BENCHMARK_REPEATS !== "5") {',
      '  throw new Error(`Expected BENCHMARK_REPEATS=5, got ${process.env.BENCHMARK_REPEATS ?? "missing"}`);',
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
    await runFixtureValidation(fixture, {
      runGoldens: false,
      runBenchmark: true,
      benchmarkRepeats: '5',
    });
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite accepts repeated benchmark artifacts with additive fields', async function () {
  const fixture = await createValidationFixture();
  const repeatedBenchmark = buildRepeatedBenchmarkArtifact();

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
      repoLabel: 'one-tool',
      repoEnvVar: 'ONE_TOOL_DIR',
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

test('runValidationSuite rejects mismatched repeated benchmark metadata', async function () {
  const fixture = await createValidationFixture();
  const expectedBenchmark = buildRepeatedBenchmarkArtifact({
    coldProcessValues: [39, 40, 41, 42, 43],
    representativeSampleIndex: 2,
  });
  const actualBenchmark = {
    ...expectedBenchmark,
    benchmark_repeat_count: 3,
    benchmark_samples: expectedBenchmark.benchmark_samples.slice(0, 3),
  };

  await writeFile(fixture.expectedBenchmarkPath, `${JSON.stringify(expectedBenchmark, null, 2)}\n`);
  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const actualBenchmark = ${JSON.stringify(actualBenchmark)};`,
      'await writeFile(process.env.OUTPUT_PATH, `${JSON.stringify(actualBenchmark, null, 2)}\\n`);',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /benchmark repeat count mismatch/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite rejects repeated benchmark artifacts missing timing statistics', async function () {
  const fixture = await createValidationFixture();
  const expectedBenchmark = buildRepeatedBenchmarkArtifact({
    coldFindingsValues: [120, 100, 140],
  });
  const actualBenchmark = structuredClone(expectedBenchmark);
  delete actualBenchmark.benchmark_metric_statistics['cold.findings.elapsed_ms'];

  await writeFile(fixture.expectedBenchmarkPath, `${JSON.stringify(expectedBenchmark, null, 2)}\n`);
  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const actualBenchmark = ${JSON.stringify(actualBenchmark)};`,
      'await writeFile(process.env.OUTPUT_PATH, `${JSON.stringify(actualBenchmark, null, 2)}\\n`);',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /missing benchmark metric statistics for cold\.findings\.elapsed_ms/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite rejects repeated benchmark artifacts with sample/statistics mismatches', async function () {
  const fixture = await createValidationFixture();
  const expectedBenchmark = buildRepeatedBenchmarkArtifact({
    coldFindingsValues: [120, 100, 140],
  });
  const actualBenchmark = structuredClone(expectedBenchmark);
  actualBenchmark.benchmark_metric_statistics['cold.findings.elapsed_ms'].values_ms = [120, 101, 140];

  await writeFile(fixture.expectedBenchmarkPath, `${JSON.stringify(expectedBenchmark, null, 2)}\n`);
  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const actualBenchmark = ${JSON.stringify(actualBenchmark)};`,
      'await writeFile(process.env.OUTPUT_PATH, `${JSON.stringify(actualBenchmark, null, 2)}\\n`);',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /benchmark statistics mismatch for cold\.findings\.elapsed_ms/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('runValidationSuite rejects repeated benchmark artifacts with benchmark median mismatches', async function () {
  const fixture = await createValidationFixture();
  const expectedBenchmark = buildRepeatedBenchmarkArtifact({
    coldFindingsValues: [120, 100, 140],
  });
  const actualBenchmark = structuredClone(expectedBenchmark);
  actualBenchmark.benchmark.cold.findings.elapsed_ms = 101;

  await writeFile(fixture.expectedBenchmarkPath, `${JSON.stringify(expectedBenchmark, null, 2)}\n`);
  await writeFile(
    fixture.benchmarkScript,
    [
      'import { writeFile } from "node:fs/promises";',
      '',
      `const actualBenchmark = ${JSON.stringify(actualBenchmark)};`,
      'await writeFile(process.env.OUTPUT_PATH, `${JSON.stringify(actualBenchmark, null, 2)}\\n`);',
      '',
    ].join('\n'),
  );

  try {
    await assert.rejects(
      runFixtureValidation(fixture, {
        runGoldens: false,
        runBenchmark: true,
      }),
      /benchmark median mismatch for cold\.findings\.elapsed_ms/,
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});
