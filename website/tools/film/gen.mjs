// The showcase film on moonframe.dev: a stylised motion graphic in the site's
// palette, with the repository's real screenshots embedded. Writes one SVG per
// frame into $FILM_WORK/frames (default target/film/frames); render.sh
// prepares the images it reads, renders the frames and encodes the videos.
// Usage: node gen.mjs            -> every frame
//        node gen.mjs 120 300    -> only those frames (for checking a scene)
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const WORK = process.env.FILM_WORK || path.join(REPO, "target/film");
const OUT = path.join(WORK, "frames");
fs.mkdirSync(OUT, { recursive: true });
const W = 1920, H = 1080, FPS = 30, DUR = 60.0;
const ONLY = process.argv.slice(2).map(Number);

// ---------- palette (website/site/index.html) ----------
const C = {
  paper: "#f6eedc", paper2: "#eee2c8", ink: "#2a1a10", ink2: "#4a3426", dim: "#6e5a4a",
  teal: "#2e6462", acc: "#a33a14", accHi: "#d9541f", sunY: "#f0b33c", sunO: "#e8662c",
  sunR: "#c43e24", brown: "#3d2318", cream: "#f4e4c8", rule: "#2a1a1022",
};
const SERIF = "Iowan Old Style, Georgia, serif";
const SANS = "Helvetica Neue, Helvetica, Arial, sans-serif";
const MONO = "JetBrainsMono Nerd Font Mono, Menlo, monospace";

// ---------- easing / helpers ----------
const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const lerp = (a, b, t) => a + (b - a) * t;
const prog = (t, a, b) => clamp((t - a) / (b - a));
const eIO = (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
const eOut = (x) => 1 - Math.pow(1 - x, 3);
const eBack = (x) => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); };
const fadeIO = (t, a, b, f = 0.45) => Math.min(eOut(prog(t, a, a + f)), 1 - eIO(prog(t, b - f, b)));
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const hex = (c) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
const mix = (a, b, t) => "#" + hex(a).map((v, i) => Math.round(lerp(v, hex(b)[i], t)).toString(16).padStart(2, "0")).join("");
const tw = (s, size, k = 0.53) => s.length * size * k;
const wrap = (s, n) => { const out = [""]; for (const w of s.split(" ")) { if ((out[out.length - 1] + " " + w).trim().length > n) out.push(w); else out[out.length - 1] = (out[out.length - 1] + " " + w).trim(); } return out; };
const typed = (s, t, a, b) => s.slice(0, Math.round(s.length * prog(t, a, b)));
let uid = 0;

// ---------- primitives ----------
const text = (x, y, s, { size = 16, fill = C.ink, weight = 400, family = SANS, anchor = "start", op = 1, ls = 0, style = "normal" } = {}) =>
  `<text x="${x.toFixed(1)}" y="${y.toFixed(1)}" font-family="${family}" font-size="${size}" font-weight="${weight}" font-style="${style}" fill="${fill}" text-anchor="${anchor}" opacity="${op.toFixed(3)}" letter-spacing="${ls}" xml:space="preserve">${esc(s)}</text>`;
const rect = (x, y, w, h, { rx = 0, fill = "none", stroke = "none", sw = 1, op = 1, filter = "" } = {}) =>
  `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${Math.max(0, w).toFixed(1)}" height="${Math.max(0, h).toFixed(1)}" rx="${rx}" fill="${fill}" stroke="${stroke}" stroke-width="${sw}" opacity="${op.toFixed(3)}" ${filter ? `filter="url(#${filter})"` : ""}/>`;
const g = (inner, { op = 1, tx = 0, ty = 0, s = 1, cx = 0, cy = 0, rot = 0 } = {}) =>
  op <= 0.001 ? "" :
  `<g opacity="${op.toFixed(3)}" transform="translate(${(tx + cx).toFixed(1)} ${(ty + cy).toFixed(1)}) rotate(${rot.toFixed(2)}) scale(${s.toFixed(4)}) translate(${-cx} ${-cy})">${inner}</g>`;
function img(href, x, y, w, h, { rx = 14, shadow = true, slice = true } = {}) {
  const id = `c${uid++}`;
  return `<clipPath id="${id}"><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}"/></clipPath>` +
    (shadow ? rect(x, y, w, h, { rx, fill: "#000", op: 0.28, filter: "shadow" }) : "") +
    `<image href="img/${href}" x="${x}" y="${y}" width="${w}" height="${h}" preserveAspectRatio="${slice ? "xMidYMin slice" : "xMidYMid meet"}" clip-path="url(#${id})"/>` +
    rect(x, y, w, h, { rx, stroke: "#00000022", sw: 1 });
}
function sun(cx, cy, r, op = 1) {
  const id = `s${uid++}`;
  const band = (y0, y1, fill) => `<rect x="${cx - r}" y="${cy - r + y0 * 2 * r}" width="${2 * r}" height="${(y1 - y0) * 2 * r}" fill="${fill}"/>`;
  return `<g opacity="${op.toFixed(3)}"><clipPath id="${id}"><circle cx="${cx}" cy="${cy}" r="${r}"/></clipPath><g clip-path="url(#${id})">
    ${band(0, 0.235, C.sunY)}${band(0.255, 0.315, C.sunO)}${band(0.335, 0.375, C.sunR)}${band(0.392, 1, C.sunR)}</g></g>`;
}
function cursor(x, y, press = 0) {
  const ring = press > 0 && press < 1 ? `<circle cx="${x}" cy="${y}" r="${(6 + 18 * press).toFixed(1)}" fill="none" stroke="${C.accHi}" stroke-width="2.5" opacity="${(1 - press).toFixed(3)}"/>` : "";
  return `${ring}<g transform="translate(${x.toFixed(1)} ${y.toFixed(1)}) scale(1.05)" filter="url(#shadow)">
    <path d="M0 0 L0 22 L5.5 16.5 L9.5 25.5 L13.5 23.8 L9.6 15 L17 15 Z" fill="#fff" stroke="#111" stroke-width="1.4" stroke-linejoin="round"/></g>`;
}
const pressAt = (t, at) => (t >= at && t < at + 0.35 ? (t - at) / 0.35 : 0);
function pill(x, y, label, { fill = C.brown, color = C.cream, size = 22, dot = null, padX = 22, h = 48, op = 1, family = SANS, weight = 500 } = {}) {
  const w = tw(label, size, family === MONO ? 0.6 : 0.52) + padX * 2 + (dot ? 30 : 0);
  return g(rect(x, y, w, h, { rx: h / 2, fill }) +
    (dot ? `<circle cx="${x + padX + 9}" cy="${y + h / 2}" r="9" fill="${dot}"/>` : "") +
    text(x + padX + (dot ? 30 : 0), y + h / 2 + size * 0.36, label, { size, fill: color, weight, family }), { op });
}
function agentIcon(name, x, y, size) {
  const src = fs.readFileSync(path.join(REPO, "assets/icons", `agent-${name}.svg`), "utf8").replace(/currentColor/g, C.ink);
  const vb = (src.match(/viewBox="([^"]*)"/) || [, "0 0 24 24"])[1].split(/\s+/).map(Number);
  const inner = src.replace(/[\s\S]*?<svg[^>]*>/, "").replace(/<\/svg>[\s\S]*/, "");
  const root = src.match(/<svg([^>]*)>/)[1];
  const fill = (root.match(/\sfill="([^"]*)"/) || [, ""])[1];
  return `<g transform="translate(${x} ${y}) scale(${size / vb[2]}) translate(${-vb[0]} ${-vb[1]})" ${fill ? `fill="${fill}"` : ""}>${inner}</g>`;
}

