import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  buildPacketValidation,
  buildEngineeringReport,
  buildRawToolSummary,
  buildScanCoverageBreakdown,
  buildValidationReport,
  formatScanCoverageBreakdownMarkdown,
  parseArgs,
} from '../evals/run-external-repo-validation.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const fixtureRoot = path.join(__dirname, 'fixtures', 'public-repo-feedback');

async function readFixture(name) {
  return JSON.parse(await readFile(path.join(fixtureRoot, name), 'utf8'));
}

test('parseArgs requires repo root and infers repo label', function () {
  assert.throws(
    function shouldThrow() {
      parseArgs(['node', 'script']);
    },
    /Missing required --repo-root/,
  );

  const args = parseArgs(['node', 'script', '--repo-root', '/tmp/public-repo']);
  assert.equal(args.repoRoot, '/tmp/public-repo');
  assert.equal(args.repoLabel, 'public-repo');
});

test('buildRawToolSummary preserves Public Repo confidence and finding counts', async function () {
  const analysis = await readFixture('analysis.json');
  const summary = buildRawToolSummary(analysis);

  assert.equal(summary.repo_root, '<external-repo-root>');
  assert.equal(summary.scan_summary.mode, 'git');
  assert.equal(summary.scan_summary.kept_files, 958);
  assert.equal(summary.scan_summary.candidate_files, 6964);
  assert.equal(summary.scan_summary.untracked_candidates, 4);
  assert.equal(summary.scan_summary.exclusions.total, 6006);
  assert.equal(summary.scan_summary.resolution.internal_confidence_0_10000, 9794);
  assert.equal(summary.findings_summary.kind_counts.exact_clone_group, 3);
  assert.equal(summary.findings_summary.dead_private_source_lane, 'experimental_debt_signals');
  assert.equal(summary.findings_summary.dead_private_candidate_count, 1);
  assert.equal(summary.findings_summary.kind_counts.experimental_dead_private_code_cluster, 4);
  assert.equal(summary.session_end_summary.pass, true);
});

test('buildScanCoverageBreakdown preserves exclusions and resolution detail', async function () {
  const analysis = await readFixture('analysis.json');
  const breakdown = buildScanCoverageBreakdown(analysis);
  const markdown = formatScanCoverageBreakdownMarkdown(breakdown);

  assert.equal(breakdown.repo_root, '<external-repo-root>');
  assert.equal(breakdown.candidate_file_coverage.mode, 'git');
  assert.equal(breakdown.candidate_file_coverage.tracked_candidates, 6960);
  assert.equal(breakdown.candidate_file_coverage.untracked_candidates, 4);
  assert.equal(breakdown.exclusions.total, 6006);
  assert.equal(breakdown.exclusions.bucketed.vendor, 5993);
  assert.equal(breakdown.resolution.resolved, 949);
  assert.equal(breakdown.resolution.internal_confidence_0_10000, 9794);
  assert.equal(breakdown.confidence.scan_confidence_0_10000, 1376);
  assert.match(markdown, /Candidate-file coverage only/);
  assert.match(markdown, /tracked candidates: `6960`/);
  assert.match(markdown, /vendor: `5993`/);
  assert.match(markdown, /internal resolution confidence: `9794 \/ 10000`/);
});

test('buildValidationReport calls out dead-private precision and scan trust gaps', async function () {
  const analysis = await readFixture('analysis.json');
  const summary = buildRawToolSummary(analysis);
  const scanCoverageBreakdown = buildScanCoverageBreakdown(analysis);
  const packetValidation = buildPacketValidation({
    samples: [
      {
        kind: 'exact_clone_group',
        clone_evidence: {
          files: ['packages/examples/pages/run-detail.tsx'],
          instances: [{ file: 'packages/examples/pages/run-detail.tsx', lines: 81 }],
          recent_edit_reasons: ['identical logic spans 3 files'],
        },
      },
    ],
    scan_metadata: {
      confidence: {
        scan_confidence_0_10000: 1376,
        rule_coverage_0_10000: 0,
      },
    },
  });
  const report = buildValidationReport({
    repoRootPath: '<external-repo-root>',
    repoLabel: 'public-repo',
    branch: 'main',
    commit: '0724ba9a',
    workingTreeClean: true,
    rawToolAnalysis: analysis,
    rawToolSummary: summary,
    packetValidation,
    scanCoverageBreakdown,
  });

  assert.match(report, /clean-repo gating stayed quiet/);
  assert.match(report, /clone review packets now preserve concrete evidence/);
  assert.match(report, /review packets now surface scan confidence and rule coverage/);
  assert.match(report, /scan coverage breakdown artifact now preserves candidate coverage/);
  assert.match(report, /dead-private precision is not good enough yet/);
  assert.match(report, /cell, cell, cell/);
  assert.match(report, /Public Repo still scans with low confidence/);
  assert.doesNotMatch(report, /clone packet output is too lossy/);
});

test('buildEngineeringReport separates high-confidence work from skeptical dead-private cases', async function () {
  const analysis = await readFixture('analysis.json');
  const report = buildEngineeringReport({
    repoRootPath: '<external-repo-root>',
    repoLabel: 'public-repo',
    branch: 'main',
    commit: '0724ba9a',
    rawToolAnalysis: analysis,
  });

  assert.match(report, /Priority 1: Break The Dependency Cycles/);
  assert.match(report, /Priority 1: Reduce Template And Example Duplication Drift/);
  assert.match(report, /ToastSuccess, ToastError, ToastWarning/);
  assert.match(report, /getDerivedStateFromError, componentDidCatch/);
});

test('buildEngineeringReport says when no dead-private candidates surfaced', async function () {
  const analysis = await readFixture('analysis.json');
  analysis.findings.experimental_findings = [];
  analysis.findings.experimental_debt_signals = [];

  const report = buildEngineeringReport({
    repoRootPath: '<external-repo-root>',
    repoLabel: 'public-repo',
    branch: 'main',
    commit: '0724ba9a',
    rawToolAnalysis: analysis,
  });

  assert.match(report, /Priority 2: Review Experimental Dead-Private Candidates/);
  assert.match(report, /none surfaced in this run/);
});
