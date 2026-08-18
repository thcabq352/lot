const PATHS = ["director", "writer", "dp", "editor", "producer", "filmmaker"];
const AMOUNTS = ["mute", "nudge", "coach", "walkthrough"];

const emptyEl = document.getElementById("empty");
const bayEl = document.getElementById("bay");
const noticeEl = document.getElementById("notice");
const railEl = document.getElementById("rail");
const eventEl = document.getElementById("event");
const schoolEl = document.getElementById("school");
const schoolOnEl = document.getElementById("school-on");
const schoolExtraEl = document.getElementById("school-extra");
const pathEl = document.getElementById("path");
const nameEl = document.getElementById("name");
const gateEl = document.getElementById("gate");
const deskEl = document.getElementById("desk");
const cardEl = document.getElementById("card");
const cardKickerEl = document.getElementById("card-kicker");
const cardLedeEl = document.getElementById("card-lede");
const cardRowsEl = document.getElementById("card-rows");
const cardConfirmEl = document.getElementById("card-confirm");
const cardGateEl = document.getElementById("card-gate");

let viewing = null;
let lastStatus = null;

function invoke(name, args) {
  const core = window.__TAURI__ && window.__TAURI__.core;
  if (!core) {
    return Promise.resolve({ ok: false, error: "open via lot-ui" });
  }
  return core.invoke(name, args || {});
}

function titlePhase(id) {
  return id ? id.charAt(0).toUpperCase() + id.slice(1) : "";
}

function showNotice(msg) {
  if (!msg) {
    noticeEl.hidden = true;
    noticeEl.textContent = "";
    return;
  }
  noticeEl.hidden = false;
  noticeEl.textContent = msg;
}

function clearSchoolExtra() {
  schoolExtraEl.replaceChildren();
  schoolExtraEl.hidden = true;
}

function fillSelect(id, values, current) {
  const sel = document.createElement("select");
  sel.id = id;
  for (const v of values) {
    const opt = document.createElement("option");
    opt.value = v;
    opt.textContent = v;
    if (v === current) opt.selected = true;
    sel.appendChild(opt);
  }
  sel.addEventListener("change", onSchoolExtra);
  return sel;
}

function renderSchool(school) {
  const on = !!(school && school.enabled);
  schoolEl.dataset.on = on ? "true" : "false";
  schoolOnEl.checked = on;
  clearSchoolExtra();
  if (!on) {
    return;
  }
  const pathLabel = document.createElement("label");
  pathLabel.append("Path", fillSelect("school-path", PATHS, school.path || "writer"));
  const amountLabel = document.createElement("label");
  amountLabel.append(
    "Amount",
    fillSelect("school-amount", AMOUNTS, school.help || "nudge")
  );
  schoolExtraEl.append(pathLabel, amountLabel);
  schoolExtraEl.hidden = false;
}

function activePhase(st) {
  return viewing || (st && st.phase) || "writer";
}

function renderRail(st, selected) {
  const phases = Array.isArray(st.phases) ? st.phases : [];
  const dirty = new Set(st.dirty || []);
  const current = st.phase || "";
  railEl.replaceChildren();
  for (const id of phases) {
    const li = document.createElement("li");
    li.dataset.phase = id;
    li.tabIndex = 0;
    if (id === current) {
      li.classList.add("lit");
      li.setAttribute("aria-current", "step");
    }
    if (id === selected) li.classList.add("view");
    if (dirty.has(id)) li.classList.add("dirty");
    const tick = document.createElement("span");
    tick.className = "tick";
    tick.setAttribute("aria-hidden", "true");
    const name = document.createElement("span");
    name.textContent = titlePhase(id);
    li.append(tick, name);
    railEl.appendChild(li);
  }
}

function renderEvent(ev) {
  if (!ev || !ev.kind) {
    eventEl.hidden = true;
    eventEl.textContent = "";
    return;
  }
  eventEl.hidden = false;
  eventEl.replaceChildren();
  const kind = document.createElement("strong");
  kind.textContent = ev.kind;
  eventEl.append("last  ", kind, "  ·  ", ev.who || "human", "  ·  rev ", String(ev.rev ?? ""));
}

