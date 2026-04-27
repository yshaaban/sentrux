import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildAdvisorEvidence,
  buildAdvisorPreflight,
  buildAdvisorSummaryMarkdown,
  buildBeforeAfterComparison,
  buildRulesBootstrap,
  canApplyGeneratedRulesToWorkspace,
  createRepoAdvisorWorkspace,
  formatAdvisorPreflightMarkdown,
  formatBeforeAfterComparisonMarkdown,
  formatRulesBootstrapMarkdown,
  parseRepoAdvisorArgs,
  validateRepoAdvisorSetup,
} from '../lib/repo-advisor.mjs';

test('parseRepoAdvisorArgs defaults to safe working-tree analysis outside target repo', function () {
  const args = parseRepoAdvisorArgs(['node', 'script', '--repo-root', '/tmp/crew-mail']);

  assert.equal(args.repoRoot, '/tmp/crew-mail');
  assert.equal(args.repoLabel, 'crew-mail');
  assert.equal(args.analysisMode, 'working-tree');
  assert.equal(args.applySuggestedRules, true);
  assert.match(args.outputDir, /repo-advisor/);
});

test('parseRepoAdvisorArgs accepts a positional repo root for low-friction CLI use', function () {
  const args = parseRepoAdvisorArgs(['node', 'script', '/tmp/mail-simulator']);

  assert.equal(args.repoRoot, '/tmp/mail-simulator');
  assert.equal(args.repoLabel, 'mail-simulator');
  assert.equal(args.analysisMode, 'working-tree');
});

test('parseRepoAdvisorArgs rejects missing and invalid numeric option values', function () {
  assert.throws(
    function parseMissingRepoRoot() {
      parseRepoAdvisorArgs(['node', 'script', '--repo-root']);
    },
    /Missing value for --repo-root/,
  );
  assert.throws(
    function parseInvalidFindingsLimit() {
      parseRepoAdvisorArgs([
        'node',
        'script',
        '--repo-root',
        '/tmp/crew-mail',
        '--findings-limit',
        'many',
      ]);
    },
    /--findings-limit must be a positive integer/,
  );
});

test('createRepoAdvisorWorkspace rejects live analysis with external rules source', async function () {
  await assert.rejects(
    createRepoAdvisorWorkspace({
      repoRoot: '/tmp/crew-mail',
      repoLabel: 'crew-mail',
      analysisMode: 'live',
      rulesSource: '/tmp/custom.rules.toml',
    }),
    /live cannot safely use --rules-source/,
  );
});

test('canApplyGeneratedRulesToWorkspace limits writes to isolated workspaces', function () {
  assert.equal(
    canApplyGeneratedRulesToWorkspace(true, {
      safety: { isolated_workspace: true },
    }),
    true,
  );
  assert.equal(
    canApplyGeneratedRulesToWorkspace(true, {
      safety: { isolated_workspace: false },
    }),
    false,
  );
  assert.equal(
    canApplyGeneratedRulesToWorkspace(false, {
      safety: { isolated_workspace: true },
    }),
    false,
  );
});

test('buildAdvisorPreflight records setup safety and report artifact paths', async function () {
  const args = parseRepoAdvisorArgs([
    'node',
    'script',
    '--repo-root',
    process.cwd(),
    '--output-dir',
    process.cwd(),
  ]);
  const workspace = {
    workRoot: '/tmp/sentrux-workspace',
    analysisMode: 'working_tree',
    safety: {
      mutates_target_repo: false,
      isolated_workspace: true,
      note: 'analyzed a local clone with working-tree changes overlaid',
    },
  };
  const paths = {
    advisorSummaryPath: '/tmp/report/ADVISOR_SUMMARY.md',
    engineeringReportPath: '/tmp/report/ENGINEERING_REPORT.md',
    reportPath: '/tmp/report/REPORT.md',
    rawToolAnalysisPath: '/tmp/report/raw-tool-analysis.json',
  };

  const preflight = await buildAdvisorPreflight(args, workspace, paths);
  const markdown = formatAdvisorPreflightMarkdown(preflight);

  assert.equal(preflight.passed, true);
  assert.equal(preflight.safety.mutates_target_repo, false);
  assert.equal(preflight.artifact_paths.engineering_report, '/tmp/report/ENGINEERING_REPORT.md');
  assert.match(markdown, /target mutation: `none by default`/);
  assert.match(markdown, /analysis_workspace/);
});

