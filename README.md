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

P2 adds an opt-in workstation sync for the default lead agent:

```sh
cargo run -p roster-cli -- sync
```

`roster sync` installs roster-managed lead artifacts under `.roster/lead/` and
harness-native agent files at `.codex/agents/lead.md`,
`.claude/agents/lead.md`, and `.pi/agents/lead.md` beneath the target home
directory. The curated primitive subset is reference-only: skill bodies stay in
harness-kit until the P3 primitives migration. Existing harness-kit bootstrap
globals such as `.codex/AGENTS.md`, `.claude/CLAUDE.md`, and `.pi/settings.json`
are not overwritten during the parallel run.

Rollback is manifest-driven:

```sh
cargo run -p roster-cli -- sync --disable
```

For tests or staged installs, pass `--home <path>` to either command. Disable
removes only files recorded in `.roster/lead/manifest.json`, and harness-agent
files outside `.roster/lead/` are removed only when they still carry the roster
sync marker.
