# Antigravity IDE Harness Notes

Antigravity IDE is a Google-family developer environment surface projected by Roster.

## Split between CLI and IDE paths

- **Antigravity CLI (`~/.gemini/antigravity-cli`)**: Configured and used via the terminal tool `agy`. Skills are loaded from `~/.gemini/antigravity-cli/skills` (which defaults to a symlink to `~/.gemini/config/skills`).
- **Antigravity IDE (`~/.gemini/antigravity-ide` & `~/.gemini/config`)**: Used within the editor. Global configurations are stored in `~/.gemini/config`, while IDE-specific extensions, workspace states, and skills are linked/configured in `~/.gemini/antigravity-ide/`.

`roster sync --catalog full --all-agents` populates global skills into:
- `~/.gemini/antigravity-cli/skills/` (resolving to `~/.gemini/config/skills/`)
- `~/.gemini/antigravity-ide/skills/`

Roster sync also links composed `AGENTS.md` into the Antigravity CLI and IDE roots.
IDE settings remain user-owned; Roster does not overwrite editor policy or
workspace state.
