import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildMiniMaxOpenAIRequest,
  buildSchemaInstruction,
  buildSystemPrompt,
  DEFAULT_MINIMAX_BASE_URL,
  DEFAULT_MINIMAX_MODEL,
  parseStructuredResponseText,
  resolveMiniMaxApiKey,
  resolveMiniMaxBaseUrl,
  runMiniMaxOpenAI,
  shouldRetryWithoutResponseFormat,
} from '../evals/providers/minimax-openai.mjs';

const PUBLIC_SAFE_CWD = process.cwd();

test('buildMiniMaxOpenAIRequest uses structured json-schema output with deterministic defaults', function () {
  const requestBody = buildMiniMaxOpenAIRequest({
    prompt: 'return a json object',
    appendSystemPrompt: 'respond with json only',
    jsonSchema: {
      type: 'object',
      required: ['task_kind'],
      properties: {
        task_kind: { const: 'bounded_adjudication' },
      },
    },
  });

  assert.equal(requestBody.model, DEFAULT_MINIMAX_MODEL);
  assert.equal(requestBody.temperature, 0);
  assert.deepEqual(requestBody.messages, [
    {
      role: 'system',
      content:
        'respond with json only\n\nReturn only a valid JSON object that matches this schema exactly.\n{"type":"object","required":["task_kind"],"properties":{"task_kind":{"const":"bounded_adjudication"}}}',
    },
    {
      role: 'user',
      content: 'return a json object',
    },
  ]);
  assert.equal(requestBody.response_format.type, 'json_schema');
  assert.equal(requestBody.response_format.json_schema.strict, true);
});

test('buildSystemPrompt appends schema instructions after any caller-supplied system prompt', function () {
  assert.equal(
    buildSchemaInstruction({ type: 'object' }),
    'Return only a valid JSON object that matches this schema exactly.\n{"type":"object"}',
  );
  assert.equal(
    buildSystemPrompt('reply in json', { type: 'object' }),
    'reply in json\n\nReturn only a valid JSON object that matches this schema exactly.\n{"type":"object"}',
  );
});

test('parseStructuredResponseText strips think tags before parsing json', function () {
  const parsed = parseStructuredResponseText(
    '<think>reason about the bundle</think>{"task_kind":"bounded_adjudication","summary":"keep"}',
  );

  assert.deepEqual(parsed, {
    task_kind: 'bounded_adjudication',
    summary: 'keep',
  });
});

test('resolveMiniMax env helpers prefer explicit MiniMax env vars and trim trailing slashes', function () {
  const env = {
    MINIMAX_API_KEY: ' mmk ',
    OPENAI_API_KEY: ' ignored ',
    MINIMAX_BASE_URL: 'https://api.minimax.io/v1/',
    OPENAI_BASE_URL: 'https://example.invalid/v1/',
  };

  assert.equal(resolveMiniMaxApiKey(env), 'mmk');
  assert.equal(resolveMiniMaxBaseUrl(env), DEFAULT_MINIMAX_BASE_URL);
});

test('shouldRetryWithoutResponseFormat only retries likely schema-format compatibility failures', function () {
  assert.equal(
    shouldRetryWithoutResponseFormat(
      { ok: false, status: 400 },
      'unsupported response_format json_schema',
      { response_format: { type: 'json_schema' } },
    ),
    true,
  );
  assert.equal(
    shouldRetryWithoutResponseFormat(
      { ok: false, status: 500 },
      'unsupported response_format json_schema',
      { response_format: { type: 'json_schema' } },
    ),
    false,
  );
  assert.equal(
    shouldRetryWithoutResponseFormat(
      { ok: false, status: 400 },
      'rate limit exceeded',
      { response_format: { type: 'json_schema' } },
    ),
    false,
  );
});

test('runMiniMaxOpenAI parses a structured response payload from the OpenAI-compatible endpoint', async function () {
  const calls = [];
  const fetchImpl = async function mockFetch(url, init) {
    calls.push({
      url,
      init,
    });

    return {
      ok: true,
      status: 200,
      text: async function text() {
        return JSON.stringify({
          choices: [
            {
              message: {
                content:
                  '<think>trace the evidence</think>{"task_kind":"bounded_adjudication","bundle_id":"bundle-1","repo_name":"parallel-code","decision":{"verdict":"keep","ranking_action":"hold_position","summary":"keep it"},"cited_evidence_ids":["e1"],"confidence_0_1":0.8,"audit":{"structured_evidence_only":true,"requires_human_review":false,"auto_apply_eligible":false}}',
              },
            },
          ],
        });
      },
    };
  };

  const result = await runMiniMaxOpenAI({
    cwd: PUBLIC_SAFE_CWD,
    prompt: 'review the evidence bundle',
    jsonSchema: {
      type: 'object',
    },
    env: {
      MINIMAX_API_KEY: 'test-key',
    },
    fetchImpl,
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${DEFAULT_MINIMAX_BASE_URL}/chat/completions`);
  assert.equal(result.provider, 'minimax-openai');
  assert.equal(result.exit_code, 0);
  assert.equal(result.timed_out, false);
  assert.equal(result.stdout_json.task_kind, 'bounded_adjudication');
  assert.equal(result.stdout_json.bundle_id, 'bundle-1');
});

test('runMiniMaxOpenAI retries once without response_format when the endpoint rejects json_schema formatting', async function () {
  const requestBodies = [];
  const fetchImpl = async function mockFetch(url, init) {
    requestBodies.push(JSON.parse(init.body));

    if (requestBodies.length === 1) {
      return {
        ok: false,
        status: 400,
        statusText: 'Bad Request',
        text: async function text() {
          return JSON.stringify({
            error: {
              message: 'unsupported response_format json_schema',
            },
          });
        },
      };
    }

    return {
      ok: true,
      status: 200,
      text: async function text() {
        return JSON.stringify({
          choices: [
            {
              message: {
                content:
                  '{"task_kind":"bounded_adjudication","bundle_id":"bundle-2","repo_name":"parallel-code","decision":{"verdict":"keep","ranking_action":"hold_position","summary":"fallback ok"},"cited_evidence_ids":["e1"],"confidence_0_1":0.7,"audit":{"structured_evidence_only":true,"requires_human_review":false,"auto_apply_eligible":false}}',
              },
            },
          ],
        });
      },
    };
  };

  const result = await runMiniMaxOpenAI({
    cwd: PUBLIC_SAFE_CWD,
    prompt: 'review the evidence bundle',
    jsonSchema: {
      type: 'object',
    },
    env: {
      MINIMAX_API_KEY: 'test-key',
    },
    fetchImpl,
  });

  assert.equal(requestBodies.length, 2);
  assert.equal(Boolean(requestBodies[0].response_format), true);
  assert.equal(Boolean(requestBodies[1].response_format), false);
  assert.equal(result.exit_code, 0);
  assert.equal(result.stdout_json.bundle_id, 'bundle-2');
});
