// moonframe.dev's home page: components, plates, the viewer, the install
// pickers, the hero demo and the copy buttons.
mountComponents(document.getElementById("cbrowser"), "agent-run");

// The apps in examples/ that have a screenshot, with the README's one-liners.
const APPS = [
  ["sessions","Sessions","coding sessions across your repositories"],
  ["workbench","Workbench","a terminal, an editor and files, in tabs and splits"],
  ["daybook","Daybook","a planner with today, an inbox and a month"],
  ["ledger","Ledger","cash against a floor, and funds with their curves"],
  ["audience","Inkwell","a newsletter's audience, signups and sources"],
  ["pipeline","Tally","a sales kanban with KPIs and an agent panel"],
  ["stage","Stage","the components on a bento board"],
  ["parlance","Parlance","a desktop chat app with streaming answers"],
  ["cinder","Cinder","coding-agent sessions grouped by repository"],
  ["cairn","Cairn","chat with a coding agent in your folders, with changes and a terminal"],
  ["pilot","Pilot","a desktop app for the pi coding agent"],
  ["msgvault","msgvault","a reader for a mail, chat and calendar archive"],
  ["tidings","Tidings","a mail client in two layouts"],
  ["trellis","Trellis","a terminal multiplexer around work trees"],
  ["hangar","Hangar","parallel coding agents on one repository, each in a worktree of its own"],
  ["switchyard","Switchyard","a workspace for coding agents across repositories and worktrees"],
  ["lantern","Lantern","a Halloween terminal workspace, with a pumpkin"],
  ["tessel","Tessel","an agents hub with pixel-icon cards"],
  ["helpdesk","Tandem","an IT help desk inbox"],
  ["research","Sorrel","an agent's research workspace"],
  ["cadence","Cadence","a workflow automation dashboard"],
  ["vigil","Vigil","workflow operations at a glance"],
  ["duly","Duly","compliance notices for a public buyer"],
  ["emberline","Emberline","a CRM's leads report in the dark"],
  ["gantry","Gantry","an infra console, in dark monospace"],
  ["hearth","Hearth","a code review bot's analytics"],
  ["storefront","Storefront","a store's admin with its audit log"],
  ["folio","Folio","a quiet place to write, with focus mode"],
  ["orbit","Orbit","a task workspace with connected boards, lists, calendars and timelines"],
];
// The apps that ship on their own (a [release] table in their moonframe.toml),
// each a Homebrew cask moonframe-<id> and an asset of the apps-v release.
const RELEASED = {
  cairn:["Cairn","chat with a coding agent in your folders","Needs Claude Code, Codex or Gemini, signed in."],
  cinder:["Cinder","run a coding agent in a folder, beside your CLI sessions","Needs Claude Code, Codex or Gemini, signed in."],
  daybook:["Daybook","a planner for tasks and events, starting empty",""],
  folio:["Folio","a quiet place to write markdown",""],
  hangar:["Hangar","parallel coding agents on one repository, each in its own worktree","Needs git."],
  lantern:["Lantern","a terminal workspace with a process monitor",""],
  pilot:["Pilot","a desktop app for the pi coding agent","Needs pi installed."],
  sessions:["Sessions","coding sessions across your repositories",""],
  switchyard:["Switchyard","coding agents across repositories and worktrees",""],
  trellis:["Trellis","a terminal multiplexer around work trees",""],
  workbench:["Workbench","a terminal, an editor and files, side by side",""],
};
const brew = id => `brew install moonframehq/tap/moonframe-${id}`;
{
  const pick = document.getElementById("app-pick"), what = document.getElementById("app-what"), code = document.getElementById("app-cmd");
  const choose = id => {
    const [title, d, needs] = RELEASED[id];
    for (const b of pick.children) b.setAttribute("aria-pressed", b.dataset.id === id);
    what.innerHTML = `<b>${title}</b>: ${d}.${needs ? ` <i>${needs}</i>` : ""}`;
    code.textContent = brew(id);
    code.nextElementSibling.textContent = "Copy";
  };
  pick.innerHTML = Object.entries(RELEASED).map(([id, [title]]) =>
    `<button type="button" data-id="${id}" aria-pressed="false">${title}</button>`).join("");
  pick.onclick = e => { const b = e.target.closest("button"); if (b) choose(b.dataset.id); };
  choose("folio");
}
function roman(n) {
  let label = "";
  for (const [value, glyph] of [[1000,"M"],[900,"CM"],[500,"D"],[400,"CD"],[100,"C"],[90,"XC"],[50,"L"],[40,"XL"],[10,"X"],[9,"IX"],[5,"V"],[4,"IV"],[1,"I"]]) {
    while (n >= value) { label += glyph; n -= value; }
  }
  return label;
}
document.getElementById("app-count").textContent = APPS.length;
document.getElementById("plates").innerHTML = APPS.map(([id, title, d], i) =>
  `<figure class="plate" data-i="${i}"><img src="img/${id}.webp" alt="${title}" loading="lazy"><p><span>PL. ${roman(i + 1)}</span><b>${title}.</b> ${d[0].toUpperCase() + d.slice(1)}.</p></figure>`).join("");

