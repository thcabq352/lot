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

1. **Writer** — brief, style, cast, draft, revise, lock
2. **Breakdown** — import/parse (ScriptBreak-equivalent, including `NAME (quietly)`)
3. **Wall / Picture** — beats, lock shot cards
4. **Slate** — prompts on shots
5. **Dailies** — ingest `01-foo.mp4` → shot 01 (do not rename the shot), circle, FCPXML
6. **Cut** — same FCPXML interchange. Resolve Studio is optional later.

Stage / Motion / Stems stay engines until native. Do not port Wasserman Electron.

## Brains

Grok (xAI OAuth) #1 when online. Local OpenAI-compat stays. Cursor #1 for Lot repo work. `lot doctor` lists what is actually up. Stills later: `backend=grok|comfy`, no silent swap.

## William bar

When a human UI exists it must pass the William bar in `docs/plan-agent-first-film-lot.md`. Do not start Tauri from this skill.
