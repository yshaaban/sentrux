import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildBenchmarkComparison,
  buildBenchmarkPolicy,
  classifyBenchmarkMetric,
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
  });
});
