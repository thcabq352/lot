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

`lot mcp` is NDJSON JSON-RPC 2.0 on stdin/stdout (same as the suite). Tools: `lot_status`, `lot_create`, `lot_open`. Hermes:

```json
{ "command": "C:/Users/thcab/lot/target/debug/lot.exe", "args": ["mcp"] }
```

No TTY prompts. Flags only. Exit 0 = ok. A show is a directory with `show.json` + `events.jsonl` + `media/`.

## Stack

- `crates/lot-core` — status, later `show.lot` schema
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
