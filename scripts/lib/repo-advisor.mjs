import { cp, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

import { createDisposableRepoClone } from './disposable-repo.mjs';
import { pathExists, readJson } from './eval-runtime/common.mjs';
import { buildExternalValidationPaths } from './external-validation/artifacts.mjs';
import {
  buildEngineeringReport,
  buildValidationReport,
} from './external-validation/report.mjs';
import {
  buildRawToolSummary,
  buildScanCoverageBreakdown,
  formatScanCoverageBreakdownMarkdown,
  sanitizeRepoArtifactLabel,
} from './external-validation/scan-coverage.mjs';
import { collectRepoIdentity } from './repo-identity.mjs';
import { defaultLaneActionLimit } from './signal-policy.mjs';

const DEFAULT_REPO_EXCLUDES = [
  'node_modules/**',
  'dist/**',
  'build/**',
  'coverage/**',
  'target/**',
  '.sentrux/cache/**',
];

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function asArray(value) {
  if (Array.isArray(value)) {
    return value;
  }
  if (hasText(value)) {
    return [value];
  }
  return [];
}

function formatOptionalList(value, delimiter = ', ') {
  const items = asArray(value);
  return items.length > 0 ? items.join(delimiter) : 'not specified';
}

function readOptionValue(argv, index, optionName) {
  const value = argv[index + 1];
  if (!hasText(value) || value.startsWith('--')) {
    throw new Error(`Missing value for ${optionName}`);
  }
  return value;
}

function parsePositiveInteger(value, optionName) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${optionName} must be a positive integer`);
  }
  return parsed;
}

function normalizeMode(value) {
  switch (value) {
    case 'head':
    case 'head_clone':
      return 'head';
    case 'live':
      return 'live';
    case 'working-tree':
    case 'working_tree':
    case undefined:
    case null:
      return 'working-tree';
    default:
      throw new Error(`Unsupported analysis mode: ${value}`);
  }
}

function slugify(value) {
  return String(value ?? 'repo')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'repo';
}

function shouldCopyAdvisorPath(sourceRoot, sourcePath) {
  const rel = path.relative(sourceRoot, sourcePath).split(path.sep).join('/');
  if (rel === '') {
    return true;
  }

  return ![
    '.git',
    'node_modules',
    'target',
    'dist',
    'build',
    'coverage',
    '.next',
    '.sentrux/cache',
  ].some((ignoredPath) => rel === ignoredPath || rel.startsWith(`${ignoredPath}/`));
}

function defaultAdvisorOutputDir(repoLabel) {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return path.join(os.homedir(), '.sentrux', 'repo-advisor', `${timestamp}-${slugify(repoLabel)}`);
}

function advisorRulesPath(workRoot) {
  return path.join(workRoot, '.sentrux', 'rules.toml');
}

function collectRepoIdentityMaybe(repoRootPath) {
  try {
    return collectRepoIdentity(repoRootPath);
  } catch (error) {
    return {
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export function parseRepoAdvisorArgs(argv) {
  const result = {
    repoRoot: null,
    repoLabel: null,
    outputDir: null,
    previousAnalysisPath: null,
    analysisMode: 'working-tree',
    findingsLimit: 25,
    deadPrivateLimit: 10,
    rulesSource: null,
    applySuggestedRules: true,
    keepWorkspace: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      result.repoRoot = readOptionValue(argv, index, value);
      index += 1;
      continue;
    }
    if (value === '--repo-label') {
      result.repoLabel = readOptionValue(argv, index, value);
      index += 1;
      continue;
    }
    if (value === '--output-dir') {
      result.outputDir = readOptionValue(argv, index, value);
      index += 1;
      continue;
    }
    if (value === '--previous-analysis') {
      result.previousAnalysisPath = readOptionValue(argv, index, value);
      index += 1;
      continue;
    }
    if (value === '--analysis-mode') {
      result.analysisMode = normalizeMode(readOptionValue(argv, index, value));
      index += 1;
      continue;
    }
    if (value === '--findings-limit') {
      result.findingsLimit = parsePositiveInteger(readOptionValue(argv, index, value), value);
      index += 1;
      continue;
    }
    if (value === '--dead-private-limit') {
      result.deadPrivateLimit = parsePositiveInteger(readOptionValue(argv, index, value), value);
      index += 1;
      continue;
    }
    if (value === '--rules-source') {
      result.rulesSource = readOptionValue(argv, index, value);
      index += 1;
      continue;
    }
    if (value === '--no-apply-suggested-rules') {
      result.applySuggestedRules = false;
      continue;
    }
    if (value === '--keep-workspace') {
      result.keepWorkspace = true;
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.repoRoot) {
    throw new Error('Missing required --repo-root');
  }

  result.repoRoot = path.resolve(result.repoRoot);
  result.repoLabel = result.repoLabel ?? path.basename(result.repoRoot);
  result.outputDir = path.resolve(result.outputDir ?? defaultAdvisorOutputDir(result.repoLabel));
  result.rulesSource = result.rulesSource ? path.resolve(result.rulesSource) : null;
  result.previousAnalysisPath = result.previousAnalysisPath
    ? path.resolve(result.previousAnalysisPath)
    : null;

  return result;
}

async function createCopiedWorkspace(sourceRoot, repoLabel, rulesSource) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), `sentrux-repo-advisor-${slugify(repoLabel)}-`));
  const workRoot = path.join(tempRoot, repoLabel);
  await mkdir(path.dirname(workRoot), { recursive: true });
  await cp(sourceRoot, workRoot, {
    recursive: true,
    filter(source) {
      return shouldCopyAdvisorPath(sourceRoot, source);
    },
  });
  if (rulesSource) {
    const rulesPath = advisorRulesPath(workRoot);
    await mkdir(path.dirname(rulesPath), { recursive: true });
    await cp(rulesSource, rulesPath);
  }

  return {
    tempRoot,
    workRoot,
    analysisMode: 'directory-copy',
    async cleanup() {
      await rm(tempRoot, { recursive: true, force: true });
    },
  };
}

export async function createRepoAdvisorWorkspace({
  repoRoot,
  repoLabel,
  analysisMode,
  rulesSource,
}) {
  const normalizedMode = normalizeMode(analysisMode);
  if (normalizedMode === 'live') {
    if (rulesSource) {
      throw new Error(
        '--analysis-mode live cannot safely use --rules-source because rules would need to be written into the target repo',
      );
    }
    return {
      sourceRoot: repoRoot,
      workRoot: repoRoot,
      tempRoot: null,
      analysisMode: 'live',
      safety: {
        mutates_target_repo: true,
        isolated_workspace: false,
        note: 'live mode scans the target path directly and analyzer internals may write .sentrux state into that repo',
      },
      async cleanup() {},
    };
  }

  const cloneMode = normalizedMode === 'head' ? 'head_clone' : 'working_tree';
  try {
    const clone = await createDisposableRepoClone({
      sourceRoot: repoRoot,
      label: slugify(repoLabel),
      rulesSource,
      analysisMode: cloneMode,
    });
    return {
      sourceRoot: repoRoot,
      workRoot: clone.workRoot,
      tempRoot: clone.tempRoot,
      analysisMode: clone.analysisMode,
      safety: {
        mutates_target_repo: false,
        isolated_workspace: true,
        note: normalizedMode === 'head'
          ? 'analyzed a local clone at HEAD'
          : 'analyzed a local clone with working-tree changes overlaid',
      },
      cleanup: clone.cleanup,
    };
  } catch {
    const copied = await createCopiedWorkspace(repoRoot, slugify(repoLabel), rulesSource);
    return {
      sourceRoot: repoRoot,
      workRoot: copied.workRoot,
      tempRoot: copied.tempRoot,
      analysisMode: copied.analysisMode,
      safety: {
        mutates_target_repo: false,
        isolated_workspace: true,
        note: 'git clone was unavailable; analyzed an isolated directory copy',
      },
      cleanup: copied.cleanup,
    };
  }
}

function extractOnboardingBrief(rawToolAnalysis) {
  return rawToolAnalysis.briefs?.repo_onboarding ?? rawToolAnalysis.brief_repo_onboarding ?? null;
}

function extractRepoShape(rawToolAnalysis) {
  return extractOnboardingBrief(rawToolAnalysis)?.repo_shape ?? {};
}

function candidateFromBoundaryRoot(boundaryRoot) {
  const root = boundaryRoot.root ?? 'this boundary';
  return {
    kind: 'boundary_root',
    id: boundaryRoot.root ?? boundaryRoot.kind ?? 'unknown_boundary',
    risk: 'needs_review',
    confidence: 'medium',
    evidence: asArray(boundaryRoot.evidence),
    recommendation: `Review whether ${root} should become an explicit concept or module contract.`,
  };
}

function candidateFromModuleContract(moduleContract) {
  const root = moduleContract.root ?? 'this module root';
  return {
    kind: 'module_contract',
    id: moduleContract.id ?? moduleContract.root ?? 'module_contract',
    risk: moduleContract.confidence === 'high' ? 'high_confidence' : 'needs_review',
    confidence: moduleContract.confidence ?? 'unknown',
    evidence: asArray(moduleContract.evidence),
    recommendation: `Use public API files for ${root} and block cross-module deep imports after review.`,
  };
}

function fallbackStarterRulesToml(rawToolAnalysis) {
  const scan = rawToolAnalysis.scan ?? {};
  const primaryLanguage = scan.language_summary?.[0]?.language ?? null;
  const lines = ['[project]'];
  if (primaryLanguage) {
    lines.push(`primary_language = "${primaryLanguage}"`);
  }
  lines.push(`exclude = [${DEFAULT_REPO_EXCLUDES.map((entry) => `"${entry}"`).join(', ')}]`);
  lines.push('');
  lines.push('# Add the first enforced concept, contract, state model, or module contract after review.');
  return `${lines.join('\n')}\n`;
}

export function buildRulesBootstrap(rawToolAnalysis, { configuredRulesPath = null } = {}) {
  const repoShape = extractRepoShape(rawToolAnalysis);
  const starterRulesToml =
    repoShape.starter_rules_toml ?? repoShape.working_rules_toml ?? fallbackStarterRulesToml(rawToolAnalysis);
  const workingRulesToml = repoShape.working_rules_toml ?? starterRulesToml;
  const inferredRules = extractOnboardingBrief(rawToolAnalysis)?.inferred_rules ?? null;
  const boundaryRoots = Array.isArray(repoShape.boundary_roots) ? repoShape.boundary_roots : [];
  const moduleContracts = Array.isArray(repoShape.module_contracts) ? repoShape.module_contracts : [];
  const candidates = [
    ...boundaryRoots.map(candidateFromBoundaryRoot),
    ...moduleContracts.map(candidateFromModuleContract),
  ];

  if (inferredRules) {
    for (const [kind, count] of Object.entries(inferredRules)) {
      if (Number.isFinite(count) && count > 0) {
        candidates.push({
          kind: `inferred_${kind}`,
          id: kind,
          risk: 'high_confidence',
          confidence: 'high',
          evidence: [
            `${count} inferred ${kind.replaceAll('_', ' ')} from repo shape and semantic facts`,
          ],
          recommendation:
            'Keep inferred rules in the analysis workspace first; promote to checked-in rules only after a maintainer reviews the generated TOML.',
        });
      }
    }
  }

  return {
    configured_rules_path: configuredRulesPath,
    should_write_to_target: false,
    generated_rules_toml: workingRulesToml,
    starter_rules_toml: starterRulesToml,
    candidate_count: candidates.length,
    candidates,
    risk_summary: {
      high_confidence: candidates.filter((candidate) => candidate.risk === 'high_confidence').length,
      needs_review: candidates.filter((candidate) => candidate.risk === 'needs_review').length,
      risky: candidates.filter((candidate) => candidate.risk === 'risky').length,
    },
  };
}

export function formatRulesBootstrapMarkdown(bootstrap) {
  const lines = ['# Calibrated Rules Bootstrap', ''];
  lines.push(`- existing rules: \`${bootstrap.configured_rules_path ?? 'none'}\``);
  lines.push('- target mutation: `none`');
  lines.push(`- candidates: \`${bootstrap.candidate_count}\``);
  lines.push(
    `- risk summary: \`${bootstrap.risk_summary.high_confidence} high confidence, ${bootstrap.risk_summary.needs_review} needs review, ${bootstrap.risk_summary.risky} risky\``,
  );
  lines.push('');
  lines.push('## Candidate Rules');
  lines.push('');
  if (bootstrap.candidates.length === 0) {
    lines.push('- none surfaced');
  }
  for (const candidate of bootstrap.candidates) {
    lines.push(`- \`${candidate.kind}:${candidate.id}\` (${candidate.risk})`);
    if (candidate.evidence.length > 0) {
      lines.push(`  - evidence: ${candidate.evidence.join('; ')}`);
    }
    lines.push(`  - recommendation: ${candidate.recommendation}`);
  }
  lines.push('');
  lines.push('## Generated TOML');
  lines.push('');
  lines.push('```toml');
  lines.push(bootstrap.generated_rules_toml.trimEnd());
  lines.push('```');
  lines.push('');
  return `${lines.join('\n')}\n`;
}

