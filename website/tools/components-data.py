#!/usr/bin/env python3
"""Export app/components/*.luau metadata to website/site/data/components.js.

The Components browser on moonframe.dev reads it: each component's title,
category, icon, description, when, props and example. Run from the repo root:
python3 website/tools/components-data.py
"""
import glob
import json
import os
import re
import struct

SHOTS = "website/site/img/components"


def webp_size(path):
    """A WebP's pixel width and height, read from its header."""
    with open(path, "rb") as image:
        head = image.read(30)
    if head[:4] != b"RIFF" or head[8:12] != b"WEBP":
        raise ValueError(f"{path} is not a WebP")
    chunk = head[12:16]
    if chunk == b"VP8 ":
        width, height = struct.unpack("<HH", head[26:30])
        return width & 0x3FFF, height & 0x3FFF
    if chunk == b"VP8L":
        bits = int.from_bytes(head[21:25], "little")
        return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    if chunk == b"VP8X":
        return int.from_bytes(head[24:27], "little") + 1, int.from_bytes(head[27:30], "little") + 1
    raise ValueError(f"{path}: unknown WebP chunk {chunk!r}")


def shot(name):
    """The preview's size in pixels (2x), so the page can keep its room before it loads."""
    path = os.path.join(SHOTS, name + ".webp")
    return list(webp_size(path)) if os.path.exists(path) else None


def field(source, key):
    match = re.search(r"^\t" + key + r'\s*=\s*"((?:[^"\\]|\\.)*)"', source, re.M)
    return match.group(1).replace('\\"', '"') if match else None


def block(source, key):
    """The balanced `{ ... }` table assigned to a top-level key, as text."""
    match = re.search(r"^\t" + key + r"\s*=\s*\{", source, re.M)
    if not match:
        return None
    start, depth = match.end() - 1, 0
    for index in range(start, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    return None


def usage(source):
    """The component's own `usage = [[...]]` text: the code its Components window copies."""
    match = re.search(r"^\tusage\s*=\s*\[(=*)\[\n?(.*?)\]\1\]", source, re.M | re.S)
    return match.group(2).rstrip() if match else None


def top_level_local(source, name):
    """A top-level `local name ...` or `local function name` statement, as text."""
    match = re.search(r"^local (function )?" + re.escape(name) + r"\b[^\n]*", source, re.M)
    if not match:
        return None
    start, line = match.start(), match.group(0)
    if match.group(1):
        end = re.search(r"^end\b", source[start:], re.M)
        return source[start : start + end.end()] if end else line
    long_string = re.search(r"\[(=*)\[", line)
    if long_string and "]" + long_string.group(1) + "]" not in line[long_string.end() :]:
        end = source.find("]" + long_string.group(1) + "]", start + long_string.end())
        return source[start : end + len(long_string.group(1)) + 2] if end >= 0 else line
    if line.rstrip().endswith("{"):
        end = re.search(r"^\}", source[start:], re.M)
        return source[start : start + end.end()] if end else line
    return line


KEYWORDS = set(
    "and break continue do else elseif end false for function if in local nil not or repeat return then true until while type typeof".split()
)


def needed_locals(source, code, seen=None):
    """Definitions of the file's top-level locals that `code` uses, dependencies first."""
    seen = set() if seen is None else seen
    bare = re.sub(r'\[(=*)\[.*?\]\1\]|"(?:[^"\\]|\\.)*"|`[^`]*`|--[^\n]*', "", code, flags=re.S)
    out = []
    # Names the code reads: not table keys (`name =`), fields (`.name`), or its own locals.
    defined = set(re.findall(r"\blocal (?:function )?([A-Za-z_]\w*)", bare))
    used = re.findall(r"(?<![.:\w])([A-Za-z_]\w*)\b(?!\s*=[^=])", bare)
    for name in dict.fromkeys(used):
        if name in seen or name in defined or name in KEYWORDS:
            continue
        definition = top_level_local(source, name)
        if definition:
            seen.add(name)
            out += needed_locals(source, definition, seen) + [definition]
    return out


components = []
for path in sorted(glob.glob("app/components/*.luau")):
    source = open(path, encoding="utf-8").read()
    props = []
    for match in re.finditer(r'\[?"?([A-Za-z_][\w-]*)"?\]?\s*=\s*"((?:[^"\\]|\\.)*)"', block(source, "props") or ""):
        props.append([match.group(1), match.group(2).replace('\\"', '"')])
    example = block(source, "example")
    if example:
        example = "\n".join(line[1:] if line.startswith("\t") else line for line in example.split("\n"))
    name = os.path.basename(path)[: -len(".luau")]
    # Copyable code: the component's own usage text, or its example with any
    # of the file's locals it needs (helpers like `loose`, tables like EXAMPLE).
    variable = name.replace("-", "_")
    code = usage(source) or f'local {variable} = use "{name}"\n\n{variable} {example or "{}"}'
    helpers = needed_locals(source, code)
    if helpers:
        first, _, rest = code.partition("\n\n")
        code = first + "\n\n" + "".join(h + "\n\n" for h in helpers) + rest
    components.append(
        {
            "name": name,
            "title": field(source, "title") or name,
            "category": field(source, "category") or "Other",
            "icon": field(source, "icon"),
            "description": field(source, "description") or "",
            "when": field(source, "when") or "",
            "props": props,
            "example": example,
            "code": code,
            "shot": shot(name),
        }
    )

with open("website/site/data/components.js", "w", encoding="utf-8") as out:
    out.write("window.COMPONENTS = " + json.dumps(components, ensure_ascii=False) + ";\n")
print(f"{len(components)} components")
