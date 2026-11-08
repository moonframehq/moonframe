#!/usr/bin/env python3
"""Writes a cask for each released app into OUT_DIR, as TOKEN.rb, from
homebrew/Casks/app.rb and the SHA256SUMS of the apps' release, for
scripts/release/update-homebrew.sh.

    render-app-casks.py VERSION SHA256SUMS OUT_DIR
"""

import json
from pathlib import Path
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
TEMPLATE = HERE.parent.parent / "homebrew/Casks/app.rb"


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: render-app-casks.py VERSION SHA256SUMS OUT_DIR")
    version, sums, out = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise SystemExit(f"not a release version: {version}")
    known = {}
    for line in sums.read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        known[name.lstrip("*")] = digest.lower()
    apps = json.loads(subprocess.run([sys.executable, HERE / "apps.py", "json"], check=True,
                                     stdout=subprocess.PIPE, text=True).stdout)
    # The template's leading comment is for this repository, not the tap.
    template = "".join(line for line in TEMPLATE.read_text().splitlines(keepends=True)
                       if not line.startswith("#"))
    out.mkdir(parents=True, exist_ok=True)
    for app in apps:
        archive = app["title"].replace(" ", "-")
        digest = known.get(f"{archive}-{version}-universal-apple-darwin.zip")
        if digest is None or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise SystemExit(f"no checksum for {archive} {version} in {sums}")
        cask = template
        for key, value in (("TOKEN", app["cask"]), ("VERSION", version), ("SHA256", digest),
                           ("ARCHIVE", archive), ("TITLE", app["title"]), ("NAME", app["name"]),
                           ("DESCRIPTION", app["description"])):
            cask = cask.replace(f"@{key}@", value)
        (out / f"{app['cask']}.rb").write_text(cask)
        print(app["cask"])


if __name__ == "__main__":
    main()
