#!/usr/bin/env python3
"""Types each provider's `app.<name>` and `provider:<name>` messages.

A provider declares its types in `types.luau` beside its `provider.toml`:
`export type Published` (what `app.<name>` holds) and, optionally,
`export type Message` (what `send "provider:<name>"` takes). Writes, between
the markers in types/moonframe.d.luau, every alias of each app/providers/
<name>/types.luau, renamed `Provider<Name><Alias>`, an `app.<name>` field
for each `Published`, and a `send` overloaded on `"provider:<name>"` for each
`Message`. A provider without types keeps `app.<name>` as `any`.

Where a provider keeps `published.json`, an example of what it publishes,
`--check` also holds that file's top level to `Published`: every key
declared, every field not optional present, each of the kind declared. The
provider's own tests hold its real output to the same file, so neither side
can drift from the other unnoticed.

`--check` fails when the definitions are out of date or an example does not
match. scripts/luau-types.py calls `definitions` to type an app outside app/
with its own providers too.
"""

import argparse
import importlib.util
import json
import pathlib
import re
import sys

ALIASES_START = "-- generated: provider types (just provider-types) start\n"
ALIASES_END = "-- generated: provider types end\n"
FIELDS_START = "\t-- generated: provider fields (just provider-types) start\n"
FIELDS_END = "\t-- generated: provider fields end\n"

# The sibling script is loaded by path; leave no __pycache__ beside it.
sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("component_types", HERE / "component-types.py")
component_types = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(component_types)


def pascal(name: str) -> str:
    return component_types.pascal(name)


def providers(roots):
    """Each provider with a types.luau, by name; a later root wins."""
    found = {}
    for root in roots:
        for path in sorted((root / "providers").glob("*/types.luau")):
            found[path.parent.name] = path.parent
    return dict(sorted(found.items()))


def declared(folder: pathlib.Path):
    """types.luau's aliases as {name: (generics, body)}, comments kept for
    the editor's hover."""
    source = (folder / "types.luau").read_text()
    return {name: (generics, body) for name, generics, body in component_types.aliases(source)}


def generate(found):
    """The aliases section and the fields section."""
    types, fields, overloads = [], [], []
    for name, folder in found.items():
        aliases = declared(folder)
        if "Published" not in aliases:
            raise SystemExit(f"{folder}/types.luau: no `export type Published`")
        prefix = "Provider" + pascal(name)
        local = re.compile(r"\b(" + "|".join(re.escape(alias) for alias in aliases) + r")\b")
        for alias, (generics, body) in aliases.items():
            body = local.sub(lambda m: prefix + m.group(1), body)
            # A body led by a comment starts on a line of its own.
            space = "\n\t" if body.startswith("--") else " "
            types.append(f"type {prefix}{alias}{generics} ={space}{body}\n")
        key = name if re.fullmatch(r"[A-Za-z_]\w*", name) else f'["{name}"]'
        fields.append(f"\t{key}: {prefix}Published?,\n")
        if "Message" in aliases:
            overloads.append(f'((to: "provider:{name}") -> (msg: {prefix}Message) -> Send)')
    overloads.append("((to: string) -> (msg: Data) -> Send)")
    aliases_text = (
        "".join(types)
        + "\n-- A message for `update` to return: `send \"provider:desk\" { watch = id }`\n"
        + "-- is `{ to = \"provider:desk\", msg = { watch = id } }`, its `msg` typed\n"
        + "-- by the provider's `Message` where it declares one.\n"
        + "declare send:\n\t"
        + "\n\t& ".join(overloads)
        + "\n"
    )
    return aliases_text, "".join(fields)


def replace(text: str, start: str, end: str, body: str, where: str) -> str:
    if start not in text or end not in text:
        raise SystemExit(f"{where}: missing the markers {start.strip()!r}")
    before, rest = text.split(start, 1)
    _, after = rest.split(end, 1)
    return before + start + body + end + after


def definitions(text: str, found, where: str) -> str:
    aliases_text, fields_text = generate(found)
    text = replace(text, ALIASES_START, ALIASES_END, aliases_text, where)
    return replace(text, FIELDS_START, FIELDS_END, fields_text, where)


# What published.json is checked against: the top level of `Published`.


