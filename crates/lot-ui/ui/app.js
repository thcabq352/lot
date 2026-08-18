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

function renderRail(st) {
  const phases = Array.isArray(st.phases) ? st.phases : [];
  const dirty = new Set(st.dirty || []);
  const current = st.phase || "";
  railEl.replaceChildren();
  for (const id of phases) {
    const li = document.createElement("li");
    if (id === current) {
      li.classList.add("lit");
      li.setAttribute("aria-current", "step");
    }
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

function render(st) {
  if (!st || st.ok === false) {
    showNotice((st && st.error) || "no show —");
  } else {
    showNotice("");
  }
  const hasShow = !!(st && st.show);
  emptyEl.hidden = hasShow;
  bayEl.hidden = !hasShow;
  if (hasShow) {
    bayEl.classList.remove("rise");
    void bayEl.offsetWidth;
    bayEl.classList.add("rise");
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
    renderRail(st);
    renderEvent(st.last_event);
  }
  renderSchool(st && st.school);
}

async function refresh() {
  const st = await invoke("status");
  render(st);
}

async function submit(act) {
  const path = pathEl.value;
  const name = nameEl.value.trim();
  const out = await invoke(act, { path, name: name || null });
  if (!out.ok) {
    showNotice(out.error || "no show —");
    return;
  }
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

refresh();
