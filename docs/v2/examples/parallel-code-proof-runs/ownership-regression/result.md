# Parallel-Code Proof: Ownership Regression

This scenario seeds an out-of-policy `task_git_status` write from `task-presentation-status.ts`.

- before task_git_status findings: 0
- after task_git_status findings: 2
- gate blocked after mutation: true

## After Findings

- `multi_writer_concept`: Concept 'task_git_status' is mutated from 2 files
- `writer_outside_allowlist`: Concept 'task_git_status' is written outside its allowed writer set at src/app/task-presentation-status.ts