// Scene caption: eyebrow, serif headline, sans line.
function caption(t, a, b, n, eyebrow, head, sub, { x = 110, y = 128, width = 1700 } = {}) {
  const o1 = fadeIO(t, a + 0.1, b, 0.5), o2 = fadeIO(t, a + 0.25, b, 0.5), o3 = fadeIO(t, a + 0.45, b, 0.5);
  const dy = (o) => (1 - o) * 14;
  return text(x, y - 52 + dy(o1), `${String(n).padStart(2, "0")}  ·  ${eyebrow.toUpperCase()}`, { size: 18, fill: C.acc, family: MONO, weight: 600, ls: 1.5, op: o1 }) +
    text(x, y + 8 + dy(o2), head, { size: 62, family: SERIF, fill: C.ink, op: o2 }) +
    (sub ? text(x, y + 56 + dy(o3), sub, { size: 25, fill: C.dim, op: o3 }) : "");
}

// ---------- timeline ----------
// Each scene's own clock: a scene draws itself for times within its range,
// and its cues (clicks, typing) are written in these times.
const S = {
  intro: [0, 4.2], live: [4.0, 13.4], ask: [13.2, 21.6], inspect: [21.4, 27.0], comps: [26.8, 32.4],
  term: [32.2, 38.6], agents: [38.4, 43.8], sandbox: [43.6, 48.4], apps: [48.2, 55.4], outro: [55.2, 60],
};
// The order the film plays them in. Each starts 0.2s before the previous one
// ends, so they cross-fade; PLACE maps film time back onto a scene's clock.
// The chapter times in website/site/index.html and film/chapters.vtt follow it.
const ORDER = ["intro", "ask", "inspect", "agents", "live", "comps", "term", "sandbox", "apps", "outro"];
const PLACE = {};
{
  let at = 0;
  for (const k of ORDER) {
    const [a, b] = S[k];
    PLACE[k] = { start: at, end: at + (b - a), shift: at - a };
    at += b - a - 0.2;
  }
}
// Scene numbers in the captions, in playing order (the intro and outro have none).
const NUM = Object.fromEntries(ORDER.filter((k) => k !== "intro" && k !== "outro").map((k, i) => [k, i + 1]));

// ---------- 1. intro ----------
function intro(t) {
  const [a, b] = S.intro;
  const out = 1 - eIO(prog(t, b - 0.5, b));
  const rise = eOut(prog(t, 0.0, 1.4));
  let s = sun(960, lerp(1500, 1180, rise), 620, 0.95 * out);
  const k = eBack(prog(t, 0.35, 1.1));
  s += g(`<image href="img/icon.png" x="880" y="250" width="160" height="160"/>`, { op: clamp(k) * out, s: Math.max(0.01, k), cx: 960, cy: 330, rot: (1 - clamp(k)) * -25 });
  s += text(960, 535, "Moonframe", { size: 112, family: SERIF, style: "italic", anchor: "middle", fill: C.ink, op: eOut(prog(t, 0.9, 1.6)) * out });
  const tag = "Malleable software.";
  s += text(960, 640, tag, { size: 58, family: SERIF, anchor: "middle", fill: C.ink2, op: eOut(prog(t, 1.5, 2.2)) * out });
  s += text(960, 700, "Native desktop apps you reshape while they run.", { size: 28, anchor: "middle", fill: C.dim, op: eOut(prog(t, 2.0, 2.6)) * out });
  return s;
}

