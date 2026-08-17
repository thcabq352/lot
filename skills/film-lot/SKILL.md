# film-lot

Hermes skill for **Lot** — agent-first film kernel. One binary, same verbs on CLI and MCP.

## Trigger

“open the lot,” “write a screenplay,” “break this down,” “take this show to dailies.”

## Door

```
lot mcp
```

Hermes:

```json
{ "command": "C:/Users/thcab/lot/target/debug/lot.exe", "args": ["mcp"] }
```

Flags only. No TTY. No folder pickers. `--json` / MCP `path` for a show.

## First call

`lot_status` (or `lot status --json`). Read `school`, `renderer`, `phase`, `doctor`.

If `school.enabled` is false, skip all pedagogy.

## Phase router (no LangGraph)

1. **Writer** — brief, style, cast, draft, revise, lock. Formats: feature | 30min | 15s | episodic | advertisement | music-video (`ad`, `mv`).
2. **Breakdown** — import/parse (ScriptBreak-equivalent, including `NAME (quietly)`)
3. **Wall / Picture** — beats, lock shot cards
4. **Stills / Board** — `stills generate --backend grok|comfy` (no silent swap), then `board export` toward Slate
5. **Slate** — prompts on shots
6. **Dailies** — ingest `01-foo.mp4` → shot 01 (do not rename the shot), circle, FCPXML
7. **Stems** — soundtrack cue (Grok/local) + attach or `LOT_SOUNDTRACK_CMD`; VO generate (SAPI / piper / espeak / say) or attach. Never a fake track.
8. **Cut** — same FCPXML interchange. Resolve Studio is optional later.

Stage / Motion stay engines until native. Stems soundtrack + VO are in-kernel. Do not port Wasserman Electron.

## Brains

Grok (xAI OAuth) #1 when online. Local OpenAI-compat stays. Cursor #1 for Lot repo work. `lot doctor` lists what is actually up. Stills: `backend=grok|comfy`, no silent swap.

## William bar

When a human UI exists it must pass the William bar in `docs/plan-agent-first-film-lot.md`. Do not start Tauri from this skill.
