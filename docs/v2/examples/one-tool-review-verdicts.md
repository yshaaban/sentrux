# One-Tool Review Verdicts

Repo: `one-tool`

Source report: `docs/v2/examples/one-tool-onboarding.json`

This review set captures the current public `one-tool` onboarding story after inferred runtime rules were enabled for repos without a checked-in `rules.toml`.

Current maintainer stance:

- keep the `src/mcp/index.ts` <-> `src/mcp/server.ts` cycle visible as a leading watchpoint because it is a small, high-leverage boundary cut
- keep `src/commands/groups/text.ts` ahead of the adjacent command surfaces because it is the strongest contained composition-root cleanup target
- keep `src/commands/groups/fs.ts` visible, but behind the cycle and the text command

Machine-readable verdicts live in `one-tool-review-verdicts.json`.