function selectBrief(rawToolAnalysis, mode) {
  return rawToolAnalysis.briefs?.[mode] ?? rawToolAnalysis[`brief_${mode}`] ?? null;
}

function primaryTargets(rawToolAnalysis) {
  const brief = selectBrief(rawToolAnalysis, 'pre_merge') ?? selectBrief(rawToolAnalysis, 'patch');
  return Array.isArray(brief?.primary_targets) ? brief.primary_targets : [];
}

function missingObligations(rawToolAnalysis) {
  const gateObligations = rawToolAnalysis.gate?.missing_obligations;
  if (Array.isArray(gateObligations)) {
    return gateObligations;
  }

  const brief = selectBrief(rawToolAnalysis, 'pre_merge') ?? selectBrief(rawToolAnalysis, 'patch');
  return Array.isArray(brief?.missing_obligations) ? brief.missing_obligations : [];
}

function normalizedKindCounts(rawToolAnalysis) {
  return buildRawToolSummary(rawToolAnalysis).findings_summary?.kind_counts ?? {};
}

function actionIdentity(action) {
  return `${action.kind ?? 'unknown'}:${action.scope ?? action.concept_id ?? 'unknown'}`;
}

export function buildBeforeAfterComparison(previousAnalysis, currentAnalysis) {
  const previousActions = primaryTargets(previousAnalysis);
  const currentActions = primaryTargets(currentAnalysis);
  const previousActionKeys = new Set(previousActions.map(actionIdentity));
  const currentActionKeys = new Set(currentActions.map(actionIdentity));
  const resolvedActions = previousActions.filter((action) => !currentActionKeys.has(actionIdentity(action)));
  const newActions = currentActions.filter((action) => !previousActionKeys.has(actionIdentity(action)));
  const previousKindCounts = normalizedKindCounts(previousAnalysis);
  const currentKindCounts = normalizedKindCounts(currentAnalysis);
  const allKinds = [...new Set([...Object.keys(previousKindCounts), ...Object.keys(currentKindCounts)])].sort();

  return {
    gate_decision_before: previousAnalysis.gate?.decision ?? null,
    gate_decision_after: currentAnalysis.gate?.decision ?? null,
    primary_action_count_before: previousActions.length,
    primary_action_count_after: currentActions.length,
    missing_obligation_count_before: missingObligations(previousAnalysis).length,
    missing_obligation_count_after: missingObligations(currentAnalysis).length,
    resolved_primary_actions: resolvedActions.map((action) => ({
      kind: action.kind ?? null,
      scope: action.scope ?? null,
      summary: action.summary ?? null,
    })),
    new_primary_actions: newActions.map((action) => ({
      kind: action.kind ?? null,
      scope: action.scope ?? null,
      summary: action.summary ?? null,
    })),
    finding_kind_deltas: allKinds.map((kind) => ({
      kind,
      before: previousKindCounts[kind] ?? 0,
      after: currentKindCounts[kind] ?? 0,
      delta: (currentKindCounts[kind] ?? 0) - (previousKindCounts[kind] ?? 0),
    })),
  };
}

