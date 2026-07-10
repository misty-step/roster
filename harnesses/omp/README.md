# OMP Harness Notes

OMP (oh-my-pi) is a supported Roster harness the operator can drive directly.
Roster materializes declared identities for OMP; the active harness owns native
subagent execution, and Powder owns durable receipts.

## Config Surfaces

**Global (`~/.omp/agent/config.yml`).** Model roles, fallback chains, LSP,
compaction, memory, advisor. The global config is user-owned and shared across
all repos.

**Roster source (`harnesses/omp/`).** Keep portable OMP-specific notes and
projection inputs here. Operator-local `.omp/` files remain user-owned unless a
shaped ticket proves a managed projection is worth adding.

## Model Composition

OMP's role-based routing assigns different models to different work types in
the user's global config. Keep repo source limited to portable doctrine and
projection inputs; provider declarations live in `primitives/providers.yaml`
and model translations in `primitives/models.yaml`.

## What's Different from Codex / Claude Code

- **TTSR rules** — regex-matched invariants injected mid-stream; survive
  compaction. If Roster needs source-controlled OMP rules later, they
  should be generated from shared doctrine or shaped as explicit bootstrap
  inputs, not committed as root `.omp/` drift.
- **Hashline edits** — content-hash anchored; rejects stale patches before
  corrupting files. No string-not-found loops.
- **In-process search** — ripgrep, glob, find linked into the binary. No
  fork-exec per search call.
- **LSP wired into writes** — rename, diagnostics, go-to-def through the
  language server. Requires `rustup component add rust-analyzer` for Rust.
- **Persistent Python + JS kernels** — kernels can call back into agent tools
  (`read`, `search`, `task`) over a loopback bridge.
- **Memory** — `mnemopi` per-project local recall between sessions.
- **Advisor** — passive per-turn review by a different model family.

## Harness-Agnostic Principle

The shared doctrine (`primitives/shared/AGENTS.md`, references, and skills) is
harness-agnostic and portable across all harnesses. OMP-specific config does
not leak into shared doctrine.
