import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildBoundedAdjudicationArtifact,
  BOUNDED_ADJUDICATION_OUTPUT_SCHEMA,
} from '../lib/eval-runtime/provider-task-runner/adjudication.mjs';
import { evaluateTask } from '../lib/eval-runtime/provider-task-runner/evaluation.mjs';
import {
  buildOutputSchema,
  buildTaskPrompt,
  defaultChecksForTask,
} from '../lib/eval-runtime/provider-task-runner/task-schemas.mjs';
import {
  buildRunIndex,
  buildTaskResultSummary,
} from '../lib/eval-runtime/provider-task-runner/results.mjs';

function createScenario() {
  return {
    scenario_id: 'bounded-adjudication-demo',
    repo: {
      name: 'demo-repo',
      root_env: 'SENTRUX_TEST_REPO_ROOT',
      default_root: '.',
    },
    tasks: [],
  };
}

function createTask() {
  return {
    task_id: 'adjudicate-incomplete-propagation',
    kind: 'bounded_adjudication',
    prompt: 'Decide whether incomplete_propagation should remain the lead intervention.',
    evidence_bundle: {
      bundle_id: 'bundle-123',
      repo_name: 'demo-repo',
      adjudication_target: {
        finding_kind: 'incomplete_propagation',
        summary: 'The lead finding may be too aggressive for the current patch.',
        current_rank: 1,
        current_lane: 'agent_default',
        severity: 'medium',
        confidence_0_1: 0.62,
        expected_fix_surface: 'repair_packet',
      },
      diff_slice: {
        summary: 'Patch changes a DTO and one validation branch.',
        files: ['src/user.ts', 'src/user.test.ts'],
        changed_symbols: ['UserDto', 'validateUser'],
        hunks: [
          {
            id: 'h1',
            path: 'src/user.ts',
            header: '@@ -14,6 +14,10 @@',
            summary: 'Added a new DTO field without followthrough.',
          },
        ],
      },
      evidence_items: [
        {
          id: 'e-prop-miss',
          kind: 'finding',
          summary: 'The finding identifies a changed DTO field without downstream updates.',
          source: 'check-review-packet',
          path: 'src/user.ts',
          line: 14,
        },
        {
          id: 'e-test-gap',
          kind: 'verification_gap',
          summary: 'Tests still cover only the previous DTO shape.',
          source: 'session-corpus',
          path: 'src/user.test.ts',
          line: 22,
        },
      ],
      dependent_surfaces: [
        {
          id: 'd-validator',
          kind: 'validator',
          path: 'src/validate-user.ts',
          rationale: 'Validation logic consumes the DTO.',
        },
      ],
      candidate_fix_sites: [
        {
          id: 'f-validator',
          path: 'src/validate-user.ts',
          symbol: 'validateUser',
          rationale: 'Primary propagation site for the DTO change.',
        },
      ],
      verification_surfaces: [
        {
          id: 'v-user-test',
          kind: 'test',
          path: 'src/user.test.ts',
          command: 'pnpm test src/user.test.ts',
          rationale: 'Confirms the DTO followthrough.',
        },
      ],
      source_artifacts: ['signal-scorecard.json', 'session-corpus.json'],
      phase_tracking: {
        phase_id: 'phase_3_bounded_llm_adjudication',
        status: 'scaffold_only',
        milestone: 'structured_bundle_contract',
      },
    },
  };
}

function createProviderStatus() {
  return {
    exit_code: 0,
    timed_out: false,
    stdout_json: { ok: true },
  };
}

function createValidResponse() {
  return {
    task_kind: 'bounded_adjudication',
    bundle_id: 'bundle-123',
    repo_name: 'demo-repo',
    decision: {
      verdict: 'rerank_lower',
      ranking_action: 'lower_rank',
      summary: 'The evidence is real but not strong enough for the lead slot.',
      rationale: 'The candidate is plausible, but the proof remains patch-local and incomplete.',
    },
    cited_evidence_ids: ['e-prop-miss', 'e-test-gap'],
    cited_fix_site_ids: ['f-validator'],
    cited_verification_surface_ids: ['v-user-test'],
    confidence_0_1: 0.54,
    evidence_gaps: ['No direct proof that the validator is the only missing surface.'],
    audit: {
      structured_evidence_only: true,
      requires_human_review: true,
      auto_apply_eligible: false,
    },
  };
}