export function formatBeforeAfterComparisonMarkdown(comparison) {
  const lines = ['# Before/After Analysis Comparison', ''];
  lines.push(
    `- gate decision: \`${comparison.gate_decision_before ?? 'unknown'} -> ${comparison.gate_decision_after ?? 'unknown'}\``,
  );
  lines.push(
    `- primary actions: \`${comparison.primary_action_count_before} -> ${comparison.primary_action_count_after}\``,
  );
  lines.push(
    `- missing obligations: \`${comparison.missing_obligation_count_before} -> ${comparison.missing_obligation_count_after}\``,
  );
  lines.push('');
  lines.push('## Resolved Primary Actions');
  lines.push('');
  if (comparison.resolved_primary_actions.length === 0) {
    lines.push('- none');
  }
  for (const action of comparison.resolved_primary_actions) {
    lines.push(`- \`${action.kind}:${action.scope}\` - ${action.summary ?? 'no summary'}`);
  }
  lines.push('');
  lines.push('## New Primary Actions');
  lines.push('');
  if (comparison.new_primary_actions.length === 0) {
    lines.push('- none');
  }
  for (const action of comparison.new_primary_actions) {
    lines.push(`- \`${action.kind}:${action.scope}\` - ${action.summary ?? 'no summary'}`);
  }
  lines.push('');
  lines.push('## Finding Kind Deltas');
  lines.push('');
  for (const delta of comparison.finding_kind_deltas.filter((entry) => entry.delta !== 0)) {
    const sign = delta.delta >= 0 ? '+' : '';
    lines.push(`- \`${delta.kind}\`: \`${delta.before} -> ${delta.after}\` (${sign}${delta.delta})`);
  }
  if (!comparison.finding_kind_deltas.some((entry) => entry.delta !== 0)) {
    lines.push('- none');
  }
  lines.push('');
  return `${lines.join('\n')}\n`;
}

