# VISION — Roster

Roster is a public library and small compiler for composing agents.

It curates opinionated, reusable agent primitives; resolves them from public,
personal, client, and experimental sources; and produces an inspectable bundle
that a Harness can run. Roster is for humans and control planes that want to
answer, precisely: **what role should this agent perform, what guidance and
tools does that role carry, and what was actually handed to the Harness?**

The factory is Roster's demanding first user, not its product boundary. A
stranger should be able to adopt Roster without adopting Misty Step's
workstation, Powder, Bitterblossom, or operating doctrine.

## The function

Roster does three things:

1. **Curate a library.** Compositional primitives—skills, MCP declarations,
   and concise guidance—plus packs, roles, and Harness adapters live in plain
   files with provenance.
2. **Resolve composition.** One role expands into a deterministic, explainable
   set of primitives across explicitly configured source roots.
3. **Dispatch thinly.** Roster hands an immutable resolved bundle to a selected
   Harness adapter and gets out of the way.

Everything else belongs to a neighboring product. Bitterblossom defines
workflows and dispatches agents. Powder or another selected work system records
tasks, claims, and evidence. Harnesses own process execution, authentication,
sessions, caches, UI preferences, and native configuration.

## Words matter

- A **primitive** is a reusable skill, MCP declaration, or guidance fragment.
- A **pack** is a dumb additive set of primitive references. It has no
  conditions, inheritance, exclusions, model logic, or hidden precedence.
- A **role** is the complete Roster-owned semantic composition of primitives.
  It has a name, description, and one additive `include` list. Packs and roles
  cannot include Harness adapters.
- A **model** is the inference engine.
- A **Harness** is the executable host: Codex, Claude Code, Pi, OpenCode, OMP,
  or another runtime.
- An **agent definition** binds `name + role + model + Harness + optional args`.
  It does not add primitives outside its role.
- An **agent instance** is that definition running in an environment. Roster
  does not supervise the instance or own its run history.

If two agents need different primitives, they have different roles. This keeps
the effective behavior legible and prevents a second composition language from
growing inside launch configuration.

## The library

The target public source tree is deliberately unsurprising:

```text
primitives/
  skills/
  mcps/
  guidance/
packs/
roles/
examples/      # optional bindings; active agents live with operator config
```

Personal and private composition lives outside the repository, normally under
`~/.roster/`, with the same relevant directories plus `config.yaml`. Client and
eval roots may be added explicitly. Private roles can compose public primitives
without copying them into Git or exposing client-specific instructions.

Every item has a source-qualified canonical identity such as
`core/skill:deliver` or `acme/role:builder`. Shorthand is accepted only when it
is unambiguous. Two sources may carry the same short name because they remain
different canonical identities; conflicting exact identities fail. There are
no implicit overrides. A customized primitive receives a new identity.

YAML is the canonical declaration format. JSON is an optional CLI serialization
for machine consumers, not an authoring requirement.

## Composition

Roles have one `include` list whose entries may name compositional primitives or
packs. Resolution is additive set union over canonical identities: duplicates
collapse, provenance does not. The manifest explains every primitive's source
and every pack or role that included it. The agent definition's Harness binding
selects exactly one adapter after semantic resolution; adapters never appear in
role or pack membership.

The public roles are allowed to have taste. Roster's default SDLC roles prefer
Powder by explicitly including a Powder work-management pack alongside general
work-management practice. An operator who prefers Linear composes another role
from the same general pack plus Linear primitives. Provider preference is
visible composition, not a privileged schema field and not deterministic card
logic inside Roster.

Guidance is a first-class primitive for concise, always-loaded philosophy,
principles, preferences, and practices. Skills remain triggered workflows; MCP
declarations make tools available. Guidance should not grow into another name
for a skill or a verbose system prompt.

## The resolved bundle

`roster resolve` produces the canonical intermediate representation:

```text
AGENTS.md
skills/
mcps.yaml
manifest.yaml
```

The bundle is immutable after resolution. Its YAML manifest records source
roots, canonical identities, inclusion provenance, resolved agent binding, file
digests, and every elicitation adaptation. It contains references and launch
material, never secret values.

The runtime `AGENTS.md` must tell the model what role it is playing, what
guidance applies, what skills and tools are available, where they are, and when
to use them. The resolver should generate the inventory/router from resolved
metadata so it cannot drift. Roster will evaluate three prompt shapes—authored,
hybrid, and generated—before declaring one universally superior. The initial
design favors a hybrid: optional concise role-authored framing around generated
guidance, skill, and MCP routing sections. Source templates are never mutated.

Bundles are temporary by default. `resolve --output` and `--keep-bundle` make
them durable for inspection, control-plane handoff, or reproduction.

## Harness adapters

Harness adapters are a small built-in Tier 1 boundary. An adapter reads the
immutable semantic bundle, writes only an ephemeral Harness-native projection,
selects the primary model, forwards only validated native arguments, launches
the process, and preserves its exit and signal behavior.
It may not add primitives, rewrite role guidance, or otherwise change semantic
composition.