// ---------- 2. live luau ----------
const MOD_CODE = [
  ["-- app/mods/counter/main.luau", "c"],
  ["return mod {", ""],
  ['  region = "left",', ""],
  ["  init = function(previous)", ""],
  ["    return previous or { count = 0 }", ""],
  ["  end,", ""],
  ["  update = function(state, msg)", ""],
  ['    if msg == "add" then state.count += 1 end', ""],
  ["  end,", ""],
  ["  view = function(state, app)", ""],
  ["    return ui.panel {", ""],
  ['      title = "Counter",', "title"],
  ["      ui.text(`Clicked {state.count} times`),", ""],
  ['      ui.button { "Add", msg = "add" },', ""],
  ["    }", ""],
  ["  end,", ""],
  ["}", ""],
];
const STYLE_CODE = [
  ["-- app/style.luau", "c"],
  ["return style {", ""],
  ["  colors = {", ""],
  ['    accent = "#4f46e5",', "accent"],
  ["  },", ""],
  ["  rules = {", ""],
  ["    panel = { radius = 6, border = true },", "radius"],
  ['    ["item:hover"] = { background = "accent/8" },', ""],
  ["  },", ""],
  ["}", ""],
];
function codeLine(x, y, s, size = 21) {
  // light Luau highlighting
  if (s.trim().startsWith("--")) return text(x, y, s, { size, family: MONO, fill: "#8a7a6a" });
  const parts = s.split(/("[^"]*"|`[^`]*`|\b(?:return|function|end|if|then|or|local)\b|\b\d+\b)/);
  let cx = x, out = "";
  for (const p of parts) {
    if (!p) continue;
    let fill = "#f1e3c8";
    if (/^["`]/.test(p)) fill = "#e9a65a";
    else if (/^(return|function|end|if|then|or|local)$/.test(p)) fill = "#e46b4a";
    else if (/^\d+$/.test(p)) fill = "#7fc2bd";
    out += text(cx, y, p, { size, family: MONO, fill });
    cx += p.length * size * 0.6;
  }
  return out;
}
function live(t) {
  const [a, b] = S.live;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.live, "Under the hood", "It's all Luau, reloaded live.", "The agent's edits land in Luau files that reload in place: no rebuild, no restart, state kept.");
  // ---- the request: after the clicks, the agent is asked for the change
  const PROMPT = "rename it Moon counter, make it orange and rounder";
  const qo = eOut(prog(t, 6.2, 6.45));
  if (qo > 0) {
    let q = rect(110, 262, 800, 52, { rx: 14, fill: C.ink });
    q += text(132, 296, "you", { size: 17, family: MONO, fill: "#8a7a6a" });
    q += text(180, 296, "› " + typed(PROMPT, t, 6.35, 6.85), { size: 20, family: MONO, fill: C.cream });
    s += g(q, { op: qo * o, ty: (1 - qo) * -10 });
  }
  // ---- code card, edited by the agent
  const cx = 110, cy = 330, cw = 800, ch = 620;
  const styleTab = t >= 8.6;
  let c = rect(cx, cy, cw, ch, { rx: 18, fill: "#000", op: 0.3, filter: "shadow" }) + rect(cx, cy, cw, ch, { rx: 18, fill: "#24160e" });
  c += rect(cx, cy, cw, 54, { rx: 18, fill: "#1a0f09" }) + rect(cx, cy + 36, cw, 18, { fill: "#1a0f09" });
  const tabs = [["counter/main.luau", !styleTab], ["style.luau", styleTab]];
  let tx = cx + 20;
  for (const [n, act] of tabs) {
    const w = tw(n, 18, 0.6) + 36;
    c += rect(tx, cy + 10, w, 36, { rx: 9, fill: act ? "#3a2618" : "none" }) + text(tx + 18, cy + 34, n, { size: 18, family: MONO, fill: act ? C.cream : "#8a7a6a" });
    tx += w + 8;
  }
  if (t >= 6.9 && t < 10.3) c += text(cx + cw - 24, cy + 34, "agent editing…", { size: 17, family: MONO, fill: C.sunY, anchor: "end" });
  const lines = styleTab ? STYLE_CODE : MOD_CODE;
  let ly = cy + 90;
  const caretOn = Math.floor(t * 2.5) % 2 === 0;
  for (const [ln0, tag] of lines) {
    let ln = ln0, editing = false;
    if (tag === "title") {
      if (t >= 6.9) { const add = typed("Moon counter", t, 6.9, 7.6); ln = `      title = "${add}",`; editing = t < 8.0; if (t < 7.05) ln = `      title = "",`; }
    }
    if (tag === "accent") {
      if (t >= 9.0) { ln = `    accent = "${typed("#d9541f", t, 9.0, 9.5)}",`; editing = t < 9.65; }
    }
    if (tag === "radius") {
      if (t >= 9.7) { ln = `    panel = { radius = ${t < 9.85 ? "" : "18"}, border = true },`; editing = t < 10.2; }
    }
    if (editing) c += rect(cx + 8, ly - 26, cw - 16, 36, { rx: 6, fill: "#e9a65a", op: 0.12 });
    c += codeLine(cx + 34, ly, ln);
    if (editing && caretOn) {
      const at = ln.lastIndexOf('"') > 0 && tag !== "radius" ? ln.lastIndexOf('"') : ln.indexOf(",") ;
      c += rect(cx + 34 + at * 21 * 0.6, ly - 22, 3, 28, { fill: C.sunY });
    }
    ly += styleTab ? 46 : 31;
  }
  // save chip
  const saves = [8.0, 10.3];
  for (const sv of saves) {
    const so = fadeIO(t, sv, sv + 1.1, 0.2);
    if (so > 0) c += pill(cx + cw - 210, cy + ch - 70, "agent saved", { fill: C.teal, size: 20, h: 44, op: so, family: MONO });
  }
  s += g(c, { op: o, tx: (1 - eOut(prog(t, a, a + 0.7))) * -60 });

  // ---- the window
  const wx = 990, wy = 270, ww = 820, wh = 680;
  const k2 = eIO(prog(t, 10.3, 10.9));
  const accent = mix("#4f46e5", "#d9541f", k2);
  const radius = lerp(6, 18, k2);
  let w = rect(wx, wy, ww, wh, { rx: 16, fill: "#000", op: 0.25, filter: "shadow" }) + rect(wx, wy, ww, wh, { rx: 16, fill: "#f4f4f6" });
  w += rect(wx, wy, ww, 52, { rx: 16, fill: "#ececf0" }) + rect(wx, wy + 36, ww, 16, { fill: "#ececf0" });
  ["#ff5f57", "#febc2e", "#28c840"].forEach((f, i) => { w += `<circle cx="${wx + 26 + i * 22}" cy="${wy + 26}" r="7" fill="${f}"/>`; });
  w += text(wx + ww / 2, wy + 33, "Moonframe", { size: 17, anchor: "middle", fill: "#555", weight: 600 });
  // left region with the counter panel
  const px = wx + 22, py = wy + 76, pw = 330, ph = 250;
  const title = t >= 8.0 ? "Moon counter" : "Counter";
  const flash = (sv) => 1 - eOut(prog(t, sv, sv + 0.7));
  const fl = Math.max(t >= 8 ? flash(8.0) : 0, t >= 10.3 ? flash(10.3) : 0);
  w += rect(px, py, pw, ph, { rx: radius, fill: "#fff", stroke: "#dedee4" });
  if (fl > 0) w += rect(px - 4, py - 4, pw + 8, ph + 8, { rx: radius + 4, stroke: C.sunO, sw: 4, op: fl });
  w += text(px + 22, py + 40, title, { size: 21, weight: 600, fill: accent });
  const clicks = [5.3, 5.8, 6.3].filter((c0) => t >= c0 + 0.08).length;
  w += text(px + 22, py + 100, `Clicked ${clicks} times`, { size: 26, fill: "#222" });
  const bx = px + 22, by = py + 140, bw = 110, bh = 50;
  const pr = Math.max(...[5.3, 5.8, 6.3].map((c0) => (t >= c0 && t < c0 + 0.15 ? 1 : 0)));
  w += rect(bx, by + pr * 2, bw, bh, { rx: Math.min(radius, 14), fill: accent }) + text(bx + bw / 2, by + 33 + pr * 2, "Add", { size: 21, anchor: "middle", fill: "#fff", weight: 600 });
  // filler rows in the main region
  for (let i = 0; i < 6; i++) {
    w += rect(wx + 380, wy + 80 + i * 68, 410, 54, { rx: radius * 0.7, fill: "#fff", stroke: "#e4e4ea" });
    w += rect(wx + 400, wy + 100 + i * 68, 120 + (i * 53) % 170, 12, { rx: 6, fill: i === 1 ? accent : "#d8d8e0", op: i === 1 ? 0.5 : 1 });
  }
  w += rect(wx, wy + wh - 40, ww, 40, { fill: "#ececf0" }) + text(wx + 20, wy + wh - 14, "⎇ main  +3", { size: 16, fill: "#666", family: MONO });
  // reload toast
  for (const sv of saves) {
    const to = fadeIO(t, sv + 0.1, sv + 1.9, 0.3);
    if (to > 0) w += pill(wx + 395, wy + 470, "reloaded · state kept", { dot: C.sunY, size: 22, op: to });
  }
  // cursor clicks on Add
  if (t >= 4.6 && t < 6.9) {
    const k = eIO(prog(t, 4.6, 5.2));
    const cxp = lerp(wx + 560, bx + 70, k), cyp = lerp(wy + 600, by + 32, k);
    w += cursor(cxp, cyp, Math.max(pressAt(t, 5.3), pressAt(t, 5.8), pressAt(t, 6.3)));
  }
  s += g(w, { op: o, tx: (1 - eOut(prog(t, a + 0.15, a + 0.85))) * 60 });
  // annotation: state kept
  const ko = fadeIO(t, 10.9, b, 0.4);
  if (ko > 0) s += g(text(wx + 22, wy + wh + 50, "count = 3 survived both reloads", { size: 22, family: MONO, fill: C.acc }), { op: ko * o });
  return s;
}

// ---------- 3. ask for a change ----------
const MONTHS = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN"];
const VALS = [0.44, 0.66, 0.53, 0.58, 0.96, 0.49];
function ask(t) {
  const [a, b] = S.ask;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.ask, "Send feedback", "Ask the running app to change.", "A coding agent edits it, checks it, screenshots it — and the window reloads in front of you.");
  const x = 110, y = 270, w = 1080, h = 680;
  let r = rect(x, y, w, h, { rx: 16, fill: "#000", op: 0.35, filter: "shadow" }) + rect(x, y, w, h, { rx: 16, fill: "#161616" });
  r += rect(x, y, w, 52, { rx: 16, fill: "#1e1e1e" }) + rect(x, y + 36, w, 16, { fill: "#1e1e1e" });
  ["#ff5f57", "#febc2e", "#28c840"].forEach((f, i) => { r += `<circle cx="${x + 26 + i * 22}" cy="${y + 26}" r="7" fill="${f}"/>`; });
  r += text(x + 110, y + 33, "Leads Report", { size: 17, fill: "#ddd", weight: 600 }) + text(x + 230, y + 33, "13,487 LEADS", { size: 13, fill: "#777", family: MONO });
  // ? button top right
  const qx = x + w - 40, qy = y + 26;
  r += `<circle cx="${qx}" cy="${qy}" r="13" fill="none" stroke="#999" stroke-width="1.6"/>` + text(qx, qy + 6, "?", { size: 17, anchor: "middle", fill: "#ccc", weight: 600 });
  const kp = [["Pipeline Value", "$485,000", "12.5% ↓"], ["Lead-to-Deal Rate", "18.5%", "12.5% ↓"], ["Contacted Leads", "92", "+25% ↑"]];
  kp.forEach(([l, v, d], i) => {
    const kx = x + 24 + i * 345;
    r += rect(kx, y + 72, 330, 120, { rx: 10, fill: "#1d1d1d", stroke: "#2a2a2a" });
    r += text(kx + 20, y + 104, l, { size: 16, fill: "#aaa" }) + text(kx + 20, y + 146, v, { size: 32, family: MONO, fill: "#eee" });
    r += text(kx + 20, y + 176, d, { size: 14, family: MONO, fill: d.includes("+") ? "#3fbf6f" : "#e0564a" });
  });
  r += rect(x + 24, y + 210, w - 48, 446, { rx: 10, fill: "#1d1d1d", stroke: "#2a2a2a" });
  r += text(x + 46, y + 246, "Sales Revenue", { size: 16, fill: "#aaa" }) + text(x + 46, y + 290, "$1,631,241", { size: 34, family: MONO, fill: "#eee" });
  const applied = eOut(prog(t, 19.0, 19.6));
  const chartX = x + 80, chartB = y + 600, bw = 110, gap = 52;
  MONTHS.forEach((m, i) => {
    const bh = VALS[i] * 250;
    const bxx = chartX + i * (bw + gap);
    const best = i === 4;
    const fill = best ? mix("#3a3a3a", "#f97316", applied) : "#333";
    if (best && applied > 0) r += rect(bxx - 6, chartB - bh - 6, bw + 12, bh + 12, { rx: 20, fill: "#f97316", op: 0.25 * applied, filter: "glow" });
    r += rect(bxx, chartB - bh * (best ? 1 + 0.03 * Math.sin(Math.PI * applied) : 1), bw, bh * (best ? 1 + 0.03 * Math.sin(Math.PI * applied) : 1), { rx: 18, fill });
    r += text(bxx + bw / 2, chartB + 32, m, { size: 15, family: MONO, anchor: "middle", fill: best && applied > 0.5 ? "#f6a25e" : "#888" });
  });
  // the window reload flash
  const fl = 1 - eOut(prog(t, 19.0, 19.8));
  if (t >= 19.0 && fl > 0) r += rect(x - 3, y - 3, w + 6, h + 6, { rx: 18, stroke: C.sunO, sw: 5, op: fl });
  // cursor to ? at 13.8
  if (t >= 13.6 && t < 14.4) r += cursor(lerp(x + 800, qx, eIO(prog(t, 13.6, 14.0))), lerp(y + 400, qy, eIO(prog(t, 13.6, 14.0))), pressAt(t, 14.05));
  s += g(r, { op: o, tx: (1 - eOut(prog(t, a, a + 0.7))) * -50 });

  // ---- feedback box
  const fo = eOut(prog(t, 14.1, 14.5)) * o;
  const fx = 1250, fy = 270, fw = 560, fh = 680;
  let f = rect(fx, fy, fw, fh, { rx: 20, fill: "#000", op: 0.25, filter: "shadow" }) + rect(fx, fy, fw, fh, { rx: 20, fill: "#fffaf0", stroke: "#e2d4b8" });
  f += text(fx + 30, fy + 52, "Send feedback", { size: 26, weight: 600, fill: C.ink });
  f += text(fx + 30, fy + 84, "What should change in this app?", { size: 18, fill: C.dim });
  f += rect(fx + 30, fy + 108, fw - 60, 130, { rx: 12, fill: "#fff", stroke: t < 16.3 ? C.accHi : "#e2d4b8", sw: t < 16.3 ? 2 : 1 });
  const msg = typed("Highlight the best month in orange.", t, 14.6, 16.0);
  const ml = wrap(msg, 30);
  ml.forEach((ln, i) => { f += text(fx + 50, fy + 150 + i * 34, ln, { size: 23, fill: C.ink }); });
  if (t < 16.3 && Math.floor(t * 2.5) % 2 === 0) {
    const ln = ml[ml.length - 1];
    f += rect(fx + 52 + tw(ln, 23, 0.5), fy + 130 + (ml.length - 1) * 34, 2.5, 28, { fill: C.ink });
  }
  const sp = t >= 16.3 && t < 16.45 ? 2 : 0;
  f += rect(fx + fw - 160, fy + 256 + sp, 130, 50, { rx: 12, fill: t >= 16.3 ? C.teal : C.acc }) + text(fx + fw - 95, fy + 288 + sp, t >= 16.3 ? "Sent" : "Send", { size: 20, anchor: "middle", fill: "#fff", weight: 600 });
  if (t >= 15.9 && t < 16.6) f += cursor(lerp(fx + 300, fx + fw - 95, eIO(prog(t, 15.9, 16.25))), fy + 282, pressAt(t, 16.3));
  // agent steps
  const steps = [
    [16.6, 17.3, "Reading the report mod"],
    [17.3, 18.0, "Editing the chart's bars"],
    [18.0, 18.5, "just check  ·  just types"],
    [18.5, 19.0, "Screenshot, looking at the result"],
  ];
  f += text(fx + 30, fy + 360, "AGENT", { size: 15, family: MONO, fill: C.acc, ls: 2, weight: 600, op: eOut(prog(t, 16.5, 16.8)) });
  steps.forEach(([s0, s1, lbl], i) => {
    const so = eOut(prog(t, s0, s0 + 0.25));
    if (so <= 0) return;
    const yy = fy + 404 + i * 50;
    const done = t >= s1;
    if (done) f += `<circle cx="${fx + 44}" cy="${yy - 7}" r="11" fill="${C.teal}"/>` + `<path d="M${fx + 38} ${yy - 7} l4 4 l8 -8" stroke="#fff" stroke-width="2.4" fill="none"/>`;
    else f += `<circle cx="${fx + 44}" cy="${yy - 7}" r="10" fill="none" stroke="${C.sunO}" stroke-width="2.6" stroke-dasharray="40 30" transform="rotate(${(t * 400) % 360} ${fx + 44} ${yy - 7})"/>`;
    f += text(fx + 68, yy, lbl, { size: 20, fill: done ? C.ink2 : C.ink, op: so });
  });
  const ap = eBack(prog(t, 19.3, 19.8));
  if (ap > 0) f += g(pill(fx + 30, fy + fh - 80, "applied live · state kept", { fill: C.teal, dot: C.sunY, size: 22, h: 52 }), { op: clamp(ap), s: Math.max(0.01, ap), cx: fx + 160, cy: fy + fh - 54 });
  s += g(f, { op: fo, tx: (1 - fo) * 40 });
  return s;
}

// ---------- 4. inspect ----------
const WIN_W = 1800, WIN_H = 1155;
const REGIONS = [
  [22.5, [12, 68, 318, 1036], "explorer", "app/mods/explorer/main.luau"],
  [23.4, [346, 68, 996, 782], "ui.terminal", "app/mods/terminal/main.luau"],
  [24.3, [1360, 68, 430, 512], "changes", "app/mods/changes/main.luau"],
  [25.2, [0, 1114, 1800, 41], "status bar", "app/mods/system/main.luau"],
];
function inspect(t) {
  const [a, b] = S.inspect;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.inspect, "Inspect Element  ⌥⌘I", "Point at anything.", "The inspector tells your agent which file and line drew it. Then say “add a branch list here”.");
  const sc = 1160 / WIN_W, ix = 110, iy = 260, iw = WIN_W * sc, ih = WIN_H * sc;
  let r = img("window.png", ix, iy, iw, ih, { rx: 14 });
  let cur = null, prev = null;
  for (let i = 0; i < REGIONS.length; i++) if (t >= REGIONS[i][0]) { cur = i; }
  if (cur !== null) {
    const [t0, box, kind, file] = REGIONS[cur];
    const pb = cur > 0 ? REGIONS[cur - 1][1] : box;
    const k = eIO(prog(t, t0, t0 + 0.35));
    const bx = lerp(pb[0], box[0], k) * sc + ix, by = lerp(pb[1], box[1], k) * sc + iy;
    const bw = lerp(pb[2], box[2], k) * sc, bh = lerp(pb[3], box[3], k) * sc;
    r += rect(bx, by, bw, bh, { rx: 6, fill: "#3b82f6", op: 0.16 }) + rect(bx, by, bw, bh, { rx: 6, stroke: "#3b82f6", sw: 3 });
    const label = `${kind}  ·  ${file}`;
    const lw = tw(label, 17, 0.6) + 28;
    const lx = clamp(bx, ix, ix + iw - lw), ly = by + bh + 8 > iy + ih - 40 ? by - 44 : by + 10;
    r += rect(lx + (by + bh + 8 > iy + ih - 40 ? 0 : 10), ly, lw, 36, { rx: 8, fill: "#1e3a8a" }) + text(lx + (by + bh + 8 > iy + ih - 40 ? 0 : 10) + 14, ly + 24, label, { size: 17, family: MONO, fill: "#fff" });
    r += cursor(bx + bw * 0.55, by + Math.min(bh * 0.4, 60) + 10, pressAt(t, t0 + 0.4));
    prev = cur;
  }
  s += g(r, { op: o });
  // side card: the selection handed to the agent
  const co = eOut(prog(t, 23.0, 23.5)) * o;
  const cx = 1330, cy = 300, cw = 480;
  let c = rect(cx, cy, cw, 560, { rx: 20, fill: "#24160e" });
  c += text(cx + 28, cy + 50, "selection.json", { size: 18, family: MONO, fill: "#8a7a6a" });
  const file = cur !== null ? REGIONS[cur][3] : "";
  const kind = cur !== null ? REGIONS[cur][2] : "";
  c += codeLine(cx + 28, cy + 100, "{", 19);
  c += codeLine(cx + 28, cy + 136, `  "element": "${kind}",`, 19);
  c += codeLine(cx + 28, cy + 172, `  "file": "${file.replace("app/mods/", "mods/")}",`, 19);
  c += codeLine(cx + 28, cy + 208, `  "line": ${[34, 9, 71, 84][cur ?? 0]}`, 19);
  c += codeLine(cx + 28, cy + 244, "}", 19);
  c += rect(cx + 28, cy + 300, cw - 56, 1, { fill: "#4a3426" });
  c += text(cx + 28, cy + 350, "You", { size: 17, family: MONO, fill: C.sunY });
  const say = typed("add a list of branches here", t, 25.6, 26.5);
  c += text(cx + 28, cy + 392, say, { size: 24, fill: C.cream, family: SERIF, style: "italic" });
  s += g(c, { op: co, tx: (1 - co) * 40 });
  return s;
}

