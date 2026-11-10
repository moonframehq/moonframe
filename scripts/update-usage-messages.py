#!/usr/bin/env python3
"""Snapshot usage's English messages from its pinned Sheprd checkout.

Run with the path to that checkout whenever sheprd-usage is updated.
Usage messages are embedded; Moonframe keeps its own UI.
"""

import argparse
from pathlib import Path
import re
import subprocess
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    rev = manifest["workspace"]["dependencies"]["herdr-usage"]["rev"]
    actual = subprocess.check_output(
        ["git", "-C", str(args.checkout), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual != rev:
        parser.error(f"expected checkout at {rev}, got {actual}")
    locales = args.checkout / "crates/sheprd-gpui/locales/en-US"
    files = sorted(locales.glob("usage-*.ftl"))
    if not files:
        parser.error(f"no usage messages in {locales}")
    content = "\n".join(file.read_text() for file in files)
    available = set(re.findall(r"^([a-z][\w-]+)\s*=", content, re.MULTILINE))
    used = set()
    for source in (args.checkout / "crates/sheprd-usage/src").rglob("*.rs"):
        used.update(re.findall(r'Text::message\(\s*"([^"]+)"', source.read_text()))
    if missing := used - available:
        parser.error(f"missing usage messages: {', '.join(sorted(missing))}")
    destination = root / "crates/moonframe-usage/locales"
    outputs = {
        "REVISION": f"{rev}\n",
        "LICENSE": (args.checkout / "LICENSE").read_text(),
    }
    outputs.update({f"en-US/{file.name}": file.read_text() for file in files})
    for name, text in outputs.items():
        path = destination / name
        if args.check:
            if not path.exists() or path.read_text() != text:
                parser.exit(1, f"out of date: {path}\n")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)


if __name__ == "__main__":
    main()
