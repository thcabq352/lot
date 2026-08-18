//! Headless School exam pack. Rubrics are data. Never blocks export.

use crate::model::{Scene, Shot};
use crate::show::{append_event, bump, require_write_current, write_show, Show, ShowError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const RUBRICS_JSON: &str = include_str!("../packs/school/rubrics.json");
const FIXTURE_NO_WANT: &str = include_str!("../packs/school/fixtures/no-want.json");
const FIXTURE_AXIS_FAIL: &str = include_str!("../packs/school/fixtures/axis-fail.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rubric {
    pub id: String,
    pub track: String,
    pub rule: String,
    pub counter_example: String,
    pub apply: String,
    pub cite: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Score {
    pub id: String,
    pub pass: bool,
    pub rule: String,
    pub counter_example: String,
    pub apply: String,
    pub cite: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoreReport {
    pub ok: bool,
    pub fixture: Option<String>,
    pub scene: Option<String>,
    pub scores: Vec<Score>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExamReport {
    pub ok: bool,
    pub passed: bool,
    pub fixture: Option<String>,
    pub show_id: Option<String>,
    pub scores: Vec<Score>,
}

#[derive(Debug, Deserialize)]
struct RubricFile {
    items: Vec<Rubric>,
}

#[derive(Debug, Deserialize)]
struct FixtureFile {
    id: String,
    scene: Scene,
    #[serde(default)]
    shots: Vec<Shot>,
}

fn rubrics() -> Vec<Rubric> {
    serde_json::from_str::<RubricFile>(RUBRICS_JSON)
        .map(|f| f.items)
        .unwrap_or_default()
}

pub fn rubric(id: &str) -> Result<Value, ShowError> {
    let id = id.trim();
    rubrics()
        .into_iter()
        .find(|r| r.id == id)
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .ok_or_else(|| ShowError::Msg(format!("no rubric — {id}")))
}

fn load_fixture(id: &str) -> Result<FixtureFile, ShowError> {
    let raw = match id.trim() {
        "no-want" => FIXTURE_NO_WANT,
        "axis-fail" => FIXTURE_AXIS_FAIL,
        other => {
            return Err(ShowError::Msg(format!("no fixture — {other}")));
        }
    };
    serde_json::from_str(raw).map_err(|e| ShowError::Msg(e.to_string()))
}

fn want_pass(scene: &Scene) -> bool {
    let blob = format!(
        "{} {} {}",
        scene.synopsis,
        scene.text,
        scene.characters.join(" ")
    )
    .to_ascii_lowercase();
    if blob.contains("nobody wants") || blob.contains("no one wants") || blob.contains("no want") {
        return false;
    }
    let markers = [
        "want", "need", "must", "gotta", "have to", "will not", "won't", "don't",
    ];
    markers.iter().any(|m| blob.contains(m)) || !scene.characters.is_empty()
}

fn screen_dir(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_lowercase();
    if t.contains("camera left") || t.contains("screen left") {
        Some("left")
    } else if t.contains("camera right") || t.contains("screen right") {
        Some("right")
    } else {
        None
    }
}

fn axis_pass(shots: &[Shot]) -> bool {
    if shots.iter().any(|s| {
        let d = format!("{} {}", s.desc, s.angle).to_ascii_lowercase();
        d.contains("crossed axis") || d.contains("crosses the axis")
    }) {
        return false;
    }
    let mut prev_dir: Option<&str> = None;
    let mut prev_was_setup = false;
    for s in shots {
        let dir = screen_dir(&s.desc).or_else(|| screen_dir(&s.angle));
        let reverse = s.angle.to_ascii_lowercase().contains("reverse");
        if reverse && prev_was_setup {
            if let (Some(a), Some(b)) = (prev_dir, dir) {
                if a == b {
                    return false;
                }
            }
        }
        if let Some(d) = dir {
            prev_dir = Some(d);
            prev_was_setup = !reverse;
        }
    }
    true
}

fn score_world(scene: Option<&Scene>, shots: &[Shot]) -> Vec<Score> {
    rubrics()
        .into_iter()
        .map(|r| {
            let pass = match r.id.as_str() {
                "want-vs-need" => scene.map(want_pass).unwrap_or(false),
                "axis" => axis_pass(shots),
                _ => true,
            };
            Score {
                id: r.id,
                pass,
                rule: r.rule,
                counter_example: r.counter_example,
                apply: r.apply,
                cite: r.cite,
            }
        })
        .collect()
}

fn pick_scene<'a>(show: &'a Show, scene: Option<&str>) -> Result<Option<&'a Scene>, ShowError> {
    if let Some(key) = scene.map(str::trim).filter(|s| !s.is_empty()) {
        return show
            .scenes
            .iter()
            .find(|s| s.id == key || s.num == key)
            .map(Some)
            .ok_or_else(|| ShowError::Msg(format!("unknown scene: {key}")));
    }
    Ok(show.scenes.first())
}

pub fn school_score(scene: Option<&str>, fixture: Option<&str>) -> Result<ScoreReport, ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    if let Some(fid) = fixture.map(str::trim).filter(|s| !s.is_empty()) {
        let fx = load_fixture(fid)?;
        let scores = score_world(Some(&fx.scene), &fx.shots);
        return Ok(ScoreReport {
            ok: true,
            fixture: Some(fx.id),
            scene: Some(fx.scene.id),
            scores,
        });
    }
    let (_, show) = crate::show::require_current()?;
    let sc = pick_scene(&show, scene)?;
    let shots: Vec<Shot> = match sc {
        Some(s) => show
            .shots
            .iter()
            .filter(|sh| sh.scene_id == s.id || sh.scene_id.is_empty())
            .cloned()
            .collect(),
        None => show.shots.clone(),
    };
    Ok(ScoreReport {
        ok: true,
        fixture: None,
        scene: sc.map(|s| s.id.clone()),
        scores: score_world(sc, &shots),
    })
}

pub fn school_exam(fixture: Option<&str>) -> Result<ExamReport, ShowError> {
    let score = school_score(None, fixture)?;
    let passed = !score.scores.is_empty() && score.scores.iter().all(|s| s.pass);
    let show_id = if fixture.is_some() {
        None
    } else {
        crate::show::require_current().ok().map(|(_, s)| s.id)
    };
    Ok(ExamReport {
        ok: true,
        passed,
        fixture: score.fixture,
        show_id,
        scores: score.scores,
    })
}

pub fn school_get() -> Result<(std::path::PathBuf, Show), ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    crate::show::require_current()
}