// ---------- 5. components ----------
const COMPONENTS = fs.readdirSync(path.join(REPO, "app/components")).filter((f) => f.endsWith(".luau")).map((f) => f.slice(0, -5)).sort();
function comps(t) {
  const [a, b] = S.comps;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.comps, "Components", "", "Tabs, tables, charts, a command palette, a markdown editor, dialogs, sheets, toasts…");
  const n = Math.round(COMPONENTS.length * eOut(prog(t, a + 0.3, a + 2.4)));
  s += g(text(110, 136, `${n} components, ready to use.`, { size: 62, family: SERIF, fill: C.ink }), { op: fadeIO(t, a + 0.25, b, 0.5) });
  // tile wall
  const cols = 5, tw0 = 236, th0 = 50, gx = 12, gy = 12, x0 = 110, y0 = 260;
  let wall = "";
  const HL = new Set(["command-palette", "table", "markdown-editor", "line-chart", "chat-message", "kanban-column", "terminal", "week-calendar", "dialog", "toast"]);
  for (let i = 0; i < Math.min(55, Math.ceil(COMPONENTS.length / 2)); i++) {
    const name = COMPONENTS[i * 2 % COMPONENTS.length];
    const r0 = Math.floor(i / cols), c0 = i % cols;
    const at = a + 0.3 + (r0 + c0) * 0.06;
    const k = eBack(prog(t, at, at + 0.4));
    if (k <= 0) continue;
    const hl = HL.has(name) && t > a + 2.0;
    const x = x0 + c0 * (tw0 + gx), y = y0 + r0 * (th0 + gy);
    wall += g(rect(x, y, tw0, th0, { rx: 10, fill: hl ? C.brown : "#fffaf0", stroke: hl ? C.brown : "#e2d4b8" }) +
      text(x + 16, y + 32, name.length > 20 ? name.slice(0, 19) + "…" : name, { size: 17, family: MONO, fill: hl ? C.cream : C.ink2 }),
      { op: clamp(k), s: Math.max(0.01, k), cx: x + tw0 / 2, cy: y + th0 / 2 });
  }
  s += g(wall, { op: o });
  // components window
  const ko = eOut(prog(t, a + 1.9, a + 2.5));
  const wx = 1400, wy = 270, ww = 420, wh = 420 * 1161 / 1600;
  s += g(img("components.png", wx, wy, ww, wh, { rx: 14 }) +
    pill(wx + 20, wy + wh + 30, 'local list = use "list"', { family: MONO, size: 22, fill: C.ink, color: C.sunY }),
    { op: ko * o, tx: (1 - ko) * 60, rot: (1 - ko) * 4, cx: wx + ww / 2, cy: wy + wh / 2 });
  return s;
}