function opt(el) {
  const v = el && el.value ? el.value.trim() : "";
  return v || null;
}

function joinList(items) {
  return (items || []).filter(Boolean).join("  ·  ");
}

function leaf(path) {
  if (!path) return "";
  return String(path).split(/[/\\]/).filter(Boolean).pop() || "";
}

function addRow(title, meta) {
  const li = document.createElement("li");
  li.append(title);
  if (meta) {
    const m = document.createElement("span");
    m.className = "meta";
    m.textContent = meta;
    li.appendChild(m);
  }
  cardRowsEl.appendChild(li);
}

function field(label, id, placeholder) {
  const wrap = document.createElement("label");
  wrap.className = "field";
  const input = document.createElement("input");
  input.id = id;
  input.type = "text";
  input.spellcheck = id === "wall-text";
  if (placeholder) input.placeholder = placeholder;
  wrap.append(label, input);
  return wrap;
}

function actionButton(label, act) {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.dataset.act = act;
  btn.textContent = label;
  return btn;
}

function renderWriter(writer) {
  const w = writer || {};
  const briefNow = document.getElementById("brief-now");
  const brief = (w.brief || "").trim();
  briefNow.textContent = brief || "no brief";
  briefNow.classList.toggle("empty", !brief);
  const briefText = document.getElementById("brief-text");
  if (briefText && briefText !== document.activeElement) {
    briefText.value = w.brief || "";
  }

  const styleBits = [
    joinList(w.genres),
    joinList(w.styles_living),
    joinList(w.styles_canon),
    w.format || "",
  ].filter(Boolean);
  const styleNow = document.getElementById("style-now");
  styleNow.textContent = styleBits.length ? styleBits.join("  ·  ") : "no style";
  const formatEl = document.getElementById("style-format");
  if (formatEl && formatEl !== document.activeElement) {
    formatEl.value = w.format || "";
  }

  const castNow = document.getElementById("cast-now");
  castNow.replaceChildren();
  for (const member of w.cast || []) {
    if (!member || !member.name) continue;
    const li = document.createElement("li");
    li.append(member.name);
    if (member.function) {
      const fn = document.createElement("span");
      fn.className = "fn";
      fn.textContent = member.function;
      li.append(fn);
    }
    castNow.appendChild(li);
  }

  const draftNow = document.getElementById("draft-now");
  const path = w.draft_path || "";
  const leaf = path.split(/[/\\]/).filter(Boolean).pop() || "";
  const bits = [];
  if (leaf) bits.push(leaf);
  else bits.push("no draft");
  bits.push(w.locked ? "locked" : "open");
  const prov = w.draft_provenance;
  if (prov && (prov.backend || prov.model)) {
    bits.push([prov.backend, prov.model].filter(Boolean).join(" "));
  }
  draftNow.textContent = bits.join("  ·  ");
}

function renderCard(sec) {
  const phase = (sec && sec.phase) || "";
  const card = (sec && sec.card) || {};
  cardKickerEl.textContent = titlePhase(phase);
  cardRowsEl.replaceChildren();
  const lede = cardLede(phase, card);
  cardLedeEl.textContent = lede.text;
  cardLedeEl.classList.toggle("empty", !!lede.empty);
  fillCardRows(phase, card);
  renderConfirm(phase, card);
  renderGate(sec && sec.handoff);
}

