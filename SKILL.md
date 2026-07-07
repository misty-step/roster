---
name: roster
description: |
  Use when an agent needs to enumerate roster agent declarations, read one
  agent's full prompt-native brief, or materialize a declaration for a
  specific harness. Roster is the agent declaration repository for Misty
  Step Factory: `agents/<name>/role.yaml` + `instructions.md` in, prompt-native
  text out. It declares agents; it does not spawn or run them.
argument-hint: "[list|show|brief|materialize]"
---

# Roster

Roster keeps agent identity in one plain-file tree
(`agents/<name>/role.yaml` + `agents/<name>/instructions.md`) and renders it as
prompt-native text for any harness. Treat declarations as the source of truth
for an agent's role, model policy, permissions, and skill/MCP bindings — never
edit a materialized output by hand. Read `VISION.md` before changing agent
declaration shape, primitive registries, provider routing, or phase scope.

For local MCP or CLI use, set `ROSTER_ROOT` to the roster checkout (defaults to
the current directory otherwise). The CLI additionally accepts `--root` per
invocation, which takes precedence.

## Operating Contract

- Use `list` to see every declared agent before dispatching or editing one.
- Use `show` to read a single declaration's full rendered role text.
- Use `brief` to compose a dispatch-ready packet: role prompt, skill paths,
  MCP selection, permissions, subagent rights, evidence contract, and — with
  `--card`/`card` — the live Powder card context folded in.
- Use `materialize` to render one declaration into a harness-native shape
  (`claude`, `codex`, or `bb`). This is read-only rendering, not installation.
- Never hand-edit a materialized output (a rendered `.claude/agents/*.md`,
  `.codex/agents/*.md`, or bb TOML). Fix the source `role.yaml`/
  `instructions.md` and re-render.
- Identity changes (name, model policy, permissions, skills, MCPs) go through
  `role.yaml` + `instructions.md`, then re-materialize — never a direct patch
  to a rendered artifact.
- `sync` is the one mutating verb and it is opt-in and home-directory-scoped:
  it writes harness-native agent files and a manifest under the caller's
  `$HOME` (`.codex/agents/`, `.claude/agents/`, `.pi/agents/`,
  `.roster/orchestrator/`). Only run it with the operator's awareness of what
  it will touch on that machine; `sync --disable` rolls back exactly what the
  manifest recorded. `sync` has intentionally no MCP tool (see
  `crates/roster-mcp/src/lib.rs`) — it is CLI-only because an MCP call has no
  reliable notion of "the caller's home" the way a local CLI invocation does.

## Expected MCP Tools

- `list`: list roster agents from `role.yaml` declarations (optional `root`).
- `show`: show one roster agent declaration as prompt-native text (`agent`,
  optional `root`).
- `brief`: render a prompt-native dispatch brief for one roster agent
  (`agent`, optional `root`, `add_skills`, `add_mcps`).
- `materialize`: render one roster agent declaration for a specific harness —
  `claude`, `codex`, or `bb` (`agent`, `harness`, optional `root`).

## HTTP API

`roster-api` serves the same four verbs over HTTP by dispatching through
`roster_mcp::call_tool` — one implementation, two transports. `root` is
fixed at server startup (`roster-api --root <path>`), never per-request, and
`--bind` defaults to loopback (`127.0.0.1`).

```sh
roster-api --root . --port 4101
curl -s http://127.0.0.1:4101/v1/agents
curl -s http://127.0.0.1:4101/v1/agents/cerberus
curl -s "http://127.0.0.1:4101/v1/agents/sweep/brief?add_skill=extra-skill"
curl -s "http://127.0.0.1:4101/v1/agents/cerberus/materialize?harness=codex"
```

Same scope as the MCP server (no `--card` equivalent for `brief`). See
`README.md`'s "HTTP API face" section for the full route table, a live
transcript, and the recorded UI-face waiver.

## CLI

```sh
roster list
roster show cerberus
roster materialize cerberus --harness codex
roster brief cerberus
roster brief sweep --card roster-012
roster sync
roster sync --disable
```

Pass `--root <path>` to any command to point at a roster checkout other than
the current directory (the MCP server uses the `ROSTER_ROOT` env var for the
same purpose when no `root` argument is given).

## Local Gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Red Lines

- Never hand-edit a materialized output. Change `role.yaml`/`instructions.md`
  and re-materialize instead.
- Identity changes (name, model policy, permissions, skills, MCPs) go through
  `role.yaml`, not a patch to rendered text.
- Run `sync` only with the operator's awareness — it writes into `$HOME`.
- No secret values in declarations or the primitive registries; env refs only.
