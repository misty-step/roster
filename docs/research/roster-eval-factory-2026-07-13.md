# Roster evaluation factory: research and inventory

Research date: 2026-07-13. This is a source-dated synthesis for the
`roster-eval-factory` card, not a benchmark runtime or a model leaderboard.

## Decision at this milestone

Keep Roster's compiler boundary intact. Roster should continue to declare and
resolve roles, packs, guidance, skills, MCPs, and agent bindings. Crucible
should own run records, comparisons, uncertainty, and transcripts/evidence;
Bench should own benchmark packages and publication; Powder should own durable
findings and work state. No evaluation runtime, workflow semantics, or public
benchmark corpus belongs in the Roster catalog.

The live dogfood supports one small Roster change: strengthen the existing
always-loaded Powder guidance with a deduplicated feedback-card contract. The
change is in `primitives/guidance/work-ledger.md`; it adds no role, pack, skill,
MCP, evaluator, or model default. It is justified by a reproduced Powder CLI
behavior documented below.

The model comparison does not justify a composition or default-model change.
DeepSeek scored 31/32 and GLM 32/32 on the current 32-task corpus, but Crucible
reported the paired comparison as underpowered and inside its noise floor. The
later SWE-Bench Pro audit below also rules out treating a familiar benchmark
score as clean capability evidence without task-quality review.

## Product and authority boundaries

| Surface | Owns | Does not own |
| --- | --- | --- |
| Roster | deterministic catalog validation, source-qualified resolution, immutable bundles, thin Tier 1 launch projection | eval execution, benchmark publication, board workflow, model policy |
| Crucible | EvalSpec validation, trials, graders, run ledger, transcripts/evidence, comparisons, uncertainty | Roster declarations or Powder workflow |
| Bench | benchmark task packages, fixtures, references, mutants, consumer-facing publication inputs | a privileged evaluation engine or Roster composition |
| Powder | cards, claims, durable activity, relations, completion state | benchmark semantics or Roster compilation |

## Primary-source benchmark and eval inventory

The following sources were read on 2026-07-13. Descriptions are paraphrases;
the links are the primary source or the project's official documentation.

| Source and date | What it measures or specifies | Actionable implication here |
| --- | --- | --- |
| Anthropic, “Demystifying evals for AI agents,” 2026 | A trial is a task, agent/harness, grader, transcript, and outcome; outcome and trajectory can require different graders. It recommends starting from observed failures, balancing positive and negative examples, isolating trials, and reading transcripts. | Keep the Roster corpus decision-shaped and deterministic where possible. Treat the current run as evidence about one narrow oracle task, not proof of general agent quality. |
| Terminal-Bench 2.0, arXiv 2601.11868, 2026-01-17 | 89 difficult terminal tasks with isolated environments, human-authored solutions, and verification tests, distributed through a harness. | This is a model/harness capability benchmark lead for Crucible + Bench, not a reason to add terminal execution to Roster. |
| SWE-Bench Pro, arXiv 2509.16941, 2025-09-21 | 1,865 long-horizon software tasks across 41 maintained repositories, with public, held-out, and commercial partitions. | A benchmark-family lead for Crucible + Bench, not a Roster capability oracle; task quality must be checked before using any split or score. |
| OpenAI, “Why SWE-bench Verified no longer measures frontier coding capabilities,” 2026-02-23 | An audit found material flaws in at least 59.4% of the sampled Verified tasks and cites contamination and test-quality concerns. | Do not use SWE-bench Verified as a Roster capability oracle; preserve clean local fixtures and exact provenance. |
| OpenAI, “Separating signal from noise in coding evaluations,” 2026-07-08 | A SWE-Bench Pro audit found 200/731 public-split tasks (27.4%) flagged by automated analysis, 249 (34.1%) identified by human annotation, and estimates ~30% of tasks are broken; it retracts the earlier recommendation to adopt SWE-Bench Pro. | This supersedes the February-era recommendation: do not treat SWE-Bench Pro, including held-out or commercial partitions, as clean capability evidence without prompt/test/gold-patch quality review and retained provenance. |
| τ-bench, arXiv 2406.12045, 2024-06-17 | Tool-using agents are judged by final database state under domain policies and API tools; pass^k exposes reliability across repeated trials. | A future work-ledger or dispatch eval should grade state and repeat reliability, not prose or trajectory style. It belongs in Crucible/Bench. |
| WebArena, arXiv 2307.13854, 2023-07-25 | Agents complete realistic multi-site tasks against reproducible web environments and are graded on functional end state. | Useful prior art for future Harness isolation tests; outside the present Roster catalog. |
| Inspect AI task docs, current | The minimal task surface is dataset + solver + scorer, with configurable epochs, sandboxes, limits, and trace inspection. | Confirms the right extension point is an eval harness adapter, not imperative logic in Roster declarations. |
| OpenAI Evals repository and build-eval guide, current | A registry-backed framework supports custom/private evals and a shared eval shape. | Catalog conventions are useful prior art; Roster should reference executable eval artifacts without becoming their runner. |

