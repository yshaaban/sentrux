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

function buildRepoHeadCheckPacket(actions) {
  return buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 1,
      repoRoot: '/tmp/sentrux',
      kinds: [],
    },
    { actions },
  );
}

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
    assert.match(formatPacketMarkdown(packet), /source mode: `bundle`/);
    assert.match(formatPacketMarkdown(packet), /scan trust \/ coverage:/);
    assert.match(formatPacketMarkdown(packet), /files=src\/a\.ts, src\/b\.ts/);
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

test('buildPacketFromArtifactInput uses scan metadata from the source that contributed retained samples', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-review-packet-metadata-source-'));
  try {
    const codexBatchPath = path.join(tempRoot, 'codex-session-batch.json');
    const replayBatchPath = path.join(tempRoot, 'diff-replay-batch.json');
    const taskDir = path.join(tempRoot, 'task-one');
    const replayDir = path.join(tempRoot, 'replay-one');
    await writeJson(path.join(taskDir, 'codex-session.json'), {
      repo_root: '/tmp/parallel-code',
      confidence: {
        scan_confidence_0_10000: 9100,
      },
      scan_trust: {
        candidate_files: 10,
        kept_files: 8,
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
    await writeJson(path.join(replayDir, 'diff-replay.json'), {
      repo_root: '/tmp/parallel-code',
      confidence: {
        scan_confidence_0_10000: 7400,
      },
      scan_trust: {
        candidate_files: 22,
        kept_files: 14,
      },
      session_end: {
        introduced_findings: [
          {
            kind: 'session_introduced_clone',
            scope: 'src/b.ts',
            severity: 'medium',
            summary: 'New clone',
            evidence: ['e2'],
          },
        ],
      },
    });
    await writeJson(codexBatchPath, {
      repo_root: '/tmp/parallel-code',
      results: [{ task_id: 'task-one', output_dir: taskDir }],
    });
    await writeJson(replayBatchPath, {
      repo_root: '/tmp/parallel-code',
      results: [{ replay_id: 'replay-one', output_dir: replayDir }],
    });

    const source = await loadArtifactInput({
      bundlePath: null,
      codexBatchPath,
      replayBatchPath,
    });
    const packet = buildPacketFromArtifactInput(
      {
        tool: 'session_end',
        limit: 10,
        repoRoot: '/tmp/parallel-code',
        kinds: ['session_introduced_clone'],
      },
      source,
    );

    assert.equal(packet.samples.length, 1);
    assert.equal(packet.samples[0].source_kind, 'replay-batch');
    assert.equal(packet.scan_metadata.source_kind, 'replay-batch');
    assert.equal(packet.scan_metadata.source_label, 'replay-one');
    assert.equal(packet.scan_metadata.scan_trust.candidate_files, 22);
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
      confidence: {
        rule_coverage_0_10000: 10000,
        scan_confidence_0_10000: 8282,
        semantic_rules_loaded: true,
      },
      scan_trust: {
        candidate_files: 780,
        tracked_candidates: 776,
        untracked_candidates: 4,
        kept_files: 646,
        mode: 'git',
        overall_confidence_0_10000: 8282,
        partial: false,
        scope_coverage_0_10000: 8282,
        truncated: false,
        fallback_reason: null,
        exclusions: {
          total: 134,
          bucketed: {
            vendor: 120,
            generated: 3,
            build: 2,
            fixture: 5,
            cache: 1,
          },
          ignored_extension: 2,
          too_large: 1,
          metadata_error: 0,
        },
        resolution: {
          resolved: 1968,
          unresolved_internal: 1,
          unresolved_external: 528,
          unresolved_unknown: 85,
          internal_confidence_0_10000: 9933,
        },
      },
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
  assert.equal(packet.scan_metadata.source_kind, 'repo-head');
  assert.equal(packet.scan_metadata.scan_trust.kept_files, 646);
  const markdown = formatPacketMarkdown(packet);

  assert.match(markdown, /kept files: `646 \/ 780`/);
  assert.match(markdown, /tracked candidates: `776`/);
  assert.match(markdown, /scan mode: `git`/);
  assert.match(markdown, /exclusions: `134 total, 120 vendor, 3 generated, 2 build, 5 fixture, 1 cache, 2 ignored_extension, 1 too_large, 0 metadata_error`/);
  assert.match(markdown, /internal resolution confidence: `9933 \/ 10000`/);
});

test('buildPacketFromRepoHeadPayload reuses scan metadata and clone scope when tool payload omits them', function () {
  const packet = buildPacketFromRepoHeadPayload(
    {
      tool: 'findings',
      limit: 1,
      repoRoot: '/tmp/sentrux',
      kinds: [],
    },
    {
      findings: [
        {
          kind: 'exact_clone_group',
          summary: 'Clone group',
          files: ['src/a.ts', 'src/b.ts'],
          instances: [
            { file: 'src/a.ts', lines: 11 },
            { file: 'src/b.ts', lines: 11 },
          ],
        },
      ],
    },
    {
      confidence: {
        scan_confidence_0_10000: 9100,
        rule_coverage_0_10000: 9000,
        semantic_rules_loaded: true,
      },
      scan_trust: {
        candidate_files: 12,
        kept_files: 9,
        tracked_candidates: 9,
        untracked_candidates: 3,
      },
    },
  );

  assert.equal(packet.samples[0].scope, 'src/a.ts | src/b.ts');
  assert.equal(packet.scan_metadata.confidence.scan_confidence_0_10000, 9100);
  assert.equal(packet.scan_metadata.scan_trust.kept_files, 9);
  const markdown = formatPacketMarkdown(packet);

  assert.match(markdown, /Findings Review Packet/);
  assert.match(markdown, /src\/a\.ts \\\| src\/b\.ts/);
});

test('buildPacketFromRepoHeadPayload keeps repair packets incomplete when only a fix hint is present', function () {
  const packet = buildRepoHeadCheckPacket([
    {
      kind: 'cycle_cluster',
      scope: 'cycle:src/a.ts|src/b.ts',
      summary: 'Two files still depend on each other.',
      evidence: ['src/a.ts imports src/b.ts', 'src/b.ts imports src/a.ts'],
      fix_hint: 'Break the cycle before merging.',
    },
  ]);

  assert.equal(packet.samples[0].repair_packet.complete, false);
  assert.deepEqual(packet.samples[0].repair_packet.likely_fix_sites, []);
  assert.equal(packet.samples[0].repair_packet.required_fields.repair_surface, false);
  assert.match(packet.samples[0].repair_packet.missing_fields.join(', '), /repair_surface/);
});

test('buildPacketFromRepoHeadPayload marks repair packets complete when likely fix sites are concrete', function () {
  const packet = buildRepoHeadCheckPacket([
    {
      kind: 'dependency_sprawl',
      scope: 'src/app.ts',
      summary: 'Entry surface fans out across too many modules.',
      evidence: ['imports 18 modules directly'],
      likely_fix_sites: ['src/app.ts', 'src/app-shell.ts'],
      fix_hint: 'Pull wiring into a smaller composition root.',
    },
  ]);

  assert.equal(packet.samples[0].repair_packet.complete, true);
  assert.equal(packet.samples[0].repair_packet.required_fields.repair_surface, true);
  assert.deepEqual(packet.samples[0].repair_packet.likely_fix_sites, [
    'src/app.ts',
    'src/app-shell.ts',
  ]);
});
