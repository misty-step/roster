# roster

Roster is the agent declaration repository for Misty Step Factory. It keeps
agent identities, prompts, model policy, primitive references, and materializers
in one plain-file tree.

## Install (one machine, push-button to verified-live)

```sh
git clone https://github.com/misty-step/roster && cd roster
cargo install --locked --path crates/roster-cli     # `roster`
cargo install --locked --path crates/roster-mcp     # `roster-mcp` (MCP face)
cargo install --locked --path crates/roster-api     # `roster-api` (HTTP face)
cargo install --locked --path crates/roster-hooks   # `roster-hooks` (Claude hooks)
roster sync --catalog full --all-agents             # the machine is now roster-managed
```

# Third-party harness integrations roster does not own (run once, idempotent):
herdr integration install claude   # SessionStart hook counterspell needs to bind panes
```

`roster sync` installs everything declared here: the full skill catalog
(symlinked into every detected harness), every agent identity
(`~/.claude/agents/`, `~/.codex/agents/`, `~/.pi/agents/`), and the composed
**home doctrine** — shared operating doctrine + the orchestrator identity +
its skill/MCP bindings — as the global instructions file for Claude, Codex,
pi/omp, and OpenCode. Result: any agent you launch in any harness boots as
the declared orchestrator and dispatches other roster agents ad hoc.
Hook wiring for Claude lives in `harnesses/claude/settings.json` (merge into
`~/.claude/settings.json`). Everything sync writes is marker-tracked and
reversible: `roster sync --disable`.

Verified-live check:

```sh
roster check                      # catalog gate: frontmatter, paths, index, markers
roster list                       # all agents parse
claude -p "state your session identity per your loaded instructions"
# → "Session Identity: orchestrator (roster)"
```

Both binaries read the roster checkout via `--root <path>` (CLI) or the
`ROSTER_ROOT` env var (MCP server); default is the current directory. See
`SKILL.md` for the MCP tool contract and agent-facing operating rules.

Day-to-day verbs:

```sh
roster list                                  # who exists
roster show cerberus                         # one declaration, prompt-native
roster materialize cerberus --harness codex  # per-harness install/brief form
roster brief cerberus --card <powder-id>     # dispatch brief with live card folded in
```

## `role.yaml` field reference

Every declaration lives at `agents/<name>/role.yaml` + `agents/<name>/instructions.md`
and is validated by `roster_core::Roster::load` (`deny_unknown_fields`: an
unrecognized key or a missing `instructions.md` fails the load, not just a lint
warning). `agents/*/role.yaml` is the inventory; use `roster list` and
`roster show <name>` for current values instead of copying that catalog into
documentation.

| Field | Type | What it's for |
|---|---|---|
| `schema_version` | string | Declaration format version; currently `roster.role.v1`. |
| `name` | string | Agent id; must match the directory name and be unique. |
| `description` | string | One-sentence purpose echoed by `list`, `show`, and `brief`. |
| `model_policy.preferred` | {model, reasoning} | Concrete model id plus reasoning effort. |
| `model_policy.fallbacks` | list\<{model, reasoning}\> | Ordered fallback ids, each with its own reasoning. |
| `permissions.filesystem` | string | Adds native write/edit tools only when it contains `write`. |
| `permissions.commands` | string | Adds unrestricted shell tools only for `allowed` or `verification-only`; `read-only` stays shell-free. |
| `permissions.network` | string | Adds native search only when exactly `allowed`. |
| `permissions.secrets` | string | Declared secret posture; documentation unless a target renderer maps it. |
| `permissions.mutations` | string | External mutation scope. It never implies filesystem write access; narrow known scopes map to narrow MCP tools. |
| `skills` | list\<{name, path, reason}\> | Skill files the agent should read, with the reason each belongs. |
| `mcps` | list\<string\> | Required MCP servers. Materialization fails when a target cannot bind them. |
| `mcps_contextual` | list\<string\>, optional | Optional MCP servers, rendered as “bind when present.” |
| `subagent_rights.*` | bool | Declared dispatch, spawn, and peer-harness rights. |
| `evidence_expectations` | list\<string\> | Output-shaped evidence contract rendered verbatim. |

### Concrete model ids and per-harness resolution

Model policy v2 (roster-924) retired the abstract tier vocabulary
(`codex-class`/`fable-class`/`openrouter-class`) that `model_policy` used to
carry. `preferred` and every `fallbacks` entry are now always a CONCRETE,
invocable model id (`gpt-5.5`, `claude-fable-5`, `claude-opus-4-8`,
`openrouter/<provider>/<model>`) paired with its own `reasoning` value — no
role-level `reasoning` field, no symbol a human has to decode against
doctrine. What each concrete id is *for* is still doctrine (`VISION.md`:
`claude-fable-5` → strategy/planning/review, rarely high reasoning;
`gpt-5.5` → implementation lanes at high/xhigh; the OpenRouter ids → cheap
sweeps), but rendering never needs to consult that doctrine — the id itself
is what gets invoked.

Most concrete ids are already the token their target harness needs
(`openrouter/`-prefixed ids for `bb`; `sonnet`/`opus`/`haiku`/`inherit` for
`claude`) and resolve with no lookup at all. The handful that need real
per-harness translation — `claude-fable-5`, `gpt-5.5`, and
`openrouter/deepseek/deepseek-v4-flash` — go through
`primitives/models.yaml` (retires `primitives/tiers.yaml`; still a distinct
concept from the pre-existing `primitives/providers.yaml`, which is an
unrelated peer-harness-CLI dispatch table migrated from harness-kit's
`agents.yaml` at P0). `roster_core::Models::load` reads it, consulted only
for `model_policy.preferred` — never for fallbacks:

- **`--harness claude`** (`render_claude_agent` / `claude_model` in
  `roster-core/src/lib.rs`): looks `preferred.model` up in `models.yaml`'s
  `models` table for the `claude` column (e.g. `claude-fable-5` → `inherit`,
  `gpt-5.5` → `sonnet`, `openrouter/deepseek/deepseek-v4-flash` → `haiku`).
  If `preferred.model` isn't in the table, a small conservative literal-id
  map applies (`claude-opus-4-8` → `opus`, etc.); anything still unrecognized
  falls back to `inherit` (the subagent runs on the session's own model)
  rather than guessing.
- **`--harness codex`** and **`brief`** (`render_brief`): print
  `preferred`/`fallbacks` as literal `model (reasoning: x)` text under
  `## Model Policy`. No table resolution happens here — codex materializes
  straight from the declaration; nothing in this repo yet targets the
  `~/.codex/roles/<name>.toml` shape the roster-910 dispatch-matrix research
  found on this machine.
- **`--harness bb`** (`render_bb_agent` / `bb_model`): required-MCP agents fail
  before rendering because BB has no MCP binding surface. MCP-free agents
  resolve to the first
  `openrouter/`-prefixed model found in `preferred` then `fallbacks`,
  stripping the prefix (this is how ai-scout and sweep resolve,
  since their `preferred` id is already `openrouter/`-prefixed). Only if
  neither is `openrouter/`-prefixed does it fall through to `models.yaml`'s
  `bb` column for `preferred.model` (e.g. `claude-fable-5` and `gpt-5.5` both
  → `openrouter/moonshotai/kimi-k2.7-code`, prefix stripped the same way —
  this is how incident-hound resolves). If the id isn't in the table either,
  `render_bb_agent` returns `Err` — it never emits an uninvokable id like
  `model = "gpt-5.5"` into the generated TOML.

### Default ad hoc subagent pool

`primitives/subagent-pool.yaml` declares, once, the models every roster
agent should reach for when it spawns an ad hoc subagent mid-task (distinct
from the declared `agents/*` identities dispatched through `roster brief` —
this is the fallback pool for one-off fan-out a lane decides it needs).
Every agent's `subagent_rights.may_spawn_subagents` is `true` (roster-924),
and each `instructions.md` carries one line pointing at the file rather than
re-listing the pool per agent: "Dispatch ad hoc subagents where useful;
favor the pool declared in `primitives/subagent-pool.yaml`." The pool itself
is operator-curated and extensible — add rows on capability-ledger evidence,
not vibes.

## Add a new agent (quickstart)

1. Create `agents/<name>/role.yaml` and `agents/<name>/instructions.md`
   following the field reference above.
2. Run `cargo run -p roster-cli -- list` and confirm the new name appears.
3. Run `cargo test --workspace`. `roster_core::Roster::load` will accept the
   new agent, but note `crates/roster-core/tests/loader.rs`'s
   `loads_seed_agents_from_repo` test hardcodes the exact seed-agent name list
   — adding an agent means updating that assertion too, or the
   test fails on the list mismatch (not a validation error).

Historical loader proof from the founding roster follows. Its names and models
are intentionally non-authoritative; `roster list` is the live inventory. A
throwaway `agents/example/role.yaml` demonstrated that discovery succeeds before
the exact-list regression assertion is updated:

```
$ cargo run -q -p roster-cli -- list
builder	gpt-5.5	high	Delivery lane that takes one ticket from a stated goal to a working, gated change — red-green-refactor discipline, live proof, gates never lowered to reach green.
cerberus	gpt-5.5	xhigh	Code-review master agent that turns available change context into grounded findings, verdicts, and a review artifact without overstating inspected evidence.
designer	claude-fable-5	medium	Visible-artifact critique and polish across UI, design-system primitives, docs pages, and diagrams.
example	openrouter/deepseek/deepseek-v4-flash	medium	Minimal placeholder agent used by the README quickstart to demonstrate adding a new agent.
incident-hound	gpt-5.5	xhigh	Live-system incident root cause — production failures, not diff review.
oracle	openrouter/deepseek/deepseek-v4-flash	high	AI-awareness sidekick — current on state-of-the-art models, harnesses, and agent tooling; advises routing (local vs open vs frontier), critiques deterministic-where-a-model-belongs, and pushes the fleet to use AI more than instinct suggests.
orchestrator	claude-fable-5	high	Master orchestrator — frames factory work, grooms and shapes the board, composes and dispatches lanes, compares evidence, verifies outcomes, and closes the workspace cleanly.
sweep	openrouter/deepseek/deepseek-v4-flash	medium	Cheap read-only research and repository sweep lane for broad scanning, source collection, and concise discrepancy reports.
verifier	gpt-5.5	high	Adversarial verification lane that reproduces a claim live before trusting it — read-only, findings-only, never fixes what it verifies.

$ cargo test --workspace
...
test loads_seed_agents_from_repo ... FAILED
thread 'loads_seed_agents_from_repo' panicked at crates/roster-core/tests/loader.rs:29:5:
assertion `left == right` failed
  left: ["builder", "cerberus", "designer", "example", "incident-hound", "oracle", "orchestrator", "sweep", "verifier"]
 right: ["builder", "cerberus", "designer", "incident-hound", "oracle", "orchestrator", "sweep", "verifier"]
```

The `list` step proves the loader picked up the new declaration; the `test`
step proves it validated correctly (schema-wise) and only failed on the
seed-count assertion, exactly as described above.

## `roster brief --card` transcript (real, live)

This is the actual output of `roster brief sweep --card roster-924`, run
against roster-924 — the live Powder card that shipped model policy v2 —
with `POWDER_API_BASE_URL`/`POWDER_API_KEY` set from `~/.secrets`:

```
$ cargo run -q -p roster-cli -- brief sweep --card roster-924
# Roster Brief: sweep

## Role

Cheap read-only research and repository sweep lane for broad scanning, source collection, and concise discrepancy reports.

## Model Policy

- Preferred: openrouter/deepseek/deepseek-v4-flash (reasoning: medium)
- Fallbacks: openrouter/moonshotai/kimi-k2.7-code (reasoning: medium), openrouter/qwen/qwen3-coder-next (reasoning: medium)

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

Dispatch ad hoc subagents where useful; favor the pool declared in
`primitives/subagent-pool.yaml`.

## Skills To Read

- research: /Users/phaedrus/Development/roster/primitives/skills/research/SKILL.md (source-backed web and repository research)
- orient: /Users/phaedrus/Development/roster/primitives/skills/orient/SKILL.md (fast repository orientation from live evidence)
- diagnose: /Users/phaedrus/Development/roster/primitives/skills/diagnose/SKILL.md (structured triage when the sweep finds a contradiction)

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
- May spawn subagents: true
- May use peer harnesses: false

## Evidence Contract

- Report sources, commands, and searched paths.
- Separate confirmed facts from plausible inferences.
- Return a concise packet with findings, gaps, and next checks.
- Do not mutate files, trackers, remotes, or external systems.

## Powder Card

- ID: roster-924
- Title: Model policy v2: concrete models + per-entry reasoning; kill the class tiers; declared subagent pool

### Acceptance

- *-class strings gone from every role.yaml and from the loader schema; concrete model ids + per-entry reasoning everywhere
- orchestrator: claude-fable-5/high with gpt-5.5/xhigh fallback (ruled binding)
- tiers.yaml retired; harness-token translation (if needed) lives in primitives/models.yaml keyed by concrete id
- subagent pool declared in primitives; all 8 agents may_spawn_subagents true with the dispatch-encouragement instructions line
- materialize green for all 8 agents on claude+bb; agents page regenerated + republished; gates green

### Body

Operator ruling 2026-07-06 (voice): the *-class notations make no sense -- the real axes are (1) primitives/config/tools/prompt (the identity), (2) the harness it runs in, (3) the model and reasoning. Redesign: model_policy becomes CONCRETE -- preferred {model, reasoning} and fallbacks [{model, reasoning}] (per-entry reasoning; schema change, loader deny_unknown_fields updated). RULED BINDINGS: orchestrator = claude-fable-5 reasoning HIGH, fallback gpt-5.5 reasoning XHIGH. [...]
```

This is exactly the "dynamic composition" seam `VISION.md` calls out as the
critical design consideration: one declaration (`agents/sweep/role.yaml` +
`instructions.md`), one CLI invocation, and the brief carries role prompt,
skill paths, required/contextual MCP servers, permissions, subagent rights,
evidence contract, and — when `--card` is passed — the live Powder card
context, ready for any harness to consume as prompt-native text.

## HTTP API face

`roster-api` serves the same four core verbs as the CLI and MCP server
(`list`/`show`/`brief`/`materialize`) over HTTP, so any service that wants
roster's registry without shelling out or speaking MCP's stdio JSON-RPC can
reach it as plain JSON. It is a thin transport: every route dispatches
through `roster_mcp::call_tool`, the same dispatcher the MCP server exercises
— "same semantics as the CLI" holds because it's the same code path, not a
second implementation kept in sync by hand.

```sh
roster-api --root . --bind 127.0.0.1 --port 4101
```

`--root` is fixed once at startup, unlike the CLI's `--root` flag or the MCP
server's `ROSTER_ROOT` env var. Those are trusted local invocations; an HTTP
server may be reached by callers who should not get to pick which directory
on the host gets read, so root is a server-configuration concern here, not a
per-request one. `--bind` defaults to loopback for the same reason: nothing
else scopes access once a port is reachable off-host.

Real transcript, run against a live `roster-api --root . --port 4101`:

```
$ curl -s http://127.0.0.1:4101/health
{"status":"ok"}

$ curl -s http://127.0.0.1:4101/v1/agents/orchestrator | python3 -c \
    "import json,sys; print(json.load(sys.stdin)['content'][0]['text'][:120])"
# orchestrator

Master orchestrator — frames factory work, grooms and shapes the board, composes and dispatch

$ curl -s "http://127.0.0.1:4101/v1/agents/sweep/materialize?harness=codex" | python3 -c \
    "import json,sys; print(json.load(sys.stdin)['content'][0]['text'][:80])"
# Roster Brief: sweep

## Role

Cheap read-only research and repository sweep lane f

$ curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:4101/v1/agents/nope-nobody
404
```

Routes:

| Method | Path                                | Verb           |
|--------|--------------------------------------|----------------|
| GET    | `/health`                            | liveness check |
| GET    | `/v1/agents`                         | `list`         |
| GET    | `/v1/agents/{agent}`                 | `show`         |
| GET    | `/v1/agents/{agent}/brief`           | `brief`; optional `?add_skill=a,b&add_mcp=x,y` (comma-separated) |
| GET    | `/v1/agents/{agent}/materialize`     | `materialize`; required `?harness=claude\|codex\|bb\|omp` |

Every response body is the same JSON shape MCP tool calls return
(`content`/`structuredContent`/`isError`); HTTP status maps validation
failures to `404` (unknown agent/tool) or `400` (missing/invalid field), and
anything else to `500`. `brief`'s CLI-only `--card` (live Powder context) has
no HTTP or MCP equivalent yet — this face matches the MCP server's existing
scope, not the CLI's full surface, so there's exactly one non-CLI-parity gap
today and it's the same one MCP already has.

**UI face:** `GET /` (below) supersedes the "no UI planned" waiver this
section originally recorded (roster-941) — the operator asked for a
persistent, live-reading UI (roster-928), so it now exists.

## Persistent roster UI

`GET /` on `roster-api` serves a full HTML page — every declared agent's
identity, model policy, permissions, skills, and the default subagent pool —
read straight off the live checkout on every request. No regenerate step:
edit a `role.yaml`, reload the page, see the change. This supersedes
`scripts/generate-agents-page.py`'s regenerate-then-publish workflow (the
"koan-minimal v1"); the page content and house styling are a faithful port
of that same already-shipped design (`primitives/skills/artifact/scripts/
artifact_create.py`'s house shelf template), so this was a serving-mechanism
change, not a redesign.

```sh
roster-api --root . --port 4101
open http://127.0.0.1:4101/
```

Read-only v1: identity changes still go through `role.yaml` +
`instructions.md` in git, never this page (footer says so on every load).
Verified live at desktop and 390px mobile widths with Playwright: zero
console/page errors, no horizontal overflow at 390px, and the per-agent
`<details>` drill-down works with touch/click at both widths.

Reachable from the Sanctum portal as a supervised box app once vendored into
`misty-step/bastion` per that repo's established pattern (see `bastion.toml`
and the `vendor/{cairn,crucible,powder}` precedent) and given a fleet
directory (`registry.toml`) entry — see `misty-step/bastion`'s own docs for
that half of the deploy.
