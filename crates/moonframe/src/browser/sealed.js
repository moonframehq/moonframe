// The host's script in a `ui.document`, the only one that runs there: the
// page's own are off. It tells the host of link clicks, scrolling, the
// selection and, while editable, the HTML, through `window.ipc`, and takes
// the host's calls on `window.__moonframeDocument`.
(() => {
  if (window.__moonframeDocument) return;
  const post = (message) => {
    try {
      window.ipc.postMessage(JSON.stringify(message));
    } catch (_) {}
  };
  // At most once every `wait` ms, the last call always arriving.
  const throttled = (wait, run) => {
    let timer = null;
    let last = 0;
    return () => {
      const now = Date.now();
      if (timer) return;
      const due = Math.max(0, last + wait - now);
      timer = setTimeout(() => {
        timer = null;
        last = Date.now();
        run();
      }, due);
    };
  };
  // Once calls stop for `wait` ms.
  const settled = (wait, run) => {
    let timer = null;
    return () => {
      clearTimeout(timer);
      timer = setTimeout(run, wait);
    };
  };
  const MAX_TEXT = 65536;
  // What `html` takes back, in bytes (`MAX_HTML`).
  const MAX_HTML = 8 * 1024 * 1024;
  let editable = false;

  // The document as the mod gave it, edits included: without what the host
  // added (marked `data-moonframe`) or the editing switch.
  const html = () => {
    const root = document.documentElement.cloneNode(true);
    root.querySelectorAll("[data-moonframe]").forEach((node) => node.remove());
    const body = root.querySelector("body");
    if (body) body.removeAttribute("contenteditable");
    return "<!doctype html>\n" + root.outerHTML;
  };

  const scrolled = throttled(100, () => {
    const page = document.scrollingElement || document.documentElement;
    post({
      kind: "scroll",
      y: page.scrollTop,
      height: window.innerHeight,
      content: page.scrollHeight,
    });
  });

  const selected = settled(150, () => {
    const selection = document.getSelection();
    const text = selection ? selection.toString().slice(0, MAX_TEXT) : "";
    let rect = { x: 0, y: 0, width: 0, height: 0 };
    if (text && selection.rangeCount > 0) {
      const box = selection.getRangeAt(0).getBoundingClientRect();
      rect = { x: box.left, y: box.top, width: box.width, height: box.height };
    }
    post({ kind: "select", text, ...rect });
  });

  // An edit, or that the document grew past what the mod can give back;
  // `more` goes with it.
  const report = (kind, more) => {
    const text = html();
    if (new TextEncoder().encode(text).length > MAX_HTML) post({ kind, large: true, ...more });
    else post({ kind, html: text, ...more });
  };
  const edited = settled(500, () => {
    if (editable) report("change");
  });
  // Typed into since the last `done` (or the load): what `done` tells.
  let typed = false;
  document.addEventListener("input", () => {
    typed = true;
  }, true);

  const reveal = (id) => {
    const target = document.getElementById(id);
    if (target) target.scrollIntoView({ block: "start" });
  };

  document.addEventListener(
    "click",
    (event) => {
      const link = event.target && event.target.closest && event.target.closest("a[href]");
      if (!link) return;
      event.preventDefault();
      // Edited, a link is text to place the caret in; with the command (or
      // control) key it opens, as editors do.
      if (editable && !(event.metaKey || event.ctrlKey)) return;
      const href = link.getAttribute("href") || "";
      if (href.startsWith("#")) {
        reveal(decodeURIComponent(href.slice(1)));
        return;
      }
      post({ kind: "link", url: href, text: (link.textContent || "").trim().slice(0, 1024) });
    },
    true,
  );
  // Nothing dropped on it is opened: a dropped file would leave the document.
  document.addEventListener("dragover", (event) => event.preventDefault(), true);
  document.addEventListener("drop", (event) => {
    if (!editable) event.preventDefault();
  }, true);
  window.addEventListener("scroll", scrolled, { passive: true });
  window.addEventListener("resize", scrolled, { passive: true });
  document.addEventListener("selectionchange", selected);
  document.addEventListener("input", edited, true);
  window.addEventListener("blur", () => {
    if (!editable) return;
    report("done", { edited: typed });
    typed = false;
  });
  document.addEventListener("DOMContentLoaded", scrolled);

  Object.defineProperty(window, "__moonframeDocument", {
    value: Object.freeze({
      scrollToY: (y) => window.scrollTo(0, y),
      scrollToId: reveal,
      setEditable: (on) => {
        editable = !!on;
        const body = document.body;
        if (!body) return;
        if (editable) body.setAttribute("contenteditable", "true");
        else body.removeAttribute("contenteditable");
      },
    }),
  });
})();
