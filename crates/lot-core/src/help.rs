//! `lot help --json` is the contract. Stale website ≠ the spec.

use serde_json::{json, Value};

pub fn help_spec() -> Value {
    json!({
        "name": crate::NAME,
        "version": crate::VERSION,
        "door": ["cli", "mcp"],
        "school_default": "off",
        "notice": "Flags only. No TTY prompts. Exit 0 = ok. Do not click folder dialogs. --agent / LOT_AGENT / MCP agent: who writes (unset = human, no auto-claim). --cap / LOT_CAP / MCP cap: read|write|render|export|spend (unset = all). Jail = this show.lot + LOT_MEDIA_ROOTS; fountain is scene text (AC-013). Show budget: lot budget --spend/--render (hit cap → stop).",
        "verbs": [
            verb("lot status", "lot_status", "kernel", "First call. Kernel + current show."),
            verb("lot create <dir> --name", "lot_create", "kernel", "Create a show.lot and make it current."),
            verb("lot open <dir>", "lot_open", "kernel", "Open a show.lot and make it current."),
            verb("lot doctor", "lot_doctor", "kernel", "Probe ffmpeg, Comfy, brains, Studio, Blockout."),
            verb("lot --cap read|write|render|export|spend", "", "kernel", "AC-012. Unset = all. read cannot circle or stills generate. write cannot Comfy/Grok spend without render/spend."),
            verb("lot --agent <id>", "", "kernel", "Who writes. Unset = human (no auto-claim). Second agent gets locked_by."),
            verb("lot lock", "lot_lock", "kernel", "Claim the show. One writer at a time."),
            verb("lot unlock [--force]", "lot_unlock", "kernel", "Release the show lock. Holder or --force."),
            verb("lot budget --spend --render", "lot_budget", "kernel", "Per-show spend/render cap. Hit cap → stop. Unset = unlimited. Agent caps are separate."),
            verb("lot log [--n] [--export]", "lot_log", "kernel", "Audit: who/what/rev. --export writes audit/export.jsonl with tokens redacted."),
            verb("lot handoff [--commit]", "lot_handoff", "kernel", "Advance phase. Default is dry-run. --commit writes only when the gate passes. cut — no next."),
            verb("lot show", "lot_show", "kernel", "Read lot://show. Meta, phase, lock, last event. Not the fountain."),
            verb("lot scene --id", "lot_scene", "kernel", "Read lot://scenes/{id}."),
            verb("lot shot --id|--num", "lot_shot", "kernel", "Read lot://shots/{id}."),
            verb("lot take --id", "lot_take", "kernel", "Read lot://takes/{id}."),
            verb("lot import --file", "lot_import", "kernel", "Import .scriptbreak / .cork-board.json / canvas / .blockout / .sbref / Slate project.json / .ctake. Does not delete the source. No invented glTF or still."),
            verb("lot help --json", "lot_help", "kernel", "This spec. The binary is the contract."),
            verb("lot snapshot", "lot_snapshot", "kernel", "Freeze show.json + fountain at the current rev."),
            verb("lot restore --rev", "lot_restore", "kernel", "Restore a snapshot. Later drafts do not eat earlier ones."),
            verb("lot writer brief --text", "lot_writer_brief", "writer", "Set the brief."),
            verb("lot writer style --genre --living --canon --format", "lot_writer_style", "writer", "Dated packs. Influence, not endorsement."),
            verb("lot writer cast --name", "lot_writer_cast", "writer", "Add/update a character or replace via --from-json."),
            verb("lot writer draft", "lot_writer_draft", "writer", "Fountain via Grok or Ollama. No brain → no fake script."),
            verb("lot writer revise --notes", "lot_writer_revise", "writer", "Revise existing fountain."),
            verb("lot writer lock", "lot_writer_lock", "writer", "Lock the writer contract."),
            verb("lot writer unlock", "lot_writer_unlock", "writer", "Unlock the writer."),
            verb("lot breakdown import --file", "lot_breakdown_import", "breakdown", "Import .txt / .fountain / .scriptbreak. Jail: no other-show paths. Fountain is scene text (AC-013). Does not delete the source."),
            verb("lot breakdown parse", "lot_breakdown_parse", "breakdown", "Parse fountain into scenes + default shots."),
            verb("lot wall add --text", "lot_wall_add", "wall", "Cork Board beat card."),
            verb("lot picture lock --shot", "lot_picture_lock", "picture", "Lock a shot card. Does not rename the shot."),
            verb("lot stage place --shot --who --mark", "lot_stage_place", "stage", "2D floor mark. 3D stays in Blockout."),
            verb("lot stage camera --shot --size --lens --move", "lot_stage_camera", "stage", "Camera card on the shot."),
            verb("lot stage export", "lot_stage_export", "stage", "Write stage/block.json. No fake glTF."),
            verb("lot motion plate --file --shot --mode", "lot_motion_plate", "motion", "Attach a plate. Does not rename the shot."),
            verb("lot motion marks --shot --move --notes", "lot_motion_marks", "motion", "Camera / performance marks. No MediaPipe."),
            verb("lot motion export", "lot_motion_export", "motion", "motion/previs.json. No fake OpenPose."),
            verb("lot motion analyze --shot", "lot_motion_analyze", "motion", "ffprobe or LOT_MOTION_CMD."),
            verb("lot stills generate --shot --backend", "lot_stills_generate", "board", "backend grok|comfy. Records seed, prompt hash, duration, VRAM cap. Unset LOT_COMFY_WORKFLOW uses packs/comfy-flux-still.json. No silent swap. No fake PNG."),
            verb("lot stills describe --shot", "lot_stills_describe", "board", "Look at a still/plate. Grok vision or Ollama VL. No invented look."),
            verb("lot board export", "lot_board_export", "board", "board/board.json from shots + stills + slate."),
            verb("lot slate set --shot --prompt", "lot_slate_set", "slate", "Canon prompt. --target writes a rewrite only."),
            verb("lot slate compile --shot --target", "lot_slate_compile", "slate", "Brain or LOT_PROMPT_SERVER. No invented rewrite."),
            verb("lot slate target --id", "lot_slate_target", "slate", "Default compile target."),
            verb("lot slate lora --id --weight --model", "lot_slate_lora", "slate", "LoRA metadata on a shot or the show."),
            verb("lot dailies ingest --file", "lot_dailies_ingest", "dailies", "01-foo.mp4 binds to shot 01 without renaming it."),
            verb("lot dailies circle --take", "lot_dailies_circle", "dailies", "Requires --take. No GUI."),
            verb("lot dailies export", "lot_dailies_export", "dailies", "FCPXML of circled takes."),
            verb("lot stems soundtrack --brief", "lot_stems_soundtrack", "stems", "Cue sheet. --generate needs LOT_SOUNDTRACK_CMD. Never a silent stub."),
            verb("lot stems vo --text --generate", "lot_stems_vo", "stems", "SAPI / piper / espeak / say, or --file."),
            verb("lot finish --upscale --fps", "lot_finish", "cut", "Optional pickup. ffmpeg or LOT_UPSCALE_CMD. No stub."),
            verb("lot cut export", "lot_cut_export", "cut", "Same FCPXML interchange."),
            verb("lot mcp", "", "kernel", "NDJSON JSON-RPC 2.0 on stdin/stdout.")
        ]
    })
}