function evidenceActionFromTarget(target, rank) {
  return {
    rank,
    kind: target.kind ?? null,
    scope: target.scope ?? null,
    source: target.source ?? null,
    severity: target.severity ?? null,
    trust_tier: target.trust_tier ?? null,
    leverage_class: target.leverage_class ?? null,
    blocking: target.blocking ?? false,
    why_now: asArray(target.why_now),
    likely_fix_sites: asArray(target.likely_fix_sites),
    repair_packet_complete: target.repair_packet?.complete ?? null,
    repair_packet_completeness_0_10000: target.repair_packet?.completeness_0_10000 ?? null,
  };
}

export function buildAdvisorEvidence({
  repoLabel,
  sourceRepoRoot,
  analyzedRepoRoot,
  workspace,
  rawToolAnalysis,
  rulesBootstrap,
  previousComparison = null,
}) {
  const actions = primaryTargets(rawToolAnalysis);
  const largeFilePrimarySlotCount = actions.filter((action) => action.kind === 'large_file').length;
  const actionLimit = defaultLaneActionLimit();

  return {
    schema_version: 1,
    repo_label: repoLabel,
    source_repo_root: sourceRepoRoot,
    analyzed_repo_root: analyzedRepoRoot,
    analysis_mode: workspace.analysisMode,
    safety: workspace.safety,
    source_identity: collectRepoIdentityMaybe(sourceRepoRoot),
    analyzed_identity: collectRepoIdentityMaybe(analyzedRepoRoot),
    default_lane: {
      control_arm: 'current_policy',
      max_primary_actions: actionLimit,
      primary_action_count: actions.length,
      primary_action_over_limit: actions.length > actionLimit,
      large_file_primary_slot_count: largeFilePrimarySlotCount,
      large_file_primary_slot_rate_0_10000:
        actions.length === 0 ? 0 : Math.round((largeFilePrimarySlotCount / actions.length) * 10000),
      active_product_questions: [
        'which families belong in the default lane',
        'whether large_file should remain eligible for the default lane',
      ],
    },
    top_actions: actions.map((target, index) => evidenceActionFromTarget(target, index + 1)),
    missing_obligation_count: missingObligations(rawToolAnalysis).length,
    blocking_finding_count: rawToolAnalysis.gate?.blocking_finding_count ?? null,
    introduced_finding_count: rawToolAnalysis.gate?.introduced_finding_count ?? null,
    rules_bootstrap: {
      candidate_count: rulesBootstrap.candidate_count,
      risk_summary: rulesBootstrap.risk_summary,
      applied_to_analysis_workspace: false,
      should_write_to_target: false,
    },
    previous_comparison: previousComparison,
  };
}