`dispatch` means only resolve plus one selected launch. Roster does not choose
among eligible agents, route work, retry, supervise, or interpret the result;
those richer dispatch responsibilities belong to a human or control plane.

This narrow protocol prevents Roster from becoming a lowest-common-denominator
Harness abstraction. OMP's advisor, smol model, and other internal topology
remain OMP-owned configuration; Roster does not normalize it. Agent definitions
may use a small allowlisted native argument surface for runtime permissions and
sandboxing, never to append or replace Roster-managed guidance, skills, MCPs,
models, or configuration. The manifest records the validated arguments.

## Model elicitation: evidence before taxonomy

Canonical primitives are written for maximally capable models: concise, clear
about why the work matters, ambitious, and encouraging curiosity and fun. They
preserve degrees of freedom instead of restating procedures a frontier model
already understands. Frontier models receive no elicitation adaptation by
default.

Smaller models may benefit from more bounded tasks, explicit progress cues,
examples, or narrower tool surfaces. Roster will not pre-emptively fork every
skill by model family. Crucible and Bench should first test capability,
scaffolding need, autonomy horizon, task decomposition, and prompt articulation
with paired evidence. Only demonstrated adaptations enter Roster.

An elicitation adaptation may append to `AGENTS.md` or modify the articulation
of a materialized skill copy, but it must preserve the primitive's purpose,
workflow, authority, and available tool surface. It applies only inside the
bundle, never to canonical source. The manifest names every target, action,
source, and before/after digest. A behaviorally distinct articulation receives
a new canonical primitive identity and enters through the role's `include`
list. Model-driven primitive inclusion or replacement would intentionally add a
second semantic composition seam; it is outside v0.2 and requires explicit
future vision revision plus evidence.

Model labels and capability mappings are review-dated evidence, not ontology.
Roster may consume a mapping produced elsewhere; it is not the research ledger.

## Epic-shaped work and agent routing

Most meaningful work begins as an ambitious epic suitable for a highly capable
model. Decomposition converts ambiguity into bounded child contracts that less
capable models can execute. The parent remains authoritative; children are
reversible execution projections. Child evidence rolls into an epic-state
packet rather than a transcript concatenation, allowing a strong model to
recompose, judge, and integrate the whole.

Roster owns reusable skills and guidance for shaping, decomposition, slice
execution, recomposition, and integration. It does not own the hierarchy or
router:

- Powder should represent parent/child work, child acceptance, evidence rollup,
  and parent-state recomposition.
- Bitterblossom should own decompose → dispatch → collect → recompose workflows
  and task-to-agent routing.
- Crucible should evaluate capability, scaffolding, decomposition, outcome, and
  dispatch quality.
- Bench should publish reproducible benchmark families and comparisons.

## Product surface

The CLI is small: bare `roster` opens the agent picker; `init`, `list`, `show`,
`resolve`, `dispatch`, `inspect`, `rescue`, and `check` cover the public
surface. A hidden `authority request` seam supports an optional provider for
one named mid-session operation without making credentials or permissions part
of agent composition. Roster does not need a standing MCP server, HTTP API,
web UI, sync daemon, or workstation doctor until real demand proves the
filesystem and CLI insufficient.

## Non-goals

Roster is not:

- an execution runtime, agent control plane, workflow engine, router, scheduler,
  run ledger, task system, or approval plane;
- a dotfile manager or workstation convergence authority;
- an owner of Harness authentication, secrets, sessions, caches, health, or
  unmarked configuration;
- an SDK framework that consumers must embed;
- a universal permissions or capability-grant system;
- a place for Powder-, Bitterblossom-, Canary-, or OMP-specific product logic;
- a compatibility museum for pre-1.0 experiments.

Roster remains `0.x`. Breaking changes are welcome when they clarify the
function or delete machinery. Migrations preserve valuable user-authored
primitives and user-owned Harness state, not obsolete APIs.

## Migration truth

v0.2 retired the rich role schema, coupled `instructions.md`, workstation
`sync`/`doctor`, hooks, HTTP/UI/MCP services, and card-aware materializers.
Curated skills, external provenance, and useful plain-file declarations remain.
The deterministic spine is now validation, resolution, manifest production,
receipts, and thin launch mechanics.

## What excellent looks like

Today, one private role can compose public primitives, resolve to a bundle
whose router and provenance are obvious, and launch through Codex, Claude Code,
or OMP without changing permanent Harness configuration.

In the medium term, control planes such as Bitterblossom consume the same
bundle contract as local dispatch, while eval evidence—not remembered model
folklore—drives any capability adaptation.

Long term, Roster becomes a small, trustworthy public vocabulary for agent
composition: opinionated enough to improve the agent, modular enough to fit
another operator, and simple enough that its declarations matter more than its
code.
