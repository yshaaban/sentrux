import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import test from "node:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { analyzeProject } from "../dist/analysis.js";
import {
  ExhaustivenessFallbackKind,
  ExhaustivenessProofKind,
  ExhaustivenessSiteSemanticRole,
  ExhaustivenessSiteKind,
  TransitionKind,
} from "../dist/types.js";
import {
  dispatchRequest,
  PROTOCOL_VERSION,
  toProjectModel,
  toRequest,
} from "../dist/protocol.js";
import { parseHeaders } from "../dist/transport.js";

function encodeMessage(message) {
  const body = JSON.stringify(message);
  return `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n${body}`;
}

function decodeMessages(buffer) {
  const messages = [];
  let remaining = buffer;

  while (true) {
    const headerEnd = remaining.indexOf("\r\n\r\n");
    if (headerEnd < 0) {
      break;
    }

    const headerText = remaining.subarray(0, headerEnd).toString("utf8");
    const headers = parseHeaders(headerText);
    const contentLength = Number(headers.get("content-length"));
    if (!Number.isFinite(contentLength) || contentLength < 0) {
      throw new Error(`Invalid content length in response header: ${headers.get("content-length")}`);
    }

    const messageStart = headerEnd + 4;
    const messageEnd = messageStart + contentLength;
    if (remaining.length < messageEnd) {
      break;
    }

    const body = remaining.subarray(messageStart, messageEnd).toString("utf8");
    messages.push(JSON.parse(body));
    remaining = remaining.subarray(messageEnd);
  }

  return { messages, remaining };
}

