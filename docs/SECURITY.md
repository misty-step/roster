# Publication security

Roster is a public declarations library. A publication gate must protect both
credential values and operator-private identity across the surfaces that can
leave the repository.

## Gate surfaces

`./scripts/publication-security-gate` is the deterministic, value-free gate for:

- tracked and newly added non-ignored source files (the complete detector set);
  the Rust `public_library_contains_no_operator_identifiers` oracle remains an
  independent structural defense;
- staged diffs, including local pre-merge work;
- every generated public bundle and retained receipt/log directory supplied by
  CI;
- the commit range introduced by a pull request, including author/committer
  metadata and message text; and
- pull-request title and body from `GITHUB_EVENT_PATH`.

Diagnostics contain only a surface, a repository- or supplied-root-relative
location, a line number, and a detector name.
They never print the matched line or credential bytes. `--self-test` generates
negative fixtures in memory, mutates one detector class at a time, and confirms
that every detector fails closed. The fixtures are not written to the public
repository.

## Detector policy

The custom gate covers absolute home paths, private Tailnet hostnames,
non-allowlisted email addresses, embedded URL credentials, sensitive URL paths
and queries, private endpoint host labels, PEM private keys, and representative
AWS, GitHub, OpenAI, Slack, JWT, bearer, Vault, and credential-assignment
shapes. Gitleaks and TruffleHog remain the maintained secret-pattern backstops;
CI pins their container versions and withholds their reports from artifacts.
Unreadable or binary files in supplied bundle, receipt, or explicit paths fail
closed as `unreadable-artifact`; tracked binary source assets remain the
responsibility of the repository scanner backstops.

The only intentional allowances are:

- `example.com`, `users.noreply.github.com`, and `hey@herdr.dev` for public
  documentation and fixture identities;
- public URL hosts used by the product documentation (`crates.io`,
  `github.com`, `misty-step.github.io`, `openrouter.ai`, and
  `www.rust-lang.org`);
- symbolic environment roots such as `$HOME`, which are not machine paths; and
- two exact vendor-documentation fixtures: `/home/sprite/` only inside the
  `skills/sprites` primitive, and `powder.internal` only inside the
  `skills/powder` primitive. These are public example literals, not Roster
  checkout paths, and credentials in either fixture remain forbidden.

No other allowance applies to a credential embedded in a URL, a
secret-shaped value, private endpoint path, absolute home path, Tailnet
hostname, or personal email.
The Gitleaks configuration has no source-value allowlist. Allowances are owned
by this file and the gate source, reviewed with the publication card, and last
reviewed 2026-07-16.

## CI contract

The required CI job runs, in addition to the repository gate and `roster check`:

- Gitleaks `v8.30.1` with `--redact` and a withheld JSON report;
- TruffleHog `v3.95.9` over Git history with JSON output withheld;
- the gate mutation self-test;
- a clean resolution of every public agent in `examples/config.yaml` followed
  by a generated-bundle and receipt/log scan; and
- pull-request commit and prose checks when the event is a pull request.

A publication is not green when any detector exits non-zero. A detector failure
is reported by location and class only, so evidence artifacts remain safe to
retain.
