You are working in the public `one-tool` repo.

Goal:
- remove the small dependency cycle between `src/mcp/index.ts` and `src/mcp/server.ts`
- keep behavior unchanged
- preserve the external MCP surface

Constraints:
- prefer a narrow boundary split over broad rewrites
- do not weaken tests or remove useful structure
- keep the entry surface thin

Success means:
- the cycle is gone
- the change is small, explicit, and easy to review
- no unrelated cleanup is mixed into the patch
