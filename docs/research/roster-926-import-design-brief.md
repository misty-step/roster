# Research Brief: Skill-Authoring Best Practices & Primitive-Import Prior Art
**roster-926, acceptance criterion 1** — researched 2026-07-06. All claims dated; "official docs" vs "blog/community opinion" flagged explicitly.

## Rules we should adopt (shortlist)

1. **Description is the trigger, not a summary.** Anthropic: Claude decides whether to load a skill from `name`+`description` alone — write it "pushy" and trigger-dense, not a polite abstract. *(official, 2025-10-16)*
2. **Progressive disclosure, three tiers minimum.** Frontmatter → SKILL.md body → `references/*` loaded on demand. Don't inline everything. *(official, 2025-10-16)*
3. **Classify every skill model-invoked vs user-invoked up front** (`disable-model-invocation: true` for the latter) — this is now a real, shipping frontmatter field in Claude Code, not just Pocock's heuristic. *(official, code.claude.com/docs, current)*
4. **Positive framing over negation.** State the target behavior; prohibition-style instructions ("don't do X") reliably backfire. *(Pocock, community, current)*
5. **Five failure-mode checklist before shipping a skill**: premature completion, duplication, sediment (stale carried-over context), sprawl, no-op. Test each explicitly. *(Pocock, community, current)*
6. **Single source of truth per meaning; delete anything that fails a "no-op test"** (would behavior change if this line were removed? If not, cut it). *(Pocock, community, current)*
7. **Pin imports to content, not branch state.** Store a hash (tree SHA or SHA-256) and/or explicit version, not "latest" — this is the convergent pattern across every registry examined (Vercel lockfile, Anthropic community marketplace CI pin). *(official + community, current)*
8. **No word count ceiling as hard official law** — Anthropic doesn't mandate a number; "under ~500 lines" is community distillation of the token-budget principle, not an Anthropic-stated threshold. Treat as a heuristic, not a gate.

---

## 1. Skill-authoring state of the art (SKILL.md)

**Anthropic official** ("Equipping agents for the real world with Agent Skills," anthropic.com/engineering, published 2025-10-16): evaluation-driven development — find capability gaps by running representative tasks, then build skills to close them, don't speculate. Progressive disclosure is named as *the* core design principle: name/description load first (level 1), full SKILL.md loads on demand (level 2), bundled files load only when referenced (level 3). Split SKILL.md into separate reference files once it gets unwieldy, and keep mutually-exclusive contexts in separate files so unrelated tokens aren't paid for together. Description is explicitly framed as a triage device ("Claude will use these when deciding whether to trigger the skill") — the guidance is to make it decisive, since Claude under-triggers by default. Security note: only install skills from trusted sources; audit less-trusted ones. Anthropic does **not** publish a hard line-count ceiling in this source — the widely-cited "under 500 lines" figure is a third-party distillation (e.g. generativeprogrammer.com), not an Anthropic number.

**Claude Code current docs** (code.claude.com/docs/en/plugins, current as of 2026-07-06 fetch): confirms `disable-model-invocation` as a real frontmatter key that determines whether a skill's description stays in Claude's context for autonomous triggering (model-invoked) or is stripped entirely and only reachable by explicit slash-invocation (user-invoked). Skills are namespaced when shipped in a plugin (`/plugin-name:skill-name`) to prevent collisions.

