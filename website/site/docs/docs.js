// Search filters the sidebar; ⌘K or / focuses it, Enter opens the first match.
const q = document.getElementById("q"), links = [...document.querySelectorAll("aside a")], none = document.getElementById("none");
// On phones the sidebar folds behind Contents; typing a search opens it.
const side = document.getElementById("side"), menu = document.getElementById("menu");
menu.onclick = () => { const open = side.classList.toggle("open"); menu.setAttribute("aria-expanded", open); menu.textContent = open ? "Contents ▴" : "Contents ▾"; };
q.addEventListener("input", () => { if (q.value && !side.classList.contains("open")) menu.click(); });
q.addEventListener("input", () => {
  const term = q.value.trim().toLowerCase();
  let shown = 0;
  for (const a of links) { const hit = !term || a.textContent.toLowerCase().includes(term); a.hidden = !hit; shown += hit; }
  for (const h of document.querySelectorAll("aside h4")) {
    let next = h.nextElementSibling, any = false;
    while (next && next.tagName === "A") { any ||= !next.hidden; next = next.nextElementSibling; }
    h.hidden = !any;
  }
  none.hidden = shown > 0;
});
q.addEventListener("keydown", e => { if (e.key === "Enter") { const first = links.find(a => !a.hidden); if (first) location.href = first.href; } });
addEventListener("keydown", e => {
  if ((e.key === "k" && (e.metaKey || e.ctrlKey)) || (e.key === "/" && document.activeElement !== q)) { e.preventDefault(); q.focus(); }
});