function cardLede(phase, card) {
  if (phase === "breakdown") {
    const n = card.scenes || 0;
    if (!n) return { text: "no scenes", empty: true };
    return { text: `${n} scene${n === 1 ? "" : "s"}  ·  ${card.shots || 0} shots`, empty: false };
  }
  if (phase === "wall") {
    const n = card.beats || 0;
    return n ? { text: `${n} beat${n === 1 ? "" : "s"}`, empty: false } : { text: "no beats", empty: true };
  }
  if (phase === "picture") {
    const n = card.shots || 0;
    if (!n) return { text: "no shots", empty: true };
    return { text: `${card.locked || 0} of ${n} locked`, empty: false };
  }
  if (phase === "stage") {
    const n = card.marks || 0;
    if (!n && !card.block) return { text: "no marks", empty: true };
    return { text: n ? `${n} mark${n === 1 ? "" : "s"}` : "stage exported", empty: false };
  }
  if (phase === "motion") {
    if (!(card.cards || []).some((c) => c.move || c.notes || c.plate) && !card.previs) {
      return { text: "no marks", empty: true };
    }
    return { text: `${card.plates || 0} plate${card.plates === 1 ? "" : "s"}`, empty: false };
  }
  if (phase === "board") {
    const n = card.stills || 0;
    return n ? { text: `${n} still${n === 1 ? "" : "s"}`, empty: false } : { text: "no stills", empty: true };
  }
  if (phase === "slate") {
    const n = card.prompts || 0;
    return n ? { text: `${n} prompt${n === 1 ? "" : "s"}`, empty: false } : { text: "no slate", empty: true };
  }
  if (phase === "dailies") {
    const n = card.takes || 0;
    if (!n) return { text: "no takes", empty: true };
    return { text: `${card.circled || 0} circled  ·  ${n} take${n === 1 ? "" : "s"}`, empty: false };
  }
  if (phase === "stems") {
    const brief = (card.brief || "").trim();
    return brief ? { text: brief, empty: false } : { text: "no stems", empty: true };
  }
  if (phase === "cut") {
    const n = card.circled || 0;
    if (card.fcpxml || card.finish) {
      return { text: n ? `${n} circled` : "cut files", empty: false };
    }
    return n ? { text: `${n} circled`, empty: false } : { text: "no cut", empty: true };
  }
  return { text: titlePhase(phase), empty: true };
}

function fillCardRows(phase, card) {
  if (phase === "breakdown") {
    for (const sc of card.slugs || []) {
      if (!sc) continue;
      addRow([sc.num, sc.slug].filter(Boolean).join("  "), null);
    }
    const chars = joinList(card.characters);
    const locs = joinList(card.locations);
    if (chars) addRow(chars, "cast");
    if (locs) addRow(locs, "locations");
    const f = leaf(card.fountain_path);
    if (f) addRow(f, "fountain");
    return;
  }
  if (phase === "wall") {
    for (const beat of card.cards || []) {
      if (!beat || !beat.text) continue;
      addRow(beat.text, [beat.id, beat.act].filter(Boolean).join("  ·  ") || null);
    }
    return;
  }
  if (phase === "picture") {
    for (const sh of card.cards || []) {
      if (!sh) continue;
      const ref = leaf(sh.ref);
      addRow(
        [sh.num, sh.name].filter(Boolean).join("  "),
        [sh.locked ? "locked" : "open", sh.size, ref].filter(Boolean).join("  ·  ")
      );
    }
    return;
  }
  if (phase === "stage") {
    for (const sh of card.cards || []) {
      if (!sh) continue;
      const cam = [sh.size, sh.angle, sh.lens, sh.move].filter(Boolean).join("  ·  ");
      const who = (sh.marks || []).map((m) => [m.who, m.mark].filter(Boolean).join(" ")).filter(Boolean).join("  ·  ");
      addRow([sh.num, sh.name].filter(Boolean).join("  "), [cam, who].filter(Boolean).join("  ·  ") || null);
    }
    const block = leaf(card.block_path);
    if (block) addRow(block, "block");
    return;
  }
  if (phase === "motion") {
    for (const sh of card.cards || []) {
      if (!sh) continue;
      const meta = [sh.mode, sh.move, sh.notes, leaf(sh.plate) || (sh.plate ? "plate" : "")].filter(Boolean).join("  ·  ");
      addRow([sh.num, sh.name].filter(Boolean).join("  "), meta || null);
    }
    const previs = leaf(card.previs_path);
    if (previs) addRow(previs, "previs");
    return;
  }
  if (phase === "board") {
    for (const sh of card.cards || []) {
      if (!sh) continue;
      const still = leaf(sh.still);
      addRow([sh.num, sh.name].filter(Boolean).join("  "), still ? [still, sh.backend].filter(Boolean).join("  ·  ") : "no still");
    }
    const pack = leaf(card.board_path);
    if (pack) addRow(pack, "board");
    return;
  }
  if (phase === "slate") {
    if (card.target) addRow(card.target, "target");
    for (const sh of card.cards || []) {
      if (!sh || !sh.prompt) continue;
      addRow(sh.prompt, [sh.num, (sh.targets || []).join(" ")].filter(Boolean).join("  ·  "));
    }
    return;
  }
  if (phase === "dailies") {
    for (const tk of card.cards || []) {
      if (!tk) continue;
      addRow(tk.filename || tk.id, [tk.circled ? "circled" : "open", leaf(tk.path)].filter(Boolean).join("  ·  "));
    }
    return;
  }
  if (phase === "stems") {
    if (card.cue) addRow(leaf(card.cue) || card.cue, "cue");
    if (card.soundtrack) addRow(leaf(card.soundtrack) || card.soundtrack, "soundtrack");
    if (card.vo_text) addRow(card.vo_text, "vo");
    if (card.vo) addRow(leaf(card.vo) || card.vo, "vo file");
    return;
  }
  if (phase === "cut") {
    if (card.fcpxml_path) addRow(leaf(card.fcpxml_path), "fcpxml");
    if (card.edl_path) addRow(leaf(card.edl_path), "edl");
    if (card.finish) addRow(leaf(card.finish) || card.finish, card.upscaled ? "finish · upscaled" : "finish");
  }
}

