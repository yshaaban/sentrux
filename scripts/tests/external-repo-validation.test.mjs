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

  const args = parseArgs(['node', 'script', '--repo-root', '/tmp/one-tool']);
  assert.equal(args.repoRoot, '/tmp/one-tool');
  assert.equal(args.repoLabel, 'one-tool');
});

test('buildRawToolSummary preserves external-repo confidence and finding counts', async function () {
  const analysis = await readFixture('analysis.json');
  const summary = buildRawToolSummary(analysis);

  assert.equal(summary.repo_root, '/workspace/one-tool');
  assert.equal(summary.scan_summary.mode, 'git');
  assert.equal(summary.scan_summary.kept_files, 958);
  assert.equal(summary.scan_summary.candidate_files, 6964);
  assert.equal(summary.scan_summary.untracked_candidates, 4);
  assert.equal(summary.scan_summary.exclusions.total, 6006);
  assert.equal(summary.scan_summary.resolution.internal_confidence_0_10000, 9794);
  assert.equal(summary.findings_summary.kind_counts.exact_clone_group, 3);
  assert.equal(summary.findings_summary.dead_private_source_lane, 'experimental_debt_signals');
  assert.equal(summary.findings_summary.dead_private_candidate_count, 1);
  assert.equal(summary.findings_summary.dead_private_reviewer_lane_status, 'canonical_with_legacy_watchlist');
  assert.equal(summary.findings_summary.dead_private_canonical_candidate_count, 1);
  assert.equal(summary.findings_summary.dead_private_legacy_candidate_count, 4);
  assert.equal(summary.findings_summary.dead_private_overlap_count, 1);
  assert.equal(summary.findings_summary.dead_private_legacy_only_count, 3);
  assert.equal(summary.findings_summary.kind_counts.experimental_dead_private_code_cluster, 4);
  assert.equal(summary.scan_summary.mixed_repo_context.kept_candidate_ratio_0_10000, 1376);
  assert.equal(summary.scan_summary.mixed_repo_context.excluded_candidate_ratio_0_10000, 8624);
  assert.equal(summary.scan_summary.mixed_repo_context.dominant_exclusion_bucket, 'vendor');
  assert.equal(summary.scan_summary.mixed_repo_context.dominant_exclusion_share_0_10000, 9978);
  assert.equal(summary.session_end_summary.pass, true);
});

test('buildScanCoverageBreakdown preserves exclusions and resolution detail', async function () {
  const analysis = await readFixture('analysis.json');
  const breakdown = buildScanCoverageBreakdown(analysis);
  const markdown = formatScanCoverageBreakdownMarkdown(breakdown);

  assert.equal(breakdown.repo_root, '/workspace/one-tool');
  assert.equal(breakdown.candidate_file_coverage.mode, 'git');
  assert.equal(breakdown.candidate_file_coverage.tracked_candidates, 6960);
  assert.equal(breakdown.candidate_file_coverage.untracked_candidates, 4);
  assert.equal(breakdown.exclusions.total, 6006);
  assert.equal(breakdown.exclusions.bucketed.vendor, 5993);
  assert.equal(breakdown.resolution.resolved, 949);
  assert.equal(breakdown.resolution.internal_confidence_0_10000, 9794);
  assert.equal(breakdown.confidence.scan_confidence_0_10000, 1376);
  assert.equal(breakdown.mixed_repo_context.kept_candidate_ratio_0_10000, 1376);
  assert.equal(breakdown.mixed_repo_context.excluded_candidate_ratio_0_10000, 8624);
  assert.equal(breakdown.mixed_repo_context.tracked_candidate_ratio_0_10000, 9994);
  assert.equal(breakdown.mixed_repo_context.untracked_candidate_ratio_0_10000, 6);
  assert.equal(breakdown.mixed_repo_context.dominant_exclusion_bucket, 'vendor');
  assert.equal(breakdown.mixed_repo_context.dominant_exclusion_count, 5993);
  assert.equal(breakdown.mixed_repo_context.dominant_exclusion_share_0_10000, 9978);
  assert.match(
    breakdown.mixed_repo_context.interpretation,
    /Low top-line confidence is dominated by candidate exclusions in a mixed repo/,
  );
  assert.match(markdown, /Candidate-file coverage only/);
  assert.match(markdown, /tracked candidates: `6960`/);
  assert.match(markdown, /vendor: `5993`/);
  assert.match(markdown, /## Mixed-Repo Context/);
  assert.match(markdown, /kept candidate ratio: `1376 \/ 10000`/);
  assert.match(markdown, /dominant exclusion bucket: `vendor`/);
  assert.match(markdown, /mixed-repo interpretation: Low top-line confidence is dominated by candidate exclusions in a mixed repo/);
  assert.match(markdown, /internal resolution confidence: `9794 \/ 10000`/);
});

test('buildScanCoverageBreakdown keeps the generic interpretation when kept-file counts are unavailable', async function () {
  const analysis = await readFixture('analysis.json');
  delete analysis.scan.scan_trust.kept_files;

  const breakdown = buildScanCoverageBreakdown(analysis);

  assert.equal(breakdown.mixed_repo_context.kept_candidate_ratio_0_10000, null);
  assert.equal(
    breakdown.mixed_repo_context.interpretation,
    'Top-line scan confidence should be read alongside candidate exclusions and kept-file resolution.',
  );
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
    repoRootPath: '/workspace/one-tool',
    repoLabel: 'one-tool',
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
  assert.match(report, /dead-private review routing is explicit/);
  assert.match(
    report,
    /dead-private precision (is not good enough yet|still needs broader external validation)/,
  );
  assert.match(report, /one-tool still scans with low confidence/);
  assert.match(report, /5993 files, 9978 \/ 10000 of measured exclusions/);
  assert.match(report, /legacy-only candidate\(s\) remain outside the canonical reviewer queue/);
  assert.doesNotMatch(report, /clone packet output is too lossy/);
});

test('buildEngineeringReport separates high-confidence work from skeptical dead-private cases', async function () {
  const analysis = await readFixture('analysis.json');
  const report = buildEngineeringReport({
    repoRootPath: '/workspace/one-tool',
    repoLabel: 'one-tool',
    branch: 'main',
    commit: '0724ba9a',
    rawToolAnalysis: analysis,
  });

  assert.match(report, /Priority 1: Break The Dependency Cycles/);
  assert.match(report, /Priority 1: Reduce Template And Example Duplication Drift/);
  assert.match(report, /reviewer queue: `experimental_debt_signals` \(1 candidate\(s\), status=canonical_with_legacy_watchlist\)/);
  assert.match(report, /legacy watchlist only: `3` additional candidate\(s\) remain in experimental_findings outside the reviewer queue/);
  assert.match(report, /BannerSuccess, BannerError, BannerWarning/);
  assert.match(report, /row, row, row/);
});

test('buildEngineeringReport says when no dead-private candidates surfaced', async function () {
  const analysis = await readFixture('analysis.json');
  analysis.findings.experimental_findings = [];
  analysis.findings.experimental_debt_signals = [];

  const report = buildEngineeringReport({
    repoRootPath: '/workspace/one-tool',
    repoLabel: 'one-tool',
    branch: 'main',
    commit: '0724ba9a',
    rawToolAnalysis: analysis,
  });

  assert.match(report, /Priority 2: Review Experimental Dead-Private Candidates/);
  assert.match(report, /none surfaced in this run/);
});
