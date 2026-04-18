import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildPacketFromArtifactInput,
  loadArtifactInput,
  parseArgs,
} from '../../evals/build-check-review-packet.mjs';
import { writeJson } from './helpers.mjs';

test('loadArtifactInput reads a single bundle artifact', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-bundle-'));
  try {
    const bundlePath = path.join(tempRoot, 'codex-session.json');
    await writeJson(bundlePath, {
      repo_root: '/tmp/parallel-code',
      confidence: {
        rule_coverage_0_10000: 9000,
        scan_confidence_0_10000: 9100,
        semantic_rules_loaded: true,
      },
      scan_trust: {
        candidate_files: 12,
        kept_files: 9,
        overall_confidence_0_10000: 9100,
        partial: false,
        scope_coverage_0_10000: 9000,
        exclusions: {
          total: 3,
        },
        resolution: {
          resolved: 8,
          unresolved_internal: 1,
          unresolved_external: 0,
          unresolved_unknown: 0,
        },
      },
      initial_check: {
        actions: [
          {
            kind: 'exact_clone_group',
            scope: 'src/a.ts',
            severity: 'high',
            summary: 'Clone group',
            evidence: ['e1'],
            files: ['src/a.ts', 'src/b.ts'],
            instances: [
              { file: 'src/a.ts', func: 'resume', lines: 11, commit_count: 4 },
              { file: 'src/b.ts', func: 'resume', lines: 11, commit_count: 7 },
            ],
            total_lines: 22,
            max_lines: 11,
            reasons: ['identical logic spans 2 files', 'youngest clone file was touched recently'],
            asymmetric_recent_change: true,
          },
        ],
      },
      session_end: {
        introduced_findings: [
          {
            kind: 'missing_test_coverage',
            scope: 'src/b.ts',
            severity: 'watchpoint',
            summary: 'Missing test',
            evidence: ['e2'],
          },
        ],
      },
    });

    const source = await loadArtifactInput({ bundlePath, codexBatchPath: null, replayBatchPath: null });
    const packet = buildPacketFromArtifactInput(
      { tool: 'check', limit: 10, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(source.source_mode, 'bundle');
    assert.equal(packet.repo_root, '/tmp/parallel-code');
    assert.equal(packet.source_mode, 'bundle');
    assert.equal(packet.scan_metadata.source_kind, 'bundle');
    assert.equal(packet.scan_metadata.source_label, 'codex-session');
    assert.equal(packet.scan_metadata.scan_trust.candidate_files, 12);
    assert.equal(packet.samples[0].kind, 'exact_clone_group');
    assert.deepEqual(packet.samples[0].clone_evidence.files, ['src/a.ts', 'src/b.ts']);
    assert.equal(packet.samples[0].clone_evidence.total_lines, 22);
    assert.equal(packet.samples.length, 1);
    assert.equal(packet.summary.sample_count, 1);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('loadArtifactInput reads codex batch bundles from per-task artifacts', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-codex-batch-'));
  try {
    const batchPath = path.join(tempRoot, 'codex-session-batch.json');
    const taskOneDir = path.join(tempRoot, 'task-one');
    const taskTwoDir = path.join(tempRoot, 'task-two');
    await writeJson(path.join(taskOneDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [],
      },
      final_check: {
        actions: [
          {
            kind: 'forbidden_raw_read',
            scope: 'src/a.ts',
            severity: 'high',
            summary: 'Raw read',
            evidence: ['e1'],
          },
        ],
      },
    });
    await writeJson(path.join(taskTwoDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'closed_domain_exhaustiveness',
            scope: 'src/b.ts',
            severity: 'high',
            summary: 'Exhaustiveness',
            evidence: ['e2'],
          },
        ],
      },
    });
    await writeJson(batchPath, {
      repo_root: '/tmp/parallel-code',
      results: [
        { task_id: 'task-one', task_label: 'Task one', output_dir: taskOneDir },
        { task_id: 'task-two', task_label: 'Task two', output_dir: taskTwoDir },
      ],
    });

    const source = await loadArtifactInput({ codexBatchPath: batchPath, bundlePath: null, replayBatchPath: null });
    const packet = buildPacketFromArtifactInput(
      { tool: 'check', limit: 10, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(source.source_mode, 'codex-batch');
    assert.equal(packet.source_paths.length, 3);
    assert.deepEqual(
      packet.samples.map((sample) => sample.review_id),
      ['check-1', 'check-2'],
    );
    assert.deepEqual(
      packet.samples.map((sample) => sample.kind),
      ['closed_domain_exhaustiveness', 'forbidden_raw_read'],
    );
    assert.equal(packet.samples[0].snapshot_label, 'initial_check');
    assert.equal(packet.samples[0].source_label, 'Task two');
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('loadArtifactInput reads replay batch bundles and session_end findings', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-replay-batch-'));
  try {
    const batchPath = path.join(tempRoot, 'diff-replay-batch.json');
    const replayOneDir = path.join(tempRoot, 'replay-one');
    await writeJson(path.join(replayOneDir, 'diff-replay.json'), {
      repo_root: '/tmp/parallel-code',
      session_end: {
        introduced_findings: [
          {
            kind: 'large_file',
            scope: 'src/c.ts',
            severity: 'high',
            summary: 'Large file',
            evidence: ['e3'],
          },
        ],
      },
    });
    await writeJson(batchPath, {
      repo_root: '/tmp/parallel-code',
      results: [{ output_dir: replayOneDir }],
    });

    const source = await loadArtifactInput({ replayBatchPath: batchPath, bundlePath: null, codexBatchPath: null });
    const packet = buildPacketFromArtifactInput(
      { tool: 'session_end', limit: 10, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(source.source_mode, 'replay-batch');
    assert.equal(packet.samples.length, 1);
    assert.equal(packet.samples[0].kind, 'large_file');
    assert.equal(packet.samples[0].scope, 'src/c.ts');
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildPacketFromArtifactInput reranks batch samples before truncation', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-rerank-'));
  try {
    const batchPath = path.join(tempRoot, 'codex-session-batch.json');
    const taskOneDir = path.join(tempRoot, 'task-one');
    const taskTwoDir = path.join(tempRoot, 'task-two');
    await writeJson(path.join(taskOneDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'large_file',
            scope: 'src/a.ts',
            severity: 'high',
            summary: 'Large file',
          },
        ],
      },
    });
    await writeJson(path.join(taskTwoDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'forbidden_raw_read',
            scope: 'src/b.ts',
            severity: 'high',
            summary: 'Raw read',
          },
        ],
      },
    });
    await writeJson(batchPath, {
      repo_root: '/tmp/parallel-code',
      results: [
        { task_id: 'task-one', task_label: 'Task one', output_dir: taskOneDir },
        { task_id: 'task-two', task_label: 'Task two', output_dir: taskTwoDir },
      ],
    });

    const source = await loadArtifactInput({ codexBatchPath: batchPath, bundlePath: null, replayBatchPath: null });
    const packet = buildPacketFromArtifactInput(
      { tool: 'check', limit: 1, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(packet.samples.length, 1);
    assert.equal(packet.samples[0].kind, 'forbidden_raw_read');
    assert.equal(packet.samples[0].source_label, 'Task two');
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildPacketFromArtifactInput reuses scan metadata from the retained top-ranked sample', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-metadata-selection-'));
  try {
    const batchPath = path.join(tempRoot, 'codex-session-batch.json');
    const taskOneDir = path.join(tempRoot, 'task-one');
    const taskTwoDir = path.join(tempRoot, 'task-two');
    await writeJson(path.join(taskOneDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        confidence: {
          scan_confidence_0_10000: 3000,
        },
        scan_trust: {
          kept_files: 3,
        },
        actions: [
          {
            kind: 'large_file',
            scope: 'src/a.ts',
            severity: 'high',
            summary: 'Large file',
          },
        ],
      },
    });
    await writeJson(path.join(taskTwoDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        confidence: {
          scan_confidence_0_10000: 9000,
        },
        scan_trust: {
          kept_files: 9,
        },
        actions: [
          {
            kind: 'forbidden_raw_read',
            scope: 'src/b.ts',
            severity: 'high',
            summary: 'Raw read',
          },
        ],
      },
    });
    await writeJson(batchPath, {
      repo_root: '/tmp/parallel-code',
      results: [
        { task_id: 'task-one', task_label: 'Task one', output_dir: taskOneDir },
        { task_id: 'task-two', task_label: 'Task two', output_dir: taskTwoDir },
      ],
    });

    const source = await loadArtifactInput({
      codexBatchPath: batchPath,
      bundlePath: null,
      replayBatchPath: null,
    });
    const packet = buildPacketFromArtifactInput(
      { tool: 'check', limit: 1, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(packet.samples[0].kind, 'forbidden_raw_read');
    assert.equal(packet.samples[0].source_label, 'Task two');
    assert.equal(packet.scan_metadata.source_label, 'Task two');
    assert.equal(packet.scan_metadata.confidence.scan_confidence_0_10000, 9000);
    assert.equal(packet.scan_metadata.scan_trust.kept_files, 9);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildPacketFromArtifactInput dedupes exact same-source samples before truncation', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-dedupe-'));
  try {
    const bundlePath = path.join(tempRoot, 'codex-session.json');
    await writeJson(bundlePath, {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'incomplete_propagation',
            scope: 'defect_injection_toolchain',
            summary: 'Short propagation summary',
            evidence: ['scripts/a.ts'],
            likely_fix_sites: ['scripts/a.ts'],
          },
          {
            kind: 'incomplete_propagation',
            scope: 'defect_injection_toolchain',
            summary: 'Longer propagation summary with more repair detail',
            evidence: ['scripts/a.ts', 'scripts/b.ts'],
            fix_hint: 'Update the remaining sibling surfaces.',
            likely_fix_sites: ['scripts/a.ts'],
          },
        ],
      },
    });

    const source = await loadArtifactInput({ bundlePath, codexBatchPath: null, replayBatchPath: null });
    const packet = buildPacketFromArtifactInput(
      { tool: 'check', limit: 10, repoRoot: '/tmp/parallel-code' },
      source,
    );

    assert.equal(packet.samples.length, 1);
    assert.equal(packet.samples[0].kind, 'incomplete_propagation');
    assert.equal(
      packet.samples[0].summary,
      'Longer propagation summary with more repair detail',
    );
    assert.deepEqual(packet.samples[0].evidence, ['scripts/a.ts', 'scripts/b.ts']);
    assert.equal(packet.samples[0].repair_packet.fix_hint, 'Update the remaining sibling surfaces.');
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('parseArgs accepts combined artifact inputs and kind filters', function () {
  const args = parseArgs([
    'node',
    'script',
    '--bundle',
    'a.json',
    '--codex-batch',
    'b.json',
    '--replay-batch',
    'c.json',
    '--kind',
    'forbidden_raw_read',
  ]);

  assert.equal(args.bundlePath, 'a.json');
  assert.equal(args.codexBatchPath, 'b.json');
  assert.equal(args.replayBatchPath, 'c.json');
  assert.deepEqual(args.kinds, ['forbidden_raw_read']);
});