## Current local inventory

Counts were taken from the clean Roster checkout at commit `510a40c` on
2026-07-13:

| Catalog or eval surface | Count | Evidence command or path |
| --- | ---: | --- |
| Public roles | 10 | `rg --files roles` |
| Public packs | 12 | `rg --files packs` |
| Guidance files | 13 | `rg --files primitives/guidance` |
| Skill entrypoints | 32 | `rg --files primitives/skills \| rg '/SKILL\\.md$'` |
| Skill eval specs | 31 | `rg --files primitives/skills \| rg '/evals/[^/]+\\.md$'` |
| Skill waivers | 22 | `rg --files primitives/skills \| rg '/evals/WAIVER\\.md$'` |
| MCP registry entries | 16 | `rg -c '^  - id:' primitives/mcps/registry.yaml` |
| Roster-owned Crucible specs | 1 | `evals/card-oracle-triage-v0.json` |

The effective `smith` composition is the `agent-creator` role plus the
`agent-creation` pack. Its declared guidance is engineering, work-ledger,
agent-creator, and delegation; its selected skills are orient, roster, powder,
harness-engineering, skill-eval, eval-design, mcp-design, and research; and its
selected MCPs are Powder and Crucible. The role and pack remain semantic
composition; model, Harness, reasoning, and native args remain binding data.

The Roster-owned eval is `card-oracle-triage-v0`: 32 real fleet-card fixtures,
balanced 16 READY / 16 NOT_READY classes, a deterministic regex grader, Wilson
95% uncertainty, and a declared minimum effect of interest of 0.3. Its oracle
asks whether a cold agent can execute a stated completion check; it explicitly
does not infer readiness from workflow status. The local Bench checkout also
contains the `seam-agency-v0` Harbor package with task environments,
verification tests, references, and mutants. That package is a consumer input
for Crucible rather than a Roster primitive.

## Dogfood evidence

### Roster graph and bundle

Before mutation:

```text
roster check: ok (725 primitive files)
roster graph: ok (1 agents from /tmp/roster-smith-luna.yaml)
```

The live `roster show smith` binding was `agent-creator`, Codex,
`gpt-5.6-luna`, xhigh, with the guidance, skill, and MCP composition listed
above. The published private work contract returned HTTP 200; its
operator-specific artifact URL is intentionally omitted from this public
transcript.

The fresh resolved-bundle proof used the effective temporary config
`/tmp/roster-smith-luna.yaml` and these exact commands from the Roster checkout:

```sh
cargo run --locked -p roster-cli -- --config /tmp/roster-smith-luna.yaml --cwd "$ROSTER_ROOT" check
cargo run --locked -p roster-cli -- --config /tmp/roster-smith-luna.yaml --cwd "$ROSTER_ROOT" show smith
cargo run --locked -p roster-cli -- --config /tmp/roster-smith-luna.yaml --cwd "$ROSTER_ROOT" resolve smith --output "$CRUCIBLE_ROOT/runs/local/roster-resolved-smith-bundle-20260714"
cargo run --locked -p roster-cli -- --config /tmp/roster-smith-luna.yaml --cwd "$ROSTER_ROOT" dispatch smith --dry-run
```

All four exited 0. The retained bundle is
`$CRUCIBLE_ROOT/runs/local/roster-resolved-smith-bundle-20260714/`
(`AGENTS.md`, `skills/`, `mcps.yaml`, and `manifest.yaml`); its manifest names
the `agent-creator` role, the selected guidance/skills/MCPs, and the
`gpt-5.6-luna` Codex binding.

### Crucible run and comparison

The spec validated as runnable. The CLI run used the Roster spec and retained
these artifacts:

- DeepSeek report and evidence:
  `$CRUCIBLE_ROOT/runs/local/roster-card-oracle-triage-v0-20260713/run-report.json`
  and `prompt-run.json` — 31/32, Wilson 95% [0.8426, 0.9945].
- GLM report and evidence:
  `$CRUCIBLE_ROOT/runs/local/roster-card-oracle-triage-v0-20260713-glm/run-report.json`
  and `prompt-run.json` — 32/32, Wilson 95% [0.8928, 1.0000].
- Paired comparison ledger:
  `$CRUCIBLE_ROOT/runs/local/roster-card-oracle-triage-v0-20260713/crucible-runs.sqlite`.

The comparison delta was 0.03125. Crucible classified the paired McNemar
result as `inside_noise_floor`, with resolution ratio 0.1311 and required N
244; the READY-class difference was one task and was also underpowered. The
only DeepSeek miss was `bl-powder-kanban-ui`, while all 16 NOT_READY fixtures
were correct. This is a useful follow-up fixture for oracle interpretation,
not evidence to change Roster's model selection.

