# Skill authoring standard (roster house conventions)

The judgment layer lives in the vendored
`primitives/skills/.external/mattpocock-writing-great-skills/SKILL.md`
(predictability, invocation choice, the five failure modes, the no-op test,
positive framing) and Anthropic's progressive-disclosure guidance
(`docs/research/roster-926-import-design-brief.md`). This file adds only the
house conventions those sources leave open. Apply the no-op test to every
line you add to a skill — including lines from this standard.

## Frontmatter

- `name`, `description`, `argument-hint` — all three, always.
- Model-invoked skills: description is the trigger — pushy, trigger-phrase
  dense ("Use when: …. Trigger: /x, /y"). Claude under-triggers by default.
- Hand-only skills: `disable-model-invocation: true`; description becomes a
  one-line human-facing summary.
- `argument-hint` uses bracketed-token form: `"[--foo|--bar] [target]"`.

## Body shape

- H1 is the trigger form: `# /<name>`.
- Three tiers: frontmatter → SKILL.md body → `references/` on demand.
  A body pushing past ~900 words is a signal to extract, not a violation —
  the test is whether the inline prose is load-bearing for every invocation.
- Fenced examples must not contain lines that scan as real headings
  (indent them or use placeholder text).

## Canonical section names

- `## Completion Gate` — the single wrap-up header. It delegates to the
  shared Completion Evidence core (`primitives/shared/AGENTS.md`) first,
  then adds only phase-specific fields. Retire: Output, Verdict, Completion,
  Done means, Verification-as-wrap-up.
- `## Gotchas` — bespoke traps only. If a frontier model already knows it,
  it fails the no-op test.

## Shared doctrine: point, never restate

One-line pointers, verbatim, instead of local rewordings:

- Critics: `Critics get the artifact and the oracle only — never the
  author's reasoning trail (shared AGENTS.md: Fresh context beats
  self-review).`
- Delegation: `Delegate per the shared Roster contract (shared AGENTS.md:
  Roster).`
- Compression: `julius-caveman for interim synthesis only; findings, code,
  commits, and final artifacts stay normal English.`
- Vision: `Read root VISION.md when present; if missing or stale, route to
  /vision.`
- Verification loops: point at
  `primitives/shared/references/verification-system-first.md` — do not
  re-derive it.
- Simplicity: the delete-first/Ponytail load trigger lives inside
  `primitives/shared/references/delete-first.md`; skills just point there.

A skill that needs a *different* rule than the shared line says is making a
claim — state the delta explicitly and say why, next to the pointer.

## Routing tables

One signal→skill routing table exists, in `/orient`. `/next` and any other
skill that routes consumes it by pointer and adds only its own framing.
Never fork the table.

## Fable 5 deltas

From Anthropic Fable 5 prompting guidance (2026-07); apply alongside the no-op
test, one rule each:

- Deletion is the top rewrite move. Skills built for prior models over-prescribe
  and degrade Fable output; strip step-by-step enumerations and micromanaged
  procedure before adding anything.
- Prefer one intent-level instruction over an enumerated checklist: goal,
  boundaries, verification hook, and trust the model to structure the work.
- Never instruct the model to echo or transcribe its internal reasoning as
  response text — it trips the reasoning-extraction refusal. Hunt
  "show your thinking"-shaped lines and cut them.
- A skill that over-fires earns a `Do not use when: …` clause beside its
  `Use when:`.
- Completion-reporting skills point at the shared Completion Evidence core for
  grounded-progress ("report only work a tool result backs") — never restate it.
