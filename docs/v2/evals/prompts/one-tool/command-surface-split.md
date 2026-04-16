You are working in the public `one-tool` repo.

Goal:
- reduce one high-pressure command surface without changing behavior
- keep orchestration, adapters, and side effects easier to read and maintain

Focus:
- `src/commands/groups/text.ts`
- if that turns out to be the wrong surface, use another clearly high-pressure command-group file with the same characteristics

Constraints:
- no feature changes
- no clever abstractions
- keep the public command surface stable
- prefer a contained split with explicit helper ownership

Success means:
- one command surface is materially simpler
- the patch is easy to justify in code review
- follow-up maintenance risk is lower than before
