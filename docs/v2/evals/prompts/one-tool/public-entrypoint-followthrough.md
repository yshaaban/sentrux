You are working in the public `one-tool` repo.

Inspect the public entrypoint and MCP surfaces and make the smallest safe change that keeps one shared contract propagated across the exported entrypoints instead of fixing only one surface.

Start with these files:

- `src/index.ts`
- `src/browser.ts`
- `src/mcp/index.ts`
- `src/mcp/server.ts`
- `package.json`
- `test/package-exports.test.ts`
- `test/mcp.test.ts`

Prefer updating the unchanged sibling entrypoint or the focused export/test coverage when the runtime or MCP contract is intentionally shared.

If those surfaces already look aligned, say so explicitly before picking the next highest-confidence public-surface followthrough target.
