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
#               identifies an opening across provider restarts too.
#   folder      where the files are
#   hidden      documents left out of `documents` past MAX_DOCUMENTS (the
#               oldest), so the line stays under the host's limit
#
# A document over MAX_TEXT bytes opens with `too_large = true` and no text:
# its text would not fit the line the host reads.
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
# trash is its `.trash` folder; favorites are kept in `.favorites`. Each
# edit is journaled in `.pending` before publishing, then recovered on
# startup if the host stopped the process before its debounced write.

import fcntl
import glob
import json
import os
import re
import select
import signal
import sys
import tempfile
import time
import uuid

signal.signal(signal.SIGPIPE, signal.SIG_DFL)

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
_profile = re.sub(rb"[^A-Za-z0-9_-]", b"-", os.environb.get(b"MOONFRAME_PROFILE", b"folio"))
DATA = (_xdg if _xdg.startswith("/") else HOME + "/.local/share") + "/moonframe/" + _profile.decode()
FOLDER = DATA + "/documents"
TRASH = FOLDER + "/.trash"
FAVORITES = FOLDER + "/.favorites"
LEGACY_JOURNAL = FOLDER + "/.pending"  # one shared file, before runs had their own
JOURNALS = FOLDER + "/.journals"
SESSION = uuid.uuid4().hex
# Each run journals its edit in a file of its own, and holds a lock beside
# it for as long as it runs: a journal whose lock can be taken belongs to a
# run that has stopped, and is the only kind another run recovers.
JOURNAL = JOURNALS + "/" + SESSION + ".json"
LOCK = JOURNALS + "/" + SESSION + ".lock"
NAME_MAX = 81  # characters in a document's name, as ID allows
QUIET = 0.6  # seconds without a keystroke before an edit is written
LOOK = 30  # seconds between looks at the folder, for files changed elsewhere
EXCERPT = 140  # characters of a document shown under its title
MAX_DOCUMENTS = 500  # newest documents listed; the rest are counted in `hidden`
MAX_TEXT = 256 * 1024  # bytes of the open document sent; larger ones aren't
MAX_LINE = 900 * 1024  # bytes of one output line; the host refuses 1 MiB
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
            body = pending[1] if pending and pending[0] == path else read(path)
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


def free(doc, trashed):
    # `doc` itself if no file by that name is in the folder it moves to,
    # else "doc (2)", "doc (3)"…, so a move never replaces another file;
    # the name is shortened to keep the suffix within NAME_MAX.
    name, n = doc, 2
    while os.path.exists(path_of(name, trashed)):
        suffix = " (%d)" % n
        name, n = doc[: NAME_MAX - len(suffix)].rstrip() + suffix, n + 1
    return name


def move(doc, found, trashed):
    # Moves a document into or out of the trash, renamed if it must be, and
    # moves its star with it; the open document follows its file. Returns
    # the name it has now.
    global current_path
    name = free(doc, trashed)
    os.replace(found[0], path_of(name, trashed))
    if current and current_path == found[0]:
        current["id"] = name
        current_path = path_of(name, trashed)
    stars = favorites()
    if name != doc and doc in stars:
        save_favorites((stars - {doc}) | {name})
    return name


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
current_path = None  # the open document's file: saves go here, never by name
opened = 0
pending = None  # (path, text) not written yet
typed = 0.0  # when the last edit came


