import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import test from 'node:test';

import {
  buildCompletionGates,
  formatCompletionGatesMarkdown,
} from '../lib/completion-gates.mjs';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function demoRubric() {
  return {
    schema_version: 1,
    rubric_id: 'demo-completion',
    rubric_version: 'test',
    phases: [
      {
        phase_id: 'phase_5_treatment',
        phase_label: 'Treatment evidence',
        gates: [
          {
            gate_id: 'help_rate',
            source: 'evidence_review',
            path: 'product_value.top_action_help_rate',
            operator: 'gte',
            threshold: 0.5,
          },
          {
            gate_id: 'default_on_complete',
            source: 'evidence_review',
            path: 'default_on_promotion.evidence_complete',
            operator: 'is_true',
          },
          {
            gate_id: 'optional_review_queue',
            source: 'evidence_review',
            path: 'summary.review_queue_count',
            operator: 'lte',
            threshold: 3,
            required: false,
          },
        ],
      },
      {
        phase_id: 'phase_7_release',
        phase_label: 'Release gate',
        gates: [
          {
            gate_id: 'phase_7_decision_record',
            source: 'decision_records',
            path: 'phase_counts.phase_7',
            operator: 'gte',
            threshold: 1,
          },
          {
            gate_id: 'public_preflight',
            source: 'evidence_review',
            path: 'release_summary.public_preflight_passed',
            operator: 'is_true',
          },
        ],
      },
    ],
    signal_groups: [
      {
        group_id: 'agent_default',
        selector: {
          where: [
            {
              gate_id: 'lane',
              path: 'product_primary_lane',
              operator: 'equals',
              value: 'agent_default',
            },
          ],
        },
        gates: [
          {
            gate_id: 'promotion_complete',
            path: 'promotion_evidence_complete',
            operator: 'is_true',
          },
          {
            gate_id: 'signal_help_rate',
            path: 'top_action_help_rate',
            operator: 'gte',
            threshold: 0.5,
          },
          {
            gate_id: 'bounded_patch_expansion',
            path: 'patch_expansion_rate',
            operator: 'lte',
            threshold: 0.25,
          },
        ],
      },
    ],
  };
}

test('buildCompletionGates evaluates phase and signal pass/fail gates deterministically', function () {
  const result = buildCompletionGates({
    rubric: demoRubric(),
    generatedAt: '2026-04-24T00:00:00.000Z',
    scorecard: {
      repo_label: 'demo-repo',
      signals: [
        {
          signal_kind: 'incomplete_propagation',
          signal_family: 'obligation',
          product_primary_lane: 'agent_default',
          promotion_status: 'trusted',
          promotion_evidence_complete: true,
          top_action_help_rate: 0.75,
          patch_expansion_rate: 0,
        },
        {
          signal_kind: 'large_file',
          signal_family: 'structure',
          product_primary_lane: 'agent_default',
          promotion_status: 'watchpoint',
          promotion_evidence_complete: false,
          top_action_help_rate: 0.25,
          patch_expansion_rate: 0.5,
        },
        {
          signal_kind: 'cycle_cluster',
          product_primary_lane: 'maintainer_watchpoint',
          promotion_evidence_complete: false,
        },
      ],
    },
    evidenceReview: {
      product_value: {
        top_action_help_rate: 0.667,
      },
      default_on_promotion: {
        evidence_complete: false,
      },
      summary: {
        review_queue_count: 4,
      },
      release_summary: {
        public_preflight_passed: true,
      },
    },
    decisionRecords: {
      phase_counts: {
        phase_7: 1,
      },
    },
  });

  assert.equal(result.generated_at, '2026-04-24T00:00:00.000Z');
  assert.equal(result.repo_label, 'demo-repo');
  assert.equal(result.summary.status, 'fail');
  assert.equal(result.summary.phase_count, 2);
  assert.equal(result.summary.phase_pass_count, 1);
  assert.equal(result.summary.phase_fail_count, 1);
  assert.equal(result.summary.signal_count, 2);
  assert.equal(result.summary.signal_pass_count, 1);
  assert.equal(result.summary.signal_fail_count, 1);
  assert.equal(result.summary.required_failure_count, 4);
  assert.equal(result.summary.optional_failure_count, 1);
  assert.equal(result.phases[0].phase_id, 'phase_5_treatment');
  assert.equal(result.phases[0].status, 'fail');
  assert.equal(result.phases[0].required_failure_count, 1);
  assert.equal(result.phases[0].optional_failure_count, 1);
  assert.equal(result.phases[1].status, 'pass');
  assert.deepEqual(
    result.signals.map((signal) => [signal.signal_kind, signal.status]),
    [
      ['incomplete_propagation', 'pass'],
      ['large_file', 'fail'],
    ],
  );
  assert.equal(result.signals[1].required_failure_count, 3);
  assert.match(formatCompletionGatesMarkdown(result), /Completion Gates/);
  assert.match(formatCompletionGatesMarkdown(result), /default_on_complete/);
  assert.match(formatCompletionGatesMarkdown(result), /large_file/);
});

