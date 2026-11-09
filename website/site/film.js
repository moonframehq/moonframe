// The tour: a muted loop plays while the section is on screen, unless the
// visitor pauses it or asks for reduced motion (now or later). The button or a
// chapter swaps in the whole film. Without script the film shows with controls.
(() => {
  const box = document.querySelector(".film .screen");
  const full = box.querySelector(".full"), preview = box.querySelector(".preview");
  const play = box.querySelector(".play"), hold = box.querySelector(".hold");
  const items = [...document.querySelectorAll(".chapters li")];
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  let started = false, visible = false, paused = motion.matches;
  full.hidden = true; preview.hidden = false; play.hidden = false; hold.hidden = false;
  function sync() {
    if (started) return;
    if (visible && !paused) preview.play().catch(() => {}); else preview.pause();
    hold.textContent = paused ? "Play preview" : "Pause preview";
    hold.setAttribute("aria-pressed", String(paused));
  }
  const seen = new IntersectionObserver(([e]) => { visible = e.isIntersecting; sync(); }, { threshold: 0.4 });
  seen.observe(box);
  motion.addEventListener("change", () => { if (motion.matches) paused = true; sync(); });
  hold.addEventListener("click", () => { paused = !paused; sync(); });
  sync();
  function start(at) {
    if (!started) {
      started = true;
      seen.disconnect(); preview.pause();
      preview.hidden = true; play.hidden = true; hold.hidden = true; full.hidden = false;
      full.preload = "auto";
    }
    if (at != null) full.currentTime = at;
    full.play().catch(() => {});
    full.focus({ preventScroll: true });
  }
  play.addEventListener("click", () => start(0));
  items.forEach((li) => li.querySelector("button").addEventListener("click", (ev) => start(Number(ev.currentTarget.dataset.t))));
  full.addEventListener("timeupdate", () => {
    let on = -1;
    items.forEach((li, i) => { if (full.currentTime >= Number(li.querySelector("button").dataset.t)) on = i; });
    items.forEach((li, i) => li.classList.toggle("on", i === on));
  });
})();
