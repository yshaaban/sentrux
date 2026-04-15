import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildAggregatedBenchmark,
  buildBenchmarkComparison,
  buildBenchmarkPolicy,
  classifyBenchmarkMetric,
  runRepeatedBenchmarkSamples,
  runCommand,
} from '../lib/benchmark-harness.mjs';

test('classifyBenchmarkMetric only trips after the configured boundaries', function () {
  const policy = buildBenchmarkPolicy({
    MAX_REGRESSION_MS: '250',
    MAX_REGRESSION_PERCENT: '20',
    WARN_REGRESSION_MS: '150',
    WARN_REGRESSION_PERCENT: '10',
  });

  assert.equal(classifyBenchmarkMetric(150, 10, policy), 'info');
  assert.equal(classifyBenchmarkMetric(151, 10, policy), 'info');
  assert.equal(classifyBenchmarkMetric(151, 11, policy), 'warn');
  assert.equal(classifyBenchmarkMetric(251, 21, policy), 'fail');
});

test('buildBenchmarkComparison returns null when benchmark format versions differ', function () {
  const policy = buildBenchmarkPolicy();
  const comparison = buildBenchmarkComparison({
    currentResult: {
      benchmark_format_version: 2,
      benchmark: {
        cold_process_total_ms: 10,
      },
    },
    previousResult: {
      benchmark_format_version: 3,
      generated_at: '2026-03-01T00:00:00.000Z',
      benchmark: {
        cold_process_total_ms: 5,
      },
    },
    compareToPath: '/tmp/previous.json',
    benchmarkPolicy: policy,
    trackedMetrics: [['cold_process_total_ms', 'cold process total']],
  });

  assert.equal(comparison, null);
});

test('buildBenchmarkComparison preserves info metrics when thresholds are not crossed', function () {
  const policy = buildBenchmarkPolicy({
    MAX_REGRESSION_MS: '250',
    MAX_REGRESSION_PERCENT: '20',
    WARN_REGRESSION_MS: '150',
    WARN_REGRESSION_PERCENT: '10',
  });

  const comparison = buildBenchmarkComparison({
    currentResult: {
      benchmark_format_version: 2,
      generated_at: '2026-04-01T00:00:00.000Z',
      benchmark: {
        cold_process_total_ms: 110,
      },
    },
    previousResult: {
      benchmark_format_version: 2,
      generated_at: '2026-03-01T00:00:00.000Z',
      benchmark: {
        cold_process_total_ms: 100,
      },
    },
    compareToPath: '/tmp/previous.json',
    benchmarkPolicy: policy,
    trackedMetrics: [['cold_process_total_ms', 'cold process total']],
  });

  assert(comparison);
  assert.equal(comparison.metrics[0].classification, 'info');
  assert.deepEqual(comparison.summary, {
    total: 1,
    fail_count: 0,
    warn_count: 0,
    info_count: 1,
    blocked_count: 0,
  });
});

test('buildAggregatedBenchmark uses median timings and records sample statistics', function () {
  const aggregate = buildAggregatedBenchmark({
    trackedMetrics: [['cold_process_total_ms', 'cold process total']],
    samples: [
      {
        sample_id: 'sample_1',
        benchmark: {
          cold_process_total_ms: 100,
          sample_label: 'first',
          stdout_log: ['first'],
          stderr_log: [],
        },
      },
      {
        sample_id: 'sample_2',
        benchmark: {
          cold_process_total_ms: 110,
          sample_label: 'second',
          stdout_log: ['second'],
          stderr_log: [],
        },
      },
      {
        sample_id: 'sample_3',
        benchmark: {
          cold_process_total_ms: 120,
          sample_label: 'third',
          stdout_log: ['third'],
          stderr_log: [],
        },
      },
    ],
  });

  assert(aggregate);
  assert.equal(aggregate.aggregate_benchmark.cold_process_total_ms, 110);
  assert.equal(aggregate.aggregate_benchmark.sample_label, 'second');
  assert.equal(aggregate.aggregate_benchmark.stdout_log, undefined);
  assert.equal(aggregate.sample_count, 3);
  assert.equal(aggregate.representative_sample_index, 1);
  assert.equal(aggregate.representative_sample_id, 'sample_2');
  assert.deepEqual(aggregate.metric_statistics['cold_process_total_ms'], {
    sample_count: 3,
    values_ms: [100, 110, 120],
    min_ms: 100,
    max_ms: 120,
    median_ms: 110,
    mean_ms: 110,
    stddev_ms: 8.2,
    spread_ms: 20,
  });
});

