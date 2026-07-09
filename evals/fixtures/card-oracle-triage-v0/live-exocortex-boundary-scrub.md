# Scrub the public boundary and close the frozen shape decisions

Status: done · Priority: p0

Make the public repo safe to read and frozen-experimental by removing private
topology from public docs and recording the operator's closed shape decisions
before any runtime implementation begins.

## Acceptance
- `rg -n -i "ph[a]edrus|/U[s]ers/|d[a]ybook|m[o]nologue|s[u]per whisper|finance[_]private|relationship[_]private|q[m]d" . --glob '!.git/**'` returns no matches.
- `docs/shaping/public-context-system.md` records the closed decisions: LanceDB as the single v0 backend, generic command adapter, frozen citation core, no cross-workspace inheritance, three read-only MCP tools, private evidence destinations, one generic skill, no v0 UI, and packet-as-evidence-session compiler.
- `docs/shaping/public-context-system.html` is regenerated from the markdown and carries the same decisions.
- No implementation scaffold, build system, runtime state, or sample private config is introduced.
