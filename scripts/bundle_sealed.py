#!/usr/bin/env python3
"""Build an app-specific executable and stage its resolved source closure."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

from sealed_plan import Plan, local_packages, manifest, profile_paths

ROOT = Path(__file__).resolve().parent.parent


def run(words, **kwargs):
    subprocess.run([str(word) for word in words], check=True, cwd=ROOT, **kwargs)


def size(path):
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def providers(plan, roots):
    """Only selected Wasm providers are built, at their original source paths."""
    builds = ROOT / "target/providers"
    for name in sorted(plan.chosen["providers"]):
        source = plan.entries["providers"][name]
        _, staged = next((a, b) for a, b in roots if source.parent == a / "providers")
        dest = staged / "providers" / name
        if source == plan.builtin / "providers/skills":
            # This process provider's runtime data normally comes from
            # stage-app.sh. Keep it only when the provider itself was kept.
            Plan.copy(ROOT / ".claude/skills", dest / "skills")
        if manifest(source / "provider.toml").get("kind") != "wasm":
            continue
        cargo = source / "Cargo.toml"
        if cargo.is_file():
            package = manifest(cargo)["package"]["name"].replace("-", "_")
            run(["cargo", "build", "--quiet", "--release", "--locked", "--target", "wasm32-wasip2",
                 "--manifest-path", cargo, "--target-dir", builds])
            shutil.copy2(builds / "wasm32-wasip2/release" / f"{package}.wasm", dest / "provider.wasm")
        if not (dest / "provider.wasm").is_file():
            raise ValueError(f"{source}: selected Wasm provider has no Cargo.toml or provider.wasm")
        for name in ("Cargo.toml", "Cargo.lock", "src", "wit", "target"):
            path = dest / name
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink(missing_ok=True)


def build(plan, out):
    if os.environ.get("MOONFRAME_BINARY"):
        raise ValueError("sealed bundles build their own executable; unset MOONFRAME_BINARY")
    target = ROOT / os.environ.get("CARGO_TARGET_DIR", "target")
    target.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sealed-stage-", dir=target) as work:
        work = Path(work)
        resources = work / "resources"
        roots = plan.stage(resources)
        providers(plan, roots)
        selected = ["moonframe/sealed", *(f"moonframe/{f}" for f in sorted(plan.chosen["features"]))]
        built = subprocess.run(["cargo", "build", "--quiet", "--locked", "--profile", "bundle",
                                "--message-format=json-render-diagnostics", "-p", "moonframe-app",
                                "--no-default-features", "--features", ",".join(selected)],
                               check=True, cwd=ROOT, stdout=subprocess.PIPE, text=True)
        artifacts = [json.loads(line) for line in built.stdout.splitlines()]
        binaries = [Path(item["executable"]) for item in artifacts
                    if item.get("reason") == "compiler-artifact" and item.get("executable")
                    and item["target"]["name"] == "moonframe"]
        if len(binaries) != 1:
            raise ValueError("Cargo did not report the sealed executable")
        binary = binaries[0]
        report = plan.report()
        report["executable_bytes"] = binary.stat().st_size
        report["resource_bytes"] = size(resources)
        report["source_bytes_before_pruning"] = sum(size(layer) for layer in plan.layers)
        report["cargo_features"] = selected
        (resources / "sealed-build.json").write_text(json.dumps(report, indent=2) + "\n")
        metadata = plan.metadata
        icon = plan.app / metadata.get("icon", "icon.png")
        values = [("name", metadata["name"]), ("title", metadata.get("title", metadata["name"])),
                  ("app", str(plan.app))]
        if "version" in metadata:
            values.append(("version", metadata["version"]))
        if icon.is_file():
            values.append(("icon", str(icon)))
        for _, value in values:
            if any(c in value for c in "\t\r\n"):
                raise ValueError("bundle metadata may not contain tabs or newlines")
        paths = work / "paths.tsv"
        paths.write_text("".join(f"{key}\t{value}\n" for key, value in values))
        env = dict(os.environ, MOONFRAME_BINARY=str(binary), MOONFRAME_BUNDLE_PATHS=str(paths),
                   MOONFRAME_STAGED_RESOURCES=str(resources))
        run(["bash", "scripts/bundle-app.sh", out, metadata["name"]], env=env)
        print(f"sealed: executable {report['executable_bytes']:,} bytes; resources "
              f"{report['resource_bytes']:,} bytes; features: {', '.join(sorted(plan.chosen['features'])) or 'core only'}",
              file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--app", type=Path, help="app folder, with local requirements")
    source.add_argument("--profile", help="installed/linked profile (resolves remote requirements too)")
    parser.add_argument("--out", type=Path, default=Path("target/bundle"))
    parser.add_argument("--plan", action="store_true", help="print dependencies without building or staging")
    args = parser.parse_args()
    try:
        if args.profile:
            user = Path(os.environ.get("MOONFRAME_USER") or
                        str(Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "moonframe"))
            app, packages = profile_paths(user, args.profile)
        else:
            app = args.app.resolve(strict=True)
            packages = local_packages(app)
        plan = Plan(ROOT / "app", app, packages)
        if args.plan:
            print(json.dumps(plan.report(), indent=2))
        else:
            if os.environ.get("CARGO_BUILD_TARGET"):
                raise ValueError("sealed bundling currently builds for the local machine; unset CARGO_BUILD_TARGET")
            build(plan, args.out.resolve())
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        print(f"sealed bundle: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
