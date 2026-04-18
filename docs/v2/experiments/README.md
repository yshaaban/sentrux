# V2 Experiment Records

Last audited: 2026-04-19

This directory holds the human-facing experiment records for the v2 program.

Use it together with the machine-readable registry in [../evals/experiments/index.json](../evals/experiments/index.json).

## Source Of Truth

- Machine-readable experiment specs live under [../evals/experiments](../evals/experiments).
- Narrative strategy and decision rules live in [../experiment-program.md](../experiment-program.md).
- Completion status stays in [../completion-execution-tracker.md](../completion-execution-tracker.md).

## Status Meanings

- `planned`: the experiment is specified but not yet producing fresh evidence
- `in_progress`: the experiment has active specs and should accumulate repo-local evidence
- `completed`: the experiment has a recorded decision and no open execution gap
- `blocked`: the experiment is real but cannot advance until a dependency is resolved

## Working Loop

1. Update or add the machine-readable spec first.
2. Run the experiment or a dry-run plan with:

```bash
node scripts/evals/run-experiment.mjs \
  --experiment docs/v2/evals/experiments/default-lane-family-ablation.json \
  --dry-run
```

3. Refresh the tracker:

```bash
node scripts/evals/build-experiment-tracker.mjs \
  --output-json .sentrux/evals/experiments/experiment-tracker.json \
  --output-md .sentrux/evals/experiments/experiment-tracker.md
```

4. Record the outcome with [decision-template.md](./decision-template.md) once the exit bar is satisfied.