function renderConfirm(phase, card) {
  cardConfirmEl.replaceChildren();
  if (phase === "breakdown" && card.fountain) {
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.appendChild(actionButton("Parse", "parse"));
    cardConfirmEl.appendChild(actions);
    cardConfirmEl.hidden = false;
    return;
  }
  if (phase === "wall") {
    const ids = document.createElement("div");
    ids.className = "row";
    ids.append(field("Id", "wall-id", "beat-1"), field("Act", "wall-act", "i"));
    const beat = field("Beat", "wall-text", "what happens");
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.append(
      actionButton("Add beat", "wall"),
      actionButton("Update", "wall-update"),
      actionButton("Remove", "wall-remove")
    );
    cardConfirmEl.append(ids, beat, actions);
    cardConfirmEl.hidden = false;
    return;
  }
  if (phase === "picture") {
    const row = document.createElement("div");
    row.className = "row";
    row.append(field("Shot", "lock-shot", "01"), field("Path", "picture-file", "path to a ref"));
    const extra = document.createElement("div");
    extra.className = "row";
    extra.append(field("Note", "picture-note", "optional"), field("Size", "picture-size", "WIDE"));
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.append(
      actionButton("Lock", "picture"),
      actionButton("Unlock", "picture-unlock"),
      actionButton("Ref", "picture-ref")
    );
    cardConfirmEl.append(row, extra, actions);
    cardConfirmEl.hidden = false;
    return;
  }
  cardConfirmEl.hidden = true;
}

function renderGate(h) {
  cardGateEl.replaceChildren();
  if (!h) {
    cardGateEl.hidden = true;
    return;
  }
  let line = "";
  if (!h.next) {
    line = "cut — no next";
  } else if (h.ready) {
    line = `ready  ${h.from || h.phase || ""} → ${h.next}`;
  } else {
    const miss = (h.missing || []).filter(Boolean).join("  ·  ");
    line = miss || "handoff blocked —";
  }
  cardGateEl.append(line);
  if (h.next) {
    const actions = document.createElement("div");
    actions.className = "actions";
    actions.appendChild(actionButton("Advance", "handoff"));
    cardGateEl.appendChild(actions);
  }
  cardGateEl.hidden = false;
}

