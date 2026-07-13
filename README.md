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
  safe native arguments, and names of agents it may delegate to.
- An **agent instance** is that definition running in a workspace.

Agents do not add primitives outside their role. If two agents need different
behavior, define two roles. Composition is source-qualified and additive: there
are no implicit overrides.

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
    model: gpt-5.6-luna
    reasoning: xhigh
    harness: codex
    args: [--search, --dangerously-bypass-approvals-and-sandbox]
    delegates: [hephaestus, cerberus, scully]
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
the selected guidance, skills, MCPs, and delegates. `manifest.yaml` records
source-qualified identities, inclusion chains, workspace-context sources and
digests, file digests, model, Harness, and safe launch arguments. Runtime bundles are deleted by default;
`dispatch --keep-bundle` retains one for inspection.

## Isolation and escape hatches

Each Tier 1 adapter creates a private projection:

- Codex receives a temporary `CODEX_HOME`, bridged auth/session state, and an
  exact MCP/skill projection. A preflight rejects MCP drift before launch.
- Claude Code starts with no inherited setting sources, one ephemeral plugin,
  one strict MCP file, and the resolved runtime instructions.
- OMP receives a private agent directory, explicit skills, and an isolation
  overlay that disables ambient discovery.

The adapter preserves the child exit code and forwards termination signals.
If an exact projection cannot be proved, dispatch fails closed. Native child
agents inherit too much parent context in current Tier 1 Harnesses, so resolved
guidance directs primary agents to dispatch named Roster agents instead.

Raw `codex`, `claude`, and `omp` remain the direct escape hatches. `roster
rescue` creates an intentionally context-free repair session when composition
itself is broken.

## Receipts and optional authority

Dispatch writes a bounded, redacted receipt under
`~/.local/state/roster/receipts/` (or `$ROSTER_STATE_DIR`). It records identity,
workspace, config, model, Harness, clocks, exit status, and retained bundle
path—never prompts, environment values, or credentials.

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