test('bounded adjudication task schemas build a structured-evidence-only prompt and schema', function () {
  const scenario = createScenario();
  const task = createTask();
  const prompt = buildTaskPrompt(scenario, '/tmp/sentrux/scenario.json', task);

  assert.equal(buildOutputSchema(task), BOUNDED_ADJUDICATION_OUTPUT_SCHEMA);
  assert.match(prompt, /MiniMax M2\.7/);
  assert.match(prompt, /Structured evidence bundle:/);
  assert.match(prompt, /allow_repo_scan/);
  assert.match(prompt, /bundle-123/);
  assert.match(prompt, /rerank_lower/);

  const checks = defaultChecksForTask(task, scenario);
  assert.equal(checks.find((check) => check.path === 'bundle_id')?.value, 'bundle-123');
  assert.deepEqual(
    checks.find(
      (check) => check.kind === 'all_items_in_set' && check.path === 'cited_evidence_ids',
    )?.allowed,
    ['e-prop-miss', 'e-test-gap'],
  );
});

test('bounded adjudication evaluation passes only when cited references stay inside the bundle', function () {
  const task = createTask();
  const validEvaluation = evaluateTask(task, createValidResponse(), createProviderStatus());

  assert.equal(validEvaluation.status, 'pass');
  assert.equal(validEvaluation.provider_failed, false);

  const invalidResponse = {
    ...createValidResponse(),
    cited_evidence_ids: ['e-prop-miss', 'not-in-bundle'],
  };
  const invalidEvaluation = evaluateTask(task, invalidResponse, createProviderStatus());

  assert.equal(invalidEvaluation.status, 'fail');
  assert.match(
    invalidEvaluation.check_results.find(
      (check) => check.kind === 'all_items_in_set' && check.path === 'cited_evidence_ids',
    ).message,
    /outside allowed set/,
  );
});

test('bounded adjudication artifacts capture audit metadata and run-level summaries', function () {
  const task = createTask();
  const scenario = createScenario();
  const responseJson = createValidResponse();
  const adjudication = buildBoundedAdjudicationArtifact(task, responseJson, scenario);

  assert.equal(adjudication.model_profile.model, 'MiniMax M2.7');
  assert.equal(adjudication.evidence_bundle.bundle_id, 'bundle-123');
  assert.equal(adjudication.evidence_bundle.policy.structured_evidence_only, true);
  assert.equal(adjudication.reference_audit.evidence.all_cited_ids_valid, true);
  assert.equal(adjudication.conservative_guardrails.auto_apply_eligible, false);
  assert.equal(adjudication.phase_tracking?.status, 'scaffold_only');

  const taskSummary = buildTaskResultSummary(
    {
      scenario,
      task,
    },
    '/tmp/out/result.json',
    {
      evaluation: {
        status: 'pass',
        score_0_100: 100,
      },
      adjudication,
    },
  );
  const index = buildRunIndex({
    runId: 'eval-demo',
    options: {
      provider: 'claude-code',
      model: 'MiniMax M2.7',
      outputDir: '/tmp/out',
    },
    scenarios: [
      {
        scenario,
        scenarioPath: '/tmp/sentrux/scenario.json',
      },
    ],
    taskResults: [taskSummary],
    startedAt: '2026-04-18T00:00:00.000Z',
    durationMs: 10,
    buildRunScenarioEntry({ scenario: scenarioEntry, scenarioPath }) {
      return {
        scenario_id: scenarioEntry.scenario_id,
        source_path: scenarioPath,
        repo: scenarioEntry.repo,
        task_count: 1,
      };
    },
  });

  assert.equal(index.bounded_adjudication?.task_count, 1);
  assert.equal(index.bounded_adjudication?.decision_counts.rerank_lower, 1);
  assert.equal(index.bounded_adjudication?.auto_apply_disabled_count, 1);
  assert.equal(index.bounded_adjudication?.human_review_required_count, 1);
  assert.equal(index.dry_run, false);
});

test('buildRunIndex preserves dry-run mode in the run index', function () {
  const index = buildRunIndex({
    runId: 'eval-dry-run',
    options: {
      provider: 'claude-code',
      model: null,
      outputDir: '/tmp/out',
      dryRun: true,
    },
    scenarios: [],
    taskResults: [],
    startedAt: '2026-04-18T00:00:00.000Z',
    durationMs: 0,
    buildRunScenarioEntry() {
      return {};
    },
  });

  assert.equal(index.dry_run, true);
});