**Matt Pocock** (`mattpocock/skills` GitHub repo, MIT license, skill at `skills/productivity/writing-great-skills/SKILL.md` — this is community/practitioner guidance, not an official standard): adds concrete micro-craft on top of Anthropic's structural principles.
- Front-load the *leading word* of each step (the imperative/outcome word first) so the model anchors on the constraint before parsing the rest of the sentence — saves interpretation tokens.
- Information hierarchy, in strict priority order: (1) in-skill ordered steps with checkable, exhaustive completion criteria, (2) in-skill flat reference facts, (3) externalized reference files reached via pointers.
- Co-locate related rules/caveats under one heading rather than scattering them (avoids the "sediment" failure mode below).
- Five named failure modes, each with an implied test: **premature completion** (claims done before every step's criteria are met), **duplication** (same meaning stated in two places, so edits diverge), **sediment** (stale context from prior invocations leaking into the current run), **sprawl** (skill grows long without proportional value), **no-op** (a line that doesn't change behavior vs. the model's default — delete it), and a sixth implicit one, **negation** (prohibitions backfire; state the positive target instead).
- Granularity rule: split a skill along invocation lines (distinct trigger phrases that should fire independently) or along sequence lines (hide steps that come after "done" so the model doesn't peek ahead and short-circuit).

**OpenAI Codex** (developers.openai.com/codex/skills, current docs, official): same SKILL.md shape (YAML frontmatter with `name`+`description`, markdown body), same progressive-disclosure loading order (name/description/path first, full body only once selected). Codex-specific addition: an optional `openai.yaml` inside the skill folder for Codex-only UI metadata and MCP tool dependency declarations — a Codex-only sidecar file, not part of the Anthropic-originated core format. AGENTS.md remains the separate, always-loaded project-instruction layer (root repo or `~/.codex/AGENTS.md`), read before any skill logic runs — confirms the two-tier model (always-on doctrine file + on-demand skill files) generalizes across harnesses, matching this repo's own AGENTS.md/SKILL.md split.

---

## 2. Locating a vendorable Pocock skill-writing skill

Confirmed: Matt Pocock **does** publish an installable, SKILL.md-format skill about writing skills.

| Field | Value |
|---|---|
| Repo | `github.com/mattpocock/skills` |
| Path | `skills/productivity/writing-great-skills/SKILL.md` |
| License | MIT |
| Format | Standard SKILL.md (frontmatter `name`, `description`; body is the vocabulary/failure-mode reference above) |
| Distribution | Also listed on the community registry skills.sh, and the repo ships a `.claude-plugin/` so it can be installed as a Claude Code plugin directly from the repo |
| Exact commit/version | Not exposed by GitHub's rendered view without a direct API call — **recommend pinning by tree SHA at vendor time** (see §3) rather than trusting "latest," since the repo is actively iterated (its own issue tracker shows an open feature request, `mattpocock/skills#3`, for native Claude Code marketplace support, meaning the packaging format itself is still moving). |

Secondary guidance surface: Pocock also writes about skill design on aihero.dev (his newsletter/course site) under "AI Engineering Posts" and a dedicated `/skills` changelog page (e.g. "Skills Changelog: /handoff, /prototype, /review and /writing") — this is narrative/rationale, not itself an installable artifact; the installable artifact is the GitHub repo above.

---

## 3. Primitive-import mechanism prior art

Three converging conventions across the ecosystem, examined 2026-07-06:

**Claude Code plugin/marketplace format** (code.claude.com/docs/en/plugins, official, current):
- Plugin identity: `.claude-plugin/plugin.json` — `name`, `description`, `version` (optional), `author`, plus `homepage`/`repository`/`license`.
- **Version semantics**: if `version` is omitted and the plugin is git-distributed, *the commit SHA itself is the version* — every commit is implicitly a new release. Setting an explicit `version` opts into semver-style update gating instead.
- Marketplace registry: `.claude-plugin/marketplace.json` lists plugins with `source` + `version`/`sha`.
- **Pinning**: the official community marketplace (`anthropics/claude-plugins-community`) pins every approved plugin to an exact commit SHA in its catalog; CI auto-bumps that pin as the upstream repo receives new commits, and the public catalog syncs nightly — so there's a deliberate, audited lag between upstream change and consumption, not a live float.
- Local dev override: `--plugin-dir` (or `--plugin-url` for a hosted zip) loads an unpinned local copy for the session only, explicitly separate from the pinned-catalog path.

**Vercel `skills` CLI / skills.sh registry** (github.com/vercel-labs/skills, vercel.com/changelog, official Vercel product — announced 2026-01-20): the clearest lockfile-based prior art.
- Two-tier lockfiles: a **global** `~/.agents/.skill-lock.json` (user-level installs, records `source`, `sourceType`, `skillFolderHash` = GitHub tree SHA, plus `installedAt`/`updatedAt` timestamps) and a **local/project** `skills-lock.json` (checked into VCS; same shape but a content SHA-256 hash instead of remote metadata, and timestamps deliberately omitted to avoid diff noise on every install).
- Provenance recorded: source identifier, content hash, optional target-subagent field — notably **no explicit semver field**; identity and "did this change" are both established purely by content hash, not a version string.
- Verbs: `sync` (crawl `node_modules`/project tree, compute local hashes, update lockfile), `install` (read lockfile, restore missing skills), `check` (diff stored hash vs. freshly fetched hash to detect upstream drift).

**Cross-cutting convergence**: every serious implementation (Anthropic's own community marketplace, Vercel's skills.sh) pins by **content hash**, not by mutable ref (branch/tag), and treats an explicit semver field as optional sugar on top of that hash rather than the source of truth. For roster-926's primitive-import design, this argues for: (a) a lockfile keyed by content hash (tree SHA for git sources, SHA-256 for local/vendored copies), (b) an explicit human-readable version/label as an *optional* convenience layer, not the pin mechanism itself, and (c) a `check`/`sync` verb pair that diffs stored-vs-fresh hash rather than polling a registry API.

---

## Citations

- [Equipping agents for the real world with Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills) — Anthropic official, 2025-10-16
- [Agent Skills — Claude Platform Docs](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview) — Anthropic official, current
- [Create plugins — Claude Code Docs](https://code.claude.com/docs/en/plugins) — Anthropic official, current (fetched 2026-07-06)
- [GitHub - anthropics/skills](https://github.com/anthropics/skills) — Anthropic official repo
- [Skill Authoring Patterns from Anthropic's Best Practices](https://generativeprogrammer.com/p/skill-authoring-patterns-from-anthropics) — third-party distillation (source of the "~500 line" figure, not Anthropic-stated)
- [GitHub - mattpocock/skills](https://github.com/mattpocock/skills) — Pocock, community, MIT license
- `skills/productivity/writing-great-skills/SKILL.md` in the above repo — fetched 2026-07-06
- [mattpocock/skills issue #3 — native Claude Code marketplace support](https://github.com/mattpocock/skills/issues/3) — open as of research date, evidence the packaging format is still moving
- [Matt Pocock Shares Writing Great Skills Guide for Predictable AI Agents](https://www.remio.ai/post/matt-pocock-shares-writing-great-skills-guide-for-predictable-ai-agents) — community summary
- [AI Engineering Posts by Matt Pocock](https://www.aihero.dev/posts) / [aihero.dev/skills](https://www.aihero.dev/skills) — Pocock's own site, narrative/rationale layer
- [Agent Skills – Codex | OpenAI Developers](https://developers.openai.com/codex/skills) — OpenAI official, current
- [Custom instructions with AGENTS.md – Codex](https://developers.openai.com/codex/guides/agents-md) — OpenAI official, current
- [GitHub - vercel-labs/skills](https://github.com/vercel-labs/skills) — Vercel official, community registry tooling
- [Introducing skills, the open agent skills ecosystem — Vercel changelog](https://vercel.com/changelog/introducing-skills-the-open-agent-skills-ecosystem) — Vercel official, 2026-01-20
- [Skill Lock File System | vercel-labs/skills | DeepWiki](https://deepwiki.com/vercel-labs/skills/5.9-skill-lock-file-system) — third-party generated docs of the official repo, fetched 2026-07-06
- [A lockfile for agent skills - Thilo Maier](https://maier.tech/notes/a-lockfile-for-agent-skills) — community commentary, not fetched in full, listed for completeness
