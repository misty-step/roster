# Decision: waive `spawns:`/`max_depth` in `role.yaml` (roster-915)

The roster-910 dispatch-mechanics receipt
(`~/.factory-lanes/campaign/roster-910-report.md`, recommendation 3) flagged
two fields worth stealing for roster's own schema: omp's `spawns:` (a
declared agent-to-agent call graph — "this agent may invoke these others")
and Codex's `max_depth` (a global nesting-depth cap). roster-915's third
acceptance criterion asks for a recorded decision, adopt or waive, with
rationale.

## Decision: waive, for now

`role.yaml` does not gain `spawns:` or `max_depth` fields in this pass.

## Rationale

1. **Roster has no dispatch runtime to enforce either field against.** Both
   fields are only meaningful where something actually *runs* dispatch and
   can refuse an out-of-graph call or a too-deep recursion. Roster declares
   and renders text (`VISION.md`'s own non-goal: "not an execution runtime:
   roster declares and materializes; planes run"). Adding `spawns:`/
   `max_depth` here would be unenforced decoration — a field every harness
   and runtime would have to remember to *also* check, with no single place
   that actually does. That's a worse failure mode than not having the
   field: a false sense of a guardrail that isn't wired to anything.

2. **The existing `subagent_rights` block already answers the coarser
   question roster actually needs to answer.** `may_dispatch` /
   `may_spawn_subagents` / `may_use_peer_harnesses` are booleans, not a
   call graph, but they match how dispatch actually works in this fleet
   today: any agent with `may_spawn_subagents: true` draws from the SAME
   shared `primitives/subagent-pool.yaml` pool, not a per-agent enumerated
   list of allowed targets. A `spawns:` field would imply per-agent
   call-graph precision the rest of the schema doesn't have yet, and
   retrofitting it means deciding a real graph for all nine current agents
   with no evidence yet of which edges actually matter.

3. **Recursion enforcement, if roster ever wants to own it, belongs at the
   dispatch layer, not the declaration layer.** Codex enforces `max_depth`
   globally, at its own runtime, not per-agent — the closer analogy for
   roster is a future dispatch-runtime concern (bitterblossom, an
   orchestrator's own judgment, or a shared primitive), not a `role.yaml`
   field with nothing behind it.

4. **This is genuinely revisitable, not closed.** If roster (or a
   consuming runtime like bitterblossom) grows an actual dispatch-time
   enforcement point, `spawns:`/`max_depth`-shaped fields are the right
   shape to add then — the omp and Codex precedent stays valid, it's just
   premature today. Re-open this decision the moment something can
   actually check it.

## What roster does instead, today

- `render_omp_agent` (`crates/roster-core/src/lib.rs`) deliberately omits
  `spawns:` (and `output:`, a separate omission — see that function's doc
  comment) from the materialized omp agent file, consistent with this
  decision.
- `subagent_rights` stays the roster-native, dispatch-agnostic signal for
  "may this agent spawn anything at all."

# Decision: Codex native role projection (roster-915, superseded 2026-07-11)

roster-915's fourth acceptance criterion: upgrade `materialize --harness
codex` from `render_brief`'s plain text to the live `[agents.<name>]` +
`~/.codex/roles/*.toml` pattern the roster-910 receipt confirmed is Codex's
actual working config shape on this machine, or explicitly defer with
reason.

## Decision: implemented

The earlier deferral was resolved by the workstation-consolidation contract.
`roster materialize --harness codex` now emits a native TOML role layer, and
`roster sync` maintains marker-bounded `[agents.<name>]` registrations in the
real `~/.codex/config.toml` while preserving every unmarked local setting and
custom role. The inert `.codex/agents/*.md` projection is retired.

## Historical rationale for the original deferral

1. **At the time of the original deferral, `sync.rs` wrote
   `.codex/agents/<name>.md`, and the receipt's
   own Codex section says the standalone-file mechanism Codex documents
   expects TOML there, not Markdown.** That means today's materialized
   Codex output may already be inert (never read by Codex's actual dispatch
   path), which makes this a real, evidenced bug worth fixing — but also
   meant fixing it correctly required re-examining what `roster sync`
   writes to `.codex/agents/`, not just what `roster materialize --harness
   codex` prints to stdout for a human to read.

2. **The receipt's own confirmed pattern for what Codex actually uses on
   this machine is `[agents.<name>]` entries inside the shared
   `~/.codex/config.toml`, not a standalone per-agent file at all.**
   Rendering that correctly means merging a new table into a config file
   roster doesn't currently own or touch — a materially different and
   riskier operation than a stdout render or a managed-marker file write
   (`sync.rs`'s existing pattern for `.claude/agents/*.md` etc.). A sloppy
   merge could silently corrupt or drop existing `[agents.<name>]` entries
   this machine's live Codex setup already depends on for genuinely
   unrelated custom agents.

3. **This deserved its own scoped card**, not a rushed addition inside a
   four-item roadmap ticket: it needs (a) a decision on whether roster
   writes directly into `~/.codex/config.toml` at all, versus emitting a
   separate role TOML plus documented manual config wiring, and (b) real
   verification against this machine's live Codex config before and after,
   the way `roster-928`'s bastion deploy was verified against sibling apps
   before calling it safe.

## Resolution

The scoped work is complete: `materialize --harness codex` emits native role
TOML, `roster sync` owns only a marker-bounded registration block, and live
Codex doctor/config parsing verifies the installed shape.