// Plates open full size. The thumbnail shows at once, blurred; the full PNG
// streams in behind it with a progress bar, then fades over it.
const lb = document.getElementById("lb"), lo = document.getElementById("lb-lo"), hi = document.getElementById("lb-hi"),
      bar = document.getElementById("lb-bar"), pct = document.getElementById("lb-pct"), cap = document.getElementById("lb-cap"), cmd = document.getElementById("lb-cmd"), how = document.getElementById("lb-how");
let lbIndex = -1, lbToken = 0;
const kb = n => n > 1048576 ? (n / 1048576).toFixed(1) + " MB" : Math.round(n / 1024) + " KB";
// One download per image, shared by everyone who asks for it (the viewer and
// the prefetch of the next image), with its progress reported to each.
const downloads = new Map();
function loadFull(url, onProgress) {
  let d = downloads.get(url);
  if (!d) {
    d = { listeners:new Set(), got:0, total:0 };
    d.promise = (async () => {
      const res = await fetch(url);
      if (!res.ok || !res.body) throw new Error(`${res.status}`);
      d.total = +res.headers.get("content-length") || 0;
      const reader = res.body.getReader(), chunks = [];
      for (;;) {
        const { done, value } = await reader.read(); if (done) break;
        chunks.push(value); d.got += value.length;
        for (const f of d.listeners) f(d.got, d.total);
      }
      return URL.createObjectURL(new Blob(chunks, { type:"image/png" }));
    })().catch(() => { downloads.delete(url); return url; }); // file:// or no streams: the <img> loads it, or fails to
    downloads.set(url, d);
  }
  d.listeners.add(onProgress); onProgress(d.got, d.total);
  return d.promise.finally(() => d.listeners.delete(onProgress));
}
// A set of images the viewer steps through: { lo, full, caption }.
const PLATES = APPS.map(([id, title, d]) => ({ lo:`img/${id}.webp`, full:`img/full/${id}.png`,
  caption:`<b>${title}</b> ${d[0].toUpperCase() + d.slice(1)}.`,
  // A released app installs on its own. The others run from a clone: link the
  // example in place, then open its profile. The link refuses a profile that
  // already exists, so the open runs either way.
  cmd: RELEASED[id] ? brew(id) : `moonframe app link examples/${id} --yes; moonframe --profile ${id}`,
  how: RELEASED[id] ? `<b>Install ${title}</b> on a Mac; Linux and Windows builds are in the <a href="https://github.com/moonframehq/moonframe/releases?q=apps-v&amp;expanded=true">apps release</a>.`
    : `<b>Run ${title}</b> from a clone of the repository, with Moonframe installed.` }));
const SHOTS = [["week-calendar","Week calendar","Day, week and month, with events"],["palette","Command palette","Spotlight, classic, compact or cards"],
  ["table","Table","Sortable, filterable, paged"],["notification-drawer","Notification drawer","Behind a bell, with a badge"]]
  .map(([id, title, d]) => ({ lo:`img/c-${id}.webp`, full:`img/full/c-${id}.png`, caption:`<b>${title}</b> ${d}.` }));
