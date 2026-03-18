# ts-bridge

Node-side TypeScript semantic bridge for Sentrux v2.

Current scope:

- persistent process over stdio
- JSON-RPC framing with `Content-Length` headers
- compiler-backed project analysis through the TypeScript compiler API
- normalized `SemanticSnapshot` batches for symbols, writes, and closed domains

Current non-goals:

- incremental file updates
- language-service persistence across runs
- references, reads, and richer pattern extraction

Useful commands:

- `npm install`
- `npm run build`
- `npm run start`
