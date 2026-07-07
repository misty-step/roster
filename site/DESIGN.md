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

## Locked Homepage

- Lock date: operator lock-in 2026-07-07, `misty-step-936`.
- Layout: Split.
- H1 tagline, exact: `Collect primitives. Compose agents. Dispatch anywhere.`
- Hero image: `site/assets/hero.jpg`, copied from the staged
  `roster-hero.jpg` production image. Provenance: `gpt-image-1`, Misty Step
  fresco language, 1920px JPEG.
- Hero image opacity: `0.35`.
- Homepage structure: hero only, one viewport, no scroll. Header has Lucide
  bot mark, uppercase wordmark, and `features · get started · changelog ·
  github`. Copy sits left, vertically centered, directly on the page with a
  `Get started` CTA.

## Lucide Mark

- Icon: `bot`
- Reason: operator-ratified 2026-07-06. Roster's product is agent
  identities as files; the bot glyph reads agentic at a glance, where the
  prior `users` mark read like a generic team-management tool.
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
| `site/assets/screenshots/01-roster-list.png`              | `roster list` terminal                        | Launch capture of the committed roster inventory at the time | The roster, from the CLI — role, model class, reasoning tier, one line each. |
| `site/assets/screenshots/02-declaration-convention.png`   | `ls agents/cerberus/` + `cat role.yaml` terminal | Real committed declaration                          | The `agents/<name>/` convention: one YAML file is the whole agent — model policy, permissions, skills, evidence bar. |
| `site/assets/screenshots/03-dynamic-composition.png`      | `roster brief cerberus --add-skill qa` terminal | Real CLI run showing an override applied at dispatch time | The composition seam: a lane brief built from the declaration, with a skill added on top — no edit to the declaration itself. |

## Footer Links

- Footer contract: mode toggle on the left. Right side reads
  `a Misty Step project`, with `Misty Step` linked to
  `https://mistystep.io`, followed by the inline GitHub glyph linked to
  `https://github.com/misty-step/roster`.
- No bare URL text, email, copyright line, or old Weave footer link.

## Release Notes Rule

`site/changelog.html` is user-facing. Write entries as product outcomes, not
commit logs. Each entry needs a date, a version or release label, and one or two
plain-language bullets. Roster has no tagged GitHub releases yet — write an
honest stub rather than inventing a version number.
