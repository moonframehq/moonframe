#!/usr/bin/env python3
"""Capture actual pattern recipes in light and dark, without focusing a window.

Build with the optional `pattern-capture` feature first (see --help).
Exports the app's own GPU scene; no Screen Recording permission is needed.
Only processes started here are stopped; the user's app is untouched.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "patterns/app"
OUT = ROOT / "website/site/docs/patterns/previews"


def capture(binary, name, dark, folder):
    app = folder / "app"
    app.mkdir()
    for item in ("components", "mods", "style.luau"):
        (app / item).symlink_to(APP / item)
    width = 820 if name == "workspace-shell" else 560
    mode = "dark" if dark else "light"
    sends = '{ to = "app:appearance", msg = "' + mode + '" }'
    if name == "workspace-switcher":
        sends += ', { to = "preview-workspace-switcher", msg = "toggle" }'
    (app / "main.luau").write_text(f'''return mod {{
    window = {{ titlebar = "hidden", traffic_lights = {{ x = 16, y = 14 }}, size = {{ width = {width}, height = 520 }} }},
    init = function() return {{}}, {{ {sends} }} end,
    view = function() return ui.column {{ grow = true, style = {{ padding = {{ 50, 24, 24, 24 }} }},
        ui.mount "preview-{name}",
    }} end,
}}
''')
    dest = OUT / f'{name}{"-dark" if dark else ""}.png'
    rendered = folder / "render.png"
    with (folder / "app.log").open("w") as log:
        process = subprocess.Popen([str(binary), "--background", "--no-user", str(app)],
                                   cwd=ROOT, stdout=log, stderr=log,
                                   env={**os.environ, "XDG_CACHE_HOME": str(folder / "cache"),
                                        "XDG_STATE_HOME": str(folder / "state"),
                                        "MOONFRAME_CAPTURE": str(rendered)})
        try:
            process.wait(timeout=30)
            if process.returncode != 0 or not rendered.is_file():
                raise RuntimeError((folder / "app.log").read_text())
            rendered.replace(dest)
            print(dest.relative_to(ROOT))
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("names", nargs="*")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/moonframe-patterns")
    args = parser.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("Native screenshots require macOS; use just patterns to browse.")
    if not args.binary.is_file():
        raise SystemExit("Build first: cargo build --release -p moonframe-app --bin moonframe-patterns --no-default-features --features pattern-capture")
    known = {p.parent.name.removeprefix("preview-") for p in (APP / "mods").glob("preview-*/main.luau")}
    names = args.names or sorted(known)
    if set(names) - known:
        raise SystemExit("Unknown pattern: " + ", ".join(sorted(set(names) - known)))
    OUT.mkdir(parents=True, exist_ok=True)
    temp = ROOT / "target/pattern-shots"
    temp.mkdir(parents=True, exist_ok=True)
    for name in names:
        for dark in (False, True):
            with tempfile.TemporaryDirectory(dir=temp) as folder:
                capture(args.binary.resolve(), name, dark, Path(folder))


if __name__ == "__main__":
    main()