function formatActionFixSites(action) {
  return formatOptionalList(asArray(action.likely_fix_sites).slice(0, 5));
}

export function buildAdvisorSummaryMarkdown({ repoLabel, rawToolAnalysis, evidence, artifactPaths }) {
  const actions = primaryTargets(rawToolAnalysis);
  const obligations = missingObligations(rawToolAnalysis);
  const lines = [`# ${repoLabel} Repo Advisor Summary`, ''];
  lines.push('## What To Fix First');
  lines.push('');
  if (actions.length === 0) {
    lines.push(
      '- No primary patch actions surfaced. Use the structural watchpoints as backlog, not immediate patch work.',
    );
  }
  for (const action of actions) {
    lines.push(
      `- \`${action.kind}\` in \`${action.scope ?? 'unknown'}\`: ${action.summary ?? 'no summary'}`,
    );
    lines.push(`  - why now: ${formatOptionalList(action.why_now)}`);
    lines.push(`  - likely fix sites: ${formatActionFixSites(action)}`);
  }
  lines.push('');
  lines.push('## What Else Must Change');
  lines.push('');
  if (obligations.length === 0) {
    lines.push('- No missing obligation surfaces were detected.');
  }
  for (const obligation of obligations.slice(0, 10)) {
    const concept = obligation.concept_id ?? obligation.concept ?? obligation.scope ?? 'unknown';
    const sites = asArray(obligation.missing_sites ?? obligation.required_update_sites)
      .slice(0, 5)
      .map((site) => (typeof site === 'string' ? site : site.path ?? JSON.stringify(site)));
    lines.push(`- \`${concept}\`: ${obligation.summary ?? obligation.message ?? 'missing follow-through'}`);
    if (sites.length > 0) {
      lines.push(`  - update sites: ${sites.join(', ')}`);
    }
  }
  lines.push('');
  lines.push('## Evidence State');
  lines.push('');
  lines.push(
    `- default lane action count: \`${evidence.default_lane.primary_action_count}/${evidence.default_lane.max_primary_actions}\``,
  );
  lines.push(`- large_file primary slots: \`${evidence.default_lane.large_file_primary_slot_count}\``);
  lines.push(`- missing obligations: \`${evidence.missing_obligation_count}\``);
  lines.push(`- rules bootstrap candidates: \`${evidence.rules_bootstrap.candidate_count}\``);
  lines.push('');
  lines.push('## Artifact Map');
  lines.push('');
  lines.push(`- engineer report: \`${artifactPaths.engineeringReportPath}\``);
  lines.push(`- validation report: \`${artifactPaths.reportPath}\``);
  lines.push(`- raw tool analysis: \`${artifactPaths.rawToolAnalysisPath}\``);
  lines.push(`- evidence: \`${artifactPaths.advisorEvidencePath}\``);
  lines.push('');
  return `${lines.join('\n')}\n`;
}

