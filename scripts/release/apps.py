#!/usr/bin/env python3
"""The example apps Moonframe's releases ship: each with a [release] table in
its moonframe.toml, so an app opts in on a line of its own.

    apps.py names           one app folder name per line
    apps.py json            what the casks need, as a JSON list

A released app must run on a clean machine. `--check` cannot see a provider
whose runtime is missing, so this refuses a process provider that needs
anything but a shell, in the app or in a package it requires.
"""

import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent.parent
EXAMPLES = ROOT / "examples"
SHELLS = {"bash", "sh"}
TOKEN = re.compile(r"moonframe-[a-z0-9]+(-[a-z0-9]+)*\Z")


def manifest(path):
    with path.open("rb") as file:
        return tomllib.load(file)


def folders(app, data):
    """The app's folder and the local packages it requires."""
    yield app
    for wanted in data["app"].get("requires", []):
        if wanted.startswith(("./", "../")):
            yield (app / wanted).resolve()


def runtimes(folder):
    """What the folder's process providers run on, other than a shell."""
    for provider in sorted(folder.glob("providers/*/provider.toml")):
        data = manifest(provider)
        command = data.get("command") or []
        if data.get("kind") == "process" and command and command[0] not in SHELLS:
            yield f"{provider.relative_to(ROOT)} runs {command[0]}"


def apps():
    found = []
    for path in sorted(EXAMPLES.glob("*/moonframe.toml")):
        data = manifest(path)
        release = data.get("release")
        if release is None:
            continue
        app = path.parent
        name = data["app"]["name"]
        if name != app.name:
            raise SystemExit(f"{path}: released apps are named after their folder")
        cask = release.get("cask", "")
        if not TOKEN.match(cask):
            raise SystemExit(f"{path}: [release] cask must be moonframe-<name>, not `{cask}`")
        description = release.get("description", "")
        if not description or len(description) > 80 or '"' in description or "\\" in description:
            raise SystemExit(f"{path}: [release] description must be one plain line of 80 at most")
        problems = [p for folder in folders(app, data) for p in runtimes(folder)]
        if problems:
            raise SystemExit(f"{path}: a released app needs nothing installed: " + "; ".join(problems))
        title = data["app"].get("title", name)
        found.append({"name": name, "title": title, "cask": cask, "description": description})
    if len({app["cask"] for app in found}) != len(found):
        raise SystemExit("two released apps share a cask")
    # Bundles and archives are named by title, spaces as dashes, on file
    # systems that may ignore case: one would replace the other.
    if len({app["title"].replace(" ", "-").casefold() for app in found}) != len(found):
        raise SystemExit("two released apps' titles name one bundle")
    return found


def main():
    what = sys.argv[1] if len(sys.argv) == 2 else ""
    if what == "names":
        print("\n".join(app["name"] for app in apps()))
    elif what == "json":
        print(json.dumps(apps(), indent=2))
    else:
        raise SystemExit("usage: apps.py names | json")


if __name__ == "__main__":
    main()
