# Lane Card

Role:
Objective:
Provider target:
Model override: none
Scope:
Inputs / oracle:
Allowed skills:
Allowed tools:
Output shape:
Do not touch:
Stop conditions:
Receipt expectation:
Lane harness: none
Glass status beats:
- Publish session start, milestone, blocked, and shipped one-line beats to
  Glass in the same session.
- Require `GLASS_URL` from the private runtime environment; public lane cards
  never embed one operator's deployment hostname.
- Use `glass publish` when the binary is available and wired to the live Glass
  store; otherwise use `${GLASS_URL%/}/mcp` or `${GLASS_URL%/}/api/posts`.
- Contract and copy-paste commands:
  `primitives/skills/sprites/references/glass-status-beats.md`.

## Launch

```sh
roster show <agent>
roster dispatch <agent>
# Paste this card into the launched session. The selected agent's complete
# role is resolved independently; child-only primitives never load in the lead.
```

If no declared agent fits one lane, use Roster's explicit ad-hoc selector and
make its repeatable `--include` values the complete ephemeral role. Add a role
and atomic binding only when that composition should recur. Durable evidence
belongs on the Powder card; Roster writes the local dispatch receipt
automatically.
