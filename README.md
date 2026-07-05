# roster

Roster is the agent declaration repository for Misty Step Factory. It keeps
agent identities, prompts, model policy, primitive references, and materializers
in one plain-file tree.

P0 provides:

- `agents/<name>/role.yaml` and `instructions.md` declarations.
- `roster list`, `roster show <agent>`, `roster materialize <agent> --harness <target>`, and `roster brief <agent>`.
- Reference-only primitive indexes for skills and MCP servers.

```sh
cargo run -p roster-cli -- list
cargo run -p roster-cli -- show cerberus
cargo run -p roster-cli -- materialize cerberus --harness codex
cargo run -p roster-cli -- brief cerberus
```

P2 adds an opt-in workstation sync for the default orchestrator agent:

```sh
cargo run -p roster-cli -- sync
```

`roster sync` installs roster-managed orchestrator artifacts under
`.roster/orchestrator/` and harness-native agent files at
`.codex/agents/orchestrator.md`, `.claude/agents/orchestrator.md`, and
`.pi/agents/orchestrator.md` beneath the target home directory. The curated
primitive subset is reference-only: skill bodies stay in harness-kit until the
P3 primitives migration. Existing harness-kit bootstrap globals such as
`.codex/AGENTS.md`, `.claude/CLAUDE.md`, and `.pi/settings.json` are not
overwritten during the parallel run.

Rollback is manifest-driven:

```sh
cargo run -p roster-cli -- sync --disable
```

For tests or staged installs, pass `--home <path>` to either command. Disable
removes only files recorded in `.roster/orchestrator/manifest.json`, and
harness-agent files outside `.roster/orchestrator/` are removed only when they
still carry the roster sync marker.

## `role.yaml` field reference

Every declaration lives at `agents/<name>/role.yaml` + `agents/<name>/instructions.md`
and is validated by `roster_core::Roster::load` (`deny_unknown_fields`: an
unrecognized key or a missing `instructions.md` fails the load, not just a lint
warning). Values below are the actual fields in use across the four seed
agents (`agents/cerberus`, `agents/oracle`, `agents/orchestrator`,
`agents/sweep`).

