import type {
  JsonRpcId,
  ProjectArchetypeMatch,
  JsonRpcRequest,
  JsonRpcResponse,
  ProjectModel,
  SemanticSnapshot,
} from "./types.js";

export const PROTOCOL_VERSION = "0.1.0";

export interface RequestHandlerDependencies {
  analyzeProject: (project: ProjectModel) => SemanticSnapshot;
}

export type DispatchOutcome =
  | { kind: "ignore" }
  | { kind: "response"; response: JsonRpcResponse }
  | { kind: "exit"; code: number; response?: JsonRpcResponse };

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isStringArray(value: unknown): value is string[] {
  if (!Array.isArray(value)) {
    return false;
  }

  for (const entry of value) {
    if (typeof entry !== "string") {
      return false;
    }
  }

  return true;
}

function isJsonRpcId(value: unknown): value is JsonRpcId {
  return typeof value === "number" || typeof value === "string" || value === null;
}

function readOptionalString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  return value;
}

function parseDetectedArchetypes(value: unknown): ProjectArchetypeMatch[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const archetypes: ProjectArchetypeMatch[] = [];
  for (const entry of value) {
    if (!isObject(entry)) {
      continue;
    }
    if (
      typeof entry.id !== "string" ||
      typeof entry.confidence !== "string" ||
      !isStringArray(entry.reasons)
    ) {
      continue;
    }

    archetypes.push({
      id: entry.id,
      confidence: entry.confidence,
      reasons: entry.reasons,
    });
  }

  return archetypes;
}

export function toRequest(value: unknown): JsonRpcRequest | null {
  if (!isObject(value)) {
    return null;
  }

  if ("id" in value && !isJsonRpcId(value.id)) {
    return null;
  }

  const id = "id" in value && isJsonRpcId(value.id) ? value.id : undefined;

  return {
    jsonrpc: typeof value.jsonrpc === "string" ? value.jsonrpc : undefined,
    id,
    method: typeof value.method === "string" ? value.method : undefined,
    params: value.params,
  };
}

export function toProjectModel(value: unknown): ProjectModel | null {
  if (!isObject(value)) {
    return null;
  }

  if (typeof value.root !== "string" || typeof value.fingerprint !== "string") {
    return null;
  }
  if (!isStringArray(value.tsconfig_paths) || !isStringArray(value.workspace_files)) {
    return null;
  }

  return {
    root: value.root,
    tsconfig_paths: value.tsconfig_paths,
    workspace_files: value.workspace_files,
    primary_language: readOptionalString(value.primary_language),
    fingerprint: value.fingerprint,
    repo_archetype: readOptionalString(value.repo_archetype),
    detected_archetypes: parseDetectedArchetypes(value.detected_archetypes),
  };
}

interface JsonRpcErrorPayload {
  code: number;
  message: string;
  data?: unknown;
}

function response(
  id: JsonRpcId,
  result?: unknown,
  error?: JsonRpcErrorPayload,
): JsonRpcResponse {
  if (error) {
    return {
      jsonrpc: "2.0",
      id,
      error,
    };
  }

  return {
    jsonrpc: "2.0",
    id,
    result: result ?? null,
  };
}

export function dispatchRequest(
  request: JsonRpcRequest,
  dependencies: RequestHandlerDependencies,
): DispatchOutcome {
  const id = request.id ?? null;
  const method = request.method;

  if (request.jsonrpc !== "2.0") {
    return {
      kind: "response",
      response: response(id, undefined, { code: -32600, message: "Invalid Request" }),
    };
  }

  if (!method) {
    return {
      kind: "response",
      response: response(id, undefined, { code: -32600, message: "Invalid Request" }),
    };
  }

  if (method === "initialize") {
    if (request.id === undefined) {
      return { kind: "ignore" };
    }

    return {
      kind: "response",
      response: response(id, {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: {
          semanticAnalysis: true,
          incrementalUpdates: false,
        },
        serverInfo: {
          name: "sentrux-ts-bridge",
          version: "0.1.0",
        },
      }),
    };
  }

  if (method === "ping") {
    if (request.id === undefined) {
      return { kind: "ignore" };
    }

    return { kind: "response", response: response(id, { ok: true }) };
  }

  if (method === "shutdown") {
    if (request.id === undefined) {
      return { kind: "ignore" };
    }

    return { kind: "response", response: response(id, null) };
  }

  if (method === "analyze_projects") {
    if (request.id === undefined) {
      return { kind: "ignore" };
    }

    const project = toProjectModel(request.params);
    if (!project) {
      return {
        kind: "response",
        response: response(id, undefined, {
          code: -32602,
          message: "Invalid params",
          data: {
            expected: "ProjectModel",
          },
        }),
      };
    }

    try {
      return {
        kind: "response",
        response: response(id, dependencies.analyzeProject(project)),
      };
    } catch (error) {
      return {
        kind: "response",
        response: response(id, undefined, {
          code: -32001,
          message: "Semantic analysis failed",
          data: {
            message: error instanceof Error ? error.message : String(error),
          },
        }),
      };
    }
  }

  if (method === "exit") {
    if (request.id === undefined) {
      return { kind: "exit", code: 0 };
    }

    return {
      kind: "exit",
      code: 0,
      response: response(id, null),
    };
  }

  if (request.id === undefined) {
    return { kind: "ignore" };
  }

  return {
    kind: "response",
    response: response(id, undefined, {
      code: -32601,
      message: "Method not found",
      data: { method },
    }),
  };
}
