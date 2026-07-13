# Roster Repo Contracts

Roster composes plain-file primitives into exact agent bundles and launches
those bundles through thin Tier 1 Harness adapters. It is not an execution
runtime, workflow engine, or workstation sync authority. Read `VISION.md`
before changing the declaration graph, CLI, adapters, or product boundary.

## Contracts

- Semantic composition lives only in roles: name, description, and one
  additive `include` list. Agent definitions bind role + model + Harness + args;
  they do not add primitives.
- Source identities are source-qualified and have no implicit override
  semantics. Resolution must remain explainable and provenance-complete.
- Harness adapters may translate and launch an immutable bundle. They may
  not mutate its meaning or normalize Harness-native model topology.
- Public roles and packs live under `roles/` and `packs/`; guidance, skills,
  and MCP declarations live under `primitives/`. Active agent definitions live
  inline in the effective `.roster/config.yaml`.
- Config, role, pack, index, and registry YAML reject unknown fields.
- `roster resolve` materializes one immutable `AGENTS.md` + skills + MCPs +
  manifest bundle. `roster dispatch` creates a temporary Tier 1 projection for
  exactly one selected agent and removes it after exit unless retained.
- Claude Code, Codex, and OMP are the only Tier 1 Harnesses in v0.2. Their
  projections require live isolation evidence; unsupported or drifting
  projections fail closed.
- Roster never syncs global Harness state. Nearest `.roster/config.yaml`
  replaces the home config; reuse happens only through explicit sources or
  imports. Unknown Harness artifacts remain user-owned.
- No secret values in declarations, bundles, manifests, fixtures, or logs.

## Source boundaries

- `primitives/skills/skills-index.yaml` and
  `primitives/skills/.external/registry.yaml` are the skill catalog and
  provenance ledger.
- Unknown or unmarked Harness artifacts are user-owned. Preserve or ask.
- Powder owns work; Bitterblossom owns workflows and dispatch. Roster may carry
  primitives for using them but no embedded card or workflow semantics.
- The retained deterministic core must serve validation, resolution, manifest
  production, explanation, or thin launch mechanics. Delete before adding.

## Gate

Run before claiming repo changes are complete:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