| Field | Type | What it's for | Actual values in use |
|---|---|---|---|
| `schema_version` | string | Declaration format version | `roster.role.v1` on all four agents |
| `name` | string | Agent id; must match the directory name and be unique across `agents/` | `cerberus`, `oracle`, `orchestrator`, `sweep` |
| `description` | string | One-sentence purpose, echoed by `roster list`/`show`/`brief` | see `roster list` output below |
| `model_policy.preferred` | string | Model tier or literal model id (see resolution table below) | `codex-class` (cerberus), `openrouter-class` (oracle, sweep), `fable-class` (orchestrator) |
| `model_policy.fallbacks` | list\<string\> | Ordered fallback tiers/models | e.g. sweep: `openrouter/moonshotai/kimi-k2.7-code`, `openrouter/deepseek/deepseek-v4-flash`, `openrouter/qwen/qwen3-coder-next` |
| `model_policy.reasoning` | string | Reasoning-effort tier, printed as-is, never parsed | `xhigh` (cerberus), `high` (oracle), `low` (orchestrator), `medium` (sweep) |
| `permissions.filesystem` | string | Free text; feeds `claude_tools()` (`Write`/`Edit` are added when this contains `"write"`) | `workspace-write` (orchestrator), `read-only` (cerberus, oracle, sweep) |
| `permissions.commands` | string | Free text; `Bash` is added by `claude_tools()` unless this is `"none"` or `"disabled-by-default"` | `allowed` (orchestrator), `verification-only` (cerberus, oracle), `read-only` (sweep) |
| `permissions.network` | string | Free text; `WebSearch` is added by `claude_tools()` only when this is exactly `"allowed"` | `allowed` (oracle, orchestrator, sweep), `disabled-by-default` (cerberus) |
| `permissions.secrets` | string | Free text, documentation only (no code branches on it today) | `env-refs-only` (orchestrator), `none` (cerberus, oracle, sweep) |
| `permissions.mutations` | string | Free text; `Write`/`Edit` are also added by `claude_tools()` when this is not `"none"` | `with-explicit-scope` (orchestrator), `none` (cerberus, oracle, sweep) |
| `skills` | list\<{name, path, reason}\> | Skill files the agent should read; `path` is an absolute filesystem path (currently all under `harness-kit`, pending the P3 primitives migration) | orchestrator has 8, cerberus has 3, oracle has 2, sweep has 3 |
| `mcps` | list\<string\> | Bare MCP server names required at dispatch time (rendered as claude/codex/brief's "MCP Servers → Required") | orchestrator: `powder`; sweep: `qmd`; cerberus, oracle: none |
| `mcps_contextual` | list\<string\>, optional (defaults empty) | MCP server names to bind only when present in the calling harness (rendered as "MCP Servers → Contextual (bind when present)"); not rendered for `bb` (no MCP concept there) | orchestrator: `qmd`, `todoist`, `bitterblossom`, `glass`; oracle: `exa`, `firecrawl`, `context7`; cerberus, sweep: none |
| `subagent_rights.may_dispatch` / `may_spawn_subagents` / `may_use_peer_harnesses` | bool | What the agent is allowed to fan work out to | orchestrator and cerberus: all `true`; oracle and sweep: all `false` (leaf lanes) |
| `evidence_expectations` | list\<string\> | Free prose bullets, no fixed vocabulary; printed verbatim under `## Evidence Contract` / `## Evidence Expectations` | see `agents/*/role.yaml` |

### Model tier vocabulary and per-harness resolution

`model_policy.preferred`/`fallbacks` carry an abstract tier string
(`codex-class`, `fable-class`, `openrouter-class`) or a literal model id
(`gpt-5.5`, `claude-opus-4-8`, `openrouter/<provider>/<model>`). What a tier
means operationally is doctrine (`VISION.md`: fable-class → strategy/planning/
review at low-to-medium reasoning, rarely high; codex-class → implementation
lanes on GPT-5.5 at high/xhigh; openrouter-class → cheap OpenRouter sweeps),
not something the CLI resolves uniformly. Each `materialize --harness` target
handles it differently, and this is current, verified behavior, not aspiration:

- **`--harness claude`** (`render_claude_agent` / `claude_model` in
  `roster-core/src/lib.rs`): ignores `model_policy` entirely and always emits
  `model: sonnet` in the frontmatter — the "Claude Code subagents are Sonnet 5"
  doctrine is hardcoded, not tier-driven.
- **`--harness codex`** and **`brief`** (`render_brief`): print `preferred`,
  `fallbacks`, and `reasoning` as literal text under `## Model Policy`. No
  resolution happens in code; a human or the orchestrator reads the tier and
  applies the doctrine above to pick a concrete model.
- **`--harness bb`** (`render_bb_agent` / `bb_model`): resolves to the first
  `openrouter/`-prefixed value found in `preferred` then `fallbacks`, stripping
  the prefix. Example: cerberus's preferred (`codex-class`) isn't
  openrouter-prefixed, so `bb_model` falls through to its second fallback and
  emits `model = "moonshotai/kimi-k2.7-code"` in the generated TOML. **Known
  gap:** if no value in `preferred`/`fallbacks` is `openrouter/`-prefixed (this
  is true of `orchestrator` today — `fable-class`, `claude-opus-4-8`,
  `gpt-5.5-pro-browser` — none match), `bb_model` emits the literal preferred
  string (`model = "fable-class"`), which is not a real invocable model. This
  only matters if a non-OpenRouter-only agent is ever bb-materialized; sweep
  (the OpenRouter-native agent) resolves correctly today.

## Add a new agent (quickstart)

1. Create `agents/<name>/role.yaml` and `agents/<name>/instructions.md`
   following the field reference above.
2. Run `cargo run -p roster-cli -- list` and confirm the new name appears.
3. Run `cargo test --workspace`. `roster_core::Roster::load` will accept the
   new agent, but note `crates/roster-core/tests/loader.rs`'s
   `loads_seed_agents_from_repo` test hardcodes the exact seed-agent name list
   — adding a real fourth seed agent means updating that assertion too, or the
   test fails on the list mismatch (not a validation error).

This was run for real while authoring this section, with a throwaway
`agents/example/role.yaml` (removed afterward — it is not a fourth seed
agent):

```
$ cargo run -q -p roster-cli -- list
cerberus	codex-class	xhigh	Code-review master agent that turns available change context into grounded findings, verdicts, and a review artifact without overstating inspected evidence.
example	openrouter-class	medium	Minimal placeholder agent used by the README quickstart to demonstrate adding a new agent.
oracle	openrouter-class	high	AI-awareness sidekick — current on state-of-the-art models, harnesses, and agent tooling; advises routing (local vs open vs frontier), critiques deterministic-where-a-model-belongs, and pushes the fleet to use AI more than instinct suggests.
orchestrator	fable-class	low	Master orchestrator — frames factory work, grooms and shapes the board, composes and dispatches lanes, compares evidence, verifies outcomes, and closes the workspace cleanly.
sweep	openrouter-class	medium	Cheap read-only research and repository sweep lane for broad scanning, source collection, and concise discrepancy reports.

$ cargo test --workspace
...
test loads_seed_agents_from_repo ... FAILED
thread 'loads_seed_agents_from_repo' panicked at crates/roster-core/tests/loader.rs:21:5:
assertion `left == right` failed
  left: ["cerberus", "example", "oracle", "orchestrator", "sweep"]
 right: ["cerberus", "oracle", "orchestrator", "sweep"]
```

The `list` step proves the loader picked up the new declaration; the `test`
step proves it validated correctly (schema-wise) and only failed on the
seed-count assertion, exactly as described above.

## `roster brief --card` transcript (real, live)

This is the actual output of `roster brief sweep --card roster-012`, run
against roster-012 — the live Powder card tracking this documentation work —
with `POWDER_API_BASE_URL`/`POWDER_API_KEY` set from `~/.secrets`:

```
$ cargo run -q -p roster-cli -- brief sweep --card roster-012
# Roster Brief: sweep

## Role

Cheap read-only research and repository sweep lane for broad scanning, source collection, and concise discrepancy reports.

## Model Policy

- Preferred: openrouter-class
- Fallbacks: openrouter/moonshotai/kimi-k2.7-code, openrouter/deepseek/deepseek-v4-flash, openrouter/qwen/qwen3-coder-next
- Reasoning: medium

## Instructions

Read: ./agents/sweep/instructions.md

# Sweep Lane

You are a cheap read-only sweep lane. Search broadly, cite exactly what you
inspected, and keep the output compact enough for the orchestrator to act on.

Use repository files, command output, and current external sources when allowed.
Separate confirmed evidence from inference. Do not edit files, update trackers,
push branches, send messages, or perform any mutating action.

Return a report with: objective, sources checked, high-signal findings,
discrepancies or gaps, and the next one or two checks that would most improve
confidence.

## Skills To Read

- research: /Users/phaedrus/Development/harness-kit/skills/research/SKILL.md (source-backed web and repository research)
- orient: /Users/phaedrus/Development/harness-kit/skills/orient/SKILL.md (fast repository orientation from live evidence)
- diagnose: /Users/phaedrus/Development/harness-kit/skills/diagnose/SKILL.md (structured triage when the sweep finds a contradiction)

## MCP Servers

### Required

- qmd

### Contextual (bind when present)

- none

## Permissions

- Filesystem: read-only
- Commands: read-only
- Network: allowed
- Secrets: none
- Mutations: none

## Subagent Rights

- May dispatch: false
- May spawn subagents: false
- May use peer harnesses: false

## Evidence Contract

- Report sources, commands, and searched paths.
- Separate confirmed facts from plausible inferences.
- Return a concise packet with findings, gaps, and next checks.
- Do not mutate files, trackers, remotes, or external systems.

## Powder Card

- ID: roster-012
- Title: Documentation floor: role.yaml field reference, a real brief --card transcript, and an add-a-new-agent quickstart

### Acceptance

- README (or a linked docs/ page) documents every role.yaml field -- schema_version, model_policy (including the tier vocabulary: codex-class/fable-class/openrouter-class and what each resolves to per harness), permissions vocabulary, skills, mcps, subagent_rights, evidence_expectations -- against the actual values used by the three seed agents.
- README includes one real, copy-pasteable transcript of `roster brief <agent> --card <id>` run against a live Powder card, showing the actual composed output -- not a hypothetical example -- since this is VISION.md's stated critical design consideration.
- README includes an add-a-new-agent quickstart: create agents/<name>/{role.yaml,instructions.md}, run `roster list` to confirm registration, run cargo test --workspace to confirm the loader validates it.
- Every command shown in the README/docs is actually run during authoring and its real output pasted in, not written from memory.

### Body

Current README is 18 lines: three bare `cargo run` invocations with no explanation of role.yaml semantics, no worked brief --card example, and no path for adding a new agent -- despite VISION.md's own 3-month bar being 'new agents are a directory, not a project.' Marketing site is explicitly out of scope (misty-step-910 owns it); this card is documentation-floor only (application-floor.md item 2).
```

This is exactly the "dynamic composition" seam `VISION.md` calls out as the
critical design consideration: one declaration (`agents/sweep/role.yaml` +
`instructions.md`), one CLI invocation, and the brief carries role prompt,
skill paths, required/contextual MCP servers, permissions, subagent rights,
evidence contract, and — when `--card` is passed — the live Powder card
context, ready for any harness to consume as prompt-native text.
