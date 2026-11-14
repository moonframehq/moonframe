//! The page the PNGs are shown on: Sheprd's mockup kit when Sheprd serves
//! it (side by side, **Pick** sends the choice to the agent, **Annotate**
//! takes notes), else plain cards whose **Pick** copies the choice.

use {super::Shot, std::path::Path};

/// The page's script: lays the variants out once the kit has loaded or
/// failed to. `MOCKUP` is `{ ask, file, variants: [{ id, title, note,
/// width, light, dark }] }`.
const SCRIPT: &str = r#"
const mode = { now: "auto" };
const bodies = [];
function picture(variant) {
  const img = document.createElement("img");
  img.alt = variant.title;
  img.width = variant.width;
  const light = variant.light || variant.dark;
  const dark = variant.dark || variant.light;
  if (mode.now === "auto" && variant.light && variant.dark) {
    const pic = document.createElement("picture");
    const source = document.createElement("source");
    source.media = "(prefers-color-scheme: dark)";
    source.srcset = dark;
    img.src = light;
    pic.append(source, img);
    return pic;
  }
  img.src = mode.now === "dark" ? dark : light;
  return img;
}
function draw() {
  bodies.forEach(([node, variant]) => node.replaceChildren(picture(variant)));
}
const question = `Apply this mockup${MOCKUP.ask ? ` of "${MOCKUP.ask}"` : ""} (${MOCKUP.file})`;
const root = document.getElementById("variants");
const list = MOCKUP.variants.map((variant) => {
  const body = document.createElement("div");
  body.className = "mf-shot";
  bodies.push([body, variant]);
  return { id: variant.id, title: `${variant.id} · ${variant.title}`, note: variant.note, body };
});
if (window.HerdrKit) {
  window.HerdrKit.variants(root, list, { question });
} else {
  root.className = "mf-cards";
  list.forEach((variant) => {
    const card = document.createElement("section");
    card.className = "mf-card";
    const head = document.createElement("div");
    head.className = "mf-head";
    const name = document.createElement("strong");
    name.textContent = variant.title;
    const pick = document.createElement("button");
    pick.textContent = "Pick";
    pick.onclick = () => {
      navigator.clipboard?.writeText(`${question}: ${variant.title}`);
      pick.textContent = "Copied";
    };
    head.append(name, pick);
    card.append(head, variant.body);
    if (variant.note) {
      const note = document.createElement("p");
      note.textContent = variant.note;
      card.append(note);
    }
    root.append(card);
  });
}
document.getElementById("title").textContent = MOCKUP.ask || "Mockups";
document.querySelectorAll("[data-mode]").forEach((button) => {
  button.onclick = () => {
    mode.now = button.dataset.mode;
    document.querySelectorAll("[data-mode]").forEach((other) =>
      other.setAttribute("aria-pressed", String(other === button)));
    draw();
  };
});
draw();
"#;

const STYLE: &str = r#"
body { font: 14px/1.4 -apple-system, system-ui, sans-serif; margin: 0; }
main { padding: 20px; }
.mf-bar { display: flex; gap: 6px; align-items: center; margin: 8px 0 16px; }
.mf-bar button[aria-pressed="true"] { font-weight: 600; }
.mf-shot img { max-width: 100%; height: auto; display: block; border-radius: 8px; }
.mf-cards { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-start; }
.mf-card { border: 1px solid #8884; border-radius: 10px; padding: 12px; }
.mf-head { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
"#;

/// `index.html` in `out`, beside the PNGs, for `file`'s variants.
pub(super) fn write(
    out: &Path,
    file: &Path,
    ask: Option<&str>,
    shots: &[Shot],
) -> Result<std::path::PathBuf, String> {
    let variants: Vec<serde_json::Value> = shots
        .iter()
        .map(|shot| {
            let image = |appearance: &str| {
                shot.files
                    .iter()
                    .find(|(shown, _)| shown == appearance)
                    .and_then(|(_, path)| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
            };
            serde_json::json!({
                "id": shot.letter,
                "title": shot.name,
                "note": shot.note,
                "width": shot.width,
                "light": image("light"),
                "dark": image("dark"),
            })
        })
        .collect();
    let data = serde_json::json!({
        "ask": ask,
        "file": file.display().to_string(),
        "variants": variants,
    });
    // `</script>` in a string would end the script early.
    let data = data.to_string().replace("</", "<\\/");
    let title = escape(ask.unwrap_or("Mockups"));
    let html = format!(
        r#"<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link rel="stylesheet" href="/.herdr/kit.css">
<script src="/.herdr/kit.js"></script>
<style>{STYLE}</style>
<body class="hk"><main class="hk-page">
<h1 id="title"></h1>
<p class="hk-lede">Drawn by Moonframe with its own components. Pick one, or annotate what to change.</p>
<div class="mf-bar hk-segmented">
<button class="hk-button" data-mode="auto" aria-pressed="true">Auto</button>
<button class="hk-button" data-mode="light" aria-pressed="false">Light</button>
<button class="hk-button" data-mode="dark" aria-pressed="false">Dark</button>
</div>
<div id="variants"></div>
</main>
<script>const MOCKUP = {data};{SCRIPT}</script>
"#
    );
    let page = out.join("index.html");
    std::fs::write(&page, html).map_err(|error| format!("{}: {error}", page.display()))?;
    Ok(page)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_lists_each_variant_and_cannot_be_closed_early() -> Result<(), String> {
        let out = tempfile::tempdir().map_err(|error| error.to_string())?;
        let shots = vec![Shot {
            letter: "A".to_owned(),
            name: "Pills </script>".to_owned(),
            note: Some("Each value in a pill.".to_owned()),
            width: 320.,
            files: vec![
                ("light".to_owned(), out.path().join("a.png")),
                ("dark".to_owned(), out.path().join("a-dark.png")),
            ],
        }];
        let page = write(out.path(), Path::new("/x/m.luau"), Some("a <bar>"), &shots)?;
        let html = std::fs::read_to_string(page).map_err(|error| error.to_string())?;
        assert!(html.contains("<title>a &lt;bar&gt;</title>"));
        assert!(html.contains(r#""light":"a.png""#) && html.contains(r#""dark":"a-dark.png""#));
        assert_eq!(html.matches("</script>").count(), 2, "{html}");
        Ok(())
    }
}
