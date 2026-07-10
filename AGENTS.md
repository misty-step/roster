# Roster Repo Contracts

Roster declares agents and materializes those declarations for execution
planes. It is not itself an execution runtime.

## Contracts

- `VISION.md` is the founding product contract. Read it before changing agent
  declaration shape, primitive registries, provider routing, or phase scope.
- Agent declarations live under `agents/<name>/` as `role.yaml` plus
  `instructions.md`. Keep declarations as data and prose; no framework code in
  agent directories.
- `role.yaml` is deterministic input and must reject unknown fields. Model-facing
  instructions stay prose in `instructions.md`.
- `primitives/skills/skills-index.yaml` and
  `primitives/skills/.external/registry.yaml` are Roster's authoritative
  primitive catalog and provenance ledger. Agent declarations reference only
  Roster-owned paths.
- `roster sync` (crates/roster-cli/src/sync.rs) materializes agent briefs, a
  skill symlink farm (`--catalog full|curated`), doctrine links, and installed
  harness config projections through one reversible manifest.
  `crates/roster-hooks` owns the live Claude hook surface, including secret
  read guards and pre-transcript output redaction. Workstation cutover was
  proven live under roster-926; Harness Kit is a retired predecessor.
- No secret values in declarations or registries; env refs only.

## Gate

Run before claiming repo changes are complete:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