let lbSet = PLATES;
async function openPlate(i, set = lbSet) {
  lbSet = set; lbIndex = (i + set.length) % set.length; const token = ++lbToken;
  const item = set[lbIndex];
  lb.classList.add("on"); document.body.style.overflow = "hidden";
  lo.src = item.lo; lo.style.opacity = 1; lo.style.filter = ""; hi.classList.remove("ready"); hi.removeAttribute("src");
  cap.innerHTML = `${item.caption} · ${lbIndex + 1} of ${set.length}`;
  how.hidden = !item.how; how.innerHTML = item.how || "";
  cmd.hidden = !item.cmd; cmd.firstElementChild.textContent = item.cmd || ""; cmd.lastElementChild.textContent = "Copy";
  bar.style.opacity = 1; bar.classList.remove("indet"); bar.firstElementChild.style.width = "0"; pct.textContent = "Loading full size…";
  const src = await loadFull(item.full, (got, total) => {
    if (token !== lbToken) return;
    if (!total) { bar.classList.add("indet"); pct.textContent = got ? `Loading… ${kb(got)}` : "Loading full size…"; return; }
    bar.classList.remove("indet");
    bar.firstElementChild.style.width = (got / total * 100) + "%"; pct.textContent = `${Math.round(got / total * 100)}% · ${kb(got)} of ${kb(total)}`;
  });
  if (token !== lbToken) return;
  hi.onload = () => { if (token !== lbToken) return; hi.classList.add("ready"); lo.style.opacity = 0; bar.style.opacity = 0; pct.textContent = "Full size"; };
  // If the full size can't load, stop the bar, show the thumbnail clearly, and offer a retry.
  hi.onerror = () => {
    if (token !== lbToken) return;
    downloads.delete(item.full);
    bar.style.opacity = 0; lo.style.filter = "drop-shadow(0 30px 50px #000a)";
    pct.innerHTML = `Couldn't load the full size. <button class="retry">Retry</button>`;
    pct.querySelector(".retry").onclick = e => { e.stopPropagation(); openPlate(lbIndex, lbSet); };
  };
  hi.src = src;
  // Warm the next one so stepping through feels instant.
  loadFull(set[(lbIndex + 1) % set.length].full, () => {});
}
function closePlate() { lb.classList.remove("on"); document.body.style.overflow = ""; lbToken++; }
document.getElementById("plates").addEventListener("click", e => { const f = e.target.closest(".plate"); if (f) openPlate(+f.dataset.i, PLATES); });
[...document.querySelectorAll("#cstrip img")].forEach((img, i) => img.onclick = () => openPlate(i, SHOTS));
const EXPERIMENTAL = [...document.querySelectorAll(".exp-row")].map(row => ({ lo:`img/${row.dataset.id}.webp`, full:`img/full/${row.dataset.id}.png`,
  caption:`<b>${row.querySelector("h3").textContent}</b> ${row.querySelector("p").textContent}` }));
document.querySelectorAll(".exp-shot").forEach((shot, i) => shot.onclick = () => openPlate(i, EXPERIMENTAL));
lb.querySelector(".x").onclick = closePlate;
cmd.onclick = async () => {
  try { await navigator.clipboard.writeText(cmd.firstElementChild.textContent); cmd.lastElementChild.textContent = "Copied"; }
  catch { getSelection().selectAllChildren(cmd.firstElementChild); }
};
// Copy buttons on the get started steps.
// The paths: the card picked shows its steps.
const gsCards = document.querySelectorAll(".gs-card");
gsCards.forEach(c => c.onclick = () => {
  gsCards.forEach(x => x.classList.toggle("on", x === c));
  document.querySelectorAll(".gs-path").forEach(p => p.classList.toggle("on", p.dataset.p === c.dataset.p));
});
// Copy buttons on the code panels.
document.querySelectorAll(".gs-code button").forEach(b => b.onclick = async () => {
  const pre = b.closest(".gs-code").querySelector("pre");
  try { await navigator.clipboard.writeText(pre.textContent); b.textContent = "Copied"; setTimeout(() => b.textContent = "Copy", 1600); }
  catch { getSelection().selectAllChildren(pre); }
});
document.querySelectorAll(".steps .copy, .gs-brew .copy").forEach(b => b.onclick = async () => {
  const label = b.lastElementChild;
  try { await navigator.clipboard.writeText(b.firstElementChild.textContent); label.textContent = "Copied"; setTimeout(() => label.textContent = "Copy", 1600); }
  catch { getSelection().selectAllChildren(b.firstElementChild); }
});
lb.querySelector(".prev").onclick = () => openPlate(lbIndex - 1);
lb.querySelector(".next").onclick = () => openPlate(lbIndex + 1);
lb.addEventListener("click", e => { if (e.target === lb) closePlate(); });
addEventListener("keydown", e => {
  if (!lb.classList.contains("on")) return;
  if (e.key === "Escape") closePlate(); else if (e.key === "ArrowLeft") openPlate(lbIndex - 1); else if (e.key === "ArrowRight") openPlate(lbIndex + 1);
});

