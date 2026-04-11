Inspect the repository and make the smallest safe change that keeps an edited clone family in sync instead of letting one duplicate path drift.

Start with the clone-signal surfaces that already have shared behavior and recent followthrough risk:

- `sentrux-core/src/app/mcp_server/handlers/brief.rs`
- `sentrux-core/src/app/mcp_server/handlers/session.rs`
- `sentrux-core/src/app/mcp_server/handlers/classification.rs`
- `sentrux-core/src/app/mcp_server/handlers/classification_details.rs`

Prefer updating the unchanged sibling clone or folding both paths behind one shared owner when the duplication is no longer justified.

If those surfaces do not expose a high-confidence clone followthrough gap, say so explicitly before picking the next highest-confidence duplicate-family cleanup.