// ---------- 6. terminals, git, providers, themes ----------
function term(t) {
  const [a, b] = S.term;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.term, "Batteries included", "Terminals, git, live data, themes.", "The window that ships: every part of it is a mod you can change.");
  const sc = 1120 / WIN_W, ix = 110, iy = 260, iw = WIN_W * sc, ih = WIN_H * sc;
  const dk = eIO(prog(t, 36.6, 37.2));
  let r = img("window.png", ix, iy, iw, ih, { rx: 14 });
  if (dk > 0) r += g(img("dark.png", ix, iy, iw, ih, { rx: 14, shadow: false }), { op: dk });
  const notes = [
    [33.2, [346, 68, 996, 1030], "Real terminals", "Ghostty's libghostty-vt or alacritty;", "kept alive by herdr-core or tmux."],
    [34.3, [1360, 68, 430, 1030], "Git, built in", "Status, diffs, commit and push", "from a native provider."],
    [35.4, [1050, 1114, 750, 41], "Live system data", "CPU, memory, network and usage", "published by providers."],
    [36.6, [0, 0, 1800, 1155], "Themes", "The window, its controls and the", "terminals all follow the theme."],
  ];
  let side = "";
  notes.forEach(([t0, box, h1, l1, l2], i) => {
    const k = eOut(prog(t, t0, t0 + 0.4));
    if (k <= 0) return;
    const active = (i === notes.length - 1 || t < notes[i + 1][0]);
    const bx = box[0] * sc + ix, by = box[1] * sc + iy, bw = box[2] * sc, bh = box[3] * sc;
    if (active && i < 3) r += rect(bx, by, bw, bh, { rx: 10, stroke: C.sunO, sw: 4, op: k });
    const ny = 300 + i * 150;
    const nx = 1300;
    side += g(
      `<circle cx="${nx + 14}" cy="${ny + 2}" r="14" fill="${active ? C.sunO : C.paper2}" stroke="${C.sunO}" stroke-width="2"/>` +
      text(nx + 14, ny + 9, String(i + 1), { size: 17, anchor: "middle", weight: 700, fill: active ? "#fff" : C.acc }) +
      text(nx + 46, ny + 12, h1, { size: 32, family: SERIF, fill: C.ink }) +
      text(nx + 46, ny + 50, l1, { size: 21, fill: C.dim }) + text(nx + 46, ny + 78, l2, { size: 21, fill: C.dim }),
      { op: k * (active ? 1 : 0.55), tx: (1 - k) * 30 });
  });
  s += g(r + side, { op: o });
  return s;
}

