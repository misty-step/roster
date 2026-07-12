# Codex CLI Harness Notes

Codex CLI is a primary Roster lane. Use it for independent
dynamic delegation when the lead is not already Codex, or when a separate
Codex process/worktree gives useful isolation.

## Native Projection

Current Codex reads user configuration from `~/.codex/config.toml`. `roster
sync` appends one marker-bounded block of `[agents.<name>]` registrations and
points each registration at a generated TOML role layer under
`~/.roster/orchestrator/codex-roles/`. Existing models, MCPs, permissions,
plugins, custom agents, and UI settings remain untouched. The old
`~/.codex/config/config.toml` template link is not a runtime surface and is no
longer installed.

The canonical MCP catalog is policy, not a second copy of every local MCP
configuration. `roster doctor` rejects any catalog-disabled server that is
still active in effective Codex, Claude, or OMP config; complete `available`
entries are Roster-launchable, while `external` entries are supplied by the
consumer runtime.

`~/.codex/AGENTS.md` remains the default orchestrator doctrine link. Roster no
longer installs `.codex/agents/*.md`; native multi-agent discovery comes from
the managed config block.

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
