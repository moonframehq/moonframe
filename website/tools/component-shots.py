#!/usr/bin/env python3
"""Screenshot every component's preview for the Components browser on moonframe.dev.

For each component in website/site/data/components.js this opens the app's
Components window on its preview alone (scripts/component-shot.sh with
--preview-only; on macOS it needs Screen Recording permission, and
Accessibility to make the window tall; window-shot.sh lists what Linux and
Windows need), trims the capture to what the preview draws, and
writes website/site/img/components/<name>.webp at 2x. Run from the repo root,
after `python3 website/tools/components-data.py` (it runs that again at the
end, so data/components.js records each picture's size):

    uv run --with pillow website/tools/component-shots.py [NAME ...]

With names, only those are captured again. `--crop-only` reuses the captures
already in target/components/ instead of opening the app.
"""
import json
import os
import re
import subprocess
import sys

from PIL import Image, ImageChops

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RAW = os.path.join(ROOT, "target", "components")
OUT = os.path.join(ROOT, "website", "site", "img", "components")
WINDOW_WIDTH = 900  # the components window's width, in points
SCALE = 2  # the pictures are 2x; the website declares them so
TITLE_BAR = 66  # the window's title bar, at 2x
EDGE = 8  # the window's border and corners, at 2x
PAD = 24  # white kept around the trimmed preview, at 2x


def names():
    with open(os.path.join(ROOT, "website", "site", "data", "components.js")) as source:
        text = source.read()
    return [c["name"] for c in json.loads(re.sub(r"^window\.COMPONENTS = |;\s*$", "", text))]


def crop(name):
    image = Image.open(os.path.join(RAW, name + ".png")).convert("RGB")
    # The crop is measured at 2x, and a 1x capture would show at half size.
    scale = image.width / WINDOW_WIDTH
    if scale != SCALE:
        raise ValueError(f"captured at {scale:g}x; capture on a {SCALE}x (Retina) display")
    # Below the title bar, and clear of the window's edges and rounded foot.
    inside = image.crop((EDGE, TITLE_BAR, image.width - EDGE, image.height - 3 * EDGE))
    background = Image.new("RGB", inside.size, inside.getpixel((inside.width // 2, inside.height - 2)))
    bbox = ImageChops.difference(inside, background).point(lambda v: 255 if v > 6 else 0).getbbox()
    if not bbox:
        raise ValueError("the preview drew nothing")
    left, top, right, bottom = bbox
    inside = inside.crop((max(left - PAD, 0), max(top - PAD, 0),
                          min(right + PAD, inside.width), min(bottom + PAD, inside.height)))
    os.makedirs(OUT, exist_ok=True)
    inside.save(os.path.join(OUT, name + ".webp"), "WEBP", quality=88, method=6)
    return inside.size


def main():
    args = sys.argv[1:]
    crop_only = "--crop-only" in args
    wanted = [a for a in args if not a.startswith("--")] or names()
    # The preview alone, in a window as tall as the screen allows.
    env = {**os.environ, "PREVIEW_ONLY": "1", "SHOT_HEIGHT": "2000"}
    failed = []
    for name in wanted:
        try:
            if not crop_only:
                subprocess.run(["scripts/component-shot.sh", name, os.path.join(RAW, name + ".png"), "app"],
                               cwd=ROOT, check=True, stdout=subprocess.DEVNULL, env=env)
            print(name, "%dx%d" % crop(name))
        except Exception as error:  # noqa: BLE001 - report and go on with the rest
            failed.append(name)
            print(name, "failed:", error, file=sys.stderr)
    # Record the new pictures' sizes, so the page keeps their room while they load.
    subprocess.run([sys.executable, "website/tools/components-data.py"], cwd=ROOT, check=True)
    if failed:
        sys.exit("failed: " + " ".join(failed))


if __name__ == "__main__":
    main()
