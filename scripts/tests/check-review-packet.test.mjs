import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildPacketFromArtifactInput,
  buildPacketFromRepoHeadPayload,
  formatPacketMarkdown,
  loadArtifactInput,
  parseArgs,
} from '../evals/build-check-review-packet.mjs';

async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

test('loadArtifactInput reads a single bundle artifact', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-bundle-'));
  try {
    const bundlePath = path.join(tempRoot, 'codex-session.json');
    await writeJson(bundlePath, {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'large_file',
            scope: 'src/a.ts',
            severity: 'high',
            summary: 'Large file',
            evidence: ['e1'],
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
    assert.equal(packet.samples.length, 1);
    assert.equal(packet.samples[0].kind, 'large_file');
    assert.equal(packet.summary.sample_count, 1);
    assert.match(formatPacketMarkdown(packet), /source mode: `bundle`/);
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
      ['forbidden_raw_read', 'closed_domain_exhaustiveness'],
    );
    assert.equal(packet.samples[0].snapshot_label, 'final_check');
    assert.equal(packet.samples[0].source_label, 'Task one');
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

test('loadArtifactInput merges codex and replay batch sources', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-combined-'));
  try {
    const codexBatchPath = path.join(tempRoot, 'codex-session-batch.json');
    const replayBatchPath = path.join(tempRoot, 'diff-replay-batch.json');
    const taskDir = path.join(tempRoot, 'task-one');
    const replayDir = path.join(tempRoot, 'replay-one');
    await writeJson(path.join(taskDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
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
    await writeJson(path.join(replayDir, 'diff-replay.json'), {
      repo_root: '/tmp/parallel-code',
      session_end: {
        introduced_findings: [
          {
            kind: 'session_introduced_clone',
            scope: 'src/b.ts',
            severity: 'medium',
            summary: 'Clone followthrough',
            evidence: ['e2'],
          },
        ],
      },
    });
    await writeJson(codexBatchPath, {
      repo_root: '/tmp/parallel-code',
      results: [{ task_id: 'task-one', task_label: 'Task one', output_dir: taskDir }],
    });
    await writeJson(replayBatchPath, {
      repo_root: '/tmp/parallel-code',
      results: [{ replay_id: 'replay-one', commit: 'abc123', output_dir: replayDir }],
    });

    const source = await loadArtifactInput({
      bundlePath: null,
      codexBatchPath,
      replayBatchPath,
    });

    assert.equal(source.source_mode, 'combined');
    assert.equal(source.entries.length, 2);
    assert.equal(source.source_paths.length, 4);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildPacketFromArtifactInput applies kind filters before truncation', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-filter-order-'));
  try {
    const bundlePath = path.join(tempRoot, 'codex-session.json');
    await writeJson(bundlePath, {
      repo_root: '/tmp/parallel-code',
      initial_check: {
        actions: [
          {
            kind: 'large_file',
            scope: 'src/a.ts',
            summary: 'Large file 1',
          },
          {
            kind: 'missing_test_coverage',
            scope: 'src/b.ts',
            summary: 'Missing tests',
          },
          {
            kind: 'forbidden_raw_read',
            scope: 'src/c.ts',
            summary: 'Raw read',
          },
        ],
      },
    });

    const source = await loadArtifactInput({
      bundlePath,
      codexBatchPath: null,
      replayBatchPath: null,
    });
    const packet = buildPacketFromArtifactInput(
      {
        tool: 'check',
        limit: 1,
        repoRoot: '/tmp/parallel-code',
        kinds: ['forbidden_raw_read'],
      },
      source,
    );

    assert.equal(packet.summary.sample_count, 1);
    assert.equal(packet.samples[0].kind, 'forbidden_raw_read');
    assert.equal(packet.samples[0].rank, 1);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildPacketFromRepoHeadPayload applies kind filters before truncation', function () {
  const packet = buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 1,
      repoRoot: '/tmp/sentrux',
      kinds: ['forbidden_raw_read'],
    },
    {
      actions: [
        {
          kind: 'large_file',
          scope: 'src/a.ts',
          summary: 'Large file 1',
        },
        {
          kind: 'missing_test_coverage',
          scope: 'src/b.ts',
          summary: 'Missing tests',
        },
        {
          kind: 'forbidden_raw_read',
          scope: 'src/c.ts',
          summary: 'Raw read',
        },
      ],
    },
  );

  assert.equal(packet.summary.sample_count, 1);
  assert.equal(packet.samples[0].kind, 'forbidden_raw_read');
  assert.equal(packet.samples[0].rank, 1);
});