fn verb(cli: &str, mcp: &str, section: &str, job: &str) -> Value {
    json!({
        "cli": cli,
        "mcp": mcp,
        "section": section,
        "job": job,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_lists_stage_snapshot_help() {
        let v = help_spec();
        let mcps: Vec<&str> = v["verbs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x["mcp"].as_str())
            .collect();
        for n in [
            "lot_stage_place",
            "lot_snapshot",
            "lot_restore",
            "lot_help",
            "lot_lock",
            "lot_unlock",
            "lot_budget",
            "lot_log",
            "lot_handoff",
            "lot_show",
            "lot_import",
            "lot_motion_plate",
            "lot_stills_describe",
        ] {
            assert!(mcps.contains(&n), "missing {n} in {mcps:?}");
        }
        assert_eq!(v["school_default"], "off");
    }
}

pub fn help_plain() -> String {
    let spec = help_spec();
    let mut out = format!(
        "lot {} — agent-first film tools. Stdio first, GUI last.\n\n",
        crate::VERSION
    );
    if let Some(verbs) = spec["verbs"].as_array() {
        let mut section = "";
        for v in verbs {
            let sec = v["section"].as_str().unwrap_or("");
            if sec != section {
                section = sec;
                out.push_str(&format!("\n[{sec}]\n"));
            }
            out.push_str(&format!("  {}\n", v["cli"].as_str().unwrap_or("")));
        }
    }
    out.push_str("\nlot help --json is the contract.\n");
    out
}
