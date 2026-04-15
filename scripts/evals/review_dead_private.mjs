#!/usr/bin/env node

import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { closeChildProcess } from '../lib/child-process.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { runClaudeCode } from './providers/claude-code.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = path.join(repoRoot, 'target', 'debug', 'sentrux');
const requestTimeoutMs = Number(process.env.EVAL_TIMEOUT_MS ?? '1800000');

const DEAD_PRIVATE_REVIEW_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['task_kind', 'repo_name', 'summary', 'verdicts', 'confidence_0_1'],
  properties: {
    task_kind: { const: 'dead_private_review' },
    repo_name: { type: 'string', minLength: 1 },
    summary: { type: 'string', minLength: 1 },
    verdicts: {
      type: 'array',
      minItems: 1,
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['scope', 'file_path', 'verdict', 'rationale', 'confidence_0_1'],
        properties: {
          scope: { type: 'string', minLength: 1 },
          file_path: { type: 'string', minLength: 1 },
          verdict: {
            enum: [
              'accept',
              'acceptable_watchpoint_only',
              'reject_false_positive',
              'reject_too_ambiguous',
            ],
          },
          rationale: { type: 'string', minLength: 1 },
          confidence_0_1: {
            type: 'number',
            minimum: 0,
            maximum: 1,
          },
          evidence_gaps: {
            type: 'array',
            items: { type: 'string' },
          },
          cited_evidence: {
            type: 'array',
            items: { type: 'string' },
          },
        },
      },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    notes: {
      type: 'array',
      items: { type: 'string' },
    },
  },
};

function parseArgs(argv) {
  const result = {
    repoRoot: null,
    repoName: null,
    outputPath: null,
    limit: 5,
    findingsLimit: 50,
    dryRun: false,
    claudeBin: process.env.CLAUDE_BIN ?? 'claude',
    model: process.env.EVAL_MODEL ?? null,
    help: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--help' || value === '-h') {
      result.help = true;
      continue;
    }
    if (value === '--dry-run') {
      result.dryRun = true;
      continue;
    }
    if (value === '--repo-root') {
      index += 1;
      result.repoRoot = argv[index];
      continue;
    }
    if (value === '--repo-name') {
      index += 1;
      result.repoName = argv[index];
      continue;
    }
    if (value === '--output') {
      index += 1;
      result.outputPath = argv[index];
      continue;
    }
    if (value === '--limit') {
      index += 1;
      result.limit = Number(argv[index]);
      continue;
    }
    if (value === '--findings-limit') {
      index += 1;
      result.findingsLimit = Number(argv[index]);
      continue;
    }
    if (value === '--claude-bin') {
      index += 1;
      result.claudeBin = argv[index];
      continue;
    }
    if (value === '--model') {
      index += 1;
      result.model = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.repoRoot) {
    throw new Error('--repo-root is required');
  }
  if (!result.repoName) {
    result.repoName = path.basename(result.repoRoot);
  }
  if (!result.outputPath) {
    result.outputPath = path.join(
      repoRoot,
      'docs/v2/evals/runs',
      `${result.repoName}-dead-private-review.json`,
    );
  }
  if (!Number.isFinite(result.limit) || result.limit < 1) {
    throw new Error(`Invalid --limit value: ${result.limit}`);
  }
  if (!Number.isFinite(result.findingsLimit) || result.findingsLimit < 1) {
    throw new Error(`Invalid --findings-limit value: ${result.findingsLimit}`);
  }

  result.repoRoot = path.resolve(result.repoRoot);
  result.outputPath = path.resolve(result.outputPath);
  return result;
}

function usage() {
  return [
    'Usage: node scripts/evals/review_dead_private.mjs --repo-root <path> [options]',
    '',
    'Options:',
    '  --repo-root <path>       Repository to analyze',
    '  --repo-name <name>       Repo label for the output payload',
    '  --output <path>          JSON output path',
    '  --limit <n>              Maximum dead_private candidates to review (default: 5)',
    '  --findings-limit <n>     Maximum findings requested from Sentrux (default: 50)',
    '  --claude-bin <path>      Claude Code CLI binary',
    '  --model <name>           Claude model override',
    '  --dry-run                Export the prompt payload without calling Claude',
  ].join('\n');
}