export function buildAdvisorPaths(outputDir, repoLabel) {
  const externalPaths = buildExternalValidationPaths(outputDir, repoLabel);
  const label = sanitizeRepoArtifactLabel(repoLabel);
  return {
    ...externalPaths,
    advisorSummaryPath: path.join(outputDir, 'ADVISOR_SUMMARY.md'),
    advisorEvidencePath: path.join(outputDir, 'advisor-evidence.json'),
    rulesBootstrapJsonPath: path.join(outputDir, 'rules-bootstrap.json'),
    rulesBootstrapMarkdownPath: path.join(outputDir, 'RULES_BOOTSTRAP.md'),
    generatedRulesPath: path.join(outputDir, `${label}.suggested.rules.toml`),
    comparisonJsonPath: path.join(outputDir, 'before-after-comparison.json'),
    comparisonMarkdownPath: path.join(outputDir, 'BEFORE_AFTER.md'),
  };
}

export async function maybeWriteGeneratedRules(workRoot, rulesBootstrap, shouldApply) {
  if (!shouldApply || !hasText(rulesBootstrap.generated_rules_toml)) {
    return {
      applied: false,
      reason: shouldApply ? 'no generated rules' : 'disabled',
      path: null,
    };
  }
  const targetPath = advisorRulesPath(workRoot);
  if (await pathExists(targetPath)) {
    return {
      applied: false,
      reason: 'workspace already has rules',
      path: targetPath,
    };
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, rulesBootstrap.generated_rules_toml, 'utf8');
  return {
    applied: true,
    reason: 'generated rules applied to isolated analysis workspace',
    path: targetPath,
  };
}

