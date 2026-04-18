import { runClaudeCode } from '../../../evals/providers/claude-code.mjs';
import { runCodexExec } from '../../../evals/providers/codex-cli.mjs';
import {
  resolveMiniMaxBaseUrl,
  runMiniMaxOpenAI,
} from '../../../evals/providers/minimax-openai.mjs';
import { nowIso } from '../common.mjs';
import { fail } from '../common.mjs';

async function runProvider(options) {
  if (options.provider === 'claude-code') {
    return runClaudeCode({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      jsonSchema: options.jsonSchema,
      appendSystemPrompt: options.appendSystemPrompt,
      timeoutMs: options.timeoutMs,
      claudeBin: options.claudeBin,
    });
  }

  if (options.provider === 'codex-cli') {
    return runCodexExec({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      jsonSchema: options.jsonSchema,
      timeoutMs: options.timeoutMs,
      codexBin: options.codexBin,
    });
  }

  if (options.provider === 'minimax-openai') {
    return runMiniMaxOpenAI({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      jsonSchema: options.jsonSchema,
      appendSystemPrompt: options.appendSystemPrompt,
      timeoutMs: options.timeoutMs,
    });
  }

  fail(`Unsupported provider: ${options.provider}`);
}

function dryRunExecutable(options) {
  if (options.provider === 'codex-cli') {
    return options.codexBin;
  }

  if (options.provider === 'minimax-openai') {
    return `${resolveMiniMaxBaseUrl()}/chat/completions`;
  }

  return options.claudeBin;
}

function buildDryRunProviderOutput(options, repoRoot) {
  return {
    provider: options.provider,
    provider_version: null,
    command: {
      executable: dryRunExecutable(options),
      args: [],
    },
    cwd: repoRoot,
    started_at: nowIso(),
    duration_ms: 0,
    exit_code: 0,
    signal: null,
    timed_out: false,
    stdout: '',
    stderr: '',
    stdout_json: null,
    stdout_jsonl: [],
    last_message: null,
  };
}

export { buildDryRunProviderOutput, runProvider };