// ---------- 7. agents ----------
const AGENTS = [["claude", "Claude"], ["codex", "Codex"], ["gemini", "Gemini"], ["opencode", "OpenCode"], ["copilot", "Copilot"]];
function agents(t) {
  const [a, b] = S.agents;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.agents, "Agent Client Protocol", "Coding agents, inside any mod.", "Run Claude, Codex, Gemini, OpenCode or Copilot, and show their plan, tool calls and questions.");
  let l = "";
  AGENTS.forEach(([id, name], i) => {
    const k = eBack(prog(t, a + 0.4 + i * 0.12, a + 0.8 + i * 0.12));
    if (k <= 0) return;
    const x = 110, y = 290 + i * 128;
    l += g(rect(x, y, 520, 108, { rx: 18, fill: "#fffaf0", stroke: "#e2d4b8" }) +
      rect(x + 18, y + 18, 72, 72, { rx: 16, fill: "#fff", stroke: "#eadcc0" }) + agentIcon(id, x + 32, y + 32, 44) +
      text(x + 114, y + 64, name, { size: 32, family: SERIF, fill: C.ink }) +
      text(x + 500, y + 64, "acp", { size: 18, family: MONO, fill: C.teal, anchor: "end" }),
      { op: clamp(k), s: Math.max(0.01, k), cx: x + 260, cy: y + 54 });
  });
  s += g(l, { op: o });
  // agent-run card
  const cx = 720, cy = 290, cw = 1090, ch = 620;
  const ko = eOut(prog(t, a + 0.6, a + 1.1));
  let c = rect(cx, cy, cw, ch, { rx: 20, fill: "#000", op: 0.3, filter: "shadow" }) + rect(cx, cy, cw, ch, { rx: 20, fill: "#1b1410" });
  c += text(cx + 32, cy + 52, "agent-run", { size: 18, family: MONO, fill: "#8a7a6a" }) + text(cx + 160, cy + 52, "Claude  ·  working", { size: 18, fill: C.sunY });
  c += text(cx + 32, cy + 108, "PLAN", { size: 15, family: MONO, ls: 2, fill: C.sunO, weight: 600 });
  const plan = [["Find where the status bar is drawn", 39.8], ["Add a branch widget from app.git", 40.6], ["Check it and take a screenshot", 42.0]];
  plan.forEach(([p, done], i) => {
    const y = cy + 150 + i * 42;
    const d = t >= done;
    c += rect(cx + 32, y - 18, 22, 22, { rx: 6, fill: d ? C.teal : "none", stroke: d ? C.teal : "#6e5a4a", sw: 2 });
    if (d) c += `<path d="M${cx + 37} ${y - 7} l4 4 l8 -8" stroke="#fff" stroke-width="2.4" fill="none"/>`;
    c += text(cx + 70, y, p, { size: 22, fill: d ? "#a89a88" : C.cream });
  });
  // tool call
  const to = eOut(prog(t, 40.0, 40.3));
  c += g(rect(cx + 32, cy + 290, cw - 64, 64, { rx: 12, fill: "#2a1d14", stroke: "#4a3426" }) +
    text(cx + 56, cy + 330, "✎  Edit", { size: 21, fill: C.sunY, family: MONO }) +
    text(cx + 180, cy + 330, "app/mods/system/main.luau", { size: 21, family: MONO, fill: C.cream }) +
    text(cx + cw - 56, cy + 330, "+12 −3", { size: 21, family: MONO, fill: "#7fc28a", anchor: "end" }), { op: to });
  // permission
  const po = eOut(prog(t, 40.7, 41.0));
  const allowed = t >= 41.45;
  c += g(rect(cx + 32, cy + 372, cw - 64, 84, { rx: 12, fill: "#2a1d14", stroke: allowed ? C.teal : C.sunO, sw: 2 }) +
    text(cx + 56, cy + 422, "Allow running “just check”?", { size: 22, fill: C.cream }) +
    rect(cx + cw - 250, cy + 392, 90, 44, { rx: 10, fill: "#3a2a1e" }) + text(cx + cw - 205, cy + 421, "Deny", { size: 19, anchor: "middle", fill: "#c8b8a0" }) +
    rect(cx + cw - 148, cy + 392, 96, 44, { rx: 10, fill: allowed ? C.teal : C.acc }) + text(cx + cw - 100, cy + 421, allowed ? "Allowed" : "Allow", { size: 19, anchor: "middle", fill: "#fff", weight: 600 }), { op: po });
  if (t >= 41.0 && t < 41.8) c += cursor(lerp(cx + 700, cx + cw - 100, eIO(prog(t, 41.0, 41.35))), lerp(cy + 560, cy + 414, eIO(prog(t, 41.0, 41.35))), pressAt(t, 41.4));
  // answer streaming
  const ans = typed("Done. The status bar now shows the current branch and how far it is ahead.", t, 42.1, 43.3);
  wrap(ans, 62).forEach((ln, i) => { c += text(cx + 32, cy + 520 + i * 38, ln, { size: 26, family: SERIF, style: "italic", fill: C.cream }); });
  s += g(c, { op: ko * o, tx: (1 - ko) * 50 });
  return s;
}

