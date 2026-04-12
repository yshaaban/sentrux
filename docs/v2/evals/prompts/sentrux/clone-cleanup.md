Make the smallest safe smoke-task duplicate cleanup inside the existing handler clone family.

Start with only these surfaces:

- `sentrux-core/src/app/mcp_server/handlers/classification.rs`
- `sentrux-core/src/app/mcp_server/handlers/classification_details.rs`
- `sentrux-core/src/app/mcp_server/handlers/brief.rs`
- `sentrux-core/src/app/mcp_server/handlers/session.rs`

Inspect that family first and only read directly referenced shared helpers if they are required to complete one local consolidation. Do not roam into unrelated handlers, run full repository scans, or run broad Cargo builds or test suites.

Prefer one obvious owner path or one tiny shared-helper extraction that removes a fresh or newly exposed duplicate without changing behavior.

If this family does not expose a high-confidence duplicate cleanup, report a no-op instead of picking another target.

Endurance note: broader clone-family followthrough belongs in the dedicated non-smoke clone tasks.
