#!/usr/bin/env python3
"""Check capability dependency boundaries; --lint also builds each selection."""

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def cargo(*args, capture=False):
    return subprocess.run(
        ["cargo", *args], cwd=ROOT, check=True, text=True,
        stdout=subprocess.PIPE if capture else None,
    ).stdout


def selection(feature):
    return ["--no-default-features", *(["--features", feature] if feature else [])]


def dependencies(package, feature):
    output = cargo(
        "tree", "--locked", "-p", package, *selection(feature),
        "--edges", "normal,build", "--prefix", "none", "--format", "{p}",
        capture=True,
    )
    return {line.split()[0] for line in output.splitlines() if line.strip()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lint", action="store_true")
    parser.add_argument("--release", action="store_true", help="lint with the release profile")
    args = parser.parse_args()
    metadata = json.loads(cargo("metadata", "--locked", "--format-version", "1",
                                "--no-deps", capture=True))
    manifests = {package["name"]: package["features"] for package in metadata["packages"]}
    library, starter = manifests["moonframe"], manifests["moonframe-app"]
    features = set(library) - {"default"}
    starter_only = {"pattern-capture", "patterns"}
    assert features == set(starter) - {"default"} - starter_only, "starter must forward every capability"
    for name in features:
        assert starter[name] == [f"moonframe/{name}"], name
    # Off in the library, on in the starter: Ghostty needs Zig, and few apps
    # need a street map, LaTeX, live video or what long runs ask of the machine.
    opt_in = {"awake", "crash", "displays", "dock", "documents", "ghostty", "guard", "map", "math", "pi", "procs", "projects", "restart",
              "socket", "speech", "video"}
    assert set(starter["default"]) == set(library["default"]) | opt_in

    optional = {
        # keep-sorted start
        "acp": "moonframe-acp",
        "chat": "moonframe-chat",
        "docs": "moonframe-docs",
        "documents": "moonframe-documents",
        "forge": "moonframe-forge",
        "git": "moonframe-git",
        "pi": "moonframe-pi",
        "procs": "moonframe-procs",
        "projects": "moonframe-projects",
        "shell": "moonframe-shell",
        "speech": "moonframe-speech",
        "usage": "moonframe-usage",
        "wasm": "wasmtime",
        # keep-sorted end
    }
    for feature in ["", "browser", "terminal", "alacritty", "ghostty", "herdr-core",
                    "map", "markdown", "prose", "updates"]:
        for package in ["moonframe", "moonframe-app"]:
            deps = dependencies(package, feature)
            terminal = feature in {"terminal", "alacritty", "ghostty"}
            assert ("moonframe-terminal" in deps) == terminal, (package, feature, "terminal")
            assert ("portable-pty" in deps) == terminal, (package, feature, "pty")
            if sys.platform in {"darwin", "win32"}:
                assert ("lb-wry" in deps) == (feature == "browser"), (package, feature, "browser")
            if feature != "browser":
                assert "lb-wry" not in deps and "objc2-web-kit" not in deps, (package, feature, "browser")
            assert ("libghostty-vt" in deps) == (feature == "ghostty"), (package, feature, "ghostty")
            if feature != "herdr-core":
                assert ("alacritty_terminal" in deps) == (feature == "alacritty"), (package, feature, "alacritty")
            for capability, dependency in optional.items():
                assert dependency not in deps, (package, feature, capability)
            if feature in {"", "browser", "terminal", "alacritty", "ghostty", "map", "markdown",
                           "prose", "updates"}:
                # HTTP only for the updater and the map's tiles and routes.
                for dependency in ["ureq", "rustls", "ring"]:
                    assert (dependency in deps) == (feature in {"map", "updates"}), (package, feature, dependency)
        print(f"ok: dependency boundaries ({feature or 'minimal'})", flush=True)

    if args.lint:
        # One package at a time: workspace feature unification would mask
        # missing cfg gates and accidentally enabled defaults.
        for feature in ["", *sorted(features)]:
            print(f"lint: {feature or 'minimal'}", flush=True)
            cargo("clippy", "--quiet", "--locked", "-p", "moonframe-app", "-p", "moonframe",
                  *(["--release"] if args.release else []),
                  "--all-targets", *selection(feature), "--", "-D", "warnings")


if __name__ == "__main__":
    main()