// ---------- 8. sandbox ----------
function sandbox(t) {
  const [a, b] = S.sandbox;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.sandbox, "What runs with what access", "Each kind of code has its limits.", "Mods and running Wasm providers reach only what they're given. Building from source runs Cargo as you.");
  const layers = [
    ["Mods", "Luau", "In the host's sandbox: no files, network or processes.", "Memory cap and instruction budget.", C.teal],
    ["Wasm providers", "wasmtime", "Run with 64 MiB, an instruction budget, and only the capabilities", "in provider.toml. Built from source with Cargo as you; bundled apps ship them prebuilt.", C.sunO],
    ["Process & native providers", "your user", "For what the sandbox can't reach: git, forges, agents.", "Source only, so you can read what runs.", C.sunR],
  ];
  // Each title's width in Iowan Old Style at 40px, measured from a frame:
  // a character count misjudges this face by a third either way.
  const titleW = [100, 290, 468];
  layers.forEach(([h, tag, l1, l2, col], i) => {
    const k = eOut(prog(t, a + 0.5 + i * 0.35, a + 1.0 + i * 0.35));
    if (k <= 0) return;
    const x = 110 + i * 40, y = 290 + i * 200, w = 1700 - i * 80;
    s += g(rect(x, y, w, 172, { rx: 22, fill: "#fffaf0", stroke: "#e2d4b8" }) + rect(x, y, 14, 172, { rx: 7, fill: col }) +
      text(x + 52, y + 66, h, { size: 40, family: SERIF, fill: C.ink }) +
      pill(x + 52 + titleW[i] + 18, y + 30, tag, { fill: col, size: 19, h: 40, family: MONO, padX: 16 }) +
      text(x + 52, y + 112, l1, { size: 24, fill: C.ink2 }) + text(x + 52, y + 146, l2, { size: 24, fill: C.dim }) +
      lock(x + w - 90, y + 50, col, i < 2),
      { op: k * o, tx: (1 - k) * -50 });
  });
  return s;
}
function lock(x, y, col, closed) {
  return `<g transform="translate(${x} ${y})"><rect x="0" y="30" width="56" height="44" rx="8" fill="${col}"/>
    <path d="M10 30 V18 a18 18 0 0 1 36 0 ${closed ? "V30" : "V12"}" fill="none" stroke="${col}" stroke-width="7" stroke-linecap="round" ${closed ? "" : 'transform="translate(14 -6)"'}/>
    <circle cx="28" cy="50" r="6" fill="#fffaf0"/></g>`;
}

