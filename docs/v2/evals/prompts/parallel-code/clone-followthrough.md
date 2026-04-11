Inspect the repository and make the smallest safe change that keeps an edited clone family in sync instead of fixing only one duplicate path.

Start with the browser cold-bootstrap and session-startup surfaces that already carry duplicated startup behavior:

- `src/app/browser-startup.ts`
- `src/app/desktop-session-startup.ts`
- `src/app/runtime-diagnostics.ts`
- `src/store/persistence-load.ts`

Prefer updating the unchanged sibling clone or collapsing both paths behind one shared helper when the duplicated behavior is still intentional.

If those surfaces do not expose a high-confidence clone followthrough gap, say so explicitly before picking the next highest-confidence duplicate-family cleanup.
