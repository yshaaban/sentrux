Inspect the repository and make the smallest safe change that keeps explicit contract-obligation surfaces in sync instead of fixing only one propagation site.

Start with the `server_state_bootstrap` contract and agent-facing surfaces that already carry followthrough pressure:

- `sentrux-core/src/metrics/v2/obligations_contract_tests.rs`
- `sentrux-core/src/app/mcp_server/handlers/check_tests.rs`
- `sentrux-core/src/app/mcp_server/handlers/session_tests.rs`
- `sentrux-core/src/app/mcp_server/handlers/classification_details.rs`

Prefer updating the unchanged sibling test or presentation surface when contract guidance changes, or tightening one shared helper only if it clearly reduces future drift.

If those surfaces do not expose a high-confidence contract followthrough gap, say so explicitly before picking the next highest-confidence obligation-propagation target.
