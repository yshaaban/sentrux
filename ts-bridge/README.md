# ts-bridge

`ts-bridge` is the Node-side TypeScript semantic bridge used by Sentrux v2.

It is an internal subsystem, not a standalone public product surface.

## Responsibilities

- discover TypeScript projects
- analyze them through the TypeScript compiler API
- emit normalized semantic snapshots back to Rust over stdio JSON-RPC
- support the v2 patch-safety analyzers that depend on symbols, writes, closed domains, and transition metadata

## Current Scope

- persistent bridge process
- compiler-backed semantic extraction
- protocol handshake and capability checks
- crash recovery and restart behavior
- test coverage for the bridge transport and request path

## Current Non-Goals

- general-purpose language-server behavior
- arbitrary editor integrations
- fully incremental symbol-level invalidation across runs

## Useful Commands

```bash
npm install
npm run build
npm run test
npm run start
```
