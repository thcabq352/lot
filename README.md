# Lot

Agent-first film tools. **Stdio first, GUI last.**

- CLI: `lot status --json` · `lot help --json` · `lot show` · `lot import` · `lot writer` · `lot breakdown` · `lot stage` · `lot stills` · `lot board` · `lot slate` · `lot motion` · `lot dailies` · `lot stems` · `lot finish` · `lot snapshot` · `lot lock` · `lot budget` · `lot log` · `lot handoff` · `lot plugin` · `lot school exam`
- MCP: `lot mcp`
- Doctor: `lot doctor --json`
- HTTP twin (optional): `lot serve [--bind 127.0.0.1:8787]` — `GET /openapi.json` · `POST /lot_status` (same names as MCP)
- Hermes skill: `skills/film-lot/SKILL.md` (MCP `lot` → `target/debug/lot.exe mcp`; Flux stills pack when `LOT_COMFY_WORKFLOW` is unset)

Not Hardline. Not Pixie. Not a Wasserman re-skin. Lot **replaces** that suite (Writer, Breakdown, Wall, Picture, Stage 2D, Motion plates/marks, Board/stills, Slate, Dailies, Stems, Cut FCPXML+EDL, lot-ui). Do not install ScriptBreak, Cork Board, Master Canvas, Blockout, Motion Previs Studio, Slate, or Circle Take to use Lot.

## Windows pack (no cargo)

```powershell
.\scripts\pack-windows.ps1
.\scripts\pack-windows.ps1 -Ffmpeg
.\scripts\pack-windows.ps1 -Installer
```

Unzip `dist/lot-0.1.0-windows-x64.zip`, or run `dist/lot-0.1.0-windows-x64-setup.exe` when NSIS (`makensis`) is on PATH. Run `lot-ui.exe` or `lot.exe status --json`. Start menu: `install-shortcuts.ps1` (or the setup.exe shortcut). Optional PATH: `install-shortcuts.ps1 -AddPath`.

**In the zip:** `lot.exe`, `lot-ui.exe`. ffmpeg/ffprobe only with `-Ffmpeg` (GPL sidecar — Lot stays MIT OR Apache-2.0; never committed).

**Never in the zip:** Ollama, ComfyUI, DaVinci Resolve, Blockout, Wasserman apps. Those stay optional on the user's machine. `lot doctor` may probe them (`no ffmpeg —` / `no ollama —` / `no comfy —` / `no resolve —` / `no blockout —`). `lot status` still succeeds.

## Dev (any box)

```bash
cargo run -p lot-cli -- status --json
```

Windows: this repo is first-class. Do not assume a menu bar.

Plan: `docs/plan-agent-first-film-lot.md`

## Agent contract

See `AGENTS.md`. If a task needs a file picker, the tool is unfinished.
