// A browsable copy of Moonframe's Components window. The list, descriptions,
// props and example code come from app/components/*.luau (data/components.js).
// Each preview is a screenshot of the component in the app's Components
// window (img/components/<name>.webp, written by tools/component-shots.py).
(function () {
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const md = s => esc(s).replace(/`([^`]+)`/g, "<code>$1</code>");

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
  .cb-detail { overflow:auto; padding:22px 26px 30px; min-height:0 }
  .cb-detail h3 { font:600 19px Inter, system-ui, sans-serif; margin:0 0 4px; letter-spacing:-.01em }
  .cb-detail .cb-desc { color:#555; margin-bottom:6px; max-width:640px }
  .cb-detail .cb-when { color:#6b6676; font-size:12.5px; max-width:640px }
  .cb-h { font-size:11.5px; color:#8a8798; font-weight:600; margin:18px 0 6px; text-transform:uppercase; letter-spacing:.08em }
  .cb-prev { border:1px dashed #dddbe5; border-radius:10px; min-height:150px; display:grid; grid-template-columns:minmax(0, 1fr); place-items:center; padding:24px; background:repeating-conic-gradient(#fafafc 0 25%, #fff 0 50%) 0 0/16px 16px }
  .cb-prev img { max-width:100%; height:auto; display:block; background:#fff; border-radius:8px; box-shadow:0 0 0 1px #ececf1, 0 6px 14px #0000000d }
  .cb-none { text-align:center; color:#8a8798; max-width:360px }
  .cb-none b { color:#2a2833; display:block; margin-bottom:4px }
  .cb-code { position:relative }
  .cb-code pre { background:#1b1a24; color:#d8d6f0; font:12px/1.65 var(--mono); padding:14px 16px; border-radius:8px; overflow:auto; max-height:260px; margin:0; white-space:pre }
  .cb-code .k { color:#c792ea } .cb-code .s { color:#addb67 } .cb-code .n { color:#f78c6c } .cb-code .c { color:#6d6b8f }
  .cb-copy { position:absolute; top:8px; right:8px; font:600 11.5px Inter, sans-serif; background:#ffffff1a; color:#fff; border:0; border-radius:6px; padding:5px 9px; cursor:pointer }
  @media (max-width:720px) {
    .cb { height:auto } .cb-main { grid-template-columns:1fr }
    .cb-side { border-right:0; border-bottom:1px solid #ececf1; max-height:240px }
    .cb-detail { padding:18px 16px 24px }
  }
  .cb-props { width:100%; border-collapse:collapse; font-size:12.5px }
  .cb-props td { border-top:1px solid #ececf1; padding:6px 10px 6px 0; vertical-align:top } .cb-props td:first-child { font-family:var(--mono); color:#4b3fd1; white-space:nowrap; width:1% }
  .cb code { font:.92em var(--mono); background:#f1f0f5; padding:0 4px; border-radius:3px }
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
        .map(c => `<div class="cb-item ${c.name === current ? "on" : ""}" data-n="${esc(c.name)}">${esc(c.title)}</div>`).join("")).join("")
        || `<div class="cb-muted" style="padding:12px">No component matches.</div>`;
    }
    function drawDetail() {
      const c = all.find(x => x.name === current); if (!c) return;
      // The component's own usage text, or its example with the helpers it needs
      // (website/tools/components-data.py builds it).
      const code = c.code;
      // Captured at 2x; a component without one points at the app. Its size
      // (data/components.js) keeps the preview's room before the image loads.
      const shot = `img/components/${encodeURIComponent(c.name)}.webp`;
      const none = `<div class="cb-none"><b>${esc(c.title)}</b>Shown live in the app's Components window. Run <code>just run</code>, then Components › ${esc(c.category)} › ${esc(c.title)}.</div>`;
      const prev = c.shot
        ? `<img src="${shot}" srcset="${shot} 2x" width="${c.shot[0]}" height="${c.shot[1]}" alt="${esc(c.title)} in the Components window">`
        : none;
      detail.innerHTML = `<h3>${esc(c.title)}</h3><div class="cb-desc">${md(c.description)}</div>${c.when ? `<div class="cb-when"><b>When:</b> ${md(c.when)}</div>` : ""}
        <div class="cb-h">Preview</div><div class="cb-prev">${prev}</div>
        <div class="cb-h">Use it</div><div class="cb-code"><pre>${luau(code)}</pre><button class="cb-copy">Copy code</button></div>
        ${c.props.length ? `<div class="cb-h">Props</div><table class="cb-props">${c.props.map(([k, d]) => `<tr><td>${esc(k)}</td><td>${md(d)}</td></tr>`).join("")}</table>` : ""}`;
      detail.scrollTop = 0;
      const img = detail.querySelector(".cb-prev img");
      if (img) img.onerror = () => { img.outerHTML = none; };
      detail.querySelector(".cb-copy").onclick = e => { navigator.clipboard?.writeText(code); e.target.textContent = "Copied"; setTimeout(() => e.target.textContent = "Copy code", 1200); };
    }
    list.addEventListener("click", e => { const it = e.target.closest(".cb-item"); if (!it) return; current = it.dataset.n; drawList(); drawDetail(); });
    search.addEventListener("input", drawList);
    drawList(); drawDetail();
  };
})();