function nowIso() {
  return new Date().toISOString();
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function parseToolPayload(response) {
  if (response.result?.isError) {
    const message = response.result.content?.[0]?.text ?? 'Unknown MCP tool error';
    throw new Error(message);
  }

  const text = response.result?.content?.[0]?.text;
  if (typeof text !== 'string') {
    throw new Error('Missing MCP text payload');
  }

  return JSON.parse(text);
}

export function deadPrivateCandidateScope(candidate) {
  return candidate?.scope ?? candidate?.files?.[0] ?? 'unknown';
}

export function deadPrivateCandidateKey(candidate) {
  return `${deadPrivateCandidateScope(candidate)}:${candidate?.kind ?? 'unknown'}`;
}

function buildReviewLaneMetadata(findings) {
  return {
    candidate_source_lane: findings.candidate_source_lane ?? null,
    candidate_source_lane_count: findings.candidate_source_lane_count ?? 0,
    candidate_source_lanes_considered: findings.candidate_source_lanes_considered ?? [],
    candidate_reviewer_lane_status: findings.candidate_reviewer_lane_status ?? null,
    candidate_reviewer_lane_reason: findings.candidate_reviewer_lane_reason ?? null,
    candidate_canonical_count: findings.candidate_canonical_count ?? 0,
    candidate_legacy_count: findings.candidate_legacy_count ?? 0,
    candidate_overlap_count: findings.candidate_overlap_count ?? 0,
    candidate_legacy_only_count: findings.candidate_legacy_only_count ?? 0,
    candidate_legacy_only_scopes: findings.candidate_legacy_only_scopes ?? [],
  };
}

function buildPromptOptions(options, findings, reviewedCandidates) {
  return {
    ...options,
    candidateSourceLane: findings.candidate_source_lane ?? null,
    candidateReviewerLaneStatus: findings.candidate_reviewer_lane_status ?? null,
    candidateReviewerLaneReason: findings.candidate_reviewer_lane_reason ?? null,
    reviewedCandidateCount: reviewedCandidates.length,
    canonicalCandidateCount: findings.candidate_canonical_count ?? 0,
    legacyCandidateCount: findings.candidate_legacy_count ?? 0,
    legacyOnlyCandidateCount: findings.candidate_legacy_only_count ?? 0,
    legacyOnlyCandidateScopes: findings.candidate_legacy_only_scopes ?? [],
  };
}

function buildReviewResultBase(options, startedAt, findings) {
  return {
    started_at: startedAt,
    repo_name: options.repoName,
    repo_root: options.repoRoot,
    task_kind: 'dead_private_review',
    ...buildReviewLaneMetadata(findings),
  };
}

export function selectDeadPrivateCandidatesFromPayload(payload) {
  const canonicalLane = Array.isArray(payload?.experimental_debt_signals)
    ? payload.experimental_debt_signals
    : [];
  const legacyLane = Array.isArray(payload?.experimental_findings)
    ? payload.experimental_findings
    : [];
  const canonicalCandidates = canonicalLane.filter(function isDeadPrivate(value) {
    return value?.kind === 'dead_private_code_cluster';
  });
  const legacyCandidates = legacyLane.filter(function isDeadPrivate(value) {
    return value?.kind === 'dead_private_code_cluster';
  });
  const selectedLane = canonicalCandidates.length > 0 ? canonicalCandidates : legacyCandidates;
  const selectedLaneName =
    canonicalCandidates.length > 0 ? 'experimental_debt_signals' : 'experimental_findings';

  const deduped = new Map();
  for (const candidate of selectedLane) {
    const key = deadPrivateCandidateKey(candidate);
    if (!deduped.has(key)) {
      deduped.set(key, candidate);
    }
  }

  const canonicalKeys = new Set(
    canonicalCandidates.map(function toCandidateKey(candidate) {
      return deadPrivateCandidateKey(candidate);
    }),
  );
  const legacyOnlyCandidates = [];
  let overlappingCandidateCount = 0;
  for (const candidate of legacyCandidates) {
    const key = deadPrivateCandidateKey(candidate);
    if (canonicalKeys.has(key)) {
      overlappingCandidateCount += 1;
      continue;
    }
    legacyOnlyCandidates.push(candidate);
  }

  let reviewerLaneStatus = 'no_candidates';
  let reviewerLaneReason = 'no dead-private candidates surfaced in either experimental lane';
  if (canonicalCandidates.length > 0) {
    reviewerLaneStatus =
      legacyOnlyCandidates.length > 0 ? 'canonical_with_legacy_watchlist' : 'canonical_only';
    reviewerLaneReason =
      'the canonical experimental_debt_signals lane is the reviewer queue; legacy-only experimental_findings stay watchlist-only context until the taxonomy is unified';
  } else if (legacyCandidates.length > 0) {
    reviewerLaneStatus = 'legacy_fallback';
    reviewerLaneReason =
      'the canonical experimental_debt_signals lane is empty, so the reviewer queue falls back to experimental_findings';
  }

  return {
    candidates: [...deduped.values()],
    source_lane: selectedLaneName,
    source_lane_count: selectedLane.length,
    considered_lanes: [
      {
        lane: 'experimental_debt_signals',
        candidate_count: canonicalLane.length,
        dead_private_candidate_count: canonicalCandidates.length,
        selected_for_review: selectedLaneName === 'experimental_debt_signals',
      },
      {
        lane: 'experimental_findings',
        candidate_count: legacyLane.length,
        dead_private_candidate_count: legacyCandidates.length,
        selected_for_review: selectedLaneName === 'experimental_findings',
      },
    ],
    canonical_candidate_count: canonicalCandidates.length,
    legacy_candidate_count: legacyCandidates.length,
    overlapping_candidate_count: overlappingCandidateCount,
    legacy_only_candidate_count: legacyOnlyCandidates.length,
    legacy_only_candidates: legacyOnlyCandidates,
    reviewer_lane_status: reviewerLaneStatus,
    reviewer_lane_reason: reviewerLaneReason,
  };
}

function createSession(binPath, homeOverride) {
  const child = spawn(binPath, ['--mcp'], {
    cwd: repoRoot,
    env: {
      ...process.env,
      HOME: homeOverride,
      SENTRUX_SKIP_GRAMMAR_DOWNLOAD: '1',
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const pending = new Map();
  let nextId = 1;
  let closed = false;

  const stdoutReader = readline.createInterface({ input: child.stdout });
  stdoutReader.on('line', function handleStdout(line) {
    const trimmed = line.trim();
    if (!trimmed || !trimmed.startsWith('{')) {
      return;
    }

    let payload;
    try {
      payload = JSON.parse(trimmed);
    } catch {
      return;
    }

    const handler = pending.get(payload.id);
    if (!handler) {
      return;
    }
    clearTimeout(handler.timer);
    pending.delete(payload.id);
    handler.resolve(payload);
  });

  const stderrReader = readline.createInterface({ input: child.stderr });
  stderrReader.on('line', function handleStderr() {
    // Drain stderr so the child process cannot block on a full pipe.
  });

  child.once('exit', function handleExit(code, signal) {
    closed = true;
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer);
      reject(
        new Error(
          `MCP session exited before response (code=${code ?? 'null'}, signal=${signal ?? 'null'})`,
        ),
      );
    }
    pending.clear();
  });

  function callTool(name, argumentsObject) {
    if (closed) {
      throw new Error('MCP session already closed');
    }

    const id = nextId++;
    const message = JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: {
        name,
        arguments: argumentsObject,
      },
    });

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timed out waiting for MCP response for tool '${name}'`));
      }, requestTimeoutMs);

      pending.set(id, { resolve, reject, timer });
      child.stdin.write(`${message}\n`, function handleWrite(error) {
        if (!error) {
          return;
        }
        clearTimeout(timer);
        pending.delete(id);
        reject(error);
      });
    });
  }

  async function close() {
    if (closed) {
      return;
    }

    await closeChildProcess(child);
  }

  return {
    callTool,
    close,
  };
}

async function runTool(session, name, argumentsObject) {
  const startedMs = nowMs();
  const response = await session.callTool(name, argumentsObject);
  return {
    elapsed_ms: Number((nowMs() - startedMs).toFixed(1)),
    payload: parseToolPayload(response),
  };
}

function sampleFunctionNames(candidate) {
  const sampleEvidence = Array.isArray(candidate.evidence)
    ? candidate.evidence.find(function findEvidence(entry) {
        return typeof entry === 'string' && entry.startsWith('sample dead functions: ');
      })
    : null;
  if (!sampleEvidence) {
    return [];
  }

  return sampleEvidence
    .replace('sample dead functions: ', '')
    .split(',')
    .map(function trimName(entry) {
      return entry.trim();
    })
    .filter(Boolean);
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function snippetWindow(lines, lineIndex, radius) {
  const start = Math.max(0, lineIndex - radius);
  const end = Math.min(lines.length, lineIndex + radius + 1);
  return lines
    .slice(start, end)
    .map(function mapLine(line, offset) {
      return `${start + offset + 1}: ${line}`;
    })
    .join('\n');
}

async function buildFileContext(repoPath, relativeFilePath, symbols) {
  const absoluteFilePath = path.join(repoPath, relativeFilePath);
  if (!existsSync(absoluteFilePath)) {
    return {
      file_path: relativeFilePath,
      snippets: [],
      note: 'file_missing',
    };
  }

  const source = await readFile(absoluteFilePath, 'utf8');
  const lines = source.split('\n');
  const snippets = [];

  for (const symbol of symbols) {
    const matcher = new RegExp(
      `\\b(function|const|let|var|async function|class)\\s+${escapeRegex(symbol)}\\b|\\b${escapeRegex(symbol)}\\s*[:=]\\s*\\(`,
    );
    const lineIndex = lines.findIndex(function findLine(line) {
      return matcher.test(line);
    });
    if (lineIndex < 0) {
      continue;
    }
    snippets.push({
      symbol,
      line: lineIndex + 1,
      excerpt: snippetWindow(lines, lineIndex, 3),
    });
  }

  return {
    file_path: relativeFilePath,
    snippets,
    note: snippets.length === 0 ? 'symbol_not_located' : null,
  };
}

async function collectDeadPrivateCandidates(repoPath, findingsLimit) {
  if (!existsSync(sentruxBin)) {
    throw new Error(`Missing sentrux binary at ${sentruxBin}. Build it first.`);
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-dead-private-'));
  const homeOverride = await prepareTypeScriptBenchmarkHome({ tempRoot });
  const session = createSession(sentruxBin, homeOverride);

  try {
    await runTool(session, 'scan', { path: repoPath });
    const findings = await runTool(session, 'findings', { limit: findingsLimit });
    const payload = findings.payload;
    const selectedCandidates = selectDeadPrivateCandidatesFromPayload(payload);

    return {
      findings,
      candidates: selectedCandidates.candidates,
      candidate_source_lane: selectedCandidates.source_lane,
      candidate_source_lane_count: selectedCandidates.source_lane_count,
      candidate_source_lanes_considered: selectedCandidates.considered_lanes,
      candidate_reviewer_lane_status: selectedCandidates.reviewer_lane_status,
      candidate_reviewer_lane_reason: selectedCandidates.reviewer_lane_reason,
      candidate_canonical_count: selectedCandidates.canonical_candidate_count,
      candidate_legacy_count: selectedCandidates.legacy_candidate_count,
      candidate_overlap_count: selectedCandidates.overlapping_candidate_count,
      candidate_legacy_only_count: selectedCandidates.legacy_only_candidate_count,
      candidate_legacy_only_scopes: selectedCandidates.legacy_only_candidates.map(
        deadPrivateCandidateScope,
      ),
      temp_root: tempRoot,
    };
  } finally {
    await session.close();
  }
}

async function buildReviewedCandidate(repoPath, candidate) {
  const filePath = deadPrivateCandidateScope(candidate);
  const scope = candidate.scope ?? filePath;
  const symbols = sampleFunctionNames(candidate);

  return {
    scope,
    file_path: filePath,
    summary: candidate.summary ?? '',
    impact: candidate.impact ?? '',
    evidence: Array.isArray(candidate.evidence) ? candidate.evidence : [],
    role_tags: Array.isArray(candidate.role_tags) ? candidate.role_tags : [],
    leverage_class: candidate.leverage_class ?? null,
    trust_tier: candidate.trust_tier ?? null,
    severity: candidate.severity ?? null,
    score_0_10000: candidate.score_0_10000 ?? null,
    metrics: candidate.metrics ?? null,
    snippets: await buildFileContext(repoPath, filePath, symbols),
  };
}

async function buildReviewPayload(options, candidates) {
  const limitedCandidates = candidates.slice(0, options.limit);
  const reviewedCandidates = [];

  for (const candidate of limitedCandidates) {
    reviewedCandidates.push(await buildReviewedCandidate(options.repoRoot, candidate));
  }

  return reviewedCandidates;
}

function buildReviewPrompt(options, reviewedCandidates) {
  const lines = [
    `Repository: ${options.repoName}`,
    `Repository root: ${options.repoRoot}`,
    'Task: review Sentrux dead_private_code_cluster candidates.',
    '',
    'Judge each candidate against the evidence below.',
    'Prefer rejecting false positives and ambiguous cases over optimistic acceptance.',
    'Use these verdicts only:',
    '- accept',
    '- acceptable_watchpoint_only',
    '- reject_false_positive',
    '- reject_too_ambiguous',
    '',
    'Canonical source lane: experimental_debt_signals (legacy fallback: experimental_findings only when the canonical lane is empty).',
    `Reviewer queue lane: ${options.candidateSourceLane ?? 'none'} (${options.reviewedCandidateCount ?? reviewedCandidates.length} candidate(s) queued).`,
    `Lane status: ${options.candidateReviewerLaneStatus ?? 'unknown'}.`,
    `Lane rule: ${options.candidateReviewerLaneReason ?? 'unknown'}.`,
    `Canonical dead-private candidates: ${options.canonicalCandidateCount ?? 0}.`,
    `Legacy dead-private candidates: ${options.legacyCandidateCount ?? 0}.`,
    `Legacy-only watchlist candidates excluded from this review queue: ${options.legacyOnlyCandidateCount ?? 0}.`,
    options.legacyOnlyCandidateScopes?.length
      ? `Legacy-only watchlist scopes: ${options.legacyOnlyCandidateScopes.join(', ')}`
      : 'Legacy-only watchlist scopes: none',
    '',
    'Candidates:',
    JSON.stringify(reviewedCandidates, null, 2),
  ];

  return lines.join('\n');
}

async function writeJson(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    console.log(usage());
    return;
  }

  const startedAt = nowIso();
  const findings = await collectDeadPrivateCandidates(
    options.repoRoot,
    options.findingsLimit,
  );
  try {
    const reviewedCandidates = await buildReviewPayload(options, findings.candidates);
    const prompt = buildReviewPrompt(
      buildPromptOptions(options, findings, reviewedCandidates),
      reviewedCandidates,
    );
    const resultBase = buildReviewResultBase(options, startedAt, findings);

    if (reviewedCandidates.length === 0) {
      const emptyResult = {
        ...resultBase,
        finished_at: nowIso(),
        summary: 'No dead_private_code_cluster candidates were available for review.',
        reviewed_candidate_count: 0,
        candidates: [],
        provider_output: null,
      };
      await writeJson(options.outputPath, emptyResult);
      console.log(options.outputPath);
      return;
    }

    if (options.dryRun) {
      const dryRunResult = {
        ...resultBase,
        finished_at: nowIso(),
        prompt,
        reviewed_candidate_count: reviewedCandidates.length,
        candidates: reviewedCandidates,
        provider_output: null,
      };
      await writeJson(options.outputPath, dryRunResult);
      console.log(options.outputPath);
      return;
    }

    const providerOutput = await runClaudeCode({
      cwd: options.repoRoot,
      prompt,
      model: options.model,
      jsonSchema: DEAD_PRIVATE_REVIEW_SCHEMA,
      appendSystemPrompt: [
        'You are reviewing Sentrux dead_private candidates.',
        'Return only JSON matching the schema.',
        'Do not edit files.',
        'Reject false positives and ambiguous candidates aggressively.',
      ].join(' '),
      claudeBin: options.claudeBin,
      timeoutMs: requestTimeoutMs,
    });

    const result = {
      ...resultBase,
      finished_at: nowIso(),
      reviewed_candidate_count: reviewedCandidates.length,
      candidates: reviewedCandidates,
      provider_output: providerOutput,
    };
    await writeJson(options.outputPath, result);
    console.log(options.outputPath);
  } finally {
    if (findings.temp_root) {
      await rm(findings.temp_root, { recursive: true, force: true });
    }
  }
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url && !process.env.NODE_TEST_CONTEXT) {
  main().catch(function handleError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
