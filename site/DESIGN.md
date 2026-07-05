# Roster DESIGN.md

This file is the product's public-site brand contract. Keep it short and exact:
agents and humans should be able to update `site/` from this file without
inventing a second design system.

Placed at `site/DESIGN.md` (not repo-root `DESIGN.md`) by convention going
forward — see `aesthetic-911` and the collision found while shipping
`bitterblossom-113`. This repo had no root `DESIGN.md` to collide with, but
the site-scoped location is applied consistently regardless.

## Brand Voice

- Plain-spoken, concrete, and operator-facing.
- Lead with the user outcome, then the proof.
- Avoid marketing fog, mascot language, and decorative claims.
- Roster is a declaration repository and a CLI, not an agent framework pitch:
  say what gets declared, composed, and dispatched — never "AI that manages
  your agents for you."

## Pitch One-Liner

`Roster declares every agent Misty Step runs — role, model policy, permissions, skills, MCPs, evidence expectations — as plain files, then composes and dispatches that declaration onto any harness at run time.`

## Lucide Mark

- Icon: `users`
- Reason: provisional — Roster predates the fleet-wide icon-selection pass
  (`aesthetic/prototypes/icon-logo-playground.html`, 2026-07-02), so no mark
  has been ratified for it yet. `users` was chosen for this site only
  (agents as first-class citizens, a roster of them) and does not ratify
  anything on the repo's behalf; the repo owns its real mark contract
  whenever it runs the same selection process the other 12 fleet products
  went through.
- Rule: the mark is an inline Lucide SVG inside `.ae-app-mark`. No bespoke
  marks, logo images, emoji marks, or colored wordmarks.

## Palette Hooks

Keeps the Aesthetic default palette — Roster is a declaration/CLI product
with no runtime UI of its own that would call for a differentiated accent.

```css
:root {
  --ae-accent: #2643d0;
  --ae-accent-dark: #8c9eff;
}
```

## Screenshot Inventory

All three are real terminal captures against this repo's actual `roster`
binary and its committed `agents/cerberus/` declaration — not mockups:

| File                                                    | Surface                                    | State                                              | Caption                                                                                          |
| --------------------------------------------------------- | --------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `site/assets/screenshots/01-roster-list.png`              | `roster list` terminal                        | Real inventory: the 3 agents committed today (`cerberus`, `lead`, `sweep`) | The roster, from the CLI — role, model class, reasoning tier, one line each. |
| `site/assets/screenshots/02-declaration-convention.png`   | `ls agents/cerberus/` + `cat role.yaml` terminal | Real committed declaration                          | The `agents/<name>/` convention: one YAML file is the whole agent — model policy, permissions, skills, evidence bar. |
| `site/assets/screenshots/03-dynamic-composition.png`      | `roster brief cerberus --add-skill qa` terminal | Real CLI run showing an override applied at dispatch time | The composition seam: a lane brief built from the declaration, with a skill added on top — no edit to the declaration itself. |

## Footer Links

- Misty Step: `https://mistystep.io`
- GitHub: `https://github.com/misty-step/roster`
- Weave: `https://misty-step.github.io/weave/` — Roster is a Weave-family
  product (listed on the hub's family grid); it succeeds harness-kit as the
  factory's agent-declaration layer and materializes onto Bitterblossom's
  runners among other planes.

## Release Notes Rule

`site/changelog.html` is user-facing. Write entries as product outcomes, not
commit logs. Each entry needs a date, a version or release label, and one or two
plain-language bullets. Roster has no tagged GitHub releases yet — write an
honest stub rather than inventing a version number.
