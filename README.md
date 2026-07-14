# Roster

Roster is a small compiler and launcher for exact agent environments.

It keeps an arbitrarily large library of skills, MCP declarations, and concise
guidance as plain files. Roles compose those primitives, and an agent definition
binds one role to one model and one Harness. At dispatch time Roster resolves
only that agent's graph into an inspectable, temporary bundle and launches
Codex, Claude Code, or OMP without installing a global skill catalog.

## Install

```sh
git clone https://github.com/misty-step/roster
cd roster
cargo install --locked --path crates/roster-cli
mkdir -p ~/.roster
cp examples/config.yaml ~/.roster/config.yaml
# Change the `core` source to this checkout's absolute path.
roster check
```

Nothing is synced into Harness configuration. Existing authentication,
sessions, caches, preferences, and raw Harness commands remain user-owned.

## The vocabulary

- A **primitive** is one skill, MCP declaration, or guidance fragment.
- A **pack** is an additive bundle of primitive or pack references.
- A **role** is the complete semantic composition for a kind of work.
- An **agent definition** binds a name, description, role, model, Harness,
  and safe native arguments. Any agent may dispatch any other agent; the
  `roster` skill teaches the CLI mechanics.
- An **ad-hoc role** is an ephemeral include list for one lane. It borrows a
  named agent's runtime binding but none of that agent's role primitives.
- An **agent instance** is that definition running in a workspace.

Durable agents do not add primitives outside their role. If behavior recurs,
define another role. For one-off work, resolve an ad-hoc role through the same
source-qualified additive include language; there are no implicit overrides.

## Configuration hierarchy

Roster searches from the working directory toward the filesystem root for the
nearest `.roster/config.yaml`. If none exists, it uses
`~/.roster/config.yaml`. A nearer config fully replaces the home config;
sharing is always explicit through `imports` or `sources`.

```yaml
schema_version: roster.config.v1
sources:
  core: /absolute/path/to/roster
agents:
  amos:
    description: Codex orchestrator for broad software work.
    role: core/role:orchestrator
    model: gpt-5.6-sol
    reasoning: high
    harness: codex
    args: [--search, --dangerously-bypass-approvals-and-sandbox]
```

Private roles and primitives can live beside a private config and compose the
public library without being committed. `roster init --source <checkout>`
creates a deliberately small local starting point and refuses to overwrite an
existing config.

## Use

```sh
roster                         # interactive agent picker; list when non-TTY
roster list                    # effective agents for this workspace
roster show amos               # binding plus exact resolved composition
roster resolve amos --output /tmp/amos
roster dispatch amos           # resolve, project, and launch
roster dispatch amos --dry-run # inspect the exact launch command
roster resolve --using amos --as dependency-scout \
  --purpose "Map one dependency." \
  --include core/pack:engineering-core \
  --include core/skill:research \
  --output /tmp/dependency-scout
roster dispatch --using amos --as dependency-scout \
  --purpose "Map one dependency." \
  --include core/pack:engineering-core \
  --include core/skill:research
roster inspect amos            # effective config, graph, recent receipts
roster check                   # config graph plus public-catalog gate
roster rescue codex            # raw skeletal Harness for Roster repair
```

`resolve` emits the canonical bundle:

```text
AGENTS.md
skills/
mcps.yaml
manifest.yaml
```

The generated `AGENTS.md` identifies the role and routes the model to exactly
the selected guidance, skills, and MCPs. `manifest.yaml` records
source-qualified identities, inclusion chains, workspace-context sources and
digests, file digests, purpose, binding, model, Harness, and safe launch
arguments. Runtime bundles are deleted by default; `dispatch --keep-bundle`
retains one for inspection.

`--using` supplies only Harness, model, reasoning, and validated native
arguments. The repeatable `--include` flags are the complete ephemeral role;
the named binding's role is not merged. Resolution is memory-only. If the same
composition becomes useful repeatedly, promote it to a declared role and agent
instead of preserving command-line folklore.

## Isolation and escape hatches

Each Tier 1 adapter creates a private projection:

- Codex receives a temporary `CODEX_HOME`, bridged auth/session state, and an
  independent MCP/skill projection; bundled and ambient skills are disabled.
  The projection marks the canonical project root untrusted, so project-local
  Codex config, hooks, and exec policies cannot alter the agent. Roster embeds
  the workspace `AGENTS.md` chain itself and disables Codex's second
  project-doc read to avoid duplicate instructions. Strict config-loader,
  enabled-skill inventory, and MCP-inventory preflights all pass before launch.
- Claude Code starts with no inherited setting sources, one ephemeral plugin,
  one strict MCP file, and the resolved runtime instructions.
- OMP receives a private agent directory, explicit skills, and an isolation
  overlay that disables ambient discovery.

The adapter preserves the child exit code and forwards termination signals.
If an exact projection cannot be proved, dispatch fails closed. Native child
agents inherit too much parent context in current Tier 1 Harnesses, so resolved
guidance directs primary agents to dispatch independently resolved named or
ad-hoc Roster agents instead.

Raw `codex`, `claude`, and `omp` remain the direct escape hatches. `roster
rescue` creates an intentionally context-free repair session when composition
itself is broken.

## Receipts and optional authority

Dispatch writes a bounded, redacted receipt under
`~/.local/state/roster/receipts/` (or `$ROSTER_STATE_DIR`). It records identity,
binding, role, exact effective primitive identities, workspace, config, model,
Harness, clocks, exit status, and retained bundle path—never prompts,
environment values, or credentials.
Preflight failures are receipted before the temporary projection is removed;
their child exit status is empty because the Harness never launched.
Bundles use `roster.bundle.v2` and dispatch receipts use `roster.receipt.v2`;
the v2 audit fields intentionally replace the pre-1.0 v1 schemas.

A config may declare an optional external authority command. Inside a launched
session, `roster authority request <capability>` asks that provider to grant,
proxy, seek approval for, or deny one named operation. Roster records only the
request metadata and result. Mint can implement this interface, but Roster does
not depend on Mint and denial never invalidates the whole session.

## Development gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Read `VISION.md` before changing composition semantics or product scope.