function renderTheory(payload, schoolOn) {
  const theoryEl = document.getElementById("theory");
  if (!schoolOn || !payload || !payload.theory) {
    theoryEl.hidden = true;
    theoryEl.textContent = "";
    return;
  }
  const t = payload.theory;
  const line = [t.apply, t.rule].find((s) => s && String(s).trim());
  if (!line) {
    theoryEl.hidden = true;
    theoryEl.textContent = "";
    return;
  }
  theoryEl.hidden = false;
  theoryEl.textContent = line;
}

function render(st) {
  if (!st || st.ok === false) {
    showNotice((st && st.error) || "no show —");
  } else {
    showNotice("");
  }
  const hasShow = !!(st && st.show);
  const firstOpen = hasShow && bayEl.hidden;
  emptyEl.hidden = hasShow;
  bayEl.hidden = !hasShow;
  if (hasShow) {
    if (firstOpen) {
      bayEl.classList.remove("rise");
      void bayEl.offsetWidth;
      bayEl.classList.add("rise");
    }
    document.getElementById("show-name").textContent = st.show_name || "untitled";
    const lock = st.locked_by ? `locked · ${st.locked_by}` : "open";
    document.getElementById("show-meta").textContent = [
      st.phase || "writer",
      st.rev != null ? `rev ${st.rev}` : null,
      st.renderer || null,
      lock,
    ]
      .filter(Boolean)
      .join("  ·  ");
    const phase = activePhase(st);
    renderRail(st, phase);
    renderEvent(st.last_event);
    renderWriter(st.writer);
    const onWriter = phase === "writer";
    deskEl.hidden = !onWriter;
    cardEl.hidden = onWriter;
    if (!onWriter) {
      const ready = st.section && st.section.phase === phase;
      if (ready) {
        renderCard(st.section);
      }
    }
  }
  renderSchool(st && st.school);
  if (!(st && st.school && st.school.enabled)) {
    renderTheory(null, false);
  }
}

async function refresh() {
  const st = await invoke("status");
  lastStatus = st;
  render(st);
  const phase = activePhase(st);
  if (st && st.show && phase !== "writer") {
    if (!(st.section && st.section.phase === phase)) {
      const sec = await invoke("section", { phase });
      if (sec && sec.ok !== false) {
        renderCard(sec);
      } else if (sec && sec.error) {
        showNotice(sec.error);
      }
    }
  }
}

async function selectPhase(id) {
  viewing = id;
  if (!lastStatus) {
    await refresh();
    return;
  }
  render(lastStatus);
  if (id === "writer") return;
  const sec = await invoke("section", { phase: id });
  if (sec && sec.ok !== false) {
    showNotice("");
    renderCard(sec);
  } else {
    showNotice((sec && sec.error) || "no show —");
  }
}

async function submit(act) {
  const path = pathEl.value;
  const name = nameEl.value.trim();
  const out = await invoke(act, { path, name: name || null });
  if (!out.ok) {
    showNotice(out.error || "no show —");
    return;
  }
  viewing = null;
  await refresh();
}

async function onSchoolToggle() {
  const enabled = schoolOnEl.checked;
  const args = { enabled };
  if (enabled) {
    args.path = "writer";
    args.amount = "nudge";
  }
  const out = await invoke("school_set", args);
  if (!out.ok) {
    schoolOnEl.checked = !enabled;
    showNotice(out.error || "school set —");
    return;
  }
  await refresh();
}

async function onSchoolExtra() {
  const path = document.getElementById("school-path");
  const amount = document.getElementById("school-amount");
  const out = await invoke("school_set", {
    enabled: true,
    path: path ? path.value : null,
    amount: amount ? amount.value : null,
  });
  if (!out.ok) {
    showNotice(out.error || "school set —");
    return;
  }
  await refresh();
}

gateEl.addEventListener("submit", (e) => {
  e.preventDefault();
  submit("create");
});

gateEl.querySelector("[data-act=open]").addEventListener("click", () => {
  submit("open");
});

schoolOnEl.addEventListener("change", onSchoolToggle);

