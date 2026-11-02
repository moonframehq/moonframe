import os
import re

from formatting import html_to_text
from state import html_dir, make_dir, write_file
from values import text

# ---- HTML mail as a page ---------------------------------------------------

# Whether the HTML loads anything from the network (pictures, backgrounds).
def remote_content(html):
    return bool(re.search(r"""(?:src|background|srcset)\s*=\s*["']?\s*(?:https?:)?//""", html, re.I)
                or re.search(r"""url\(\s*['"]?\s*(?:https?:)?//""", html, re.I))


PAGE = """<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src {images}; style-src 'unsafe-inline'; font-src data:; form-action 'none'; base-uri 'none'">
<meta name="color-scheme" content="{scheme}">
<style>
html, body {{ margin: 0; padding: 0; background: {bg}; color: {fg}; }}
body {{ font: 14px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; overflow-wrap: anywhere; }}
a {{ color: {link}; }}
h1 {{ font-size: 2.15em; line-height: 1.2; margin: 0.15em 0 0.35em; }}
h2, h3 {{ line-height: 1.3; margin: 0.5em 0 0.3em; }}
p {{ margin: 0 0 0.5em; }}
ul, ol {{ margin: 0 0 0.5em; padding-left: 1.1em; }}
blockquote {{ margin: 0.3em 0 0.5em; padding-left: 1.4em; border-left: 3px solid {rule}; color: {muted}; }}
img {{ max-width: 100%; height: auto; }}
table {{ border-collapse: collapse; max-width: 100%; }}
td, th {{ padding: 2px 12px 2px 0; vertical-align: top; text-align: left; }}
small {{ color: {muted}; }}
</style>
{kept}
</head><body>{body}</body></html>
"""


# A message's HTML as a page for the reader's web view: its body and styles
# kept; scripts, frames, forms and refreshes dropped; under a
# Content-Security-Policy that runs no script and loads nothing from the
# network (remote pictures neither, until the user asks); in the window's
# light or dark colours. Returns its file:// address.
def page(message_id, html, dark, remote):
    path = "%s/%s-%s%s.html" % (html_dir, text(message_id), "dark" if dark else "light",
                                "-remote" if remote else "")
    if os.path.isfile(path):
        return "file://" + path
    styles = [re.sub(r"@import[^;]*;", "", style, flags=re.I)
              for style in re.findall(r"(<style\b.*?</style\s*>)", html, re.I | re.S)]
    found = re.search(r"<body\b[^>]*>(.*)</body\s*>", html, re.I | re.S)
    body = found.group(1) if found else html
    body = re.sub(r"<(script|style|head|title|iframe|object|embed|form|noscript)\b.*?</\1\s*>", "",
                  body, flags=re.I | re.S)
    body = re.sub(r"<(script|meta|link|base|iframe|object|embed)\b[^>]*>", "", body, flags=re.I)
    body = re.sub(r"""\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)""", "", body, flags=re.I)
    if dark:
        bg, fg, muted, link, rule = "#0a0a0a", "#fafafa", "#a3a3a3", "#60a5fa", "#262626"
    else:
        bg, fg, muted, link, rule = "#ffffff", "#0a0a0a", "#737373", "#2563eb", "#e5e5e5"
    content = PAGE.format(
        images="data: cid: https: http:" if remote else "data: cid:",
        scheme="dark" if dark else "light",
        bg=bg, fg=fg, muted=muted, link=link, rule=rule,
        kept="\n".join(styles), body=body,
    )
    make_dir(html_dir)
    if not write_file(path, content.encode("utf-8")):
        return None
    return "file://" + path


# How tall the page draws, roughly: a view can't measure it.
def page_height(html):
    height = 6
    plain = html_to_text(html)
    for line in plain.split("\n") if plain else []:
        height += 23 * (1 + len(line) // 95) if re.search(r"\S", line) else 6
    height += 38 * len(re.findall(r"<h1\b", html, re.I))
    height += 12 * len(re.findall(r"<h[23]\b", html, re.I))
    # Pictures: their stated height, else a guess.
    for img in re.findall(r"<img\b[^>]*>", html, re.I):
        stated = re.search(r"""height\s*=\s*["']?([0-9]+)""", img, re.I)
        height += int(stated.group(1)) + 8 if stated else 160
    return min(max(height, 60), 1400)
