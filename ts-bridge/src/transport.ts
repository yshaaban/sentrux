import { analyzeProject } from "./analysis.js";
import { dispatchRequest, toRequest } from "./protocol.js";
import type { JsonRpcResponse } from "./types.js";

export function parseHeaders(headerText: string): Map<string, string> {
  const headers = new Map<string, string>();

  for (const line of headerText.split("\r\n")) {
    const separator = line.indexOf(":");
    if (separator <= 0) {
      continue;
    }

    const name = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();
    headers.set(name, value);
  }

  return headers;
}

function writeMessage(message: JsonRpcResponse): void {
  const body = JSON.stringify(message);
  const header = `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n`;
  process.stdout.write(header);
  process.stdout.write(body);
}

function writeParseError(message: string): void {
  writeMessage({
    jsonrpc: "2.0",
    id: null,
    error: {
      code: -32700,
      message: "Parse error",
      data: { message },
    },
  });
}

function writeInvalidRequest(): void {
  writeMessage({
    jsonrpc: "2.0",
    id: null,
    error: {
      code: -32600,
      message: "Invalid Request",
    },
  });
}

function handleStdoutError(error: NodeJS.ErrnoException): void {
  if (error.code === "EPIPE") {
    process.exit(0);
  }

  throw error;
}

export function main(): void {
  let buffer = Buffer.alloc(0);

  process.stdout.on("error", handleStdoutError);

  process.stdin.on("data", function handleData(chunk: Buffer): void {
    buffer = Buffer.concat([buffer, chunk]);

    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        return;
      }

      const headerText = buffer.subarray(0, headerEnd).toString("utf8");
      const headers = parseHeaders(headerText);
      const contentLength = headers.get("content-length");

      if (!contentLength) {
        buffer = buffer.subarray(headerEnd + 4);
        continue;
      }

      const bodyLength = Number.parseInt(contentLength, 10);
      if (!Number.isFinite(bodyLength) || bodyLength < 0) {
        buffer = Buffer.alloc(0);
        writeInvalidRequest();
        continue;
      }

      const messageStart = headerEnd + 4;
      const messageEnd = messageStart + bodyLength;
      if (buffer.length < messageEnd) {
        return;
      }

      const bodyText = buffer.subarray(messageStart, messageEnd).toString("utf8");
      buffer = buffer.subarray(messageEnd);

      let parsed: unknown;
      try {
        parsed = JSON.parse(bodyText);
      } catch (error) {
        writeParseError(error instanceof Error ? error.message : String(error));
        continue;
      }

      const request = toRequest(parsed);
      if (!request) {
        writeInvalidRequest();
        continue;
      }

      const outcome = dispatchRequest(request, {
        analyzeProject,
      });

      if (outcome.kind === "ignore") {
        continue;
      }

      if (outcome.kind === "exit") {
        if (outcome.response) {
          writeMessage(outcome.response);
        }
        process.exit(outcome.code);
        return;
      }

      writeMessage(outcome.response);
    }
  });

  process.stdin.on("end", function handleEnd(): void {
    process.exit(0);
  });
}