async function confirmWriter(act) {
  let out;
  if (act === "brief") {
    out = await invoke("writer_brief", {
      text: document.getElementById("brief-text").value,
    });
  } else if (act === "style") {
    out = await invoke("writer_style", {
      genre: opt(document.getElementById("style-genre")),
      living: opt(document.getElementById("style-living")),
      canon: opt(document.getElementById("style-canon")),
      format: opt(document.getElementById("style-format")),
    });
  } else if (act === "cast") {
    out = await invoke("writer_cast", {
      name: opt(document.getElementById("cast-name")),
      function: opt(document.getElementById("cast-function")),
      look: opt(document.getElementById("cast-look")),
      must_not: opt(document.getElementById("cast-must-not")),
    });
  } else if (act === "draft") {
    document.getElementById("draft-now").textContent = "drafting…";
    out = await invoke("writer_draft");
  } else if (act === "revise") {
    const notes = document.getElementById("revise-notes").value;
    document.getElementById("draft-now").textContent = "revising…";
    out = await invoke("writer_revise", { notes });
  } else if (act === "lock") {
    out = await invoke("writer_lock");
  } else if (act === "unlock") {
    out = await invoke("writer_unlock");
  } else {
    return;
  }
  if (!out.ok) {
    showNotice(out.error || "writer —");
    await refresh();
    return;
  }
  showNotice("");
  await refresh();
  renderTheory(out, !!(out.school && out.school.enabled));
}

deskEl.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-writer]");
  if (!btn) return;
  confirmWriter(btn.getAttribute("data-writer"));
});

async function confirmSection(act) {
  let out;
  if (act === "parse") {
    out = await invoke("breakdown_parse");
  } else if (act === "wall") {
    out = await invoke("wall_add", {
      act: opt(document.getElementById("wall-act")),
      text: document.getElementById("wall-text") ? document.getElementById("wall-text").value : "",
    });
  } else if (act === "wall-update") {
    out = await invoke("wall_update", {
      id: document.getElementById("wall-id") ? document.getElementById("wall-id").value : "",
      act: opt(document.getElementById("wall-act")),
      text: opt(document.getElementById("wall-text")),
    });
  } else if (act === "wall-remove") {
    out = await invoke("wall_remove", {
      id: document.getElementById("wall-id") ? document.getElementById("wall-id").value : "",
    });
  } else if (act === "picture") {
    out = await invoke("picture_lock", {
      shot: document.getElementById("lock-shot") ? document.getElementById("lock-shot").value : "",
    });
  } else if (act === "picture-unlock") {
    out = await invoke("picture_unlock", {
      shot: document.getElementById("lock-shot") ? document.getElementById("lock-shot").value : "",
    });
  } else if (act === "picture-ref") {
    out = await invoke("picture_ref", {
      shot: document.getElementById("lock-shot") ? document.getElementById("lock-shot").value : "",
      file: document.getElementById("picture-file") ? document.getElementById("picture-file").value : "",
      note: opt(document.getElementById("picture-note")),
      size: opt(document.getElementById("picture-size")),
    });
  } else if (act === "handoff") {
    out = await invoke("handoff", { commit: true });
  } else {
    return;
  }
  if (!out.ok) {
    showNotice(out.error || "lot —");
    await refresh();
    return;
  }
  showNotice("");
  if (act === "handoff") viewing = null;
  await refresh();
  renderTheory(out, !!(out.school && out.school.enabled));
}

cardEl.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-act]");
  if (!btn) return;
  confirmSection(btn.getAttribute("data-act"));
});

cardConfirmEl.addEventListener("submit", (e) => {
  e.preventDefault();
});

railEl.addEventListener("click", (e) => {
  const li = e.target.closest("li[data-phase]");
  if (!li) return;
  selectPhase(li.dataset.phase);
});

railEl.addEventListener("keydown", (e) => {
  if (e.key !== "Enter" && e.key !== " ") return;
  const li = e.target.closest("li[data-phase]");
  if (!li) return;
  e.preventDefault();
  selectPhase(li.dataset.phase);
});

refresh();
