#!/usr/bin/env python3
# Folio's documents: markdown files in a folder, one per document. Prints
# one JSON line whenever what the views show changes; reads the app's
# messages, one `{"message": ...}` per line, on stdin. Exits when the app
# closes stdin.
#
# Output (`app.library`):
#   documents   every document, newest first: { id, title, excerpt, words,
#               modified ("Today, 14:02"), favorite, trashed }
#   current     the open document: { id, text, opened }. `text` is the file
#               as it was when opened (and stays so while it is edited), so
#               the editor never gets its own keystrokes back late; `opened`
#               counts openings, so opening the same one again reloads it.
#   folder      where the files are
#
# Messages (one key each):
#   open = id                        open that document
#   save = { id = id, text = "…" }   its new text, written once typing stops
#   new = true                       a new, empty document, opened
#   favorite = id                    starred or not
#   trash = id | restore = id        into the trash and back out
#   trash_current = true             the open document into the trash
#   purge = id                       deleted for good (from the trash)
#
# The folder is $XDG_DATA_HOME/moonframe/<profile>/documents (by default
# under ~/.local/share), started with a few documents the first time. The
# trash is its `.trash` folder; favorites are kept in `.favorites`.

import json
import os
import re
import select
import signal
import sys
import time

signal.signal(signal.SIGPIPE, signal.SIG_DFL)

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
_profile = re.sub(rb"[^A-Za-z0-9_-]", b"-", os.environb.get(b"MOONFRAME_PROFILE", b"folio"))
DATA = (_xdg if _xdg.startswith("/") else HOME + "/.local/share") + "/moonframe/" + _profile.decode()
FOLDER = DATA + "/documents"
TRASH = FOLDER + "/.trash"
FAVORITES = FOLDER + "/.favorites"
QUIET = 0.6  # seconds without a keystroke before an edit is written
LOOK = 30  # seconds between looks at the folder, for files changed elsewhere
EXCERPT = 140  # characters of a document shown under its title
ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9 _.,'()-]{0,80}\Z")


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def text(value):
    return value if isinstance(value, str) else ""


SAMPLES = {
    "Welcome to Folio": """# Welcome to Folio

Folio is a place to write and nothing else. There is the text, a blinking caret, and the next sentence.

## Focus

Press **⌘D** to keep only the sentence you are writing in full ink; the rest of the page steps back. Press it again for the paragraph, and once more to see everything.

## Typewriter

Press **⌘T** and the line you are writing stays in the middle of the window, like paper in a typewriter. Your eyes stay where they are; the text moves.

## Markdown, as you type

Headings, *emphasis*, **strong words**, `code` and [links](https://example.com) take their shape while you type, and the markup fades so the words stand out.

- Lists keep their bullets quiet
- [ ] So do tasks
> And quotes lean back a little.

Press **⌘R** to read the document as it will look, and **⌘R** again to write.
""",
    "Morning pages": """# Morning pages

Three pages, longhand if you can, typed if you must. Not to write well, but to get the noise out before the day starts.

The kettle was louder than usual. I keep meaning to fix the hinge on the kitchen window, and every morning I notice it again and do nothing.

Today: finish the chapter outline, call the printer about the proofs, and walk before lunch.
""",
    "On short sentences": """# On short sentences

A short sentence is a door. It opens, you walk through, and you are somewhere new.

Long sentences have their place, of course, when a thought needs to unfold slowly, gathering its clauses like a river gathering streams, until it reaches the sea of its conclusion. But they tire the reader.

*Read your draft aloud.* Wherever you run out of breath, cut.
""",
    "Notes for the talk": """# Notes for the talk

1. Open with the story about the lost manuscript
2. Why tools should disappear
3. Three habits:
   - write first, edit later
   - one document per idea
   - keep the screen quiet

> "Simplicity is the ultimate sophistication."

Ask for questions at **minute 25**, not later.
""",
}


def ensure():
    first = not os.path.isdir(FOLDER)
    os.makedirs(TRASH, exist_ok=True)
    if first:
        # Spread the samples' times out, the welcome newest.
        now = time.time()
        for age, (name, body) in enumerate(reversed(list(SAMPLES.items()))):
            path = FOLDER + "/" + name + ".md"
            with open(path, "w", encoding="utf-8") as f:
                f.write(body)
            stamp = now - (len(SAMPLES) - 1 - age) * 3600 * 7
            os.utime(path, (stamp, stamp))


def favorites():
    try:
        with open(FAVORITES, encoding="utf-8") as f:
            return set(line.strip() for line in f if line.strip())
    except OSError:
        return set()


def save_favorites(names):
    with open(FAVORITES, "w", encoding="utf-8") as f:
        f.write("".join(name + "\n" for name in sorted(names)))


def path_of(doc, trashed=False):
    return (TRASH if trashed else FOLDER) + "/" + doc + ".md"


def where(doc):
    # The document's file and whether it is in the trash, or None.
    if not isinstance(doc, str) or not ID.match(doc):
        return None
    for trashed in (False, True):
        if os.path.isfile(path_of(doc, trashed)):
            return path_of(doc, trashed), trashed
    return None


def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def title_of(body, doc):
    for line in body.splitlines():
        line = re.sub(r"^\s{0,3}(#{1,6}\s+|>\s*|[-*+]\s+)", "", line).strip()
        if line:
            return re.sub(r"[*_`]", "", line)[:80]
    return doc