// ---------- 9. apps ----------
const APPS = [
  ["emberline", "Emberline"], ["gantry", "Gantry"], ["lantern", "Lantern"], ["parlance", "Parlance"], ["cadence", "Cadence"], ["tidings", "Tidings"], ["stage", "Stage"], ["trellis", "Trellis"],
  ["hearth", "Hearth"], ["vigil", "Vigil"], ["cinder", "Cinder"], ["tessel", "Tessel"], ["ledger", "Ledger"], ["helpdesk", "Tandem"], ["msgvault", "msgvault"], ["daybook", "Daybook"],
  ["research", "Sorrel"], ["duly", "Duly"], ["pipeline", "Tally"], ["audience", "Inkwell"], ["mailbox", "Mailbox"], ["storefront", "Storefront"], ["workbench", "Workbench"], ["sessions", "Sessions"],
];
function apps(t) {
  const [a, b] = S.apps;
  const o = fadeIO(t, a, b, 0.5);
  let s = caption(t, a, b, NUM.apps, "Whole apps", "25 example apps. Build your own.", "Each runs as a profile of its own beside the starter, and ships as a standalone release.");
  const tw1 = 340, th1 = 222, gap = 24;
  let wall = "";
  for (let row = 0; row < 3; row++) {
    const items = APPS.slice(row * 8, row * 8 + 8);
    const dir = row % 2 === 0 ? -1 : 1;
    const speed = 95;
    const span = items.length * (tw1 + gap);
    let off = ((t - a) * speed * dir + row * 170) % span;
    if (off > 0) off -= span;
    const y = 262 + row * (th1 + 52);
    const k = eOut(prog(t, a + 0.3 + row * 0.15, a + 0.9 + row * 0.15));
    for (let rep = 0; rep < 2; rep++) items.forEach(([id, name], i) => {
      const x = 60 + off + rep * span + i * (tw1 + gap);
      if (x > W || x + tw1 < 0) return;
      wall += img(`app-${id}.png`, x, y, tw1, th1, { rx: 12 }) + text(x + 4, y + th1 + 30, name, { size: 21, family: SERIF, style: "italic", fill: C.ink2 });
    });
    s += g(wall, { op: k * o, ty: (1 - k) * 40 });
    wall = "";
  }
  const co = eBack(prog(t, a + 3.4, a + 3.9));
  if (co > 0) s += g(pill(1180, 150, "moonframe app new my-app", { family: MONO, size: 24, h: 56, fill: C.ink, color: C.sunY }), { op: clamp(co) * o, s: Math.max(0.01, co), cx: 1400, cy: 178 });
  return s;
}

// ---------- 10. outro ----------
function outro(t) {
  const [a] = S.outro;
  const k = eOut(prog(t, a, a + 1.4));
  let s = sun(960, lerp(1600, 1130, k), 640, 0.95);
  const ik = eBack(prog(t, a + 0.3, a + 0.9));
  s += g(`<image href="img/icon.png" x="895" y="170" width="130" height="130"/>`, { op: clamp(ik), s: Math.max(0.01, ik), cx: 960, cy: 235 });
  s += text(960, 400, "Moonframe", { size: 104, family: SERIF, style: "italic", anchor: "middle", op: eOut(prog(t, a + 0.6, a + 1.2)) });
  s += text(960, 480, "Malleable software.", { size: 50, family: SERIF, anchor: "middle", fill: C.ink2, op: eOut(prog(t, a + 0.9, a + 1.5)) });
  const p = eOut(prog(t, a + 1.4, a + 2.0));
  const cmd = "brew install moonframehq/tap/moonframe";
  const cw = tw(cmd, 26, 0.6) + 60;
  s += g(rect(960 - cw / 2, 560, cw, 64, { rx: 14, fill: C.ink }) + text(960, 601, cmd, { size: 26, family: MONO, fill: C.sunY, anchor: "middle" }), { op: p, ty: (1 - p) * 20 });
  const q = eOut(prog(t, a + 1.8, a + 2.4));
  s += text(960, 690, "moonframe.dev   ·   MIT   ·   macOS · Linux · Windows", { size: 26, anchor: "middle", fill: C.ink2, op: q, weight: 500 });
  const fade = eIO(prog(t, 59.3, 60));
  if (fade > 0) s += rect(0, 0, W, H, { fill: C.paper, op: fade });
  return s;
}

// ---------- frame ----------
function frame(n) {
  const t = n / FPS;
  uid = 0;
  let body = "";
  const draw = { intro, live, ask, inspect, comps, term, agents, sandbox, apps, outro };
  for (const k of ORDER) {
    const p = PLACE[k];
    if (t >= p.start && t < p.end) body += draw[k](t - p.shift);
  }
  // progress hairline
  const bar = rect(0, H - 6, W * (t / DUR), 6, { fill: C.sunO, op: t < 4 || t > 55.2 ? 0 : 0.8 });
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="150%"><feGaussianBlur stdDeviation="18"/><feOffset dy="16"/></filter>
    <filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="14"/></filter>
    <radialGradient id="bg" cx="50%" cy="40%" r="75%"><stop offset="0" stop-color="${C.paper}"/><stop offset="1" stop-color="${C.paper2}"/></radialGradient>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#bg)"/>
  ${body}${bar}</svg>`;
}

const total = Math.round(DUR * FPS);
const list = ONLY.length ? ONLY : [...Array(total).keys()];
for (const n of list) fs.writeFileSync(path.join(OUT, `f${String(n).padStart(4, "0")}.svg`), frame(n));
console.log(`wrote ${list.length} frames`);
