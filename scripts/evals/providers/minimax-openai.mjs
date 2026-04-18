import { existsSync } from 'node:fs';

import { nowMs } from '../../lib/eval-runtime/common.mjs';

const DEFAULT_MINIMAX_BASE_URL = 'https://api.minimax.io/v1';
const DEFAULT_MINIMAX_MODEL = 'MiniMax-M2.7';
const INVALID_RESPONSE_MESSAGE = 'MiniMax response did not contain a valid JSON object';

function parseJsonMaybe(text) {
  if (typeof text !== 'string') {
    return null;
  }

  const trimmed = text.trim();
  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function stripThinkTags(text) {
  return text.replace(/<think>[\s\S]*?<\/think>/g, '').trim();
}

function extractJsonObjectText(text) {
  const stripped = stripThinkTags(text);
  const direct = parseJsonMaybe(stripped);
  if (direct) {
    return stripped;
  }

  const firstBrace = stripped.indexOf('{');
  const lastBrace = stripped.lastIndexOf('}');
  if (firstBrace === -1 || lastBrace === -1 || lastBrace <= firstBrace) {
    return null;
  }

  return stripped.slice(firstBrace, lastBrace + 1);
}

function parseStructuredResponseText(text) {
  const candidate = extractJsonObjectText(text);
  if (!candidate) {
    return null;
  }

  return parseJsonMaybe(candidate);
}

function normalizeBaseUrl(baseUrl) {
  return String(baseUrl ?? DEFAULT_MINIMAX_BASE_URL).replace(/\/+$/, '');
}

function resolveMiniMaxBaseUrl(env = process.env) {
  return normalizeBaseUrl(
    env.MINIMAX_BASE_URL ?? env.OPENAI_BASE_URL ?? DEFAULT_MINIMAX_BASE_URL,
  );
}

function resolveMiniMaxApiKey(env = process.env) {
  const apiKey = env.MINIMAX_API_KEY ?? env.OPENAI_API_KEY ?? null;
  return normalizeOptionalString(apiKey);
}

function normalizeOptionalString(value) {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  return trimmed;
}

function normalizeMessageContent(content) {
  if (typeof content === 'string') {
    return content;
  }
  if (!Array.isArray(content)) {
    return '';
  }

  return content
    .map(function toText(part) {
      if (typeof part === 'string') {
        return part;
      }
      if (part?.type === 'text' && typeof part.text === 'string') {
        return part.text;
      }

      return '';
    })
    .filter(Boolean)
    .join('\n');
}

function extractResponseText(responseJson) {
  const message = responseJson?.choices?.[0]?.message;
  return normalizeMessageContent(message?.content);
}

function buildSchemaInstruction(jsonSchema) {
  if (!jsonSchema) {
    return null;
  }

  return [
    'Return only a valid JSON object that matches this schema exactly.',
    JSON.stringify(jsonSchema),
  ].join('\n');
}

function buildSystemPrompt(appendSystemPrompt, jsonSchema) {
  const promptParts = [];
  if (typeof appendSystemPrompt === 'string' && appendSystemPrompt.trim()) {
    promptParts.push(appendSystemPrompt.trim());
  }

  const schemaInstruction = buildSchemaInstruction(jsonSchema);
  if (schemaInstruction) {
    promptParts.push(schemaInstruction);
  }

  return promptParts.join('\n\n');
}

function buildMiniMaxOpenAIRequest(options = {}) {
  const messages = [];
  const systemPrompt = buildSystemPrompt(options.appendSystemPrompt, options.jsonSchema);
  if (systemPrompt) {
    messages.push({
      role: 'system',
      content: systemPrompt,
    });
  }
  messages.push({
    role: 'user',
    content: options.prompt,
  });

  const requestBody = {
    model: options.model ?? DEFAULT_MINIMAX_MODEL,
    messages,
    temperature: 0,
  };

  if (options.jsonSchema) {
    requestBody.response_format = {
      type: 'json_schema',
      json_schema: {
        name: 'eval_response',
        strict: true,
        schema: options.jsonSchema,
      },
    };
  }

  return requestBody;
}

function shouldRetryWithoutResponseFormat(response, rawText, requestBody) {
  if (!requestBody.response_format || response.ok) {
    return false;
  }

  if (![400, 404, 415, 422].includes(response.status)) {
    return false;
  }

  const text = String(rawText ?? '').toLowerCase();
  return (
    text.includes('response_format') ||
    text.includes('json_schema') ||
    text.includes('unsupported') ||
    text.includes('invalid parameter')
  );
}

async function performMiniMaxRequest({
  endpoint,
  apiKey,
  requestBody,
  fetchImpl,
  controller,
}) {
  const response = await fetchImpl(endpoint, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify(requestBody),
    signal: controller.signal,
  });
  const rawText = await readResponseText(response);

  return {
    response,
    rawText,
  };
}

async function readResponseText(response) {
  try {
    return await response.text();
  } catch {
    return '';
  }
}

function buildProviderCommand(endpoint) {
  return {
    executable: endpoint,
    args: [],
  };
}

