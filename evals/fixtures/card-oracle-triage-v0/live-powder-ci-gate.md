# CI gate and license

Status: done · Priority: p0

Make Powder's quality floor external to the agent running a local command. The
repository already declares MIT in Cargo metadata and now carries a root
license file; the remaining work is to make the Rust gate run on every PR and
to keep release-intelligence automation from being the only GitHub workflow.

## Acceptance
- GitHub Actions runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` on pull requests.
- A deliberately failing test or formatting error produces a red PR check in a disposable branch.
- The Landmark release workflow remains intact and does not replace the Rust gate.
- The root `LICENSE` remains MIT for Misty Step LLC and package metadata stays consistent.
- Branch protection or an equivalent repository rule is documented or enabled so the gate is not honor-system only.