test('buildCompletionGates treats missing required evidence as a failed gate', function () {
  const result = buildCompletionGates({
    rubric: {
      schema_version: 1,
      rubric_id: 'missing-source',
      phases: [
        {
          phase_id: 'phase_missing',
          phase_label: 'Missing source',
          gates: [
            {
              gate_id: 'missing_release',
              source: 'evidence_review',
              path: 'release_summary.public_hygiene_passed',
              operator: 'is_true',
            },
          ],
        },
      ],
    },
    scorecard: {
      repo_label: 'demo-repo',
    },
    generatedAt: '2026-04-24T00:00:00.000Z',
  });

  assert.equal(result.summary.status, 'fail');
  assert.equal(result.phases[0].gates[0].actual, null);
  assert.match(result.phases[0].gates[0].message, /got missing/);
});

test('buildCompletionGates treats missing decision records as failed completion gates', function () {
  const result = buildCompletionGates({
    rubric: {
      schema_version: 1,
      rubric_id: 'decision-record-required',
      phases: [
        {
          phase_id: 'phase_2_semantic_obligation_graph',
          phase_label: 'Semantic obligations',
          gates: [
            {
              gate_id: 'phase_2_decision_record',
              source: 'decision_records',
              path: 'phase_counts.phase_2',
              operator: 'gte',
              threshold: 1,
            },
          ],
        },
      ],
    },
    scorecard: {
      repo_label: 'demo-repo',
    },
    generatedAt: '2026-04-24T00:00:00.000Z',
  });

  assert.equal(result.summary.status, 'fail');
  assert.equal(result.phases[0].gates[0].actual, null);
  assert.match(result.phases[0].gates[0].message, /got missing/);
});

test('build-completion-gates CLI reads inputs and writes JSON and Markdown', async function () {
  const tempDir = await mkdtemp(path.join(tmpdir(), 'completion-gates-'));
  try {
    const rubricPath = path.join(tempDir, 'rubric.json');
    const scorecardPath = path.join(tempDir, 'scorecard.json');
    const evidenceReviewPath = path.join(tempDir, 'evidence-review.json');
    const decisionRecordsDir = path.join(tempDir, 'decisions');
    const outputJsonPath = path.join(tempDir, 'completion-gates.json');
    const outputMarkdownPath = path.join(tempDir, 'completion-gates.md');

    await writeFile(rubricPath, `${JSON.stringify(demoRubric(), null, 2)}\n`);
    await writeFile(
      scorecardPath,
      `${JSON.stringify(
        {
          repo_label: 'cli-repo',
          signals: [
            {
              signal_kind: 'incomplete_propagation',
              product_primary_lane: 'agent_default',
              promotion_evidence_complete: true,
              top_action_help_rate: 1,
              patch_expansion_rate: 0,
            },
          ],
        },
        null,
        2,
      )}\n`,
    );
    await writeFile(
      evidenceReviewPath,
      `${JSON.stringify(
        {
          product_value: { top_action_help_rate: 1 },
          default_on_promotion: { evidence_complete: true },
          summary: { review_queue_count: 1 },
          release_summary: { public_preflight_passed: true },
        },
        null,
        2,
      )}\n`,
    );
    await mkdir(decisionRecordsDir, { recursive: true });
    await writeFile(
      path.join(decisionRecordsDir, '2026-04-24-phase-7-release-gate-keep.md'),
      '# Release Gate Keep\n',
      { flag: 'w' },
    );

    const { stdout } = await execFileAsync(
      process.execPath,
      [
        'scripts/evals/build-completion-gates.mjs',
        '--rubric',
        rubricPath,
        '--scorecard',
        scorecardPath,
        '--evidence-review',
        evidenceReviewPath,
        '--decision-records-dir',
        decisionRecordsDir,
        '--generated-at',
        '2026-04-24T00:00:00.000Z',
        '--output-json',
        outputJsonPath,
        '--output-md',
        outputMarkdownPath,
      ],
      { cwd: repoRoot },
    );
    const output = JSON.parse(await readFile(outputJsonPath, 'utf8'));
    const markdown = await readFile(outputMarkdownPath, 'utf8');

    assert.match(stdout, /Built completion gates for cli-repo: pass/);
    assert.equal(output.summary.status, 'pass');
    assert.equal(output.repo_label, 'cli-repo');
    assert.match(markdown, /phase_5_treatment/);
  } finally {
    await rm(tempDir, { force: true, recursive: true });
  }
});
