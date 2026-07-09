# Aesthetic: fleet navigation/sidebar design lab — one spine for every factory app

Status: ready · Priority: p2

Operator-directed (2026-07-08, during the Crucible UI walkthrough): "We should probably run a full /design spine exploration on misty-step/aesthetic repo for sidebars; Crucible isn't the only one that needs one."

## The problem
Every factory app (crucible, powder, cairn, glass, roster UI, portal, overmind) hand-rolls its own navigation. Crucible's sidebar-for-two-views prompted the question: the fleet has no ratified navigation SPINE — when a rail is right, when a top bar, when bottom tabs (phone), when no chrome at all, and what the shared markup/tokens are.

## Shape
A lab-registry design lab IN the aesthetic repo (prototypes/ or explorations/, per its conventions): >= 6 structurally distinct navigation patterns rendered as full-viewport pages with the real ae-* system, each shown at desktop AND phone (390px) against 2-3 real app contents (a data table app like crucible, a board app like powder, a PWA like cairn). Patterns to cover at minimum: side rail, slim top bar tabs, no-chrome/object-first, bottom tabs (phone-native), master-detail list-as-nav, command-palette-first, breadcrumb/object-URL nav. Rounds until the operator locks; the winner becomes a ratified ae-* navigation component + DESIGN.md contract that apps adopt.

## Input available
Crucible design lab 001 (2026-07-08) section NAV explores exactly these 7 patterns in Crucible's context — its verdicts are the seed evidence for the fleet-level choice. Whichever direction the operator picks for Crucible is a data point, not automatically the fleet answer (powder/cairn have different shapes).

## Oracle
- [ ] Lab with >= 6 structurally distinct nav patterns x >= 2 app contents, real ae-* CSS, desktop + 390px, no lorem.
- [ ] Operator verdict recorded per round; a locked winner.
- [ ] Winner shipped as a reusable ae-* navigation pattern (markup + tokens + usage rules) in aesthetic, with DESIGN.md updated.
- [ ] At least two factory apps adopt it (crucible first).

## Acceptance
- Round 1 lab registry contains at least 6 structurally distinct navigation patterns, including side rail, slim top tabs, no-chrome or object-first, phone-native bottom tabs, master-detail list-as-nav, command-palette-first, and breadcrumb or object-URL navigation.
- Every retained option renders real Aesthetic CSS against at least two truthful factory-app contents, with an explicit shipped-state baseline.
- The paged viewer works at desktop and 390x844 phone presets, keeps interactions live, uses stable namespaced option IDs and philosophy provenance badges, and produces no horizontal body overflow or console errors.
- Operator verdicts are recorded per round; design remains awaiting input until a winner is explicitly locked.
- After lock only: the winner ships as reusable ae-* navigation markup, tokens, and usage rules with DESIGN.md updated and is adopted by at least Crucible plus one other factory app.
