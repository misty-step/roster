# Claude Code Harness Notes

Claude Code is a primary roster lane for Harness Kit. Use it for independent
dynamic delegation when the lead is not already Claude.

## Dispatch Shape

Use print mode with permissions bypassed and pass one bounded commission as the
prompt:

```sh
claude -p --dangerously-skip-permissions --permission-mode bypassPermissions --model claude-opus-4-8 --effort xhigh "Role: reviewer. Objective: inspect this diff. Output: 5 bullets."
```

The roster entry should stay thin: launch Claude, provide the scoped prompt,
and let `harness-kit-checks dispatch-agent` capture transcript evidence and
write a sanitized receipt. Do not encode workflow semantics in the command.

## Dynamic Delegation Notes

- Commission the lane with role, objective, scope, output shape, and boundaries.
- Give only the relevant repo paths, diff, oracle, receipts, or logs.
- Ask Claude for concrete findings or decisions, not a general chat.
- Treat the output as evidence; the lead owns synthesis and verification.
- Record failed, rejected, or partially accepted attempts like successful ones.

## Merge additively — never wholesale-replace `~/.claude/settings.json`

`settings.json` here declares ONLY roster's own hooks (roster-hooks) and env.
The operator's live `~/.claude/settings.json` also carries third-party
integrations roster does not own — most importantly **herdr's SessionStart
hook** (`bash ~/.claude/hooks/herdr-agent-state.sh session`, written by
`herdr integration install claude`), which counterspell depends on to bind a
pane to its agent session. A wholesale copy of this template DROPS that hook
and silently disables counterspell's drift remediation (it hard-gates on
ambiguous panes when no pane reports a session id — root cause of the
2026-07-07 non-activation incident).

Install rule: MERGE roster's `hooks`/`env` into the existing file, preserving
any SessionStart hook roster did not author. After any settings install or
machine bootstrap, run `herdr integration install claude` to (re)assert the
herdr wiring — it is idempotent and non-destructive.
