#!/usr/bin/env python3
# Cinder's sessions: the coding-agent CLI sessions in ~/.claude/projects, read
# only. Prints one JSON line whenever what it shares changes; reads the app's
# messages, one `{"from": ..., "message": ...}` per line, on stdin. Exits when
# the app closes stdin. It never writes a file.
#
# Output (`app.history`):
#   available, source ("cli" or "sample"), total (sessions found)
#   groups      { name, path, sessions = { id, title, branch, age, pr, gone } },
#               most recent first; `pr` when the session opened a pull request,
#               `gone` when its folder no longer exists
#   transcript  the opened session, or absent:
#               { id, title, project, branch, hidden (earlier turns left out),
#                 turns = { from ("user" or "assistant"),
#                           parts = { kind ("text" or "tools"), text, tools = { name, detail } } } }
#
# Messages: { open = id } opens a transcript, { close = true } drops it,
# { refresh = true } looks again.

import json
import os
import re
import select
import sys
import time

HOME = os.environ.get("HOME", "")
ROOT = HOME + "/.claude/projects"
SAMPLE = os.environ.get("CINDER_SAMPLE", "") != "" or not os.path.isdir(ROOT)
MOST = 60  # sessions listed
TURNS = 150  # turns kept from a transcript, the latest
TEXT = 5000  # characters per text part
TOOLS = 40  # tool calls per group

cache = {}  # path => { mtime, size, meta }
opened = None
total = None
groups = []
shown = {}  # the transcript last read, by session and mtime

# Whitespace, without \x1c to \x1f, which Python's \s also takes.
WS = "\t\n\x0b\f\r \x85\xa0  -     　"
LEADING = re.compile("^[" + WS + "]+")
SPACES = re.compile("[" + WS + "]+")
VISIBLE = re.compile("[^" + WS + "]")
MARKUP = re.compile("^[" + WS + "]*(<|Caveat:)")


def decode(text):
    """A JSON value from bytes or text, or None when it doesn't parse."""
    try:
        return json.loads(text)
    except ValueError:
        return None


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def as_text(value):
    """A JSON scalar as text: whole numbers without ".0", nothing for null."""
    if value is None:
        return ""
    if isinstance(value, bool):
        return "1" if value else "0"
    if isinstance(value, float) and value == int(value):
        return str(int(value))
    return str(value)


def truthy(value):
    """True unless null, false, 0, "" or "0"."""
    return value not in (None, False, 0, "", "0")


def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def clip(text, most):
    # A number short enough is shared as the number it was.
    shown = as_text(text)
    if len(shown) > most:
        return shown[: most - 1] + "…"
    if is_number(text):
        return int(text) if text == int(text) else text
    return shown


def one_line(text):
    if is_number(text):
        return clip(text, 90)
    text = LEADING.sub("", as_text(text))
    return clip(SPACES.sub(" ", text), 90)


def typed(entry):
    """The text a person typed, or None for tool results, commands and reminders."""
    if truthy(entry.get("isSidechain")) or truthy(entry.get("isMeta")):
        return None
    message = entry.get("message")
    content = message.get("content") if isinstance(message, dict) else None
    text = None
    if isinstance(content, list):
        for block in content:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_result":
                return None
            if block.get("type") == "text":
                text = block.get("text")
                break
    elif not isinstance(content, (dict, bool)):
        text = content
    if isinstance(text, (dict, list, bool)) or text is None:
        return None
    if not VISIBLE.search(as_text(text)) or MARKUP.search(as_text(text)):
        return None
    return text


def basename(path):
    """The last part of a path, trailing slashes aside, as File::Basename gives it."""
    stripped = path.rstrip("/")
    if stripped == "":
        return "/" if path else ""
    return stripped.rsplit("/", 1)[-1]


def project_of(cwd, folder):
    """Where a session ran, as a repository name: worktrees go under their repo."""
    if cwd:
        found = re.search(r"/\.herdr/worktrees/([^/]+)/", cwd)
        if found:
            return found.group(1)
        return basename(re.sub(r"/\.claude/worktrees/[^/]+/?$", "", cwd, count=1))
    name = re.sub(r".*-", "", basename(folder), count=1)
    return name if name not in ("", "0") else "unknown"