def excerpt_of(body):
    lines = [line for line in body.splitlines() if line.strip()]
    rest = " ".join(re.sub(r"^\s{0,3}(#{1,6}\s+|>\s*|[-*+]\s+|\d+\.\s+)", "", l).strip() for l in lines[1:])
    rest = re.sub(r"[*_`#>]|\[([^\]]*)\]\([^)]*\)", lambda m: m.group(1) or "", rest)
    return rest[:EXCERPT].rstrip() + ("…" if len(rest) > EXCERPT else "")


def when(stamp):
    t = time.localtime(stamp)
    today = time.localtime()
    if t[:3] == today[:3]:
        return time.strftime("Today, %H:%M", t)
    yesterday = time.localtime(time.time() - 86400)
    if t[:3] == yesterday[:3]:
        return time.strftime("Yesterday, %H:%M", t)
    if t.tm_year == today.tm_year:
        return time.strftime("%b %-d", t)
    return time.strftime("%b %-d, %Y", t)


def listing():
    stars = favorites()
    found = []
    for trashed, folder in ((False, FOLDER), (True, TRASH)):
        try:
            names = os.listdir(folder)
        except OSError:
            names = []
        for name in names:
            if not name.endswith(".md") or not ID.match(name[:-3]):
                continue
            path = folder + "/" + name
            try:
                stamp = os.path.getmtime(path)
            except OSError:
                continue
            doc = name[:-3]
            body = pending[1] if pending and pending[0] == doc else read(path)
            found.append((stamp, {
                "id": doc,
                "title": title_of(body, doc),
                "excerpt": excerpt_of(body),
                "words": len(re.findall(r"\w+", body)),
                "modified": when(stamp),
                "favorite": doc in stars,
                "trashed": trashed,
            }))
    found.sort(key=lambda entry: (-entry[0], entry[1]["id"]))
    return [doc for _, doc in found]


def fresh_name():
    taken = set()
    for folder in (FOLDER, TRASH):
        try:
            taken.update(n[:-3] for n in os.listdir(folder) if n.endswith(".md"))
        except OSError:
            pass
    base = time.strftime("Untitled %Y-%m-%d")
    name, n = base, 2
    while name in taken:
        name, n = "%s (%d)" % (base, n), n + 1
    return name


current = None  # { id, text, opened }
opened = 0
pending = None  # (id, text) not written yet
typed = 0.0  # when the last edit came


def flush():
    global pending
    if pending is None:
        return
    doc, body = pending
    pending = None
    found = where(doc)
    if found:
        with open(found[0], "w", encoding="utf-8") as f:
            f.write(body)


def open_doc(doc):
    global current, opened
    flush()
    found = where(doc)
    if not found:
        return
    opened += 1
    current = {"id": doc, "text": read(found[0]), "opened": opened}


def receive(line):
    global pending, typed, current
    try:
        value = json.loads(line)
    except ValueError:
        return
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    if "open" in msg:
        open_doc(msg["open"])
    elif isinstance(msg.get("save"), dict):
        doc, body = msg["save"].get("id"), msg["save"].get("text")
        if isinstance(body, str) and where(doc):
            if pending and pending[0] != doc:
                flush()
            pending = (doc, body)
            typed = time.time()
    elif msg.get("new"):
        flush()
        doc = fresh_name()
        with open(path_of(doc), "w", encoding="utf-8") as f:
            f.write("# ")
        open_doc(doc)
    elif "favorite" in msg and where(msg["favorite"]):
        stars = favorites()
        stars ^= {msg["favorite"]}
        save_favorites(stars)
    elif msg.get("trash_current") and current:
        flush()
        found = where(current["id"])
        if found and not found[1]:
            os.replace(found[0], path_of(current["id"], True))
            current = None
    elif "trash" in msg or "restore" in msg:
        doc = msg.get("trash", msg.get("restore"))
        found = where(doc)
        if found and found[1] != ("trash" in msg):
            flush()
            os.replace(found[0], path_of(doc, "trash" in msg))
            if "trash" in msg and current and current["id"] == doc:
                current = None
    elif "purge" in msg:
        found = where(msg["purge"])
        if found and found[1]:
            os.remove(found[0])
            if current and current["id"] == msg["purge"]:
                current = None


def out():
    documents = listing()
    if current is None and documents:
        open_doc(next((d["id"] for d in documents if not d["trashed"]), documents[0]["id"]))
    return {"documents": documents, "current": current, "folder": FOLDER}


def main():
    ensure()
    last = ""
    buffer = b""
    stdin = sys.stdin.fileno()
    while True:
        if pending and time.time() - typed >= QUIET:
            flush()
        line = encode(out())
        if line != last:
            sys.stdout.buffer.write(line.encode("utf-8") + b"\n")
            sys.stdout.buffer.flush()
            last = line
        wait = max(0.05, QUIET - (time.time() - typed)) if pending else LOOK
        ready, _, _ = select.select([stdin], [], [], wait)
        if ready:
            try:
                got = os.read(stdin, 1 << 20)
            except OSError:
                got = b""
            if not got:
                flush()
                sys.exit(0)
            buffer += got
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                receive(line)


main()