function buildMiniMaxProviderOutput({
  endpoint,
  cwd,
  startedAt,
  durationMs,
  exitCode,
  signal = null,
  timedOut = false,
  stdout = '',
  stderr = '',
  stdoutJson = null,
  rawResponseJson,
}) {
  const providerOutput = {
    provider: 'minimax-openai',
    provider_version: null,
    command: buildProviderCommand(endpoint),
    cwd,
    started_at: startedAt,
    duration_ms: durationMs,
    exit_code: exitCode,
    signal,
    timed_out: timedOut,
    stdout,
    stderr,
    stdout_json: stdoutJson,
  };

  if (rawResponseJson !== undefined) {
    providerOutput.raw_response_json = rawResponseJson;
  }

  return providerOutput;
}

function errorMessage(error) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function exitCodeForError(timedOut) {
  if (timedOut) {
    return 124;
  }

  return 1;
}

function signalForError(timedOut) {
  if (timedOut) {
    return 'SIGABRT';
  }

  return null;
}

export async function runMiniMaxOpenAI(options = {}) {
  const {
    cwd,
    prompt,
    model = DEFAULT_MINIMAX_MODEL,
    jsonSchema = null,
    appendSystemPrompt = null,
    timeoutMs = 30 * 60 * 1000,
    env = process.env,
    fetchImpl = globalThis.fetch,
  } = options;

  if (typeof cwd !== 'string' || !cwd) {
    throw new Error('runMiniMaxOpenAI requires a cwd');
  }
  if (!existsSync(cwd)) {
    throw new Error(`MiniMax provider cwd does not exist: ${cwd}`);
  }
  if (typeof prompt !== 'string' || !prompt.trim()) {
    throw new Error('runMiniMaxOpenAI requires a non-empty prompt');
  }
  if (typeof fetchImpl !== 'function') {
    throw new Error('runMiniMaxOpenAI requires fetch support');
  }

  const apiKey = resolveMiniMaxApiKey(env);
  if (!apiKey) {
    throw new Error('runMiniMaxOpenAI requires MINIMAX_API_KEY or OPENAI_API_KEY');
  }

  const baseUrl = resolveMiniMaxBaseUrl(env);
  const endpoint = `${baseUrl}/chat/completions`;
  const requestBody = buildMiniMaxOpenAIRequest({
    prompt,
    model,
    jsonSchema,
    appendSystemPrompt,
  });

  const startedAt = new Date().toISOString();
  const startedMs = nowMs();
  const controller = new AbortController();
  let timeout = null;
  let timedOut = false;

  if (timeoutMs > 0) {
    timeout = setTimeout(function abortRequest() {
      timedOut = true;
      controller.abort();
    }, timeoutMs);
  }

  try {
    let requestResult = await performMiniMaxRequest({
      endpoint,
      apiKey,
      requestBody,
      fetchImpl,
      controller,
    });
    if (
      shouldRetryWithoutResponseFormat(
        requestResult.response,
        requestResult.rawText,
        requestBody,
      )
    ) {
      const fallbackRequestBody = {
        ...requestBody,
      };
      delete fallbackRequestBody.response_format;
      requestResult = await performMiniMaxRequest({
        endpoint,
        apiKey,
        requestBody: fallbackRequestBody,
        fetchImpl,
        controller,
      });
    }
    const durationMs = Number((nowMs() - startedMs).toFixed(1));
    const { response, rawText } = requestResult;

    if (!response.ok) {
      return buildMiniMaxProviderOutput({
        endpoint,
        cwd,
        startedAt,
        durationMs,
        exitCode: response.status,
        stdout: rawText,
        stderr: response.statusText || `HTTP ${response.status}`,
        stdoutJson: parseJsonMaybe(rawText),
      });
    }

    const responseJson = parseJsonMaybe(rawText);
    const responseText = extractResponseText(responseJson);
    const parsedResult = parseStructuredResponseText(responseText);

    return buildMiniMaxProviderOutput({
      endpoint,
      cwd,
      startedAt,
      durationMs,
      exitCode: 0,
      stdout: responseText,
      stderr: parsedResult ? '' : INVALID_RESPONSE_MESSAGE,
      stdoutJson: parsedResult,
      rawResponseJson: responseJson,
    });
  } catch (error) {
    const durationMs = Number((nowMs() - startedMs).toFixed(1));
    return buildMiniMaxProviderOutput({
      endpoint,
      cwd,
      startedAt,
      durationMs,
      exitCode: exitCodeForError(timedOut),
      signal: signalForError(timedOut),
      timedOut,
      stderr: errorMessage(error),
    });
  } finally {
    if (timeout) {
      clearTimeout(timeout);
    }
  }
}

export {
  DEFAULT_MINIMAX_BASE_URL,
  DEFAULT_MINIMAX_MODEL,
  buildMiniMaxOpenAIRequest,
  buildSchemaInstruction,
  buildSystemPrompt,
  parseStructuredResponseText,
  resolveMiniMaxApiKey,
  resolveMiniMaxBaseUrl,
  shouldRetryWithoutResponseFormat,
  stripThinkTags,
};
