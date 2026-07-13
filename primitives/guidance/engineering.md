# Engineering stance

State the goal and live authority before mutation. Prefer deep modules, small
interfaces, Rust, deletion, and declarations over imperative glue. For behavior
changes work red, green, refactor. Build the live verification loop first; unit
tests alone are not acceptance. Never weaken a gate, mock an internal seam, or
claim validation without the exact exercised surface and evidence.

Read the live repository and preserve user work. Fix causes in the highest
leverage layer. Deterministic code owns policy, persistence, approval,
sandboxing, and gates; models own semantic judgment. Close with a clean tree,
remote parity when shipping, and named residual risk.
