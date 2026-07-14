---
name: artifact
description: |
  Produce a consistently-styled, self-contained HTML report served privately over
  Tailscale. One house template (Silver Age comic-ops palette, dark/light toggle,
  and a mandatory copy-page button) so every report an agent hands the operator
  looks and behaves the same. Use when: "make an HTML artifact/report", "serve
  this over tailscale", "write up a brief/report/dashboard as a page", or any time
  you'd otherwise dump a long analysis into chat. Trigger: /artifact.
argument-hint: "[--title <t> --slug <s> --body-file <f>|--html-file <f>] [--local-only]"
---

# /artifact

The operator reads reports as HTML pages over Tailscale, not as chat walls. This
skill is the single source of truth for how those pages look and what they can do.

## The contract every artifact honors

- **One house style.** The template (CSS + JS) lives inside `scripts/artifact_create.py`
  as `HOUSE_CSS`/`HOUSE_JS` — edit there, every future report inherits it. Don't
  hand-roll a divergent stylesheet.
- **A copy-page button, always.** Every report carries a header "Copy page" button
  that copies the entire rendered document to the clipboard. Baked into the template;
  injected automatically if you pass an already-authored full HTML file.
- **Self-contained.** Inline CSS/JS, no external assets — the copied HTML is a
  complete, portable page.
- **Informational, not decorative.** Tables, callouts, diagrams that carry
  information prose can't. See the aesthetic repo for the deeper design language.

## Information design doctrine (operator ruling, 2026-07-03)

"It's a little bit silly to be leaning into HTML and then still have it be one
long-ass pile of text." An artifact that is a Markdown report in a browser has
missed the medium. Before authoring, ask: **what is the most effective
articulation of THIS information?** — then climb the ladder only as far as the
content earns:

1. **Prose** — for argument and verdicts. Short. Never the whole page.
2. **Structure** — tables for enumerable facts, callouts for rulings, phase
   lanes for sequence, comparison grids for alternatives. Layout IS analysis.
3. **Diagrams & generated images** — when shape carries the meaning (system
   maps, flows, timelines). Nano Banana renders legible labeled infographics
   in ~4s for ~$0.03 (`GEMINI_API_KEY` is in the env) — use it liberally for
   informational images, never decoration.
4. **Interactive & animated** — drill-downs, toggles, simulations, canvas/
   three.js/WebGL — when the reader needs to *explore* (a graph, a
   before/after, a what-if), not just read. Inline the library or keep it
   dependency-free; the page must stay self-contained.

Guardrails: single-column narrative spine is generally right; a noisy
dense dashboard is as much a failure as the wall of text. Strong visual
hierarchy, restrained color used semantically, progressive disclosure —
the first viewport carries the verdict, depth unfolds below. Tell a story;
every claim links to its evidence (the atlas principle applies to pages too).

## Publication boundary

`artifact_create.py` always writes a local mirror. Remote publication requires
an operator-configured `ARTIFACT_BASE_URL` plus `ARTIFACTS_API_TOKEN`; the
public skill contains neither a deployment hostname nor a vault lookup. Files
written directly into `~/artifacts/public/a/<slug>/` remain local until the
configured publisher sends them to the shelf.

## Do it

```bash
S="$ROSTER_ROOT/primitives/skills/artifact/scripts"
# quick: markdown in, styled page out
python3 $S/artifact_create.py --title "Weekly Ops" --slug weekly-ops \
  --tag "Field Memo" --summary "..." --body-file report.md
# rich: author a full HTML page (best for real reports); the copy button is
# injected if you forgot it. Match HOUSE_CSS class names for consistency.
python3 $S/artifact_create.py --title "The Factory" --slug factory \
  --tag "Field Memo" --html-file factory.html
```

The script writes a local mirror (`~/artifacts/public/a/<slug>/index.html`),
then PUTs the page to `$ARTIFACT_BASE_URL` and prints the canonical URL. Use a
1–2 word slug. Publishing needs `ARTIFACTS_API_TOKEN` from the private runtime
environment. `--local-only` skips the PUT and does not require a base URL.
Verify with `curl -s -o /dev/null -w '%{http_code}' <url>` before handing over
the link. Publish any raw HTML from anywhere on the tailnet with one line:

```bash
curl -T page.html -H "Authorization: Bearer $ARTIFACTS_API_TOKEN" \
  "$ARTIFACT_BASE_URL/a/<slug>/index.html"
```

## Serving

The operator owns the shelf host, private-network exposure, persistence, and
service manager. `scripts/artifact_serve.py` is a portable local static server
for `~/artifacts/public`; deployment-specific URLs and relay integrations enter
through environment variables or a private source. Roster does not install or
converge that service.

## Extending

Add reusable block styles (cards, timelines, phase lists) to `HOUSE_CSS` when a
report needs them, so the next report can reuse the class. Keep the template one
file; don't fork per-report CSS.
