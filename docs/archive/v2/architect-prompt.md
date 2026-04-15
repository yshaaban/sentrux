# Architectural Entropy Analysis

Analyze the codebase for structural entropy — patterns that silently accumulate bugs, inconsistency, and maintenance burden over time. Focus on changes that prevent entire categories of failure, not individual symptoms.

This is a two-phase analysis: **diagnose** the entropy, then **propose simplifications** that reduce the surface area where entropy can grow. A diagnosis without a simplification proposal is incomplete. The goal is not just "find problems" — it is "make the codebase structurally simpler so these problems stop recurring."

Do not suggest cosmetic, stylistic, or additive changes. Every finding must identify a concrete failure mode and a verification that confirms the fix.

Read the project's architecture docs, CLAUDE.md, and any `.architecture.test` files before starting. These encode the project's own rules — violations of stated intent are higher priority than violations of general best practice.

---

## 1. Authority And Ownership

One concept should have one durable writer. When multiple layers or files mutate the same state, every future change to that state is a coordination problem.

What to look for:

**1a. Multi-writer state** — The same store field, database record, or shared object is mutated from more than one module or layer. Map every write site for each stateful concept. If a concept has writers in both the "store" layer and the "workflow" layer, or both "sync" and "polling" modules, flag it. The fix is consolidating writes behind one authority module.

**1b. Layer bypass** — A module reaches past the intended public API to mutate internals directly. Example: importing `setStore` from `store/core` instead of going through the store's public barrel (`store/store.ts`). Find every import that skips the barrel or facade. The fix is routing through the intended API.

**1c. Canonical accessor bypass** — Components or app code reading raw authoritative state instead of using the designated projection or accessor function. Example: reading `store.agentSupervision[id].state` directly instead of `getTaskDotStatus(id)`. Find every raw read that has a canonical accessor available. The fix is replacing raw reads with the accessor.

**1d. Scattered domain logic** — Business rules for one concept spread across many files with no single source of truth. Example: "is this task closable?" logic duplicated with different conditions in the sidebar, the panel, and the workflow layer. Count the sites and name the canonical location.

How to simplify:

- **Consolidate writes into one authority module.** If `taskGitStatus` is written by both `git-status-sync.ts` and `git-status-polling.ts`, create a single `taskGitStatusAuthority.ts` that owns all mutations. Other modules call it — they never import the store primitive directly. This means future changes to the write logic happen in one file, not N files.
- **Introduce a projection layer.** Raw authoritative state should not leak to consumers. If 12 components need to know "is this task busy?", they should read a derived accessor (`getTaskDotStatus`), not reach into `store.agentSupervision[id].state` and interpret it themselves. The projection translates once; consumers get a stable, narrow contract.
- **Make the barrel the enforced boundary.** If `store/store.ts` is the intended public API, components should import from `store/store` only. Internal modules like `store/core` should not be importable from outside the store layer. If the language doesn't enforce this, a lint rule or architecture test should.

Output per finding:
- Concept name
- All write sites or bypass sites (file:line)
- Which layer or module should be the authority
- What breaks: describe the scenario where a future change to this concept misses one of the write sites
- Simplification: the specific consolidation — which module becomes the authority, which imports get redirected, which raw accesses get replaced with projections
- Verify: a grep or import-check that confirms the bypass no longer exists

---

## 2. Clone Drift

Duplicated logic drifts apart silently. One copy gets a bug fix, the other doesn't. One gets a new feature, the other becomes stale.

What to look for:

**2a. Exact function clones** — Two or more functions with identical or near-identical bodies in different files. Don't just check names — compare the logic. If two functions do the same filter-map-check sequence on the same data shape, they are clones even if named differently.

**2b. Divergent clones** — Two functions or modules that clearly started as copies but now differ in subtle ways. One handles an edge case the other doesn't. One uses a newer API the other still uses the old version of. The divergence is the bug. Name what each version handles that the other misses.

