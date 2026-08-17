# AGENTS.md — Lot

You are an agent in the shop. Humans confirm. You do not click folder dialogs.

## Home

`C:\Users\thcab\lot` (or this repo root). **Not** `video-buddy-suite`.

## First call

```
lot status --json
lot create <dir> --name "Title" --json
lot open <dir> --json
lot mcp
```

Optional `--show <path>` (CLI global) or MCP `path` opens that show, then runs. Omit to keep the current pointer.

`lot mcp` is NDJSON JSON-RPC 2.0 on stdin/stdout (same as the suite). Tools:

- `lot_status`, `lot_create`, `lot_open`
- `lot_writer_brief`, `lot_writer_style`, `lot_writer_cast`
- `lot_writer_draft`, `lot_writer_revise`
- `lot_writer_lock`, `lot_writer_unlock`

Hermes:

```json
{ "command": "C:/Users/thcab/lot/target/debug/lot.exe", "args": ["mcp"] }
```

No TTY prompts. Flags only. Exit 0 = ok. A show is a directory with `show.json` + `events.jsonl` + `media/`.

## Writer

Same verbs on CLI and MCP.

```
lot writer brief --text "…"
lot writer style --genre drama --living greta-gerwig --canon akira-kurosawa --format 30min
lot writer cast --name Ada --function lead --look "…" --must-not "…"
lot writer cast --from-json "[{…}]"
lot writer draft --json
lot writer revise --notes "…"
lot writer lock
lot writer unlock
```

Style IDs come from dated JSON packs in `crates/lot-core/packs/` (influence / coverage style — not endorsement). Unknown ID errors contain `unknown genre` / `unknown living` / `unknown canon`. Formats: `feature | 30min | 15s | episodic`.

Lock blocks brief, style, cast, draft, revise (error contains `locked`). Empty brief → `no brief`. No draft → revise errors with `no draft`. No brain → `no brain —` and never a fake fountain.

## Stack

- `crates/lot-core` — status, show schema, Writer, brains
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

## Do not

- Commit a real show
- Put API keys in chat or files
- Require Comfy/Resolve/GPU for `lot status`