pub fn school_set(
    enabled: Option<bool>,
    path: Option<&str>,
    level: Option<&str>,
    amount: Option<&str>,
) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    if let Some(on) = enabled {
        show.school.enabled = on;
    }
    if let Some(p) = path.map(str::trim).filter(|s| !s.is_empty()) {
        show.school.path = Some(normalize_path(p)?);
    }
    if let Some(l) = level.map(str::trim).filter(|s| !s.is_empty()) {
        show.school.level = Some(normalize_level(l)?);
    }
    if let Some(a) = amount.map(str::trim).filter(|s| !s.is_empty()) {
        show.school.help = Some(normalize_help(a)?);
    }
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "school.set", &show)?;
    Ok((dir, show))
}

fn normalize_path(id: &str) -> Result<String, ShowError> {
    match id {
        "director" | "writer" | "editor" | "producer" | "filmmaker" => Ok(id.into()),
        "dp" | "camera" | "dp-camera" => Ok("dp".into()),
        _ => Err(ShowError::Msg(format!("unknown school path — {id}"))),
    }
}

fn normalize_level(id: &str) -> Result<String, ShowError> {
    match id {
        "beginner" | "intermediate" | "working" => Ok(id.into()),
        _ => Err(ShowError::Msg(format!("unknown school level — {id}"))),
    }
}

fn normalize_help(id: &str) -> Result<String, ShowError> {
    match id {
        "mute" | "nudge" | "coach" | "walkthrough" => Ok(id.into()),
        _ => Err(ShowError::Msg(format!("unknown school amount — {id}"))),
    }
}

#[cfg(test)]
mod tests {
    fn isolate() {
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        crate::clear_caps();
        crate::clear_agent();
        let tmp = std::env::temp_dir().join(format!(
            "lot-school-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("LOT_HOME", tmp.join("home"));
    }

    #[test]
    fn score_no_want_fixture_fails_want_rubric() {
        let report = crate::school_score(None, Some("no-want")).unwrap();
        let want = report
            .scores
            .iter()
            .find(|s| s.id == "want-vs-need")
            .expect("want-vs-need");
        assert!(!want.pass, "{:?}", report.scores);
        assert!(!want.rule.is_empty());
        assert!(!want.counter_example.is_empty());
        assert!(!want.apply.is_empty());
        assert!(!want.cite.is_empty());
    }

    #[test]
    fn score_axis_fail_fixture_fails_axis_rubric() {
        let report = crate::school_score(None, Some("axis-fail")).unwrap();
        let axis = report.scores.iter().find(|s| s.id == "axis").expect("axis");
        assert!(!axis.pass, "{:?}", report.scores);
        let want = report
            .scores
            .iter()
            .find(|s| s.id == "want-vs-need")
            .expect("want-vs-need");
        assert!(want.pass, "axis-fail fixture has a want");
    }

    #[test]
    fn exam_fixture_never_blocks_export() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = std::env::temp_dir().join(format!("lot-school-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::create_show(&dir, Some("ExamExport")).unwrap();
        let exam = crate::school_exam(Some("no-want")).unwrap();
        assert!(!exam.passed, "{exam:?}");
        crate::set_brief("Ada waits.").unwrap();
        let err = crate::dailies_export().unwrap_err().to_string();
        assert!(err.contains("no circled takes"), "{err}");
        assert!(!err.contains("exam"), "{err}");
        let show = crate::read_show(&dir).unwrap();
        assert_eq!(show.writer.brief, "Ada waits.");
        assert!(!show.school.enabled);
    }

    #[test]
    fn school_off_writer_has_no_lesson_fields() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = std::env::temp_dir().join(format!("lot-school-noleak-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::create_show(&dir, Some("NoLesson")).unwrap();
        crate::set_brief("Ada will not put it on.").unwrap();
        let show = crate::read_show(&dir).unwrap();
        let v = crate::mutation_json(
            &dir,
            &show,
            serde_json::json!({ "brief": show.writer.brief }),
        );
        assert_eq!(v["school"]["enabled"], false);
        let obj = v.as_object().unwrap();
        for forbidden in ["lesson", "quiz", "theory", "school_note", "rubric"] {
            assert!(
                !obj.contains_key(forbidden),
                "school off must not leak {forbidden} in {v}"
            );
        }
    }

    #[test]
    fn rubric_resource_after_school_on() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = std::env::temp_dir().join(format!("lot-school-on-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::create_show(&dir, Some("RubricOn")).unwrap();
        crate::school_set(Some(true), None, None, None).unwrap();
        let (_, _, card) = crate::resource_read("lot://school/rubric/want-vs-need").unwrap();
        assert_eq!(card["id"], "want-vs-need");
        assert!(card["rule"].as_str().unwrap().contains("want"));
    }

    #[test]
    fn unknown_fixture_is_honest() {
        let err = crate::school_exam(Some("not-a-fixture"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no fixture —"), "{err}");
    }
}