export function canApplyGeneratedRulesToWorkspace(shouldApply, workspace) {
  return shouldApply === true && workspace.safety?.isolated_workspace === true;
}

export async function writeAdvisorArtifacts({
  paths,
  rawToolAnalysis,
  rawToolSummary,
  scanCoverageBreakdown,
  validationReport,
  engineeringReport,
  rulesBootstrap,
  advisorEvidence,
  advisorSummary,
  previousComparison,
}) {
  await mkdir(path.dirname(paths.rawToolAnalysisPath), { recursive: true });
  await writeFile(paths.rawToolAnalysisPath, `${JSON.stringify(rawToolAnalysis, null, 2)}\n`, 'utf8');
  await writeFile(paths.rawToolSummaryPath, `${JSON.stringify(rawToolSummary, null, 2)}\n`, 'utf8');
  await writeFile(
    paths.scanCoverageBreakdownJsonPath,
    `${JSON.stringify(scanCoverageBreakdown, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    paths.scanCoverageBreakdownMarkdownPath,
    formatScanCoverageBreakdownMarkdown(scanCoverageBreakdown),
    'utf8',
  );
  await writeFile(paths.reportPath, validationReport, 'utf8');
  await writeFile(paths.engineeringReportPath, engineeringReport, 'utf8');
  await writeFile(paths.repoEngineeringReportPath, engineeringReport, 'utf8');
  await writeFile(
    paths.rulesBootstrapJsonPath,
    `${JSON.stringify(rulesBootstrap, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    paths.rulesBootstrapMarkdownPath,
    formatRulesBootstrapMarkdown(rulesBootstrap),
    'utf8',
  );
  await writeFile(paths.generatedRulesPath, rulesBootstrap.generated_rules_toml, 'utf8');
  await writeFile(paths.advisorEvidencePath, `${JSON.stringify(advisorEvidence, null, 2)}\n`, 'utf8');
  await writeFile(paths.advisorSummaryPath, advisorSummary, 'utf8');
  if (previousComparison) {
    await writeFile(
      paths.comparisonJsonPath,
      `${JSON.stringify(previousComparison, null, 2)}\n`,
      'utf8',
    );
    await writeFile(
      paths.comparisonMarkdownPath,
      formatBeforeAfterComparisonMarkdown(previousComparison),
      'utf8',
    );
  }
}

export async function loadPreviousAnalysis(previousAnalysisPath) {
  if (!previousAnalysisPath) {
    return null;
  }

  return readJson(previousAnalysisPath);
}

export function buildReportsForAdvisor({
  repoRootPath,
  repoLabel,
  metadata,
  rawToolAnalysis,
  rawToolSummary,
  packetValidation,
  scanCoverageBreakdown,
}) {
  return {
    engineeringReport: buildEngineeringReport({
      repoRootPath,
      repoLabel,
      branch: metadata.branch,
      commit: metadata.commit,
      rawToolAnalysis,
    }),
    validationReport: buildValidationReport({
      repoRootPath,
      repoLabel,
      branch: metadata.branch,
      commit: metadata.commit,
      workingTreeClean: metadata.workingTreeClean,
      rawToolAnalysis,
      rawToolSummary,
      packetValidation,
      scanCoverageBreakdown,
    }),
  };
}

export function markRulesAppliedInEvidence(evidence, rulesApplication) {
  return {
    ...evidence,
    rules_bootstrap: {
      ...evidence.rules_bootstrap,
      applied_to_analysis_workspace: rulesApplication.applied,
      application_reason: rulesApplication.reason,
      workspace_rules_path: rulesApplication.path,
    },
  };
}
