# Rebuild the site as a per-primitive gallery; deepen the core system

Priority: P1 · Status: in progress · Estimate: M

## Goal

Turn `site/primitives.html` from a grid-entry catalog into a shadcn/ui-style
**gallery**: a persistent left sidebar taxonomy where every primitive has its
own focused view (live preview + canonical markup + state matrix). Built only
with the system itself (zero build, zero framework), honoring the no-page-scroll
law — the gallery is the `.ae-shell` / `.ae-rail` / `.ae-desk` app-shell pointed
at itself. Then use the sharper surface to drive the second workstream: improve
the look/feel and comprehensiveness of the core aesthetic.

Context: the comic-ops pivot (commit `9bbe0f9`) was reverted 2026-07-01; repo is
back on the minimalist `v2.8.1` system. Pivot preserved on branch
`archive/comic-ops-pivot`.

## Follow-on (workstream 2)

Once the gallery is the working surface, audit the core system for gaps in
look/feel and coverage — the gallery makes missing primitives, weak states, and
rough edges visible. File specific tickets as they surface.