async function createSampleProject() {
  const root = await mkdtemp(path.join(os.tmpdir(), "ts-bridge-test-"));
  const srcDir = path.join(root, "src");
  await mkdir(srcDir, { recursive: true });

  await writeFile(
    path.join(root, "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "NodeNext",
          moduleResolution: "NodeNext",
          jsx: "preserve",
          strict: true,
          skipLibCheck: true,
        },
        include: ["src/**/*.ts", "src/**/*.tsx"],
      },
      null,
      2,
    ),
  );

  await writeFile(
    path.join(srcDir, "domain.ts"),
    [
      'export type Mode = "idle" | "running";',
      'export type ActionKind = "open" | "close" | "archive";',
      'export type LifecycleState = "draft" | "in-review" | "approved";',
      "",
    ].join("\n"),
  );

  await writeFile(
    path.join(srcDir, "index.ts"),
    [
      'import type { ActionKind, LifecycleState, Mode } from "./domain.js";',
      "",
      "export function bump(value: number): number {",
      "  return value + 1;",
      "}",
      "",
      "export function interpret(mode: Mode): number {",
      "  switch (mode) {",
      '    case "idle":',
      "      return 1;",
      '    case "running":',
      "      return 2;",
      "    default:",
      "      return assertNever(mode);",
      "  }",
      "}",
      "",
      "export function rewrite(mode: Mode): Mode {",
      "  switch (mode) {",
      '    case "idle":',
      '      return "running";',
      '    case "running":',
      '      return "idle";',
      "    default:",
      "      return assertNever(mode);",
      "  }",
      "}",
      "",
      "export function flip(mode: Mode): Mode {",
      '  if (mode === "idle") {',
      '    return "running";',
      "  } else {",
      '    return "idle";',
      "  }",
      "}",
      "",
      "export function modeLabel(mode: Mode): string {",
      "  switch (mode) {",
      '    case "idle":',
      '      return "Idle";',
      "    default:",
      '      return "Open evidence";',
      "  }",
      "}",
      "",
      "export function maybeRender(mode: Mode): string | null {",
      "  switch (mode) {",
      '    case "idle":',
      '      return "Idle";',
      "    default:",
      "      return null;",
      "  }",
      "}",
      "",
      "export function maybeValue(mode: Mode): number | undefined {",
      "  switch (mode) {",
      '    case "idle":',
      "      return 1;",
      "    default:",
      "      return undefined;",
      "  }",
      "}",
      "",
      "export function normalizeMode(mode: Mode): Mode {",
      "  switch (mode) {",
      '    case "idle":',
      '      return "running";',
      "    default:",
      "      return mode;",
      "  }",
      "}",
      "",
      "export function modeItems(mode: Mode): string[] {",
      "  switch (mode) {",
      '    case "idle":',
      '      return ["idle"];',
      "    default:",
      "      return [];",
      "  }",
      "}",
      "",
      "export function modePayload(mode: Mode): Record<string, string> {",
      "  switch (mode) {",
      '    case "idle":',
      '      return { mode: "idle" };',
      "    default:",
      "      return {};",
      "  }",
      "}",
      "",
      "export function failMode(mode: Mode): string {",
      "  switch (mode) {",
      '    case "idle":',
      '      return "Idle";',
      "    default:",
      '      throw new Error(`Unexpected mode: ${mode}`);',
      "  }",
      "}",
      "",
      "export function actionLabel(kind: ActionKind): string {",
      '  if (kind === "open") {',
      '    return "Open";',
      '  } else if (kind === "close") {',
      '    return "Close";',
      "  } else {",
      '    return "Open evidence";',
      "  }",
      "}",
      "",
      "export function lifecycleLabel(state: LifecycleState): string {",
      "  switch (state) {",
      '    case "draft":',
      '      return "Draft";',
      "    default:",
      '      return state.replace(/-/g, " ");',
      "  }",
      "}",
      "",
      "function assertNever(value: never): never {",
      '  throw new Error(`Unexpected value: ${value}`);',
      "}",
      "",
      "export const modeNumbers: Record<Mode, number> = {",
      "  idle: 1,",
      "  running: 2,",
      "};",
      "",
      "export const modeLabels = {",
      '  idle: "idle",',
      '  running: "running",',
      "} satisfies Record<Mode, string>;",
      "",
      "export const modeTransitions: Record<Mode, Mode> = {",
      '  idle: "running",',
      '  running: "idle",',
      "};",
      "",
      "export const actionLabels = {",
      '  open: "Open",',
      '  close: "Close",',
      "};",
      "",
      "export const actionTargets = new Map<ActionKind, string>([",
      '  ["open", "/open"],',
      '  ["close", "/close"],',
      "]);",
      "",
    ].join("\n"),
  );

  await writeFile(
    path.join(srcDir, "view.tsx"),
    [
      'import type { Mode } from "./domain.js";',
      "",
      "declare global {",
      "  namespace JSX {",
      "    interface IntrinsicElements {",
      "      div: { children?: unknown };",
      "    }",
      "  }",
      "}",
      "",
      "export function ModeView(props: { mode: Mode }) {",
      '  return props.mode === "idle" ? <div>Idle</div> : null;',
      "}",
      "",
    ].join("\n"),
  );

  return root;
}

async function runBridgeRequests(requests) {
  return runBridgeInput(requests.map(encodeMessage).join(""));
}

async function runBridgeInput(input) {
  const distIndex = path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
    "dist",
    "index.js",
  );
  const child = spawn(process.execPath, [distIndex], {
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = Buffer.alloc(0);
  let stderr = "";
  const messages = [];

  child.stdout.on("data", (chunk) => {
    stdout = Buffer.concat([stdout, chunk]);
    const decoded = decodeMessages(stdout);
    messages.push(...decoded.messages);
    stdout = decoded.remaining;
  });

  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString("utf8");
  });

  child.stdin.write(input);
  child.stdin.end();

  const [code, signal] = await once(child, "exit");
  if (code !== 0) {
    throw new Error(`Bridge exited with code ${code} signal ${signal}\n${stderr}`);
  }

  if (stdout.length > 0) {
    const decoded = decodeMessages(stdout);
    messages.push(...decoded.messages);
    stdout = decoded.remaining;
  }

  return { messages, stderr };
}

