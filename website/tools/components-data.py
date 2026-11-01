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
        }
    )

with open("website/site/data/components.js", "w", encoding="utf-8") as out:
    out.write("window.COMPONENTS = " + json.dumps(components, ensure_ascii=False) + ";\n")
print(f"{len(components)} components")
