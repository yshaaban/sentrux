import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildPacketFromRepoHeadPayload,
  formatPacketMarkdown,
} from '../../evals/build-check-review-packet.mjs';
import { buildVerdictTemplate } from '../../lib/check-review-packet-model.mjs';
import { buildDefaultRepoHeadCheckPacket } from './helpers.mjs';

test('buildPacketFromRepoHeadPayload applies kind filters before truncation', function () {
  const packet = buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 1,
      repoRoot: '/tmp/parallel-code',
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

test('buildPacketFromRepoHeadPayload reranks samples before truncation', function () {
  const packet = buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 2,
      repoRoot: '/tmp/parallel-code',
      kinds: [],
    },
    {
      actions: [
        {
          kind: 'large_file',
          scope: 'src/a.ts',
          severity: 'high',
          summary: 'Large file',
        },
        {
          kind: 'missing_test_coverage',
          scope: 'src/b.ts',
          severity: 'high',
          summary: 'Missing tests',
        },
        {
          kind: 'forbidden_raw_read',
          scope: 'src/c.ts',
          severity: 'high',
          summary: 'Raw read',
        },
      ],
    },
  );

  assert.deepEqual(
    packet.samples.map((sample) => sample.kind),
    ['forbidden_raw_read', 'missing_test_coverage'],
  );
  assert.deepEqual(
    packet.samples.map((sample) => sample.rank),
    [1, 2],
  );
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
  const packet = buildDefaultRepoHeadCheckPacket([
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
  const packet = buildDefaultRepoHeadCheckPacket([
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

test('buildVerdictTemplate carries structured packet fields into the template verdicts', function () {
  const packet = buildDefaultRepoHeadCheckPacket([
    {
      kind: 'dependency_sprawl',
      scope: 'src/app.ts',
      summary: 'Entry surface fans out across too many modules.',
      evidence: ['imports 18 modules directly'],
      likely_fix_sites: ['src/app.ts', 'src/app-shell.ts'],
      fix_hint: 'Pull wiring into a smaller composition root.',
    },
  ]);

  const template = buildVerdictTemplate(packet, '/tmp/check-review-packet.md');

  assert.equal(template.verdicts[0].source_kind, 'repo-head');
  assert.equal(template.verdicts[0].source_label, 'repo-head');
  assert.equal(template.verdicts[0].snapshot_label, 'repo_head');
  assert.equal(template.verdicts[0].rank_observed, 1);
  assert.equal(template.verdicts[0].rank_preserved, true);
  assert.equal(template.verdicts[0].repair_packet_complete, true);
  assert.deepEqual(template.verdicts[0].repair_packet_missing_fields, []);
  assert.equal(template.verdicts[0].repair_packet_fix_surface_clear, true);
  assert.equal(template.verdicts[0].repair_packet_verification_clear, true);
});

test('buildVerdictTemplate demotes zero-weight large_file out of headline surfaces when higher-priority samples exist', function () {
  const packet = buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 2,
      repoRoot: '/tmp/parallel-code',
      kinds: [],
    },
    {
      actions: [
        {
          kind: 'large_file',
          scope: 'src/a.ts',
          severity: 'high',
          summary: 'Large file',
          evidence: ['src/a.ts'],
        },
        {
          kind: 'forbidden_raw_read',
          scope: 'src/b.ts',
          severity: 'high',
          summary: 'Raw read',
        },
      ],
    },
  );

  const template = buildVerdictTemplate(packet, '/tmp/check-review-packet.md');

  assert.equal(template.verdicts[0].kind, 'forbidden_raw_read');
  assert.equal(template.verdicts[0].expected_summary_presence, 'headline');
  assert.equal(template.verdicts[1].kind, 'large_file');
  assert.equal(template.verdicts[1].expected_summary_presence, 'side_channel');
  assert.match(template.verdicts[1].expected_v2_behavior, /side channel/);
  assert.doesNotMatch(template.verdicts[1].expected_v2_behavior, /lead evidence/);
});
