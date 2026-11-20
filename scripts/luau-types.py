#!/usr/bin/env python3
"""Type-checks the Luau under DIR with luau-lsp in strict mode (`just types`).

Files are checked against types/moonframe.d.luau, except those of an app
outside app/ whose own providers declare types (`providers/*/types.luau`):
each such app is checked against definitions that add its providers' types
(scripts/provider-types.py --app), so its mods read and message them typed.
Prints what luau-lsp reports and fails when it reports anything.

    scripts/luau-types.py LUAU_LSP DIR
"""

import hashlib
import importlib.util
import pathlib
import subprocess
import sys

DEFINITIONS = pathlib.Path("types/moonframe.d.luau")
BUILT_IN = pathlib.Path("app")
OUT = pathlib.Path("target/provider-types")

# The sibling script is loaded by path; leave no __pycache__ beside it.
sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("provider_types", HERE / "provider-types.py")
provider_types = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(provider_types)


def apps(top: pathlib.Path):
    """Each app under `top` (or `top` itself) with providers that declare
    types, deepest first; the built-in app's are in the definitions already."""
    roots = {path.parent.parent.parent for path in top.glob("**/providers/*/types.luau")}
    if (top / "providers").is_dir() and any((top / "providers").glob("*/types.luau")):
        roots.add(top)
    roots.discard(BUILT_IN)
    return sorted(roots, key=lambda root: len(root.parts), reverse=True)


def definitions_path(root: pathlib.Path) -> pathlib.Path:
    """Keep absolute inputs and ambiguous directory names inside OUT."""
    digest = hashlib.sha256(str(root.resolve()).encode()).hexdigest()
    return OUT / f"{digest}.d.luau"


def analyze(lsp: str, definitions: pathlib.Path, files) -> str:
    if not files:
        return ""
    result = subprocess.run(
        [lsp, "analyze", "--platform=standard", f"--definitions=@moonframe={definitions}", *map(str, files)],
        capture_output=True,
        text=True,
    )
    lines = (result.stdout + result.stderr).splitlines()
    return "\n".join(line for line in lines if not line.startswith("[INFO]"))


def main() -> int:
    lsp, top = sys.argv[1], pathlib.Path(sys.argv[2])
    files = sorted(path for path in top.glob("**/*.luau") if not path.name.endswith(".d.luau"))
    roots = apps(top)
    groups = {root: [] for root in roots}
    rest = []
    for path in files:
        root = next((root for root in roots if root in path.parents), None)
        (groups[root] if root else rest).append(path)

    text = DEFINITIONS.read_text()
    reports = [analyze(lsp, DEFINITIONS, rest)]
    for root, members in sorted(groups.items()):
        found = provider_types.providers([BUILT_IN, root])
        definitions = definitions_path(root)
        definitions.parent.mkdir(parents=True, exist_ok=True)
        definitions.write_text(provider_types.definitions(text, found, str(DEFINITIONS)))
        reports.append(analyze(lsp, definitions, members))
    output = "\n".join(report for report in reports if report)
    if output:
        print(output)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
