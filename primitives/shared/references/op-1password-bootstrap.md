# `op` (1Password) bootstrap: why the flags are load-bearing

The operative rules live in shared `AGENTS.md` ("A blocker needs proof as much
as a 'done' does"): the bootstrap export line, the sanitized-context rule, the
`op://`-references rule, and the keychain fallback. This file keeps the root
causes and incident history behind those rules so `AGENTS.md` stays short.

## The bootstrap line

```
export OP_SERVICE_ACCOUNT_TOKEN="${OP_SERVICE_ACCOUNT_TOKEN:-$(security find-generic-password -a "$USER" -s op-agent -w 2>/dev/null)}" OP_LOAD_DESKTOP_APP_SETTINGS=false OP_CACHE=false
```

On this machine the service-account token lives in the macOS Keychain (item
`op-agent`); the line reads it only if the env var is not already set.

## Why the service-account token

Without `OP_SERVICE_ACCOUNT_TOKEN`, `op` falls back to the desktop-app /
user-session integration (`op --account <name>`), which pops an interactive
authorize modal on every process and stalls the operator. With the token set,
`op` hits the API directly and never prompts. Re-fetching the token through the
interactive path on each call is the anti-pattern.

## Why `OP_LOAD_DESKTOP_APP_SETTINGS=false` (root-caused 2026-07-07, macOS Tahoe)

Without it, every `op` invocation — even in pure service-account mode — opens
the 1Password desktop app's Group Container to load app settings. App Data
Protection blocks that `open()` behind the TCC prompt "op would like to access
data from other apps", `op` hangs indefinitely per invoking context, and each
hung call leaves a wedged `op daemon --background` zombie (110 were found
accumulated). Clicking Allow never sticks: each invoking context (Codex,
launchd jobs, terminals, env-cleared runners) prompts separately while fleet
automation keeps firing.

## Why sanitized contexts must carry the line

Zsh loads the bootstrap automatically via `~/.zshenv`, but sanitized contexts
do NOT inherit it: `bash -c` / `bash -lc` scripts, MCP-server bootstrap
commands (harnesses spawn MCP servers with a minimal env), daemons /
LaunchAgents, cron, and any runner that clears the environment (e.g.
bitterblossom's local substrate). Any command that calls `op` in such a
context must carry the bootstrap line first. Root cause of the 2026-07-04
authorize-modal storm: a Codex MCP bootstrap ran bare `op read` on every Codex
launch. MCP server registrations must resolve refs in their bootstrap command,
because servers cache env at session start.

## Why `~/.secrets` holds references (harness-kit-914, 2026-07-07)

Since 2026-07-07, `~/.secrets` holds `op://` REFERENCES, not literal values:
`source ~/.secrets` gives you a reference string — resolve at point of use
(`op read "$VAR"`), never pass it as a credential. The tell that you forgot: an
auth header or key that starts with `op://`, and 401s from a service that
worked earlier.

## The credential-wall anecdote (found live 2026-06)

`SPRITES_TOKEN` / `FLY_API_TOKEN` / `GITHUB_PAT` sat in `orchestrator/.env`
while a multi-hour "credential wall" was narrated over production infra
(sprite reprovision, Fly deploys, secret reads) that was runnable from the
local checkout the whole time. Production infra you assume is "operator-only"
is usually runnable locally because the tokens are already on disk.