test('buildAdvisorPreflight rejects file paths as output directories', async function () {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-advisor-preflight-'));
  try {
    const outputFile = path.join(tempRoot, 'report-output');
    await writeFile(outputFile, 'not a directory', 'utf8');
    const args = parseRepoAdvisorArgs([
      'node',
      'script',
      '--repo-root',
      process.cwd(),
      '--output-dir',
      outputFile,
    ]);
    const workspace = {
      workRoot: '/tmp/sentrux-workspace',
      analysisMode: 'working_tree',
      safety: {
        mutates_target_repo: false,
        isolated_workspace: true,
        note: 'analyzed a local clone with working-tree changes overlaid',
      },
    };

    const preflight = await buildAdvisorPreflight(args, workspace, {
      advisorSummaryPath: path.join(tempRoot, 'ADVISOR_SUMMARY.md'),
      engineeringReportPath: path.join(tempRoot, 'ENGINEERING_REPORT.md'),
      reportPath: path.join(tempRoot, 'REPORT.md'),
      rawToolAnalysisPath: path.join(tempRoot, 'raw-tool-analysis.json'),
    });

    assert.equal(preflight.passed, false);
    assert.deepEqual(
      preflight.checks
        .filter((check) => check.id === 'output_dir')
        .map((check) => [check.status, check.severity]),
      [['fail', 'error']],
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('validateRepoAdvisorSetup rejects unusable external inputs before expensive analysis', async function () {
  await assert.rejects(
    validateRepoAdvisorSetup(
      parseRepoAdvisorArgs([
        'node',
        'script',
        '--repo-root',
        process.cwd(),
        '--rules-source',
        process.cwd(),
      ]),
    ),
    /--rules-source must point to a file, got directory/,
  );

  await assert.rejects(
    validateRepoAdvisorSetup(
      parseRepoAdvisorArgs([
        'node',
        'script',
        '--repo-root',
        process.cwd(),
        '--previous-analysis',
        `/tmp/sentrux-definitely-missing-previous-analysis-${process.pid}.json`,
      ]),
    ),
    /--previous-analysis must point to a file, got missing/,
  );
});

test('buildRulesBootstrap turns onboarding shape into reviewed rule candidates', function () {
  const bootstrap = buildRulesBootstrap({
    briefs: {
      repo_onboarding: {
        inferred_rules: {
          concepts: 2,
          module_contracts: 1,
          state_models: 0,
        },
        repo_shape: {
          working_rules_toml: '[project]\nprimary_language = "typescript"\n',
          boundary_roots: [
            {
              kind: 'client_state',
              root: 'src/store',
              evidence: ['top-level client state layer detected'],
            },
          ],
          module_contracts: [
            {
              id: 'feature_modules',
              root: 'src/features',
              confidence: 'high',
              evidence: ['feature module barrels detected'],
            },
          ],
        },
      },
    },
  }, { configuredRulesPath: null });
  const markdown = formatRulesBootstrapMarkdown(bootstrap);

  assert.equal(bootstrap.should_write_to_target, false);
  assert.equal(bootstrap.risk_summary.high_confidence, 3);
  assert.equal(bootstrap.risk_summary.needs_review, 1);
  assert.match(markdown, /feature_modules/);
  assert.match(markdown, /src\/store/);
  assert.match(markdown, /```toml/);
});

test('buildBeforeAfterComparison reports resolved and new primary actions', function () {
  const previous = {
    gate: {
      decision: 'fail',
      missing_obligations: [{ concept_id: 'policy_execution' }],
    },
    briefs: {
      pre_merge: {
        primary_targets: [
          { kind: 'incomplete_propagation', scope: 'policy_execution', summary: 'fix policy' },
          { kind: 'large_file', scope: 'src/App.tsx', summary: 'split app' },
        ],
      },
    },
    findings: {
      findings: [{ kind: 'large_file' }],
    },
  };
  const current = {
    gate: {
      decision: 'pass',
      missing_obligations: [],
    },
    briefs: {
      pre_merge: {
        primary_targets: [
          { kind: 'closed_domain_exhaustiveness', scope: 'ProviderTraceStatus', summary: 'fix status' },
        ],
      },
    },
    findings: {
      findings: [{ kind: 'closed_domain_exhaustiveness' }],
    },
  };
  const comparison = buildBeforeAfterComparison(previous, current);
  const markdown = formatBeforeAfterComparisonMarkdown(comparison);

  assert.equal(comparison.gate_decision_before, 'fail');
  assert.equal(comparison.gate_decision_after, 'pass');
  assert.equal(comparison.resolved_primary_actions.length, 2);
  assert.equal(comparison.new_primary_actions.length, 1);
  assert.match(markdown, /missing obligations: `1 -> 0`/);
});

test('buildAdvisorEvidence records default-lane control metrics and large_file slots', function () {
  const evidence = buildAdvisorEvidence({
    repoLabel: 'demo',
    sourceRepoRoot: '/tmp/source',
    analyzedRepoRoot: '/tmp/work',
    workspace: {
      analysisMode: 'working_tree',
      safety: { mutates_target_repo: false, isolated_workspace: true },
    },
    rawToolAnalysis: {
      briefs: {
        pre_merge: {
          primary_targets: [
            {
              kind: 'large_file',
              scope: 'src/App.tsx',
              repair_packet: { complete: true, completeness_0_10000: 10000 },
            },
          ],
        },
      },
      gate: {
        missing_obligations: [],
      },
    },
    rulesBootstrap: {
      candidate_count: 1,
      risk_summary: { high_confidence: 1, needs_review: 0, risky: 0 },
    },
  });

  assert.equal(evidence.default_lane.control_arm, 'current_policy');
  assert.equal(evidence.default_lane.primary_action_count, 1);
  assert.equal(evidence.default_lane.large_file_primary_slot_count, 1);
  assert.deepEqual(evidence.default_lane.active_product_questions, [
    'which families belong in the default lane',
    'whether large_file should remain eligible for the default lane',
  ]);
});

test('buildAdvisorSummaryMarkdown tolerates scalar evidence fields from tool payloads', function () {
  const summary = buildAdvisorSummaryMarkdown({
    repoLabel: 'demo',
    rawToolAnalysis: {
      briefs: {
        pre_merge: {
          primary_targets: [
            {
              kind: 'incomplete_propagation',
              scope: 'contract',
              summary: 'update contract surfaces',
              why_now: 'blocking_obligation',
              likely_fix_sites: 'src/domain.ts',
            },
          ],
        },
      },
      gate: {
        missing_obligations: [
          {
            concept_id: 'contract',
            summary: 'test is stale',
            missing_sites: 'src/domain.test.ts',
          },
        ],
      },
    },
    evidence: {
      default_lane: {
        primary_action_count: 1,
        max_primary_actions: 3,
        large_file_primary_slot_count: 0,
      },
      missing_obligation_count: 1,
      rules_bootstrap: {
        candidate_count: 0,
      },
    },
    artifactPaths: {
      engineeringReportPath: '/tmp/report/ENGINEERING_REPORT.md',
      reportPath: '/tmp/report/REPORT.md',
      rawToolAnalysisPath: '/tmp/report/raw-tool-analysis.json',
      advisorEvidencePath: '/tmp/report/advisor-evidence.json',
    },
  });

  assert.match(summary, /blocking_obligation/);
  assert.match(summary, /src\/domain.ts/);
  assert.match(summary, /src\/domain.test.ts/);
});
