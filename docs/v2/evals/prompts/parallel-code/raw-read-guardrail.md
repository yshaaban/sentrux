Make the smallest safe raw-read boundary fix around task presentation and sidebar status reads.

Start with only these surfaces:

- `<parallel-code-root>/src/components/SidebarTaskRow.tsx`
- `<parallel-code-root>/src/app/task-presentation-status.ts`
- `<parallel-code-root>/src/app/task-presentation-status.test.ts`

Use those files to look for direct store reads or presentation shortcuts that should stay behind an explicit accessor. Read adjacent imports only if one of those surfaces depends on them. Do not scan the broader repository for alternate boundary issues. Do not run full builds or full test suites; if validation is needed, use the narrowest relevant test command for the touched file.

Prefer one focused fix that strengthens the `forbidden_raw_read` boundary without introducing a larger refactor.

If these surfaces do not show a convincing boundary issue, report a no-op instead of choosing another guardrail target.

Endurance note: broader rule-boundary sweeps belong in the non-smoke lane.