test('buildBenchmarkComparison records aggregate basis metadata', function () {
  const policy = buildBenchmarkPolicy();
  const comparison = buildBenchmarkComparison({
    currentResult: {
      benchmark_format_version: 3,
      benchmark_aggregate_basis: 'median',
      benchmark_repeat_count: 3,
      benchmark: {
        cold_process_total_ms: 95,
      },
    },
    previousResult: {
      benchmark_format_version: 3,
      benchmark_aggregate_basis: 'median',
      benchmark_repeat_count: 5,
      generated_at: '2026-03-01T00:00:00.000Z',
      benchmark: {
        cold_process_total_ms: 100,
      },
    },
    compareToPath: '/tmp/previous.json',
    benchmarkPolicy: policy,
    trackedMetrics: [['cold_process_total_ms', 'cold process total']],
  });

  assert(comparison);
  assert.deepEqual(comparison.aggregate_basis, {
    current: 'median',
    previous: 'median',
    current_sample_count: 3,
    previous_sample_count: 5,
  });
});

test('buildBenchmarkComparison blocks gated verdicts when repo identities differ', function () {
  const policy = buildBenchmarkPolicy();
  const comparison = buildBenchmarkComparison({
    currentResult: {
      benchmark_format_version: 3,
      benchmark: {
        cold_process_total_ms: 110,
      },
      source_tree_identity: {
        commit: 'current',
        dirty_paths: [],
        dirty_paths_count: 0,
        dirty_paths_fingerprint: 'current-dirty',
        tree_fingerprint: 'current-tree',
        analysis_mode: 'head_clone',
      },
    },
    previousResult: {
      benchmark_format_version: 3,
      generated_at: '2026-03-01T00:00:00.000Z',
      benchmark: {
        cold_process_total_ms: 100,
      },
      source_tree_identity: {
        commit: 'previous',
        dirty_paths: [],
        dirty_paths_count: 0,
        dirty_paths_fingerprint: 'previous-dirty',
        tree_fingerprint: 'previous-tree',
        analysis_mode: 'head_clone',
      },
    },
    compareToPath: '/tmp/previous.json',
    benchmarkPolicy: policy,
    trackedMetrics: [['cold_process_total_ms', 'cold process total']],
  });

  assert(comparison);
  assert.equal(comparison.comparable, false);
  assert.equal(comparison.metrics[0].classification, 'blocked');
  assert.equal(comparison.blocked_metrics.length, 1);
  assert.equal(comparison.regressions.length, 0);
  assert.equal(comparison.warnings.length, 0);
  assert.deepEqual(comparison.summary, {
    total: 1,
    fail_count: 0,
    warn_count: 0,
    info_count: 0,
    blocked_count: 1,
  });
});

test('runRepeatedBenchmarkSamples keeps benchmark samples and enforces freshness consistency', async function () {
  const samples = await runRepeatedBenchmarkSamples({
    repeatCount: 2,
    runSample: async function runSample(sampleIndex) {
      return {
        sample_id: `sample_${sampleIndex + 1}`,
        generated_at: `2026-04-15T00:0${sampleIndex}:00.000Z`,
        benchmark: {
          cold_process_total_ms: 100 + sampleIndex,
        },
        freshnessMetadata: {
          source_tree_identity: {
            commit: 'abc123',
          },
        },
      };
    },
  });

  assert.deepEqual(samples, {
    samples: [
      {
        sample_id: 'sample_1',
        generated_at: '2026-04-15T00:00:00.000Z',
        benchmark: {
          cold_process_total_ms: 100,
        },
      },
      {
        sample_id: 'sample_2',
        generated_at: '2026-04-15T00:01:00.000Z',
        benchmark: {
          cold_process_total_ms: 101,
        },
      },
    ],
    freshnessMetadata: {
      source_tree_identity: {
        commit: 'abc123',
      },
    },
  });

  await assert.rejects(
    runRepeatedBenchmarkSamples({
      repeatCount: 2,
      runSample: async function runSample(sampleIndex) {
        return {
          sample_id: `sample_${sampleIndex + 1}`,
          generated_at: `2026-04-15T00:0${sampleIndex}:00.000Z`,
          benchmark: {
            cold_process_total_ms: 100 + sampleIndex,
          },
          freshnessMetadata: {
            source_tree_identity: {
              commit: sampleIndex === 0 ? 'abc123' : 'def456',
            },
          },
        };
      },
    }),
    /freshness metadata changed during repeated runs at sample_2/,
  );
});

test('runCommand captures stdout, stderr, and exit code', async function () {
  const result = await runCommand(
    process.execPath,
    ['-e', 'console.log("hello"); console.error("warn"); process.exit(3);'],
  );

  assert.equal(result.exit_code, 3);
  assert.equal(result.signal, null);
  assert.match(result.stdout, /hello/);
  assert.match(result.stderr, /warn/);
  assert(result.elapsed_ms >= 0);
});