**2c. Dead originals** — A function or module that was replaced by a newer version but never deleted. Nothing imports it anymore. It will confuse the next developer or agent who finds it. Confirm zero importers before flagging.

**2d. Inline pattern duplication** — The same inline code pattern (styling objects, error handling blocks, validation sequences, config shapes) repeated across 3+ sites with only superficial variation. These aren't function clones — they're pattern clones that should be extracted into a shared abstraction.

**2e. Duplicate type definitions** — The same data shape defined independently in multiple files. Different property names for the same concept across layers (e.g., `has_committed_changes` in one type, `hasCommitted` in another for the same data). Name the canonical definition.

How to simplify:

- **Extract, don't just flag.** When you find 3+ instances of the same inline pattern (button footer styles, error banners, validation sequences), create one shared implementation and replace all sites in one commit. The shared version becomes the default; future instances copy it instead of re-inventing.
- **Delete the dead original.** When a module has been superseded, delete it entirely. Do not leave it as "reference material." Its existence will mislead the next agent or developer into importing or copying from the wrong source.
- **Unify divergent clones by making the better version the only version.** When two functions do the same thing but one handles more edge cases, keep the more complete version and delete the other. If they handle *different* edge cases, merge them into one function that handles both.

Output per finding:
- Clone family name
- All instances (file:line for each)
- What diverged (if applicable) — the specific line or behavior that differs
- What breaks: the scenario where a fix to one copy doesn't propagate to the others
- Simplification: the single shared implementation that replaces all instances, or the deletion that eliminates the confusion
- Verify: grep for the duplicated pattern → 0 or 1 matches

---

## 3. State Discipline

State should be explicit, closed, layered, and exhaustively handled. Implicit state machines grow boolean fields, unguarded transitions, and impossible combinations. The goal is not just correctness — it is **structural simplicity**: fewer states, fewer representations, fewer translation layers, fewer places that need to understand state transitions.

What to look for:

**3a. Missing exhaustiveness** — Switch statements, if/else chains, `Record<>` types, or filter logic over discriminated unions that don't handle all variants. An `assertNever` or equivalent should guard the default case. Find every switch on a union type that uses `default:` fallthrough or lacks a completeness check. Count the unhandled variants.

**3b. Flat state with impossible combinations (boolean soup)** — A type with multiple boolean or optional fields that create impossible combinations. Example: `{ collapsed: boolean, closingStatus?: string, closingError?: string }` where `closingError` only makes sense when `closingStatus === 'error'` but the type allows `closingError` with any status. Count the total field combinations vs. the number of valid states. If valid states < 50% of possible combinations, the type is under-constrained.

**3c. Flat enums that should be layered** — A single flat union type with many variants where some variants share structure and others don't. Example: a `TaskState` with 9 variants where 4 of them are "running" substates (`active`, `awaiting-input`, `idle-at-prompt`, `quiet`) and 2 are "exited" substates (`exited-clean`, `exited-error`). Consumers that only care about "is it running?" must enumerate all 4 running substates. This is a flat state machine that should be a two-level hierarchy:

```
type TaskPhase = Running | Exited | Transitioning
type RunningSubstate = Active | AwaitingInput | IdleAtPrompt | Quiet
type ExitedSubstate = Clean | Error
```

