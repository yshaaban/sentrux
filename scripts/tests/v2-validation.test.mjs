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