// Fig. 1: an agent adds status bar widgets one prompt at a time; each lands in
// the running window while the terminal keeps going.
const $ = id => document.getElementById(id);
const SCENES = [
  { q:"add a CPU sparkline to the status bar", w:"w-cpu", file:"mods/system/main.luau", lines:6,
    code:`<span class="add">+ region = "status-right",</span>
<span class="add">+ ui.icon { name = "cpu" },</span>
<span class="add">+ sparkline(system.cpu_history),</span>
<span class="add">+ fixed(percent(system.cpu), 32, "end"),</span>` },
  { q:"show the branch and what changed", w:"w-branch", file:"mods/status/main.luau", lines:9,
    code:`<span class="add">+ local git = app.git</span>
<span class="add">+ local extra = if git.changed > 0 then &#96; +{git.changed}&#96; else ""</span>
<span class="add">+ ui.text { &#96;⎇ {git.branch}{extra}&#96;, class = "status-text" },</span>` },
  { q:"memory as a small meter next to it", w:"w-mem", file:"mods/system/main.luau", lines:5,
    code:`<span class="add">+ ui.icon { name = "memory-stick" },</span>
<span class="add">+ bar(system.memory, 28),</span>` },
  { q:"and network rates, down and up", w:"w-net", file:"mods/system/main.luau", lines:7,
    code:`<span class="add">+ ui.icon { name = "arrow-down" },</span>
<span class="add">+ fixed(rate(system.received), 34, "start"),</span>
<span class="add">+ ui.icon { name = "arrow-up" },</span>` },
  { q:"turn the sparkline red above 80%", w:"w-cpu", hot:true, file:"mods/system/main.luau", lines:1,
    code:`<span class="add">+ class = if sample > 0.8 then "spark spark-hot" else "spark",</span>` },
];
const log = $("log"), typed = $("typed"), toast = $("toast"), sbar = $("sbar");
let start = Date.now(), cpu = [3,5,4,7,6,9,5,4,8,11,7,6,10,9,12,8];
setInterval(() => {
  const s = Math.floor((Date.now() - start) / 1000);
  $("uptime").textContent = `terminal up ${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")} · never restarted`;
  cpu.push(sbar.classList.contains("hot") && Math.random() < .3 ? 11 + Math.random() : 2 + Math.random() * 9); cpu.shift();
  $("spark").innerHTML = cpu.map(v => `<i class="${v > 10 ? "h" : ""}" style="height:${v}px"></i>`).join("");
  $("cpu").textContent = Math.round(cpu.at(-1) / 12 * 100) + "%";
  $("tail").textContent = `GET /health 200 ${1 + (s % 3)}ms`;
}, 1000);

const wait = ms => new Promise(r => setTimeout(r, ms));
async function type(text) { typed.textContent = ""; for (const ch of text) { typed.textContent += ch; await wait(38); } }
function add(html, cls) { const d = document.createElement("div"); d.className = cls; d.innerHTML = html; log.appendChild(d); while (log.children.length > 9) log.firstChild.remove(); return d; }

async function run() {
  for (;;) {
    for (const el of document.querySelectorAll(".sbar .w")) el.classList.remove("on", "new");
    sbar.classList.remove("hot"); log.innerHTML = ""; $("chg").textContent = "nothing yet"; $("diff").innerHTML = "";
    await wait(800);
    for (const sc of SCENES) {
      for (const el of log.children) el.classList.add("old");
      await type(sc.q); await wait(350);
      typed.textContent = ""; add(sc.q, "q");
      await wait(500); add(`Editing <b>${sc.file}</b>`, "a");
      await wait(500); log.lastChild.insertAdjacentHTML("afterend", `<pre>${sc.code}</pre>`);
      await wait(700); add("✓ just check · just types", "ok");
      const w = $(sc.w); w.classList.add("on", "new"); if (sc.hot) sbar.classList.add("hot");
      $("chg").textContent = sc.file;
      $("diff").innerHTML = `<span class="a">+${sc.lines} lines</span>`;
      toast.classList.add("on"); await wait(1500); toast.classList.remove("on");
      w.classList.remove("new"); await wait(900);
    }
    await wait(2500);
  }
}
run();
