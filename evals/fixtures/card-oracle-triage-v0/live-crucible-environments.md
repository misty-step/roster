# Operating environments as first-class declarations: run eval X in env A vs env B in one command

Status: done · Priority: p1

Campaign 2026-07-07-focus, lane crucible-evals (operator-ratified reframe). Crucible becomes an operator-driven eval workbench: a first-class, declared operating environment — model + provider config + harness + tool allowlist (+ fixtures where applicable) — that an eval spec can be run *in*, so "run eval X in env A vs env B" is one command and the resulting runs land in the ledger with full config identity and are comparable via the existing MDE/noise-floor compare surface.

Constraints: build on the just-shipped config-identity/calibration machinery, do not rebuild it; do not lower calibration gates; no anonymous runs — every env-run carries full config identity. Study bb's substrate abstraction (bitterblossom-938) first; reuse/compose if it fits, record why if it doesn't. Change-based triggering is explicitly out of scope (separate shaped card).

## Acceptance
- An environment declaration (model/provider/temp/max/harness/tool_allowlist...) exists as a durable artifact on disk, validated before use
- One command runs a named eval spec across >=2 named environments and persists all runs to the ledger with full config identity
- A comparison between the env-runs renders through the existing compare surface with paired stats / resolution / attribution labels intact
- A real working session (one fleet-stakes eval, >=2 models/configs, operator-readable comparison) is executed end-to-end and its receipts recorded
- Repo gate ./scripts/check.sh passes; docs (SKILL.md/CLAUDE.md) updated with the new command surface
