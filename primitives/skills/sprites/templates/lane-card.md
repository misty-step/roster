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
- Resolve `GLASS_URL` as `${GLASS_URL:-https://bastion.tail5f5eb4.ts.net:10003}`.
- Use `glass publish` when the binary is available and wired to the live Glass
  store; otherwise use `${GLASS_URL%/}/mcp` or `${GLASS_URL%/}/api/posts`.
- Contract and copy-paste commands:
  `primitives/skills/sprites/references/glass-status-beats.md`.

## Launch

```sh
cargo run --locked -p harness-kit-checks -- dispatch-agent \
  --provider-target <provider> \
  --model-override <variant-or-id> \
  --objective "<objective>" \
  --input-ref "<path-or-ticket>" \
  --prompt-file <lane-card-path> \
  --repo <target-repo> \
  --backlog-ref <work-ref>
```

Add `--lane-harness <manifest>` only when the lane needs focused visible skills.
Use `--repo <target-repo>` when launching from outside the target repo; default
receipts, transcripts, local roster discovery, and lane-harness projection are
scoped to that repo unless explicitly overridden.
Remove `--model-override` when the card says `none`.
