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
- Resolve `GLASS_URL` as `${GLASS_URL:-https://sanctum.tail5f5eb4.ts.net:10003}`.
- Use `glass publish` when the binary is available and wired to the live Glass
  store; otherwise use `${GLASS_URL%/}/mcp` or `${GLASS_URL%/}/api/posts`.
- Contract and copy-paste commands:
  `primitives/skills/sprites/references/glass-status-beats.md`.

## Launch

```sh
roster brief <identity> --card <powder-card-id> > /tmp/lane-brief.md
# Prepend /tmp/lane-brief.md to this card, then launch it through the chosen
# harness's native subagent or peer-CLI surface.
```

Use `roster materialize <identity> --harness <harness>` when a harness-native
projection is needed. Durable receipts belong on the Powder card as a run,
comment, or link.
