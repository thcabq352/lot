# Lot

Agent-first film tools. **Stdio first, GUI last.**

- CLI: `lot status --json` · `lot help --json` · `lot writer` · `lot breakdown` · `lot stage` · `lot stills` · `lot board` · `lot slate` · `lot motion` · `lot dailies` · `lot stems` · `lot finish` · `lot snapshot`
- MCP: `lot mcp`
- Doctor: `lot doctor --json`
- HTTP twin (later): `lot serve`
- Hermes skill: `skills/film-lot/SKILL.md` (MCP `lot` → `target/debug/lot.exe mcp`; Flux stills pack when `LOT_COMFY_WORKFLOW` is unset)

Not Hardline. Not Pixie. Not a Wasserman re-skin. One kernel; old suite apps are adapters.

## Dev (any box)

```bash
cargo run -p lot-cli -- status --json
```

Windows: this repo is first-class. Do not assume a menu bar.

Plan: `docs/plan-agent-first-film-lot.md`

## Agent contract

See `AGENTS.md`. If a task needs a file picker, the tool is unfinished.
