#!/usr/bin/env python3
"""Types each component's `use "name"` from the component's own `type Props`.

Writes, between the markers in types/moonframe.d.luau, every top-level type
alias of each app/components/<name>.luau, renamed `Use<Name><Alias>` so they
cannot clash, a `UseProps` table from each name to its Props, and a `use`
that looks the name up in it. A name with no component falls back to
`(props: any) -> Element`, so an app's own components still check.
`--check` fails when the file is out of date instead of writing it.

One generic `use` rather than an overload per name: Luau's new solver gives
up ("Code is too complex to typecheck") on a function of more than eight
overloads, and type functions are the new solver's alone.
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
                # A union or intersection may go on over more lines, and
                # comments may come before or between its members.
                ahead = re.sub(r"^(?:[ \t]*(?:--[^\n]*)?\n)*", "", source[at + 1 :])
                started = strip_comments(source[match.end() : at]).strip()
                if started and not ahead.lstrip(" \t").startswith(("|", "&")):
                    break
            at += 1
        yield match.group(1), match.group(2) or "", source[match.end() : at].strip()


def strip_comments(body: str) -> str:
    lines = (re.sub(r"\s*--.*$", "", line) for line in body.split("\n"))
    return "\n".join(line for line in lines if line.strip())


def read_only(body: str) -> str:
    """Marks the named fields of a Props table and of the tables in it, and
    its own indexer, `read`.

    A component only reads its props, and the new solver holds a mutable
    field to exactly its declared type: a table built in a variable with
    `inline = true` would not fit `inline: boolean?`. Read-only fields take
    any subtype, so they check as a literal in the call does. Nested lists
    and maps stay mutable: a list built in the call would be inferred
    read-only and then fail where the caller passes it on.
    """
    out, stack, at = [], [], 0
    field = re.compile(r"\s*(?:\[|[A-Za-z_]\w*\s*:)")
    while at < len(body):
        char = body[at]
        out.append(char)
        if char in "{([":
            stack.append(char)
        elif char in "})]":
            stack.pop()
        if stack and stack[-1] == "{" and char in "{,\n":
            match = field.match(body, at + 1)
            rest = body[at + 1 :].lstrip()
            gap = len(body) - at - 1 - len(rest)
            indexer = rest.startswith("[")
            if match and not rest.startswith("read ") and (len(stack) == 1 or not indexer):
                out.append(body[at + 1 : at + 1 + gap] + "read ")
                at += gap
        at += 1
    return "".join(out)


def generate(components: pathlib.Path) -> str:
    types, names = [], []
    for path in sorted(components.glob("*.luau")):
        found = list(aliases(path.read_text()))
        if not any(name == "Props" for name, _, _ in found):
            continue
        prefix = "Use" + pascal(path.stem)
        local = re.compile(r"\b(" + "|".join(re.escape(name) for name, _, _ in found) + r")\b")
        for name, generics, body in found:
            # Only Props itself is typed. A list's records would need
            # read-only fields and indexers (`{ read [number]: { read id:
            # string } }`) to take a record with extra fields, such as a
            # GitCommit with `time`: list items are invariant otherwise.
            # The records it names are `any` until the generator writes them so.
            body = "any" if name != "Props" else read_only(strip_comments(body))
            body = local.sub(lambda m: prefix + m.group(1), body)
            types.append(f"type {prefix}{name}{generics} = {body}\n")
        names.append(f'\t["{path.stem}"]: {prefix}Props,\n')
    return (
        "".join(types)
        + "\n-- Each component's Props by its name.\n"
        + "type UseProps = {\n"
        + "".join(names)
        + "}\n"
        + "\n-- The Props of the component `name`, or `any` for a name it does not\n"
        + "-- know (an app's own components) or one only known at run time.\n"
        + "type function UseOf(name)\n"
        + '\tif name:is("singleton") then\n'
        + "\t\tlocal props = UseProps:readproperty(name)\n"
        + "\t\tif props then\n"
        + "\t\t\treturn props\n"
        + "\t\tend\n"
        + "\tend\n"
        + "\treturn types.any\n"
        + "end\n"
        + "\n-- The component `name`'s `build`: `local stat = use \"stat\"`, then\n"
        + "-- `stat { value = \"12%\" }` wherever an element goes. The `\"\"` in the\n"
        + "-- bound keeps the name's literal type (a bound of only `string` widens\n"
        + "-- it), so `UseOf` can look it up; `keyof<UseProps>` there would do the\n"
        + "-- same but is too complex for the solver at every call.\n"
        + "declare function use<K>(name: K & (\"\" | string)): (props: UseOf<K>) -> Element\n"
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