The Crucible MCP validation surface worked, but the MCP model runner refused
before execution because its process did not receive `OPENROUTER_API_KEY`; the
same shell had passed `crucible doctor` and the CLI runner completed. This is a
projected MCP-environment limitation, not a compiler failure, and is filed in
Powder under the Crucible repository as
`crucible-mcp-credential-env-propagation` (P2, ready, related to
`roster-eval-factory`).

The fresh-context critic correctly identified that this baseline was a
standalone prompt benchmark: its system prompt was authored directly in the
spec and did not come from a resolved Roster bundle. The blocker was closed by
running a separate Crucible prompt benchmark whose system prompt is the
retained bundle's `AGENTS.md` and whose task context is the retained bundle's
`skills/powder/SKILL.md`:

- Spec and full resolved system prompt:
  `$CRUCIBLE_ROOT/runs/local/roster-resolved-bundle-oracle-v0-20260714/spec.json`
- Retained Crucible report and model output/transcript evidence:
  `$CRUCIBLE_ROOT/runs/local/roster-resolved-bundle-oracle-v0-20260714/run/run-report.json`
  and `prompt-run.json`
- Result: `resolved-powder-claimability` returned `DECLINE` as expected for a
  card with no acceptance criteria; deterministic rubric 1/1, Wilson 95%
  interval [0.2065, 1.0000]. This is composition exercise evidence, not a
  general capability claim.

### Powder CLI acceptance behavior

Installed Powder was `0.1.0 (git 1d1ded8bccaf)`. A throwaway database showed
that both `create-card` and `update-card` use the first occurrence of
`--acceptance` only: later repeated values are silently discarded. The
reproduction retained the first criterion on create and the first updated
criterion on update. Existing Powder inventory already contains cards for
multi-line backlog import truncation and acceptance-field semantics, but no
card for this repeated-flag CLI behavior. One deduplicated Powder card is
therefore warranted; no second card should be filed in Roster or Crucible. It
was filed as `powder-cli-repeated-acceptance` (P2, related to
`roster-eval-factory`) and has since been completed with deployed proof in its
Powder run.

## Verification and closeout state

The post-correction Roster gate passed from the repository root on 2026-07-13:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace suite ran 46 tests with zero failures. The live temporary Smith
composition also passed the exact `check`, `show`, `resolve`, and `dispatch
--dry-run` commands recorded above; the final proof bundle was resolved to
`/tmp/roster-smith-final-proof`.

This research milestone is ready to advance only after a fresh critic accepts
the corrected diff. Commit, push, GitHub CI, clean-tree readback, and remote
parity are closeout evidence that necessarily follows that pre-commit critic;
the Powder epic must not be completed until all of them are green.

## Leads and stop conditions

The next useful eval lead is a small, held-out dogfood corpus that tests
whether an agent can distinguish an executable acceptance oracle from a goal,
process step, or workflow status, then repeats the same fixtures across a
declared model/harness axis. Promote it only if the comparison has enough
power, a clean partition, and retained transcripts. A second lead is an
end-state dispatch smoke test that checks the retained bundle and receipt,
with act and decline cases; its runner belongs to Crucible and its work card
belongs to Powder.

Remaining uncertainty includes one 32-task prompt run per model, one observed
READY false negative, no human regrade of that fixture, the one-task
resolved-bundle exercise being composition evidence rather than capability
evidence, and the MCP/CLI environment mismatch. No public benchmark score or
model default should be changed on this evidence.

## Sources

- [Anthropic — Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents) — official, read 2026-07-13.
- [Terminal-Bench 2.0](https://arxiv.org/abs/2601.11868) — primary paper, 2026-01-17; [official site](https://www.tbench.ai/).
- [SWE-Bench Pro](https://arxiv.org/abs/2509.16941) — primary paper, 2025-09-21.
- [OpenAI — Why SWE-bench Verified no longer measures frontier coding capabilities](https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/) — official, 2026-02-23.
- [OpenAI — Separating signal from noise in coding evaluations](https://openai.com/index/separating-signal-from-noise-coding-evaluations/) — official, 2026-07-08.
- [τ-bench](https://arxiv.org/abs/2406.12045) — primary paper, 2024-06-17.
- [WebArena](https://arxiv.org/abs/2307.13854) — primary paper, 2023-07-25.
- [Inspect AI task documentation](https://inspect.aisi.org.uk/tasks.html) — official documentation, read 2026-07-13.
- [OpenAI Evals](https://github.com/openai/evals) and [build-eval guide](https://github.com/openai/evals/blob/main/docs/build-eval.md) — official repository/docs, read 2026-07-13.
