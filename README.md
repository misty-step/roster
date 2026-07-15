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
defaults:
  codex: amos
sources:
  core: /absolute/path/to/roster
agents:
  amos:
    description: Codex orchestrator for broad software work.
    role: core/role:orchestrator
    model: gpt-5.6-sol
    reasoning: high
    harness: codex
    args: [--search, --sandbox, workspace-write, --ask-for-approval, on-request]
```

Private roles and primitives can live beside a private config and compose the
public library without being committed. `roster init --source <checkout>`
creates a deliberately small local starting point and refuses to overwrite an
existing config.

The public checkout contains portable declarations only. Public MCP commands
name binaries expected on `PATH`; deployment endpoints and runtime values enter
through declared environment references. Operator home paths, private hostnames,
account-specific authority, vault/Keychain lookup commands, and machine-local
registrations belong in an explicit private source beside the operator config.
Historical evidence uses symbolic roots such as `$ROSTER_ROOT` and
`$CRUCIBLE_ROOT`. This keeps the public graph reproducible without turning
Roster into a workstation configuration manager.

## Use

```sh
roster                         # interactive agent picker; list when non-TTY
roster list                    # effective agents for this workspace
roster show amos               # binding plus exact resolved composition
roster resolve amos --output /tmp/amos
roster dispatch amos           # resolve, project, and launch
roster dispatch --default codex # launch this config's explicit Codex default
roster dispatch amos --dry-run # inspect command only; does not run preflight
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

`defaults` is an optional, strict Harness-to-agent map for stable shell
shortcuts. The named agent must exist in the same effective config and use the
mapped Harness. Roster never guesses a default from role names, declaration
order, or workspace paths. This lets `roster dispatch --default claude` select
Kaylee under the home config and Penelope under an R90-local config while both
identities remain honest. Unless `--config` is passed explicitly, this shortcut
performs ordinary nearest-config discovery and does not inherit a running
agent's `ROSTER_CONFIG` pin.

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
  one strict MCP file, and the resolved runtime instructions. Its preflight
  verifies every projected CLI flag against the live capability surface,
  strictly validates the generated plugin, and checks the exact MCP projection.
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

Adapter compatibility is capability-based, not patch-version allowlisted.
Harness versions are diagnostic evidence in launch output and receipts; the
concrete preflight for the projected config, skills, MCPs, model, and supported
flags decides whether dispatch may launch. `--dry-run` deliberately skips those
live probes and says so on stderr.

Every process Roster starts—preflight, final Harness, rescue, catalog check,
and optional authority provider—uses one clean environment constructor. It
starts empty, restores a small OS/runtime allowlist, maps explicitly supplied
`ROSTER_CHILD_ENV_<NAME>` values to `<NAME>`, then applies Roster's projection
last. This gives workstation configuration a value-free declaration seam
without making Roster a credential reader or resolver. Runtime values are
never printed by dry-run or persisted in receipts.

## Receipts and optional authority

Dispatch writes a bounded, redacted receipt under
`~/.local/state/roster/receipts/` (or `$ROSTER_STATE_DIR`). It records identity,
binding, role, exact effective primitive identities, workspace, config, model,
Harness and observed version, preflight result, clocks, exit status, and
retained bundle path—never prompts, environment values, or credentials.
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
