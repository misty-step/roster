# Codex CLI Harness Notes

Codex CLI is a primary Roster lane. Use it for independent
dynamic delegation when the lead is not already Codex, or when a separate
Codex process/worktree gives useful isolation.

## Dispatch Shape

Use non-interactive exec with the configured model and reasoning effort:

```sh
codex exec --dangerously-bypass-approvals-and-sandbox --model gpt-5.6-luna --config model_reasoning_effort="xhigh" "Role: critic. Objective: review the changed files. Output: blockers only."
```

Prepend `roster brief <identity> --card <id>` when dispatching a declared
identity. The harness runs the lane; durable evidence belongs on the Powder
card.

## Dynamic Delegation Notes

- Prefer isolated worktrees for competing implementation attempts.
- Keep prompts commissioned: role, objective, scope, output, boundaries.
- Give the shaped ticket, exact files, and commands to inspect; avoid hidden
  lead reasoning.
- Reject outputs that hard-code, hand-wave verification, or ignore boundaries.
- The lead remains responsible for final code, tests, and acceptance evidence.