async function runBridgeWithClosedStdout(input) {
  const distIndex = path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
    "dist",
    "index.js",
  );
  const child = spawn(process.execPath, [distIndex], {
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString("utf8");
  });

  child.stdout.destroy();
  child.stdin.end(input);

  const [code, signal] = await once(child, "exit");
  return { code, signal, stderr };
}

test("toProjectModel validates bridge inputs", function () {
  const project = toProjectModel({
    root: "/repo",
    tsconfig_paths: ["tsconfig.json"],
    workspace_files: ["src/index.ts"],
    fingerprint: "abc123",
    detected_archetypes: [
      {
        id: "node-app",
        confidence: "high",
        reasons: ["package.json"],
      },
    ],
  });

  assert.deepEqual(project, {
    root: "/repo",
    tsconfig_paths: ["tsconfig.json"],
    workspace_files: ["src/index.ts"],
    primary_language: null,
    fingerprint: "abc123",
    repo_archetype: null,
    detected_archetypes: [
      {
        id: "node-app",
        confidence: "high",
        reasons: ["package.json"],
      },
    ],
  });
  assert.equal(toProjectModel({ root: "/repo" }), null);
});

test("toRequest rejects invalid JSON-RPC ids", function () {
  assert.equal(
    toRequest({
      jsonrpc: "2.0",
      id: { nested: true },
      method: "ping",
    }),
    null,
  );

  assert.deepEqual(
    toRequest({
      jsonrpc: "2.0",
      id: 1,
      method: "ping",
    }),
    {
      jsonrpc: "2.0",
      id: 1,
      method: "ping",
      params: undefined,
    },
  );
});

test("parseHeaders reads Content-Length framing headers", function () {
  const headers = parseHeaders("Content-Length: 42\r\nX-Bridge: yes");
  assert.equal(headers.get("content-length"), "42");
  assert.equal(headers.get("x-bridge"), "yes");
});

test("dispatchRequest returns protocol responses without side effects", function () {
  const initialize = dispatchRequest(
    { jsonrpc: "2.0", id: 1, method: "initialize" },
    {
      analyzeProject() {
        throw new Error("should not analyze on initialize");
      },
    },
  );
  assert.equal(initialize.kind, "response");
  assert.equal(initialize.response.result.protocolVersion, PROTOCOL_VERSION);

  const analyze = dispatchRequest(
    {
      jsonrpc: "2.0",
      id: 2,
      method: "analyze_projects",
      params: {
        root: "/repo",
        tsconfig_paths: ["tsconfig.json"],
        workspace_files: ["src/index.ts"],
        fingerprint: "abc123",
        detected_archetypes: [],
      },
    },
    {
      analyzeProject(project) {
        assert.equal(project.root, "/repo");
        return { ok: true };
      },
    },
  );
  assert.equal(analyze.kind, "response");
  assert.deepEqual(analyze.response.result, { ok: true });

  const invalid = dispatchRequest(
    { jsonrpc: "2.0", id: 3, method: "analyze_projects", params: { root: "/repo" } },
    {
      analyzeProject() {
        throw new Error("should not reach analysis");
      },
    },
  );
  assert.equal(invalid.kind, "response");
  assert.equal(invalid.response.error.code, -32602);

  const unknown = dispatchRequest(
    { jsonrpc: "2.0", id: 4, method: "nope" },
    {
      analyzeProject() {
        throw new Error("should not reach analysis");
      },
    },
  );
  assert.equal(unknown.kind, "response");
  assert.equal(unknown.response.error.code, -32601);

  const exit = dispatchRequest(
    { jsonrpc: "2.0", id: 5, method: "exit" },
    {
      analyzeProject() {
        throw new Error("should not reach analysis");
      },
    },
  );
  assert.equal(exit.kind, "exit");
  assert.equal(exit.response?.result, null);
});

