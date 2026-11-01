// A browsable copy of Moonframe's Components window. The list, descriptions,
// props and example code come from app/components/*.luau (data/components.js).
// Previews are drawn in HTML for the common components and are real
// screenshots of the app where one exists; the rest point at the app.
(function () {
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const md = s => esc(s).replace(/`([^`]+)`/g, "<code>$1</code>");

  const SHOTS = {
    "week-calendar": "img/c-week-calendar.webp",
    "table": "img/c-table.webp",
    "filter-dropdown": "img/c-filter-dropdown.webp",
    "notification-drawer": "img/c-notification-drawer.webp",
    "pagination": "img/c-pagination.webp",
    "side-nav": "img/c-side-nav-gallery.webp",
    "command-palette": "img/c-palette.webp",
  };

  const dot = c => `<i class="cb-dot" style="background:${c}"></i>`;
  const PREVIEWS = {
    "kpi-cell": () => `<div class="cb-card" style="width:260px"><div class="cb-row cb-muted"><span class="cb-ic" style="background:#7c5cff22;color:#7c5cff">●</span>Total readers<span style="margin-left:auto">⋯</span></div><div class="cb-big">24,816 <span class="cb-good">+6.4%</span></div><div class="cb-muted">All active subscribers</div></div>`,
    "stat-tile": () => `<div class="cb-card" style="width:240px"><div class="cb-muted">⚡ Tokens today</div><div class="cb-big">1.84M <span class="cb-good">+12%</span></div><div class="cb-muted">vs. yesterday · 3 agents</div></div>`,
    "metric": () => `<div><div class="cb-muted">Lowest week</div><div class="cb-big">$26.4M</div><div class="cb-muted">Week of Jan 4</div></div>`,
    "chips": () => `<div class="cb-row">${["Open", "Agent", "Draft", "Mine"].map((c, i) => `<span class="cb-chip ${i < 2 ? "on" : ""}">${i < 2 ? "✓ " : ""}${c}</span>`).join("")}</div>`,
    "segmented": () => `<div class="cb-seg"><span>☀ Light</span><span class="on">☾ Dark</span><span>⚙ System</span></div>`,
    "button-group": () => `<div class="cb-seg sq"><span class="on">Unified</span><span>Split</span><span>Raw</span></div>`,
    "avatar-stack": () => `<div class="cb-row">${["CB", "IB", "RO", "MC"].map((n, i) => `<span class="cb-av" style="background:${["#7c5cff", "#e0679a", "#3aa57a", "#e59a35"][i]};margin-left:${i ? -8 : 0}px">${n}</span>`).join("")}<span class="cb-muted" style="margin-left:8px">+38</span></div>`,
    "avatar": () => `<span class="cb-av" style="width:40px;height:40px;background:#7c5cff">CB</span>`,
    "progress": () => `<div style="width:280px"><div class="cb-row"><b>Indexing workspace</b><span class="cb-muted" style="margin-left:auto">64%</span></div><div class="cb-track"><i style="width:64%"></i></div><div class="cb-muted">1,284 of 2,006 files</div></div>`,
    "meter": () => `<div class="cb-row"><span class="cb-muted">Memory</span><div class="cb-track" style="width:48px;margin:0"><i style="width:62%"></i></div>62%</div>`,
    "usage-bar": () => `<div style="width:320px"><div class="cb-row"><b>Weekly</b><span class="cb-muted" style="margin-left:auto">Resets in 4d 6h</span></div><div class="cb-track" style="position:relative"><i style="width:97%;background:#3aa57a"></i><span style="position:absolute;left:61%;top:-3px;bottom:-3px;width:2px;background:#1d1b26"></span></div><div class="cb-muted">36% in reserve · Lasts until reset</div></div>`,
    "sparkline": () => `<div class="cb-spark">${[.1, .3, .2, .5, .4, .9, .6, .3, .2, .4, .7, .5].map(v => `<i style="height:${v * 28 + 2}px"></i>`).join("")}</div>`,
    "bar-chart": () => { const B = [[42, 18, 12], [55, 22, 20], [31, 12, 9], [68, 30, 26], [74, 35, 31], [12, 4, 3], [8, 2, 2]], L = "Mon Tue Wed Thu Fri Sat Sun".split(" "), C = ["#6d5cff", "#e0679a", "#c9c6d6"];
      return `<div><div class="cb-bars">${B.map((b, i) => `<div><span class="cb-stack">${b.map((v, j) => `<i style="height:${v * .9}px;background:${C[j]}"></i>`).reverse().join("")}</span><small>${L[i]}</small></div>`).join("")}</div><div class="cb-row cb-muted" style="margin-top:8px">${["Input", "Output", "Cache"].map((n, j) => dot(C[j]) + n).join("&nbsp;&nbsp;")}</div></div>`; },
    "date-tile": () => `<div class="cb-date"><small>NOV</small><b>9</b></div>`,
    "banner": () => `<div class="cb-banner">⎇ <span style="font-family:var(--mono)">feat/gitlab-merge-requests-887</span><span style="opacity:.75">worktree of ledger-api</span><span class="cb-btn" style="margin-left:auto">Open review</span></div>`,
    "empty-state": () => `<div style="text-align:center;width:300px"><div style="font-size:30px;opacity:.4">📥</div><b>No worktrees</b><div class="cb-muted">Create one to work on two branches at once.</div><span class="cb-btn p" style="margin-top:12px;display:inline-block">New worktree</span></div>`,
    "kbd": () => `<div class="cb-row"><kbd class="cb-kbd">⌘</kbd><kbd class="cb-kbd">⇧</kbd><kbd class="cb-kbd">P</kbd></div>`,
    "checks": () => `<div style="width:320px">${[["ci / test (ubuntu)", "p", "4m 12s"], ["ci / test (macos)", "p", "6m 40s"], ["ci / clippy", "f", "1 warning denied"], ["ci / build release", "w", "running 2m"], ["Review", "p", "Approved by alice"]].map(([n, s, d]) => `<div class="cb-row cb-line"><span class="cb-st ${s}">${s === "p" ? "✓" : s === "f" ? "✕" : "◌"}</span>${n}<span class="cb-muted" style="margin-left:auto">${d}</span></div>`).join("")}</div>`,
    "tabs": () => `<div class="cb-tabs">${[["ledger-api", "#e59a35", ""], ["harbor-docs", "#d64545", "2"], ["pixel-forge", "#3aa57a", ""], ["notes.md", "#aaa", ""]].map(([l, c, b], i) => `<span class="${i ? "" : "on"}">${dot(c)}${l}${b ? `<em>${b}</em>` : ""}</span>`).join("")}<span>+</span></div>`,
    "pill-tabs": () => `<div class="cb-seg"><span class="on">All</span><span>Inbox</span><span>Sent</span></div>`,
    "switch": () => `<div class="cb-row" style="width:330px;align-items:flex-start"><div><b>Follow system theme</b><div class="cb-muted">Switch between light and dark with macOS.</div></div><span class="cb-sw on" style="margin-left:auto"><i></i></span></div>`,
    "checkbox": () => `<div class="cb-row" style="width:330px;align-items:flex-start"><span class="cb-cbx">✓</span><div><b>Restore tabs on launch</b><div class="cb-muted">Reopen the terminals and pages from last time.</div></div></div>`,
    "radio-group": () => `<div style="width:340px">${[["New worktree", "Each agent gets its own checkout and branch.", 1], ["Current folder", "Agents share the working tree you are in.", 0], ["Cloud machine", "Run on a remote box; needs a connected device.", -1]].map(([l, d, s]) => `<div class="cb-row cb-line" style="align-items:flex-start;opacity:${s < 0 ? .45 : 1}"><span class="cb-radio ${s > 0 ? "on" : ""}"></span><div><b>${l}</b><div class="cb-muted">${d}</div></div></div>`).join("")}</div>`,
    "pill": () => `<span class="cb-chip" style="background:#d6454518;color:#c03c3c;border-color:#d6454533">📅 2 events</span>`,
    "tags": () => `<div class="cb-row">${["rust", "gpui", "luau", "wasm"].map(t => `<span class="cb-tag">${t}</span>`).join("")}</div>`,
    "status-dot": () => `<span class="cb-chip">${dot("#d64545")}Blocked</span>`,
    "count-bubble": () => `<span class="cb-bubble">128</span>`,
    "diff-counts": () => `<div class="cb-row" style="font-family:var(--mono)"><span class="cb-muted" style="font-family:var(--sans)">3 files</span><span class="cb-good">+38</span><span style="color:#d64545">−16</span></div>`,
    "ahead-behind": () => `<div class="cb-row" style="font-family:var(--mono)"><span>↑2</span><span class="cb-muted">↓1</span></div>`,
    "pr-badge": () => `<span class="cb-chip" style="background:#3aa57a18;color:#2d8a63;border-color:#3aa57a40">✓ #850 Ready</span>`,
    "relative-time": () => `<span class="cb-muted">updated 3 days ago</span>`,
    "key-values": () => `<div style="width:260px">${[["Branch", "main"], ["Ahead", "2 commits"], ["Changed", "14 files"]].map(([k, v]) => `<div class="cb-row cb-line"><span class="cb-muted">${k}</span><span style="margin-left:auto">${v}</span></div>`).join("")}</div>`,
    "stat": () => `<div class="cb-row" style="font-family:var(--mono);font-size:12px"><span class="cb-muted">CPU</span>12%</div>`,
    "toast": () => `<div class="cb-card" style="width:340px;border-left:3px solid #e59a35"><div class="cb-row"><b>Local Git and PR details unavailable</b><span class="cb-muted" style="margin-left:auto">✕</span></div><div class="cb-muted">The daemon socket directory must be owned by your user and not world-writable.</div><span class="cb-btn" style="margin-top:10px;display:inline-block">Reconnect</span></div>`,
    "breadcrumbs": () => `<div class="cb-row"><span class="cb-muted">‹</span><span>group/sub/app</span><span class="cb-muted">/</span><span>Merge requests</span><span class="cb-muted">/</span><b>!2 Draft: cache MR pages</b><span class="cb-muted" style="margin-left:14px">2 of 3</span></div>`,
    "search-field": () => `<div class="cb-input" style="width:260px">⌕ <span class="cb-muted">Search…</span></div>`,
    "spotlight-bars": null,
    // Drawn from the component's own example in app/components/agent-run.luau.
    "agent-run": () => `<div class="cb-run">
      <b>Make the status bar taller</b>
      <div class="cb-row"><span class="cb-ic" style="background:#d9775722;color:#c4623f">✳</span><span style="color:#c77d12">⚠</span><span style="color:#c77d12;font-weight:500">Needs your answer</span><span class="cb-muted">Claude · 3 steps</span><span class="cb-btn" style="margin-left:auto;border-color:transparent">◼ Stop</span></div>
      <div class="cb-plan"><div><span>✓</span><s>Find the status bar's row</s></div><div><span>◉</span>Make it 36px tall</div><div><span>○</span>Run just check</div></div>
      <div class="cb-tool"><span class="cb-good">✓</span>Read app/main.luau</div><div class="cb-tool"><span class="cb-muted">◌</span>Edit app/main.luau</div>
      <div class="cb-ask"><div class="cb-row"><span style="color:#c77d12">⚠</span><b>Claude asks to: Edit app/main.luau</b></div><div class="cb-row"><span class="cb-btn p">Allow</span><span class="cb-btn">Always allow</span><span class="cb-btn" style="border-color:transparent">Reject</span></div></div>
      <div class="cb-muted">The status bar is the row at the bottom of app/main.luau; I'll raise its height.</div></div>`,
    "terminal": () => term("alacritty_terminal"),
    "ghostty-terminal": () => term("libghostty-vt"),
  };
  // A shell mid-session; the engine named in the corner is the only difference.
  function term(engine) {
    const g = s => `<span style="color:#7ec699">${s}</span>`, d = s => `<span style="color:#7d7a8c">${s}</span>`, a = s => `<span style="color:#f2b33d">${s}</span>`;
    return `<div class="cb-term"><div class="cb-term-bar"><span>zsh — ~/code/moonframe</span><span>${engine}</span></div><pre>${a("~/code/moonframe")} ${d("main")} ❯ cargo build --release
   ${g("Compiling")} moonframe-luau v0.4.0
   ${g("Compiling")} moonframe-app v0.4.0
    ${g("Finished")} \`release\` profile [optimized] in 41.2s
${a("~/code/moonframe")} ${d("main")} ❯ moonframe app link examples/ledger --yes
Linked ${g("ledger")} as the profile ledger
${a("~/code/moonframe")} ${d("main")} ❯ <i></i></pre></div>`;
  }

  const STYLE = `
  .cb { --mono:"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace; font:13px/1.45 Inter, system-ui, -apple-system, sans-serif; color:#1f1e26; background:#fff; border:1px solid #1d1b26; box-shadow:0 30px 60px -30px #1d1b2666; display:grid; grid-template-rows:34px 1fr; height:640px; text-align:left }
  .cb-title { background:#2a2933; color:#ddd; display:flex; align-items:center; gap:8px; padding:0 12px; font-weight:600; font-size:12.5px }
  .cb-title i { width:11px; height:11px; border-radius:50%; background:#55545e } .cb-title span { margin-left:8px }
  .cb-main { display:grid; grid-template-columns:240px minmax(0, 1fr); min-height:0 }
  .cb-side { border-right:1px solid #ececf1; display:flex; flex-direction:column; min-height:0; background:#fbfbfc }
  .cb-search { margin:12px; border:1px solid #e3e2ea; border-radius:8px; padding:7px 10px; font:inherit; outline:none; background:#fff }
  .cb-list { overflow:auto; padding:0 8px 12px; flex:1 }
  .cb-cat { font-size:11.5px; color:#8a8798; margin:12px 8px 4px; font-weight:500 }
  .cb-item { display:flex; align-items:center; gap:8px; padding:5px 8px; border-radius:6px; cursor:pointer; color:#2a2833 }
  .cb-item:hover { background:#4b3fd10d } .cb-item.on { background:#4b3fd11a; color:#4b3fd1; font-weight:600 }
  .cb-item .cb-has { margin-left:auto; width:6px; height:6px; border-radius:50%; background:#4b3fd1; opacity:.5 }
  .cb-detail { overflow:auto; padding:22px 26px 30px; min-height:0 }
  .cb-detail h3 { font:600 19px Inter, system-ui, sans-serif; margin:0 0 4px; letter-spacing:-.01em }
  .cb-detail .cb-desc { color:#555; margin-bottom:6px; max-width:640px }
  .cb-detail .cb-when { color:#6b6676; font-size:12.5px; max-width:640px }
  .cb-h { font-size:11.5px; color:#8a8798; font-weight:600; margin:18px 0 6px; text-transform:uppercase; letter-spacing:.08em }
  .cb-prev { border:1px dashed #dddbe5; border-radius:10px; min-height:150px; display:grid; grid-template-columns:minmax(0, 1fr); place-items:center; padding:24px; background:repeating-conic-gradient(#fafafc 0 25%, #fff 0 50%) 0 0/16px 16px }
  .cb-prev img { width:auto; height:auto; max-width:100%; max-height:380px; object-fit:contain; display:block; filter:drop-shadow(0 6px 14px #0002) }
  .cb-none { text-align:center; color:#8a8798; max-width:360px }
  .cb-none b { color:#2a2833; display:block; margin-bottom:4px }
  .cb-code { position:relative }
  .cb-code pre { background:#1b1a24; color:#d8d6f0; font:12px/1.65 var(--mono); padding:14px 16px; border-radius:8px; overflow:auto; max-height:260px; margin:0; white-space:pre }
  .cb-code .k { color:#c792ea } .cb-code .s { color:#addb67 } .cb-code .n { color:#f78c6c } .cb-code .c { color:#6d6b8f }
  .cb-copy { position:absolute; top:8px; right:8px; font:600 11.5px Inter, sans-serif; background:#ffffff1a; color:#fff; border:0; border-radius:6px; padding:5px 9px; cursor:pointer }
  @media (max-width:720px) {
    .cb { height:auto } .cb-main { grid-template-columns:1fr }
    .cb-side { border-right:0; border-bottom:1px solid #ececf1; max-height:240px }
    .cb-detail { padding:18px 16px 24px } .cb-banner, .cb-run, .cb-term { width:auto; flex-wrap:wrap }
  }
  .cb-props { width:100%; border-collapse:collapse; font-size:12.5px }
  .cb-props td { border-top:1px solid #ececf1; padding:6px 10px 6px 0; vertical-align:top } .cb-props td:first-child { font-family:var(--mono); color:#4b3fd1; white-space:nowrap; width:1% }
  .cb code { font:.92em var(--mono); background:#f1f0f5; padding:0 4px; border-radius:3px }
  /* preview atoms */
  .cb-row { display:flex; align-items:center; gap:8px }
  .cb-muted { color:#8a8798; font-size:12.5px }
  .cb-card { background:#fff; border:1px solid #e3e2ea; border-radius:12px; padding:14px 16px; box-shadow:0 2px 8px #0000000a }
  .cb-big { font:700 26px Inter, sans-serif; letter-spacing:-.02em; margin:4px 0 } .cb-good { color:#2d8a63; font-size:13px; font-weight:600 }
  .cb-ic { width:22px; height:22px; border-radius:6px; display:grid; place-items:center; font-size:10px }
  .cb-chip { display:inline-flex; align-items:center; gap:6px; border:1px solid #e3e2ea; border-radius:999px; padding:4px 11px; background:#fff; font-weight:500 }
  .cb-chip.on { background:#4b3fd114; border-color:#4b3fd155; color:#4b3fd1 }
  .cb-dot { width:8px; height:8px; border-radius:50%; display:inline-block; flex-shrink:0 }
  .cb-seg { display:inline-flex; background:#f0eff4; border-radius:9px; padding:3px } .cb-seg span { padding:5px 14px; border-radius:7px; color:#555 } .cb-seg span.on { background:#fff; color:#1f1e26; font-weight:600; box-shadow:0 1px 3px #0002 }
  .cb-seg.sq { border-radius:6px } .cb-seg.sq span { border-radius:4px }
  .cb-av { width:30px; height:30px; border-radius:50%; color:#fff; display:inline-grid; place-items:center; font-size:11px; font-weight:700; border:2px solid #fff }
  .cb-track { height:6px; border-radius:3px; background:#ececf1; overflow:hidden; margin:8px 0 6px } .cb-track i { display:block; height:100%; background:#4b3fd1; border-radius:3px }
  .cb-spark { display:flex; align-items:flex-end; gap:2px; height:32px } .cb-spark i { width:5px; background:#4b3fd1; border-radius:2px }
  .cb-bars { display:flex; gap:14px; align-items:flex-end; height:150px; border-bottom:1px solid #e3e2ea; padding-bottom:4px } .cb-bars > div { display:flex; flex-direction:column; align-items:center; gap:4px }
  .cb-stack { display:flex; flex-direction:column; width:22px } .cb-stack i { display:block } .cb-stack i:first-child { border-radius:4px 4px 0 0 } .cb-bars small { color:#8a8798 }
  .cb-date { width:56px; border-radius:10px; overflow:hidden; text-align:center; border:1px solid #e3e2ea; background:#fff } .cb-date small { display:block; background:#d64545; color:#fff; font-weight:700; font-size:10.5px; padding:2px } .cb-date b { font-size:24px; display:block; padding:4px 0 }
  .cb-banner { display:flex; align-items:center; gap:12px; background:#e59a35; color:#2b1d05; border-radius:10px; padding:10px 14px; width:520px }
  .cb-btn { border:1px solid #d9d7e2; background:#fff; border-radius:7px; padding:5px 12px; font-weight:600; font-size:12.5px } .cb-btn.p { background:#4b3fd1; border-color:#4b3fd1; color:#fff }
  .cb-kbd { font:12px var(--mono); border:1px solid #d9d7e2; border-bottom-width:2px; border-radius:5px; padding:2px 7px; background:#fff }
  .cb-line { padding:7px 0; border-bottom:1px solid #f0eff4 }
  .cb-st { width:18px; height:18px; border-radius:50%; display:grid; place-items:center; font-size:10px; color:#fff } .cb-st.p { background:#3aa57a } .cb-st.f { background:#d64545 } .cb-st.w { background:#e59a35 }
  .cb-tabs { display:flex; gap:4px; background:#f0eff4; padding:4px; border-radius:9px } .cb-tabs span { display:flex; align-items:center; gap:6px; padding:5px 12px; border-radius:7px; color:#555 } .cb-tabs span.on { background:#fff; color:#1f1e26; font-weight:600; box-shadow:0 1px 3px #0002 } .cb-tabs em { font-style:normal; background:#d64545; color:#fff; border-radius:999px; font-size:10px; padding:0 5px }
  .cb-sw { width:34px; height:20px; border-radius:999px; background:#ccc; position:relative; flex-shrink:0 } .cb-sw.on { background:#4b3fd1 } .cb-sw i { position:absolute; top:2px; left:2px; width:16px; height:16px; border-radius:50%; background:#fff } .cb-sw.on i { left:16px }
  .cb-cbx { width:18px; height:18px; border-radius:5px; background:#4b3fd1; color:#fff; display:grid; place-items:center; font-size:11px; flex-shrink:0; margin-top:2px }
  .cb-radio { width:16px; height:16px; border-radius:50%; border:1.5px solid #bbb; flex-shrink:0; margin-top:3px } .cb-radio.on { border:5px solid #4b3fd1 }
  .cb-tag { font:12px var(--mono); background:#f0eff4; padding:2px 8px; border-radius:5px }
  .cb-bubble { background:#d64545; color:#fff; font-weight:700; font-size:11.5px; border-radius:999px; padding:2px 8px }
  .cb-run { width:460px; max-width:100%; display:flex; flex-direction:column; gap:8px; background:#fff; border:1px solid #e3e2ea; border-radius:12px; padding:14px 16px; box-shadow:0 2px 8px #0000000a }
  .cb-plan { background:#f0eff499; border-radius:6px; padding:6px 8px; font-size:12.5px; display:grid; gap:2px } .cb-plan span { color:#8a8798; display:inline-block; width:18px } .cb-plan s { color:#8a8798 }
  .cb-tool { font:12px var(--mono); color:#6b6676; display:flex; gap:6px }
  .cb-run .cb-row { flex-wrap:wrap }
  .cb-ask { border:1px solid #e59a3580; background:#e59a3514; border-radius:8px; padding:10px; display:grid; gap:8px }
  .cb-term { width:540px; max-width:100%; background:#1b1a24; border-radius:10px; overflow:hidden; box-shadow:0 10px 30px -10px #0006 }
  .cb-term-bar { display:flex; justify-content:space-between; padding:7px 12px; font-size:11.5px; color:#8a8798; background:#24232e }
  .cb-term pre { margin:0; padding:12px 14px 14px; font:12px/1.6 var(--mono); color:#d8d6f0; white-space:pre-wrap }
  .cb-term pre i { display:inline-block; width:7px; height:14px; background:#d8d6f0; vertical-align:-2px; animation:cbblink 1s steps(1) infinite }
  @keyframes cbblink { 50% { opacity:0 } }
  .cb-input { border:1px solid #e3e2ea; border-radius:8px; padding:7px 10px; background:#fff }
  `;

  // Light Luau highlighting for the example code.
  function luau(src) {
    return esc(src)
      .replace(/(--[^\n]*)/g, '<span class="c">$1</span>')
      .replace(/(&quot;(?:[^&]|&(?!quot;))*?&quot;)/g, '<span class="s">$1</span>')
      .replace(/\b(local|true|false|nil|function|return|end|if|then|else|loose)\b/g, '<span class="k">$1</span>')
      .replace(/(?<![\w#])(\d+(?:\.\d+)?)\b/g, '<span class="n">$1</span>');
  }

  window.mountComponents = function (root, initial) {
    const all = window.COMPONENTS || [];
    const st = document.createElement("style"); st.textContent = STYLE; document.head.appendChild(st);
    root.innerHTML = `<div class="cb"><div class="cb-title"><i></i><i></i><i></i><span>Components</span></div>
      <div class="cb-main"><div class="cb-side"><input class="cb-search" placeholder="Search ${all.length} components"><div class="cb-list"></div></div><div class="cb-detail"></div></div></div>`;
    const list = root.querySelector(".cb-list"), detail = root.querySelector(".cb-detail"), search = root.querySelector(".cb-search");
    let current = initial || "agent-run";

    function drawList() {
      const q = search.value.trim().toLowerCase();
      const shown = all.filter(c => !q || (c.title + " " + c.name + " " + c.category + " " + c.description).toLowerCase().includes(q));
      const cats = [...new Set(shown.map(c => c.category))].sort();
      list.innerHTML = cats.map(cat => `<div class="cb-cat">${esc(cat)}</div>` + shown.filter(c => c.category === cat)
        .map(c => `<div class="cb-item ${c.name === current ? "on" : ""}" data-n="${c.name}">${esc(c.title)}${SHOTS[c.name] || PREVIEWS[c.name] ? '<span class="cb-has" title="Preview"></span>' : ""}</div>`).join("")).join("")
        || `<div class="cb-muted" style="padding:12px">No component matches.</div>`;
    }
    function drawDetail() {
      const c = all.find(x => x.name === current); if (!c) return;
      // The component's own usage text, or its example with the helpers it needs
      // (website/tools/components-data.py builds it).
      const code = c.code;
      const prev = SHOTS[c.name] ? `<img src="${SHOTS[c.name]}" alt="${esc(c.title)} in the Components window">`
        : PREVIEWS[c.name] ? PREVIEWS[c.name]()
        : `<div class="cb-none"><b>${esc(c.title)}</b>Shown live in the app's Components window. Run <code>just run</code>, then Components › ${esc(c.category)} › ${esc(c.title)}.</div>`;
      detail.innerHTML = `<h3>${esc(c.title)}</h3><div class="cb-desc">${md(c.description)}</div>${c.when ? `<div class="cb-when"><b>When:</b> ${md(c.when)}</div>` : ""}
        <div class="cb-h">Preview</div><div class="cb-prev">${prev}</div>
        <div class="cb-h">Use it</div><div class="cb-code"><pre>${luau(code)}</pre><button class="cb-copy">Copy code</button></div>
        ${c.props.length ? `<div class="cb-h">Props</div><table class="cb-props">${c.props.map(([k, d]) => `<tr><td>${esc(k)}</td><td>${md(d)}</td></tr>`).join("")}</table>` : ""}`;
      detail.scrollTop = 0;
      detail.querySelector(".cb-copy").onclick = e => { navigator.clipboard?.writeText(code); e.target.textContent = "Copied"; setTimeout(() => e.target.textContent = "Copy code", 1200); };
    }
    list.addEventListener("click", e => { const it = e.target.closest(".cb-item"); if (!it) return; current = it.dataset.n; drawList(); drawDetail(); });
    search.addEventListener("input", drawList);
    drawList(); drawDetail();
  };
})();