Look for: switches where multiple cases have identical bodies (that's a missing grouping level), or predicates like `isRunning(state)` that enumerate a subset of variants manually.

**3d. Inconsistent state guards** — The same state condition checked differently in different places. Example: one site checks `task.closingStatus === 'removing'`, another checks `task.closingStatus === 'closing' || task.closingStatus === 'removing'`, another just checks `task.closingStatus` truthiness. These will diverge when a new status is added. Name the concept, list every guard site, and show how they differ. The fix is a single predicate function that all sites call.

**3e. Too many parallel state representations** — The same lifecycle concept represented by different types at different layers, with lossy mapping between them. Example: 9 supervision states at the domain layer collapsed to 5 at the transport layer collapsed to 7 at the presentation layer. Each translation layer is a maintenance burden and an information-loss risk.

Ask: do all these layers need to exist? Can the domain type serve more consumers directly, eliminating a mapping layer? Can two representations be merged? The goal is to minimize the number of distinct state types for the same concept. Each additional representation is a tax on every future state change.

**3f. States that exist but nobody distinguishes** — Variants in a union that are never handled differently from another variant. If `flow-controlled` and `paused` are always treated identically by every consumer, they may not need to be separate states. Map every consumption site for each variant. If two variants have identical handling at every site, propose merging them (or document why the distinction matters for future use).

**3g. Missing transition tests** — State types with lifecycle semantics (closing → closed, connecting → connected → disconnected) that have zero test coverage for their transitions. If a state type has more than 3 variants and no test exercises the transition logic, flag it.

**3h. Ghost fields** — Fields that are read in components or business logic but don't exist in the type definition. They work at runtime because of dynamic property setting but bypass the type checker entirely. Search for property accesses on typed objects where the property isn't in the interface.

How to simplify:

- **Replace boolean soup with discriminated unions.** Instead of `{ isLoading: boolean, error?: Error, data?: T }`, use `type State = Loading | Loaded<T> | Failed`. Each variant carries only its relevant fields. Impossible combinations become unrepresentable. This is the single highest-leverage state simplification.
- **Introduce layered/hierarchical state when flat enums grow.** When a flat enum reaches 6+ variants and consumers routinely group them (all "running" substates, all "terminal" states), restructure into a two-level hierarchy. The top level has 3-4 phases. Each phase has its substates. Consumers that care about the phase match on the top level; consumers that need detail destructure further. This eliminates the `isRunning = state === 'active' || state === 'awaiting-input' || state === 'idle-at-prompt' || state === 'quiet'` enumeration pattern that breaks every time a new running substate is added.
- **Consolidate parallel representations.** If the domain layer has 9 states and the presentation layer has 7, ask: can the presentation layer use the domain type directly with a `phase()` accessor, instead of maintaining its own parallel type? Each eliminated representation removes one mapping function, one set of exhaustive sites, and one source of translation bugs.
- **Replace scattered guard predicates with centralized named functions.** If "is this task in a terminal state?" is checked in 8 places with 3 different implementations, create one `isTerminalState(status)` function and replace all 8 sites. When a new terminal state is added, one function changes instead of 8.
- **Question the variant count.** Before accepting a 9-variant union, ask: do consumers actually distinguish all 9? If 3 of them are always handled identically, reduce to 6 + a derived predicate for the group. Fewer variants = fewer exhaustive sites = less propagation burden on every future change.

Output per finding:
- State concept name
- Type definition location (and all parallel type definitions)
- All switch/guard/mapping sites
- What breaks: the specific scenario (e.g., "adding a 10th supervision state silently falls through in 3 switches and requires updating 2 translation layers")
- Simplification: the specific restructuring — which types merge, which hierarchy replaces the flat enum, which predicates replace scattered guards, which representations are eliminated
- Verify: `grep -rn 'default:' src/ | grep <union-name>` → 0 matches, or compiler error count after adding a phantom variant, or count of distinct types representing this lifecycle → reduced by N

---

## 4. Propagation Completeness

When a closed domain changes, every site that enumerates it must update. Incomplete propagation is the #1 cause of "I added a field but forgot to..." bugs.

What to look for:

**4a. Closed-domain enumeration sites** — For every discriminated union, string literal union, `as const` array, or registry object, find ALL sites that enumerate its values: switches, `Record<Union, ...>`, exhaustive if/else chains, mapping tables, array filters. If a new variant is added, how many files need to change? If the answer is more than 3, flag the coordination burden.

**4b. Serialization/deserialization asymmetry** — Fields saved during serialization but not restored during deserialization, or vice versa. Compare the save path and load path field by field. Flag any field present in one but not the other, unless the asymmetry is explicitly documented as intentional.

**4c. Orphan fields** — Fields written but never read, or read but never written (from TypeScript source, not runtime). A field that is set in one workflow but consumed nowhere is dead weight. A field read in components but with no traceable write path is a reliability gap.

**4d. Missing registry updates** — When a project uses registry patterns (category lists, payload maps, handler tables), adding a new entry to the source list requires updating every consuming registry. Find registries where the source list and all consumer registries have different cardinality.

**4e. Cross-boundary field name drift** — The same logical field using different names across boundaries. Example: `branch_name` in the IPC layer vs `branchName` in the store layer vs `branchName` in the persistence layer. Map the translations. If a mapping is manual and inline, it will silently drop new fields.

**4f. Wide types crossing too many boundaries** — A type with 20+ fields that must be serialized, deserialized, persisted, sent over IPC, and rendered in components. Every new field must be added to every layer. The propagation cost scales as `fields × layers`. Find types where the full shape crosses 3+ boundaries. The fix is splitting into focused sub-types so each boundary only carries the fields it needs — or introducing a shared codec/mapper that handles the translation in one place instead of N inline sites.

How to simplify:

- **Reduce the number of enumeration sites, not just make them complete.** If adding a variant to a union requires updating 8 files, the fix is not "make sure you update all 8." The fix is: centralize the mapping in one place (a single `Record<Union, Config>` table) and have other sites derive from it. If the presentation layer, the transport layer, and the persistence layer all map the same union, make one canonical mapping and have the others reference it.
- **Split wide types at boundary crossings.** If `AppStore` has 65 fields and crosses 3 boundaries, split it: `TaskState` (12 fields), `UIState` (8 fields), `ConnectionState` (5 fields), etc. Each boundary carries only the sub-type it needs. This converts a 65×3 propagation surface to three focused surfaces. When a field is added to `TaskState`, only the task-related boundaries need to change.
- **Use codec/mapper modules instead of inline translations.** If the IPC boundary manually maps `{ branch_name } → { branchName }` for 15 fields, create a single `toStoreTask(ipcTask)` / `toIpcTask(storeTask)` mapper. New fields are added in one place. The inline mapping sites disappear.
- **Make registries self-describing.** If a category list, a payload map, and a handler table must all stay in sync, derive the handler table from the payload map or the category list from the payload map's keys. Fewer independent sources of truth = fewer synchronization obligations.

Output per finding:
- Concept or domain name
- Source of truth location
- All enumeration/consumption sites (count them)
- The propagation cost: "adding one variant/field requires touching N files across M boundaries"
- What breaks: "adding variant X requires updating N files; if file Y is missed, Z happens"
- Simplification: the specific structural change that reduces the propagation surface — centralized mapping, split types, derived registries, or eliminated redundant representations
- Verify: count of sites that enumerate the domain → reduced, or count of boundaries the type crosses → reduced

---

## 5. Concentration Risk

When too much logic concentrates in one file, type, or module, every future change becomes expensive and risky.

What to look for:

**5a. God interfaces** — Types or interfaces with more than ~30 fields imported by more than ~10 files. These create shotgun surgery: any structural change ripples everywhere. Name the type, its field count, its importer count, and which fields could be split into focused sub-interfaces.

**5b. Mutation hubs** — Files with an unusually high number of state mutations (store writes, setter calls, event emissions). If one file performs 15+ mutations across multiple unrelated concepts, it has become a coordination bottleneck. List the mutations grouped by concept.

**5c. Composition root overload** — Entry point files or session bootstrappers that wire together 10+ unrelated concerns. These files grow silently as features are added. Count the distinct concerns and suggest which could be extracted into focused initializers.

**5d. High churn-density files** — Files with high commit frequency relative to their size (many commits per 100 lines). These are the files where bugs are most likely to be introduced. Cross-reference with complexity — a high-churn, high-complexity file is the highest risk.

How to simplify:

- **Split god interfaces by consumer need.** If `AppStore` has 65 fields but any given consumer reads at most 8, the interface should be split so consumers depend on a narrow view. Use sub-interfaces, pick-types, or separate store slices. The goal: no consumer imports a 65-field type when it only needs 8 fields.
- **Extract unrelated concerns from hub files.** If `desktop-session.ts` wires 11 concerns from 9 directories, extract each concern into a focused initializer (`initTerminals()`, `initGitStatus()`, `initBootstrap()`). The session file calls them in sequence but doesn't contain their logic. This makes each concern independently testable and changeable.
- **Reduce mutation breadth, not just mutation count.** If a file mutates 5 different store domains, move each domain's mutations into that domain's authority module. The hub file calls the authority module's function instead of reaching into the store directly. The mutation count in the hub drops to zero; the authority modules each gain one focused function.

Output per finding:
- File or type name
- Metric (field count, mutation count, import count, concern count)
- What breaks: the scenario where a routine change to this file causes an unrelated regression
- Simplification: the specific split — which sub-types, which extracted modules, which mutations move where
- Verify: file line count or field count after fix → below threshold

---

## 6. Abstraction Discipline

Existing abstractions should be used. New abstractions should replace duplication.

What to look for:

**6a. Manual patterns that bypass existing abstractions** — If the codebase has a store mutation helper, typed event emitter, dialog component, error display pattern, or any other established abstraction, find call sites that do the same thing manually. The manual version will drift from the abstraction over time.

**6b. Premature abstraction** — An abstraction used by only 1 call site, or a utility function that wraps a trivial one-liner. These add indirection without reducing duplication. Don't flag these for addition — flag them for removal.

**6c. Re-export facades that add no value** — A file that re-exports functions from another file without transformation. If the re-export exists for API stability, note it but don't flag it. If it exists because of a copy-paste refactor that was never finished, flag it.

**6d. Unsafe type casts** — `as unknown as T` double-casts, `as any`, or bare `as T` that bypass the type system. Distinguish between necessary boundary casts (IPC, JSON parse, FFI) and avoidable ones where the type could flow naturally. Count each category.

Output per finding:
- Pattern name
- The existing abstraction (if applicable) and its location
- All manual/duplicate sites
- What breaks: the scenario where the abstraction is updated but the manual sites aren't
- Simplification: use the abstraction (and list which sites change), or delete the unused abstraction, or extract a new one from 3+ sites
- Verify: grep for the manual pattern → 0 matches

---

## 7. Structural Simplification Opportunities

Beyond fixing specific entropy patterns, look for opportunities to make the overall architecture structurally simpler. These are cross-cutting simplifications that reduce the surface area where entropy can grow in the future.

What to look for:

**7a. Layers that exist but add no value** — A module that re-exports functions from another module without transformation, or a "facade" that passes every call through unchanged. If removing the layer would not break any consumer's contract, it should be removed. Each unnecessary layer is a synchronization obligation.

**7b. Translation layers between equivalent types** — Two types representing the same data with different field names, where a mapping function manually copies every field. If the types exist in the same language and same runtime, unify them. If they exist across a boundary (IPC, persistence), make the mapping a single codec function, not inline copies at every call site.

**7c. Feature flags or conditional paths that are always one value** — A boolean flag that has been `true` in production for 6 months, or a code path gated by a check that always succeeds. These are dead branches that obscure the real logic. Remove the branch; keep only the live path.

**7d. Over-granular state that could be derived** — State that is stored and synchronized but could be computed on demand from other state. Example: storing both `taskCount` and `tasks[]` when `taskCount === tasks.length` always. Stored derived state creates a synchronization obligation. Computed derived state has zero synchronization cost.

**7e. Redundant validation at interior boundaries** — Null checks, type guards, or defensive defaults deep inside the codebase that are only necessary because an outer boundary didn't validate. If the outer boundary guarantees the input, interior checks are dead code. Identify the trust boundary and remove redundant checks behind it.

Output per finding:
- The unnecessary layer, translation, branch, or stored derivation
- Why it exists (likely historical reason)
- What removing it saves (fewer files to touch on future changes, fewer synchronization obligations, less code)
- Simplification: the specific deletion or consolidation
- Verify: the build passes and the test suite is green after removal

---

## How To Make Changes

These rules apply to every fix you implement, not just this analysis. The theme is: every change should leave the codebase simpler or at least no more complex. If a fix adds complexity, reconsider the approach.

### When adding a new variant to a union or enum:
1. Find every exhaustive site BEFORE writing the variant
2. Update all exhaustive maps, switches, and predicates in the same commit
3. If any site uses `default:` fallthrough, replace it with explicit handling + `assertNever`
4. Run the compiler — zero new errors means zero missed sites only if exhaustiveness is enforced
5. If the variant count is growing past 6-8, consider whether the union should become hierarchical (top-level phases with substates) instead of staying flat

### When adding a new field to a type:
1. Trace the type through serialization, deserialization, persistence, and IPC boundaries
2. Add the field to every layer it should appear in, in the same commit
3. If the field is optional, document why and when it will be present
4. Do NOT add the field only to the runtime type and forget persistence
5. If the type already has 20+ fields, consider whether the new field belongs in a focused sub-type instead of the main interface

### When adding state:
1. State belongs in one place. Identify the authority module and write there only.
2. Other modules read through an accessor, projection, or derived signal — never by reaching into the store internals directly
3. If you need to transform state for display, create a named projection function. Do not inline the transformation in 5 components.
4. Every new piece of state should have a clear lifecycle: when is it set, when is it cleared, what are the valid transitions
5. If the state has more than two possible conditions (true/false), it should be a discriminated union, not a boolean plus optional fields
6. If the new state interacts with existing state (both must be consistent), consider whether they should be one combined discriminated union instead of two independent fields

### When you notice growing complexity during a change:
1. If you're adding the 4th boolean to a type, stop — restructure to a discriminated union first
2. If you're adding the 3rd copy of a pattern, stop — extract a shared implementation first
3. If you're adding a mapping layer between two state types, ask whether the source type could serve the consumer directly
4. If you're adding a field to a type that crosses 3+ boundaries, ask whether the boundary serialization should use a sub-type instead

### When extracting or refactoring:
1. Delete the old code in the same commit. Do not leave it as dead code "just in case."
2. Update all import sites. Search for imports of the old location and redirect them.
3. If you split a file, update the barrel/index exports so downstream consumers don't break.
4. Run the build after the refactor — it must compile cleanly.

### When fixing a bug in duplicated code:
1. Fix ALL copies, not just the one that was reported.
2. If you can't find all copies with confidence, extract the logic into one shared function first, then fix it once.
3. After fixing, add a verification grep to confirm no other copies exist.

---

## Output Summary

After analyzing, produce a prioritized summary:

1. **Critical** — Findings where a routine change (adding a variant, field, or state) will silently break something because propagation is incomplete or exhaustiveness is missing. These should be fixed before new features are added.

2. **High** — Authority violations and clone drift that will cause bugs on the next substantial edit to the affected concept. These should be fixed in the current work cycle.

3. **Medium** — Concentration risk and abstraction drift that increase the cost of future changes but don't cause immediate bugs. These should be addressed during related refactoring.

For each priority level, estimate the total number of findings and the number of files affected. This gives the scope of the problem.

---

## Do Not Include

- Suggestions that add code without removing more code or preventing a concrete bug class
- New abstractions unless they replace 3+ duplicate sites
- Cosmetic renaming, reordering, or reformatting
- Documentation additions (comments, JSDoc) unless they prevent a specific misuse
- Findings that require runtime tracing or test execution to verify — static analysis only
- "Nice to have" improvements that don't map to a failure scenario
