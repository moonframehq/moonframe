#!/usr/bin/env python3
"""Render the social cards and write the Open Graph and X/Twitter tags.

Each page's card is website/og/card.html, rendered at 1200x630 by headless
Chrome. The image is named by a hash of its bytes (social/home-1a2b3c4d5e.png),
so a new card has a new URL and X, Slack, LinkedIn and the rest fetch it
instead of serving the one they cached. Old cards stay published, so links
shared before a change keep their image. The tags between <!-- social -->
and <!-- /social --> in each page are rewritten from PAGES below: edit the
social title and description there, not in the page.

Run from the repo root: python3 website/tools/og-images.py
Set CHROME to a Chrome or Chromium binary if none is found.
"""
import glob
import hashlib
import html
import os
import pathlib
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
SITE = ROOT / "site"
OG = SITE / "social"

PAGES = {
    "home": {
        "file": SITE / "index.html",
        "url": "https://moonframe.dev/",
        "title": "Moonframe: native desktop apps in Rust, shaped live in Luau",
        "description": "A Rust starter for native desktop apps whose panels, toolbars and status bar are Luau. "
        "Ask an agent for a change and watch the running window update: no rebuild, no restart.",
        "alt": "Moonframe. Ask for it, watch the window change: the Moonframe window beside an agent "
        "adding a CPU sparkline to its status bar.",
    },
    "docs": {
        "file": SITE / "docs" / "index.html",
        "url": "https://docs.moonframe.dev/",
        "title": "Moonframe docs: build a native app in Luau, on Rust",
        "description": "Get started with Moonframe: mods, the stylesheet, 142 components, terminals, "
        "providers, working with an agent, and whole apps as profiles.",
        "alt": "Moonframe docs. Build a native app in Luau, on Rust: quick start, mods, components, "
        "working with an agent, apps as profiles.",
    },
}


def chrome():
    candidates = [os.environ.get("CHROME")]
    candidates += sorted(glob.glob(os.path.expanduser(
        "~/Library/Caches/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-*/chrome-headless-shell")))
    candidates += [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/chromium", "/usr/bin/google-chrome",
    ]
    for path in candidates:
        if path and os.path.exists(path):
            return path
    raise SystemExit("no Chrome found: set CHROME to a Chrome or Chromium binary")


def render(page):
    out = pathlib.Path(tempfile.mkdtemp()) / f"{page}.png"
    card = (ROOT / "og" / "card.html").as_uri() + f"?page={page}"
    subprocess.run([chrome(), "--headless", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                    "--allow-file-access-from-files", "--window-size=1200,630", "--virtual-time-budget=3000",
                    f"--screenshot={out}", card], check=True, capture_output=True)
    return out.read_bytes()


def tags(page, image):
    p = {k: html.escape(v, quote=True) for k, v in PAGES[page].items() if isinstance(v, str)}
    return f"""<!-- social -->
<meta name="description" content="{p['description']}">
<link rel="canonical" href="{p['url']}">
<meta name="theme-color" content="#f6f3ec">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Moonframe">
<meta property="og:url" content="{p['url']}">
<meta property="og:title" content="{p['title']}">
<meta property="og:description" content="{p['description']}">
<meta property="og:image" content="{image}">
<meta property="og:image:type" content="image/png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="{p['alt']}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{p['title']}">
<meta name="twitter:description" content="{p['description']}">
<meta name="twitter:image" content="{image}">
<meta name="twitter:image:alt" content="{p['alt']}">
<!-- /social -->"""


OG.mkdir(parents=True, exist_ok=True)
for page, info in PAGES.items():
    data = render(page)
    name = f"{page}-{hashlib.sha256(data).hexdigest()[:10]}.png"
    (OG / name).write_bytes(data)
    image = f"https://moonframe.dev/social/{name}"
    source = info["file"].read_text(encoding="utf-8")
    block = tags(page, image)
    if "<!-- social -->" in source:
        source = re.sub(r"<!-- social -->.*?<!-- /social -->", lambda _: block, source, flags=re.S)
    else:
        source = re.sub(r"(<title>.*?</title>\n)", lambda m: m.group(1) + block + "\n", source, count=1, flags=re.S)
    info["file"].write_text(source, encoding="utf-8")
    print(f"{page}: social/{name} ({len(data) // 1024} KB)")