test("analyzeProject extracts semantic facts from a sample project", async function () {
  const root = await createSampleProject();
  try {
    const snapshot = analyzeProject({
      root,
      tsconfig_paths: ["tsconfig.json"],
      workspace_files: ["src/index.ts"],
      fingerprint: "bridge-test",
      detected_archetypes: [],
    });

    assert.equal(snapshot.project.root, root);
    assert.equal(snapshot.analyzed_files, 3);
    assert(snapshot.capabilities.includes("ClosedDomains"));
    assert(snapshot.files.some((file) => file.path === "src/index.ts"));
    assert(snapshot.symbols.some((symbol) => symbol.name === "bump"));
    assert(snapshot.closed_domains.some((domain) => domain.symbol_name === "Mode"));
    const modeDomain = snapshot.closed_domains.find((domain) => domain.symbol_name === "Mode");
    assert(modeDomain);
    assert.equal(modeDomain.defining_file, "src/domain.ts");
    const modeSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "Mode" &&
        site.site_kind === ExhaustivenessSiteKind.Switch,
    );
    assert(modeSite);
    assert.equal(modeSite.defining_file, "src/domain.ts");
    assert.equal(modeSite.fallback_kind, ExhaustivenessFallbackKind.AssertThrow);
    assert.equal(modeSite.site_expression, "mode");
    assert.equal(modeSite.site_confidence, 1);
    const fallbackKinds = new Set(
      snapshot.closed_domain_sites.map((site) => site.fallback_kind),
    );
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.Null));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.Undefined));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.GenericString));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.IdentityTransform));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.EmptyArray));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.EmptyObject));
    assert(fallbackKinds.has(ExhaustivenessFallbackKind.AssertThrow));
    assert(
      snapshot.closed_domain_sites.some(
        (site) =>
          site.fallback_kind === ExhaustivenessFallbackKind.GenericString &&
          site.site_semantic_role === ExhaustivenessSiteSemanticRole.Label,
      ),
    );
    const actionIfSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "ActionKind" &&
        site.site_expression === "kind" &&
        site.fallback_kind === ExhaustivenessFallbackKind.GenericString,
    );
    assert(actionIfSite);
    assert.deepEqual(actionIfSite.covered_variants, ["open", "close"]);
    assert.equal(actionIfSite.site_semantic_role, ExhaustivenessSiteSemanticRole.Label);
    assert(actionIfSite.site_confidence >= 0.8);
    const lifecycleLabelSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "LifecycleState" &&
        site.site_expression === "state",
    );
    assert(lifecycleLabelSite);
    assert.deepEqual(lifecycleLabelSite.covered_variants, ["draft"]);
    assert.equal(
      lifecycleLabelSite.fallback_kind,
      ExhaustivenessFallbackKind.IdentityTransform,
    );
    assert.equal(
      lifecycleLabelSite.site_semantic_role,
      ExhaustivenessSiteSemanticRole.Label,
    );
    const inferredActionMapSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "ActionKind" &&
        site.site_expression === "actionLabels",
    );
    assert(inferredActionMapSite);
    assert.deepEqual(inferredActionMapSite.covered_variants, ["open", "close"]);
    assert.equal(inferredActionMapSite.fallback_kind, ExhaustivenessFallbackKind.None);
    assert.equal(inferredActionMapSite.site_semantic_role, ExhaustivenessSiteSemanticRole.Label);
    assert(inferredActionMapSite.site_confidence >= 0.7);
    const actionTargetMapSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "ActionKind" &&
        site.site_expression === "actionTargets",
    );
    assert(actionTargetMapSite);
    assert.deepEqual(actionTargetMapSite.covered_variants, ["open", "close"]);
    assert.equal(actionTargetMapSite.site_semantic_role, ExhaustivenessSiteSemanticRole.Target);
    assert(actionTargetMapSite.site_confidence >= 0.9);
    const modeRenderSite = snapshot.closed_domain_sites.find(
      (site) =>
        site.domain_symbol_name === "Mode" &&
        site.site_expression === "props.mode" &&
        site.site_semantic_role === ExhaustivenessSiteSemanticRole.Render,
    );
    assert(modeRenderSite);
    assert.equal(modeRenderSite.site_kind, ExhaustivenessSiteKind.IfElse);
    assert.deepEqual(modeRenderSite.covered_variants, ["idle"]);
    assert.equal(modeRenderSite.fallback_kind, ExhaustivenessFallbackKind.Null);
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.site_kind === ExhaustivenessSiteKind.Switch,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.site_kind === ExhaustivenessSiteKind.IfElse,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.site_kind === ExhaustivenessSiteKind.Record,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.site_kind === ExhaustivenessSiteKind.Satisfies,
      ),
    );
    assert(
      !snapshot.closed_domain_sites.some(
        (site) =>
          site.site_kind === ExhaustivenessSiteKind.Record &&
          site.site_expression === "modeLabels",
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.proof_kind === ExhaustivenessProofKind.AssertNever,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.proof_kind === ExhaustivenessProofKind.Record,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.proof_kind === ExhaustivenessProofKind.Satisfies,
      ),
    );
    assert(
      snapshot.closed_domain_sites.some(
        (site) => site.proof_kind === ExhaustivenessProofKind.IfElse,
      ),
    );
    assert(
      snapshot.transition_sites.some(
        (site) => site.transition_kind === TransitionKind.RecordEntry,
      ),
    );
    assert(
      snapshot.transition_sites.some(
        (site) => site.transition_kind === TransitionKind.SwitchCase,
      ),
    );
    assert(
      snapshot.transition_sites.some(
        (site) => site.transition_kind === TransitionKind.IfBranch,
      ),
    );
    assert(
      snapshot.transition_sites.some(
        (site) => site.transition_kind === TransitionKind.IfElse,
      ),
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("bridge responds over stdio using Content-Length framing", async function () {
  const root = await createSampleProject();
  try {
    const { messages, stderr } = await runBridgeRequests([
      {
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
      },
      {
        jsonrpc: "2.0",
        id: 2,
        method: "analyze_projects",
        params: {
          root,
          tsconfig_paths: ["tsconfig.json"],
          workspace_files: ["src/index.ts"],
          fingerprint: "bridge-test",
          detected_archetypes: [],
        },
      },
    ]);

    assert.equal(stderr, "");
    assert.equal(messages.length, 2);
    assert.equal(messages[0].result.protocolVersion, PROTOCOL_VERSION);
    assert.equal(messages[1].result.analyzed_files, 3);
    assert(messages[1].result.symbols.some((symbol) => symbol.name === "interpret"));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("bridge returns Invalid Request for malformed JSON-RPC payloads", async function () {
  const { messages, stderr } = await runBridgeInput(
    encodeMessage({
      jsonrpc: "1.0",
      id: 1,
      method: "initialize",
    }),
  );

  assert.equal(stderr, "");
  assert.equal(messages.length, 1);
  assert.equal(messages[0].error.code, -32600);
});

test("bridge returns Parse error for invalid JSON bodies", async function () {
  const rawBody = '{"jsonrpc":"2.0","id":1,"method":"initialize"';
  const malformedMessage = `Content-Length: ${Buffer.byteLength(rawBody, "utf8")}\r\n\r\n${rawBody}`;
  const { messages, stderr } = await runBridgeInput(malformedMessage);

  assert.equal(stderr, "");
  assert.equal(messages.length, 1);
  assert.equal(messages[0].error.code, -32700);
});

test("bridge exits cleanly when stdout closes before it can write a response", async function () {
  const { code, signal, stderr } = await runBridgeWithClosedStdout(
    encodeMessage({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
    }),
  );

  assert.equal(signal, null);
  assert.equal(code, 0);
  assert.equal(stderr, "");
});
