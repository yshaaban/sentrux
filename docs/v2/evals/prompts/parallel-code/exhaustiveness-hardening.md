Make the smallest safe exhaustiveness fix in the task presentation seam.

Start with only these surfaces:

- `<parallel-code-root>/src/app/task-presentation-status.ts`
- `<parallel-code-root>/src/app/task-presentation-status.test.ts`

Read adjacent imports only when they are needed to understand a specific union, record, or `assertNever` branch in this seam. Do not scan the repository for other exhaustiveness targets. Do not run full builds or full test suites; if validation is needed, use the narrowest relevant test command for the touched file.

Prefer one explicit exhaustive mapping, switch branch, or getter hardening aligned with `closed_domain_exhaustiveness`.

If this seam already looks exhaustive, report a no-op instead of escalating to another target.

Endurance note: broader domain-surface sweeps belong in the non-smoke lane.
