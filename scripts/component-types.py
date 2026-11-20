#!/usr/bin/env python3
"""Types each component's `use "name"` from the component's own `type Props`.

Writes, between the markers in types/moonframe.d.luau, every top-level type
alias of each app/components/<name>.luau, renamed `Use<Name><Alias>` so they
cannot clash, and a `use` overloaded on the name. A name with no component
falls back to `(props: any) -> Element`, so an app's own components still
check. `--check` fails when the file is out of date instead of writing it.
"""

import argparse
import pathlib
import re
import sys

START = "-- generated: component props (just component-types) start\n"
END = "-- generated: component props end\n"
HEAD = re.compile(r"^(?:export\s+)?type\s+(\w+)(<[^>]*>)?\s*=", re.M)


def pascal(name: str) -> str:
    return "".join(part.capitalize() for part in re.split(r"[-_]", name))


def aliases(source: str):
    """Each top-level type alias as (name, generics, body)."""
    for match in HEAD.finditer(source):
        at, depth = match.end(), 0
        while at < len(source):
            char = source[at]
            if source.startswith("--", at):
                newline = source.find("\n", at)
                at = newline if newline >= 0 else len(source)
                continue
            if char in "{(<":
                depth += 1
            elif char in "})>":
                depth -= 1
            elif char == "\n" and depth == 0:
                # A union or intersection may go on over more lines.
                if not source[at + 1 :].lstrip(" \t").startswith(("|", "&")):
                    break
            at += 1
        yield match.group(1), match.group(2) or "", source[match.end() : at].strip()


def strip_comments(body: str) -> str:
    lines = (re.sub(r"\s*--.*$", "", line) for line in body.split("\n"))
    return "\n".join(line for line in lines if line.strip())


def generate(components: pathlib.Path) -> str:
    types, overloads = [], []
    for path in sorted(components.glob("*.luau")):
        found = list(aliases(path.read_text()))
        if not any(name == "Props" for name, _, _ in found):
            continue
        prefix = "Use" + pascal(path.stem)
        local = re.compile(r"\b(" + "|".join(re.escape(name) for name, _, _ in found) + r")\b")
        for name, generics, body in found:
            # Only Props itself is typed. The old solver holds a list's
            # records to exactly the declared shape, so a list built in a
            # variable or sent by a provider (a GitCommit with `time`) would
            # fail on its extra or missing fields; the records it names are
            # `any` until the new solver checks them by width.
            if name != "Props":
                body = "any"
            body = local.sub(lambda m: prefix + m.group(1), strip_comments(body))
            types.append(f"type {prefix}{name}{generics} = {body}\n")
        overloads.append(f'((name: "{path.stem}") -> (props: {prefix}Props) -> Element)')
    overloads.append("((name: string) -> (props: any) -> Element)")
    return (
        "".join(types)
        + "\n-- The component `name`'s `build`: `local stat = use \"stat\"`, then\n"
        + "-- `stat { value = \"12%\" }` wherever an element goes.\n"
        + "declare use:\n\t"
        + "\n\t& ".join(overloads)
        + "\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--components", default="app/components", type=pathlib.Path)
    parser.add_argument("--definitions", default="types/moonframe.d.luau", type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    text = args.definitions.read_text()
    if START not in text or END not in text:
        print(f"{args.definitions}: missing the generated section's markers", file=sys.stderr)
        return 1
    before, rest = text.split(START, 1)
    _, after = rest.split(END, 1)
    updated = before + START + generate(args.components) + END + after
    if updated == text:
        return 0
    if args.check:
        print(f"{args.definitions} is out of date: run `just component-types`", file=sys.stderr)
        return 1
    args.definitions.write_text(updated)
    return 0


if __name__ == "__main__":
    sys.exit(main())
