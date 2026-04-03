Inspect the repository's explicit contract surfaces and make the smallest safe change that closes the strongest propagation gap you can find.

Prefer a localized fix around `server_state_bootstrap`-style categories, payload maps, registries, and runtime bindings rather than broad cleanup.

If no convincing propagation gap exists, report that explicitly and choose the next highest-confidence repo-local maintenance target instead.