def atomic_write(path, body):
    # Replacing a complete, synced file leaves either the previous text or
    # the new text if the process is killed, never a truncated document.
    fd, temporary = tempfile.mkstemp(prefix=".folio-", dir=os.path.dirname(path))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(body)
            f.flush()
            os.fsync(f.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.remove(temporary)


def inside(path):
    # Whether a journal's path is a document of this library.
    if not isinstance(path, str) or not path.endswith(".md"):
        return False
    real = os.path.realpath(path)
    return os.path.dirname(real) in (os.path.realpath(FOLDER), os.path.realpath(TRASH))


def replay(journal):
    # Writes a journaled edit to its document if that still exists: a
    # deleted document is deliberately not recreated.
    try:
        with open(journal, encoding="utf-8") as f:
            saved = json.load(f)
    except (OSError, ValueError):
        return
    if not isinstance(saved, dict) or not isinstance(saved.get("text"), str):
        return
    path = saved.get("path")
    if path is None and saved.get("id") is not None:
        found = where(saved["id"])  # a journal from before paths were kept
        path = found[0] if found else None
    if inside(path) and os.path.isfile(path):
        atomic_write(path, saved["text"])


def gone(path):
    try:
        os.remove(path)
    except FileNotFoundError:
        pass


lock = None  # this run's lock, held open while it runs


def recover():
    # Takes this run's lock, then replays the journals of runs that stopped
    # before writing their last edit, leaving those of live runs alone.
    global lock
    os.makedirs(JOURNALS, exist_ok=True)
    lock = open(LOCK, "w")
    fcntl.flock(lock, fcntl.LOCK_EX)
    if os.path.isfile(LEGACY_JOURNAL):
        replay(LEGACY_JOURNAL)
        gone(LEGACY_JOURNAL)
    for other in glob.glob(JOURNALS + "/*.lock"):
        if other == LOCK:
            continue
        with open(other, "a") as held:
            try:
                fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                continue  # its run is still going
            journal = other[: -len(".lock")] + ".json"
            if os.path.isfile(journal):
                replay(journal)
            gone(journal)
            gone(other)


def flush():
    global pending
    if pending is None:
        return
    path, body = pending
    if os.path.isfile(path):
        atomic_write(path, body)
    # Keep recovery available until the document has been replaced. A
    # deleted document is deliberately not recreated by journal replay.
    gone(JOURNAL)
    pending = None


def open_doc(doc):
    global current, current_path, opened
    flush()
    found = where(doc)
    if not found:
        return
    opened += 1
    current_path = found[0]
    current = {"id": doc, "opened": "%s-%d" % (SESSION, opened)}
    try:
        size = os.path.getsize(found[0])
    except OSError:
        size = 0
    if size > MAX_TEXT:
        current["too_large"] = True
    else:
        current["text"] = read(found[0])


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
        # An edit of the open document goes to its file, whatever it is
        # called now; one from an earlier opening is dropped.
        save = msg["save"]
        body, token = save.get("text"), save.get("opened")
        mine = current is not None and (
            token == current["opened"] if token is not None else save.get("id") == current["id"]
        )
        if isinstance(body, str) and mine and os.path.isfile(current_path):
            if pending and pending[0] != current_path:
                flush()
            atomic_write(JOURNAL, encode({"path": current_path, "text": body}))
            pending = (current_path, body)
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
        if os.path.dirname(current_path) == FOLDER and os.path.isfile(current_path):
            move(current["id"], (current_path, False), True)
            current = None
    elif "trash" in msg or "restore" in msg:
        doc = msg.get("trash", msg.get("restore"))
        # Each looks only in the folder it moves out of: the same name may
        # be in both.
        trashed = "restore" in msg
        found = where(doc)
        if found and found[1] != trashed and isinstance(doc, str) and os.path.isfile(path_of(doc, trashed)):
            found = (path_of(doc, trashed), trashed)
        if found and found[1] == trashed:
            flush()
            was_open = current is not None and current_path == found[0]
            move(doc, found, "trash" in msg)
            if "trash" in msg and was_open:
                current = None
    elif "purge" in msg:
        # Only ever from the trash, even when a live document shares the name.
        doc = msg["purge"]
        if where(doc) and os.path.isfile(path_of(doc, True)):
            flush()
            os.remove(path_of(doc, True))
            if current and not os.path.isfile(current_path):
                current = None


def out():
    documents = listing()
    if current is None and documents:
        open_doc(next((d["id"] for d in documents if not d["trashed"]), documents[0]["id"]))
    shown = documents[:MAX_DOCUMENTS]
    return {
        "documents": shown,
        "hidden": len(documents) - len(shown),
        "current": current,
        "folder": FOLDER,
    }


def line_of(value):
    # The output line, kept under MAX_LINE: a text that escapes to more
    # than fits (quotes and newlines double) is left out like a large one.
    line = encode(value)
    if len(line.encode("utf-8")) > MAX_LINE and value["current"] and "text" in value["current"]:
        current.pop("text", None)
        current["too_large"] = True
        line = encode(value)
    return line


def main():
    ensure()
    recover()
    last = ""
    buffer = b""
    stdin = sys.stdin.fileno()
    while True:
        if pending and time.time() - typed >= QUIET:
            flush()
        line = line_of(out())
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