def age(mtime):
    seconds = int(time.time()) - mtime
    if seconds < 60:
        return "now"
    if seconds < 3600:
        return "%dm" % (seconds // 60)
    if seconds < 86400:
        return "%dh" % (seconds // 3600)
    if seconds < 86400 * 30:
        return "%dd" % (seconds // 86400)
    return "%dmo" % (seconds // (86400 * 30))


def unescape(text):
    """A JSON string's value from a match of its escaped bytes."""
    if text is None:
        return None
    value = decode(b'"' + text + b'"')
    return value if isinstance(value, str) else None


def line_at(data, at):
    """The line of `data` holding position `at`."""
    end = data.find(b"\n", at)
    return data[at : len(data) if end < 0 else end]


def meta_of(path):
    """A session's title, folder and branch. Transcripts run to tens of
    megabytes and decoding JSON is the slow part, so this reads the file whole
    and searches it, decoding only the few lines it needs."""
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError:
        return None
    ai = summary = first = cwd = branch = None
    head = data[:262144]
    found = re.search(rb'"cwd":"((?:[^"\\]|\\.)*)"', head)
    if found:
        cwd = unescape(found.group(1))
    found = re.search(rb'"gitBranch":"((?:[^"\\]|\\.)*)"', head)
    if found:
        branch = unescape(found.group(1))
    pr = b"gh pr create" in data
    # The latest title the CLI gave it wins.
    at = data.rfind(b'{"type":"ai-title"')
    if at >= 0 and (at == 0 or data[at - 1 : at] == b"\n"):
        entry = decode(line_at(data, at))
        if isinstance(entry, dict):
            ai = entry.get("aiTitle")
    at = data.find(b'{"type":"summary"')
    if ai is None and at >= 0:
        entry = decode(line_at(data, at))
        if isinstance(entry, dict):
            summary = entry.get("summary")
    if ai is None and summary is None:
        # Else the first thing typed, from the first few megabytes.
        for line in data[: 4 << 20].split(b"\n"):
            if not (len(line) < 65536 and b'"type":"user"' in line and b'"tool_result"' not in line):
                continue
            entry = decode(line)
            if isinstance(entry, dict):
                first = typed(entry)
            if first is not None:
                break
    title = next((t for t in (ai, summary, first) if t is not None), None)
    if title is None:
        return None
    return {"title": one_line(title), "cwd": cwd, "branch": branch, "pr": pr}


def scan():
    global total, groups
    if SAMPLE:
        sample()
        return
    files = []
    try:
        folders = os.listdir(ROOT)
    except OSError:
        return
    for folder in folders:
        if folder.startswith("."):
            continue
        try:
            names = os.listdir(ROOT + "/" + folder)
        except OSError:
            continue
        for name in names:
            if not name.endswith(".jsonl"):
                continue
            path = ROOT + "/" + folder + "/" + name
            try:
                info = os.stat(path)
            except OSError:
                continue
            files.append({"path": path, "dir": folder, "mtime": int(info.st_mtime), "size": info.st_size})
    files.sort(key=lambda f: -f["mtime"])
    total = len(files)

    order, by = [], {}
    listed = 0
    for file in files:
        if listed >= MOST:
            break
        hit = cache.get(file["path"])
        if not hit or hit["mtime"] != file["mtime"] or hit["size"] != file["size"]:
            hit = cache[file["path"]] = {"mtime": file["mtime"], "size": file["size"], "meta": meta_of(file["path"])}
        meta = hit["meta"]
        if not meta:
            continue
        name = project_of(meta["cwd"], file["dir"])
        if name not in by:
            order.append(name)
            by[name] = {"name": name, "path": meta["cwd"] if meta["cwd"] is not None else "", "sessions": []}
        base = basename(file["path"])
        by[name]["sessions"].append(
            {
                "id": base[: -len(".jsonl")] or base,
                "title": meta["title"],
                "branch": meta["branch"] if meta["branch"] is not None else "",
                "age": age(file["mtime"]),
                "pr": bool(meta["pr"]),
                "gone": meta["cwd"] is not None and not os.path.isdir(meta["cwd"]),
                "path": file["path"],
            }
        )
        listed += 1
    groups = [by[name] for name in order]


def find_session(wanted):
    for group in groups:
        for session in group["sessions"]:
            if session["id"] == wanted:
                return session, group
    return None, None


def strip_cwd(text, cwd):
    if cwd and text.startswith(cwd + "/"):
        return text[len(cwd) + 1 :]
    return text


def tool_detail(params, cwd):
    if not isinstance(params, dict):
        return ""
    detail = ""
    for key in ("description", "command", "file_path", "path", "pattern", "url", "query", "prompt"):
        if params.get(key) is not None:
            detail = params[key]
            break
    if isinstance(detail, (dict, list, bool)):
        detail = ""
    return one_line(detail if is_number(detail) else strip_cwd(as_text(detail), cwd))


def transcript(wanted):
    session, group = find_session(wanted)
    if not session:
        return None
    if SAMPLE:
        return sample_transcript(session, group)
    try:
        handle = open(session["path"], "rb")
    except OSError:
        return None
    cwd = group["path"]
    turns = []
    with handle:
        for line in handle:
            # Tool results are left out, and decoding them is the slow part.
            if b'"tool_result"' in line and b'"type":"user"' in line:
                continue
            if b'"type":"user"' not in line and b'"type":"assistant"' not in line:
                continue
            assistant = b'"role":"assistant"' in line
            # Thinking alone shows nothing here.
            if (
                assistant
                and b'"type":"thinking"' in line
                and b'"type":"text"' not in line
                and b'"type":"tool_use"' not in line
            ):
                continue
            # A large tool call (a whole file written): its name and a line
            # from its input are enough, without decoding it.
            if (
                assistant
                and len(line) > 16384
                and b'"type":"tool_use"' in line
                and b'"type":"text"' not in line
                and b'"isSidechain":true' not in line
            ):
                found = re.search(rb'"type":"tool_use","id":"[^"]*","name":"([^"]+)"', line)
                name = found.group(1).decode("latin-1") if found else "Tool"
                found = re.search(rb'"(?:description|file_path|command)":"((?:[^"\\]|\\.){0,300})', line)
                if not (turns and turns[-1]["from"] == "assistant"):
                    turns.append({"from": "assistant", "parts": []})
                parts = turns[-1]["parts"]
                if not (parts and parts[-1]["kind"] == "tools"):
                    parts.append({"kind": "tools", "text": "", "tools": []})
                value = unescape(found.group(1) if found else None) or ""
                value = strip_cwd(value, cwd)
                if len(parts[-1]["tools"]) < TOOLS:
                    parts[-1]["tools"].append({"name": name, "detail": one_line(value)})
                continue
            entry = decode(line)
            if not isinstance(entry, dict) or truthy(entry.get("isSidechain")):
                continue
            if entry.get("type") == "user":
                text = typed(entry)
                if text is not None:
                    turns.append({"from": "user", "parts": [{"kind": "text", "text": clip(text, TEXT), "tools": []}]})
                continue
            message = entry.get("message")
            if entry.get("type") != "assistant" or not isinstance(message, dict):
                continue
            content = message.get("content")
            if not isinstance(content, list):
                continue
            if not (turns and turns[-1]["from"] == "assistant"):
                turns.append({"from": "assistant", "parts": []})
            parts = turns[-1]["parts"]
            for block in content:
                if not isinstance(block, dict):
                    continue
                kind = block.get("type")
                if kind == "text" and VISIBLE.search(as_text(block.get("text"))):
                    parts.append({"kind": "text", "text": clip(block["text"], TEXT), "tools": []})
                elif kind == "tool_use":
                    if not (parts and parts[-1]["kind"] == "tools"):
                        parts.append({"kind": "tools", "text": "", "tools": []})
                    tools = parts[-1]["tools"]
                    if len(tools) < TOOLS:
                        name = block.get("name")
                        tools.append(
                            {"name": name if name is not None else "Tool", "detail": tool_detail(block.get("input"), cwd)}
                        )
    turns = [turn for turn in turns if turn["parts"]]
    hidden = len(turns) - TURNS if len(turns) > TURNS else 0
    del turns[:hidden]
    return {
        "id": wanted,
        "title": session["title"],
        "project": group["name"],
        "branch": session["branch"],
        "hidden": hidden,
        "turns": turns,
    }


def sample():
    """Sessions to show when there are none to read (or CINDER_SAMPLE is set)."""
    global total, groups
    count = [0]

    def s(title, branch, pr, gone, when):
        count[0] += 1
        return {
            "id": "sample-%d" % count[0],
            "title": title,
            "branch": branch,
            "age": when,
            "pr": bool(pr),
            "gone": bool(gone),
        }

    groups = [
        {
            "name": "lantern",
            "path": "~/code/lantern",
            "sessions": [
                s("Lantern dashboard and web tasks view", "feat/dashboard", 1, 0, "12m"),
                s("Next-step plan review for the job queue", "main", 0, 0, "1h"),
            ],
        },
        {
            "name": "DragKit",
            "path": "~/code/DragKit",
            "sessions": [
                s("Cmd-click drag not working on Sonoma", "fix/cmd-drag", 0, 0, "3h"),
            ],
        },
        {
            "name": "tideline",
            "path": "~/code/tideline",
            "sessions": [
                s("Phase 2 string catalogue migration", "i18n/phase-2", 0, 0, "5h"),
                s("tideline-usage catalogue migration", "i18n/usage", 0, 0, "6h"),
                s("Activity bar translations", "i18n/activity", 0, 0, "8h"),
                s("I18N.md phase 2 plan", "main", 0, 0, "9h"),
                s("Multilanguage support for the renderer", "i18n/core", 0, 0, "1d"),
                s("Merge main and resolve conflicts in the store", "merge/main", 1, 1, "1d"),
                s("Merge main and fix the lint score", "merge/lint", 1, 1, "2d"),
                s("PR #904 mods and layouts", "feat/layouts", 0, 0, "2d"),
                s("New release push", "release/1.8", 0, 0, "3d"),
                s("tideline Windows daemon connection", "fix/windows", 0, 0, "4d"),
            ],
        },
    ]
    total = 10


def sample_transcript(session, group):
    def tools(*calls):
        return {"kind": "tools", "text": "", "tools": [{"name": name, "detail": detail} for name, detail in calls]}

    def text(body):
        return {"kind": "text", "text": body, "tools": []}

    return {
        "id": session["id"],
        "title": session["title"],
        "project": group["name"],
        "branch": session["branch"],
        "hidden": 0,
        "turns": [
            {
                "from": "user",
                "parts": [text(session["title"] + ". Start by reading how it works today, then propose the change.")],
            },
            {
                "from": "assistant",
                "parts": [
                    text("I'll look at the current code first."),
                    tools(("Bash", "List the source tree"), ("Read", "src/app.rs"), ("Grep", "fn render")),
                    text(
                        "Here is what I found:\n\n- The view rebuilds its whole tree on every change.\n"
                        "- The store keeps one list per window.\n\nI'd split the list out and keep it in the "
                        "store, then render it lazily:\n\n```rust\nlet rows = store.rows(window);\n"
                        "list.set_count(rows.len());\n```\n\nShall I go ahead?"
                    ),
                ],
            },
            {"from": "user", "parts": [text("Yes, go ahead and run the tests after.")]},
            {
                "from": "assistant",
                "parts": [
                    tools(("Edit", "src/store.rs"), ("Edit", "src/app.rs"), ("Bash", "Run the tests")),
                    text("Done: the list lives in the store now, and all 214 tests pass."),
                ],
            },
        ],
    }


def publish():
    global shown
    data = {
        "available": True,
        "source": "sample" if SAMPLE else "cli",
        "total": total if total is not None else 0,
        "groups": [
            dict(group, sessions=[{k: v for k, v in s.items() if k != "path"} for s in group["sessions"]])
            for group in groups
        ],
    }
    if opened is not None:
        # Read again only when the session's file changed.
        session, _ = find_session(opened)
        key = ""
        if session:
            mtime = 0
            if session.get("path"):
                try:
                    mtime = int(os.stat(session["path"]).st_mtime)
                except OSError:
                    mtime = 0
            key = "%s:%d" % (opened, mtime)
        if shown.get("key") != key:
            shown = {"key": key, "transcript": transcript(opened)}
        if shown["transcript"]:
            data["transcript"] = shown["transcript"]
    return encode(data)


def receive(line):
    global opened
    value = decode(line)
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    wanted = msg.get("open")
    if isinstance(wanted, (int, float, str)) and not isinstance(wanted, bool):
        wanted = as_text(wanted)
    else:
        wanted = None
    if wanted is not None and re.match(r"^[A-Za-z0-9_-]{1,80}$", wanted):
        opened = wanted
    elif truthy(msg.get("close")):
        opened = None


def main():
    scan()
    last = ""
    buffer = b""
    stdin = sys.stdin.fileno()
    scanned = time.time()
    while True:
        text = publish()
        if text != last:
            sys.stdout.buffer.write(text.encode("utf-8", "replace") + b"\n")
            sys.stdout.buffer.flush()
            last = text
        readable, _, _ = select.select([stdin], [], [], 30)
        if readable:
            got = os.read(stdin, 65536)
            if not got:
                sys.exit(0)
            buffer += got
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                receive(line)
        if time.time() - scanned >= 30:
            scan()
            scanned = time.time()


if __name__ == "__main__":
    main()
