#!/usr/bin/env node

import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
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
    child.stdin.end();
    await new Promise((resolve) => {
      child.once('exit', resolve);
    });
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
    const candidates = [];

    for (const fieldName of ['experimental_debt_signals', 'experimental_findings']) {
      const values = Array.isArray(payload[fieldName]) ? payload[fieldName] : [];
      for (const value of values) {
        if (value?.kind !== 'dead_private_code_cluster') {
          continue;
        }
        candidates.push(value);
      }
    }

    const deduped = new Map();
    for (const candidate of candidates) {
      const key = `${candidate.scope ?? candidate.files?.[0] ?? 'unknown'}:${candidate.kind}`;
      if (!deduped.has(key)) {
        deduped.set(key, candidate);
      }
    }

    return {
      findings,
      candidates: [...deduped.values()],
      temp_root: tempRoot,
    };
  } finally {
    await session.close();
  }
}

async function buildReviewPayload(options, candidates) {
  const limitedCandidates = candidates.slice(0, options.limit);
  const reviewedCandidates = [];

  for (const candidate of limitedCandidates) {
    const filePath = candidate.scope ?? candidate.files?.[0] ?? 'unknown';
    const scope = candidate.scope ?? filePath;
    const symbols = sampleFunctionNames(candidate);
    reviewedCandidates.push({
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
      snippets: await buildFileContext(options.repoRoot, filePath, symbols),
    });
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
    const prompt = buildReviewPrompt(options, reviewedCandidates);

    if (reviewedCandidates.length === 0) {
      const emptyResult = {
        started_at: startedAt,
        finished_at: nowIso(),
        repo_name: options.repoName,
        repo_root: options.repoRoot,
        task_kind: 'dead_private_review',
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
        started_at: startedAt,
        finished_at: nowIso(),
        repo_name: options.repoName,
        repo_root: options.repoRoot,
        task_kind: 'dead_private_review',
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
      started_at: startedAt,
      finished_at: nowIso(),
      repo_name: options.repoName,
      repo_root: options.repoRoot,
      task_kind: 'dead_private_review',
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

main().catch(function handleError(error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