def split_top(body: str, separators: str):
    """`body` cut at each of `separators` outside brackets and strings."""
    parts, depth, start, quote = [], 0, 0, None
    for at, char in enumerate(body):
        if quote:
            quote = None if char == quote else quote
        elif char in "\"'":
            quote = char
        elif char in "{(<[":
            depth += 1
        elif char in "})>]":
            depth -= 1
        elif char in separators and depth == 0:
            parts.append(body[start:at])
            start = at + 1
    parts.append(body[start:])
    return [part.strip() for part in parts if part.strip()]


def kinds(text: str, aliases, seen=()):
    """The JSON kinds a type allows ("nil" among them when optional), or None
    for any."""
    text = text.strip()
    while text.startswith("(") and text.endswith(")"):
        text = text[1:-1].strip()
    if text.endswith("?"):
        inner = kinds(text[:-1], aliases, seen)
        return None if inner is None else inner | {"nil"}
    members = split_top(text, "|")
    if len(members) > 1:
        found = set()
        for member in members:
            kind = kinds(member, aliases, seen)
            if kind is None:
                return None
            found |= kind
        return found
    if text in ("string", "number", "boolean", "nil"):
        return {text}
    if text in ("true", "false"):
        return {"boolean"}
    if text[:1] in "\"'":
        return {"string"}
    if text.startswith("{"):
        return {"table"}
    if text in aliases and text not in seen:
        return kinds(aliases[text][1], aliases, seen + (text,))
    return None


def fields(body: str):
    """A table type's fields as {name: type}, and whether an indexer lets
    other keys in."""
    body = body.strip()
    if not (body.startswith("{") and body.endswith("}")):
        return None, False
    found, open_keys = {}, False
    for entry in split_top(body[1:-1], ",;\n"):
        key, _, kind = entry.partition(":")
        key = key.strip()
        if key.startswith("["):
            open_keys = True
        elif key.startswith("read ") or key.startswith("write "):
            found[key.split()[1]] = kind
        else:
            found[key] = kind
    return found, open_keys


def json_kind(value) -> str:
    if value is None:
        return "nil"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)):
        return "number"
    if isinstance(value, str):
        return "string"
    return "table"


def mismatches(name: str, folder: pathlib.Path):
    """How published.json differs from `Published`, a line each."""
    example = folder / "published.json"
    if not example.exists():
        return []
    aliases = declared(folder)
    aliases = {name: (generics, component_types.strip_comments(body)) for name, (generics, body) in aliases.items()}
    declared_fields, open_keys = fields(aliases["Published"][1])
    if declared_fields is None:
        return [f"{name}: `Published` must be a table type to check published.json"]
    value = json.loads(example.read_text())
    if not isinstance(value, dict):
        return [f"{name}: published.json must be an object"]
    problems = []
    for key, kind in declared_fields.items():
        allowed = kinds(kind, aliases)
        if key not in value:
            if allowed is not None and "nil" not in allowed:
                problems.append(f"{name}: `{key}` is declared but published.json has none")
            continue
        found = json_kind(value[key])
        if allowed is not None and found not in allowed:
            problems.append(f"{name}: `{key}` is a {found} in published.json, declared `{kind.strip()}`")
    for key in value:
        if key not in declared_fields and not open_keys:
            problems.append(f"{name}: published.json has `{key}`, which `Published` does not declare")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-dir", default="app", type=pathlib.Path)
    parser.add_argument("--definitions", default="types/moonframe.d.luau", type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--examples", nargs="*", default=[pathlib.Path("examples"), pathlib.Path("patterns")], type=pathlib.Path)
    args = parser.parse_args()

    text = args.definitions.read_text()
    updated = definitions(text, providers([args.app_dir]), str(args.definitions))
    if not args.check:
        if updated != text:
            args.definitions.write_text(updated)
        return 0
    problems = []
    if updated != text:
        problems.append(f"{args.definitions} is out of date: run `just provider-types`")
    # Every provider with types, here and in the example apps.
    for root in (args.app_dir, *args.examples):
        for folder in sorted({path.parent for path in root.glob("**/providers/*/types.luau")}):
            problems += mismatches(str(folder), folder)
    for problem in problems:
        print(problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
