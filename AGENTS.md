# AGENTS.md — Lot

You are an agent in the shop. Humans confirm. You do not click folder dialogs.

## Home

`C:\Users\thcab\lot` (or this repo root). **Not** `video-buddy-suite`.

## First call

```
lot status --json
lot create <dir> --name "Title" --json
lot open <dir> --json
lot doctor --json
lot mcp
```

Optional `--show <path>` (CLI global) or MCP `path` opens that show, then runs. Omit to keep the current pointer.

`lot mcp` is NDJSON JSON-RPC 2.0 on stdin/stdout. Tools:

- `lot_status`, `lot_create`, `lot_open`, `lot_doctor`
- `lot_writer_brief`, `lot_writer_style`, `lot_writer_cast`, `lot_writer_draft`, `lot_writer_revise`, `lot_writer_lock`, `lot_writer_unlock`
- `lot_breakdown_import`, `lot_breakdown_parse`
- `lot_wall_add`, `lot_picture_lock`, `lot_slate_set`
- `lot_dailies_ingest`, `lot_dailies_circle`, `lot_dailies_export`, `lot_cut_export`
- `lot_stems_soundtrack`, `lot_stems_vo`

Hermes:

```json
{ "command": "C:/Users/thcab/lot/target/debug/lot.exe", "args": ["mcp"] }
```

Skill: `skills/film-lot/SKILL.md`.

No TTY prompts. Flags only. Exit 0 = ok. A show is a directory with `show.json` + `events.jsonl` + `media/`.

## Writer

```
lot writer brief --text "…"
lot writer style --genre drama --living greta-gerwig --canon akira-kurosawa --format advertisement
# formats: feature | 30min | 15s | episodic | advertisement | music-video  (aliases: ad, mv)
lot writer cast --name Ada --function lead
lot writer draft --json
lot writer revise --notes "…"
lot writer lock
```

Empty brief → `no brief`. No brain → `no brain —` and never a fake fountain.

## Breakdown (ScriptBreak logic)

```
lot breakdown import --file script.txt --json
lot breakdown parse --json
lot breakdown status --json
```

Parser is ScriptBreak-equivalent (sluglines, `NAME (quietly)` → character ADA). Import does not delete the source `.txt` / `.scriptbreak`.

## Dailies (Circle Take)

```
lot dailies ingest --file 01-foo.mp4 --json
lot dailies circle --take tk-1 --json
lot dailies export --json
```

`01-foo.mp4` binds to shot `01` and **does not** rename the shot to `"01"`. Circle without `--take` exits non-zero (no GUI).

## Stems (soundtrack + VO)

```
lot stems soundtrack --brief "bright organ, no lyrics" --json
lot stems soundtrack --file score.wav --json
lot stems soundtrack --brief "…" --generate --json   # needs LOT_SOUNDTRACK_CMD; never a fake wav
lot stems vo --text "Don't put it on." --generate --json   # Windows SAPI / piper / espeak / say
lot stems vo --file vo.wav --json
```

No soundtrack engine → `no soundtrack engine —` and **no** silent stub. No TTS → `no vo brain —`.

## Stack

- `crates/lot-core` — schema, Writer, Breakdown, Dailies, Stems, doctor
- `crates/lot-cli` — binary `lot`
- `crates/lot-mcp` — stdio MCP (`lot mcp`)

## Rules

1. Agent first. Same verbs on CLI and MCP.
2. School default **off**. No lesson fields if off.
3. Local brains stay. Grok/Cursor are #1 when online.
4. Stills: `grok | comfy` — no silent swap.
5. Secrets never in `show.lot` or this repo.
6. Do not port Wasserman Electron apps here unless replacing an adapter.
7. No LangGraph. Hermes is the filmmaker loop; Cursor builds Lot.
8. **William bar** (canon): `docs/plan-agent-first-film-lot.md`. When a human UI exists it must be a film tool he can be proud of — one show, calm, cinematic, School as a dimmer. No gray form farm. No segregated “special” skin. Do not start Tauri until the current kernel phase is done.

## Do not

- Commit a real show
- Put API keys in chat or files
- Require Comfy/Resolve/GPU for `lot status`
- Start Phase 4 Tauri or Phase 6 installers in the same sprint as kernel work
