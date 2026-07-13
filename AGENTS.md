# Roster Repo Contracts

Roster curates agent primitives, resolves roles into immutable bundles, and
hands those bundles to Harness adapters. Read `VISION.md` before changing the
declaration graph, CLI, adapters, or migration boundary.

## Current migration

- `VISION.md` is the target contract. The checked-in v0.1 implementation still
  contains legacy workstation and service machinery; existence is not product
  authority.
- The active migration is Powder card `roster-v02-primitives-compiler`.
- Preserve curated skills, provenance, useful declarations, and all user-owned
  Harness state. Do not extend a legacy surface without an explicit temporary
  migration justification.
- Target semantic composition lives only in roles: name, description, and one
  additive `include` list. Agent definitions bind role + model + Harness + args;
  they do not add primitives.
- Target source identities are source-qualified and have no implicit override
  semantics. Resolution must remain explainable and provenance-complete.
- Target Harness adapters may translate and launch an immutable bundle. They may
  not mutate its meaning or normalize Harness-native model topology.
- No secret values in declarations, bundles, manifests, fixtures, or logs.

## Source boundaries

- `primitives/skills/skills-index.yaml` and
  `primitives/skills/.external/registry.yaml` remain the current skill catalog
  and provenance ledger until the v0.2 source graph replaces them.
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
