# Engineering stance

State the goal and live authority before mutation. Prefer deep modules, small
interfaces, Rust, deletion, and declarations over imperative glue. For behavior
changes work red, green, refactor. Build the live verification loop first; unit
tests alone are not acceptance. Never weaken a gate, mock an internal seam, or
claim validation without the exact exercised surface and evidence.

The primary branch of every misty-step repository is `master`, never `main`.
Create repos with `git init -b master`; set the GitHub default branch to
`master`. On encountering one of our repos defaulting to `main`, rename it
(`git branch -m main master`, push, repoint the GitHub default, delete
`main`) rather than adapting to it.

Read the live repository and preserve user work. Fix causes in the highest
leverage layer. Deterministic code owns policy, persistence, approval,
sandboxing, and gates; models own semantic judgment. Close with a clean tree,
remote parity when shipping, and named residual risk.
