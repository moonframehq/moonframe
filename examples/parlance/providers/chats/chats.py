#!/usr/bin/env python3
# Parlance's conversations, kept in a JSON file, and its scripted demo agent.
# Prints one JSON line whenever what the window shows changes; reads the
# app's messages, one `{"from": ..., "message": ...}` per line, on stdin.
# Exits when the app closes stdin.
#
# There is no model here. A message gets one of a few prepared markdown
# answers, picked by its words and streamed a few words at a time, so the
# window shows a chat working end to end. A real agent would be a provider
# like this one that streams a model's tokens instead.
#
# Output (`app.chats`):
#   current    the open chat's id, or null for a new chat
#   chat       { id, title, messages = { { role, text } } } (the open one)
#   streaming  true while an answer is being written
#   pinned     { id, title }: pinned chats
#   folders    { id, title, chats = { id, title } }: pinned folders
#   projects   { id, title, chats = { id, title } }
#   recents    { id, title, status? }
#   dot        the count beside "Your dot"
#
# Messages (one key each):
#   open = id             open a chat
#   new = true            start a new chat (made on the first message)
#   send = text           the user's message; the demo answers it
#   stop = true           stop the answer being written
#   regenerate = true     write the last answer again
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/chats.json (by default under
# ~/.local/share), started from sample.json the first time.

import json
import os
import random
import re
import select
import signal
import sys
import time

# Quiet death when the app stops reading.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
# The profile is cleaned byte by byte, so a non-ASCII name always maps to
# the same folder.
_profile = re.sub(rb"[^A-Za-z0-9_-]", b"-", os.environb.get(b"MOONFRAME_PROFILE", b"parlance"))
DATA = (_xdg if _xdg.startswith("/") else HOME + "/.local/share") + "/moonframe/" + _profile.decode()
FILE = DATA + "/chats.json"


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# An id compared as a string, so 5 and "5" are the same chat.
def key(value):
    if value is None:
        return ""
    if value is True:
        return "1"
    if value is False:
        return "0"
    if isinstance(value, float):
        return "%.15g" % value
    if isinstance(value, (int, str)):
        return str(value)
    return "\0ref"  # a list or table names no chat


def truthy(value):
    if isinstance(value, str):
        return value not in ("", "0")
    if isinstance(value, (list, dict)):
        return True
    return bool(value)


def scalar(value):
    return isinstance(value, (str, int, float)) and not isinstance(value, bool)


store = None


def read_json(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError):
        return None


def load():
    global store
    store = read_json(FILE)
    if not isinstance(store, dict) or not isinstance(store.get("chats"), dict):
        store = read_json("sample.json")
        if not isinstance(store, dict):
            store = {"chats": {}, "pinned": [], "folders": [], "projects": [], "recents": []}
        store["next"] = 1
        save()


def save():
    # Written whole to a temporary file, then renamed over the old one, so a
    # crash never leaves half a file. Pretty and sorted.
    try:
        os.makedirs(DATA, exist_ok=True)
        with open(FILE + ".tmp", "w", encoding="utf-8") as fh:
            fh.write(json.dumps(store, sort_keys=True, ensure_ascii=False, indent=3,
                                separators=(",", " : ")) + "\n")
        os.rename(FILE + ".tmp", FILE)
    except OSError:
        pass


def chat_of(id):
    chat = store["chats"].get(key(id))
    return chat if isinstance(chat, dict) else None


def entry(id):
    chat = chat_of(id)
    if chat is None:
        return None
    row = {"id": id, "title": chat.get("title")}
    status = store.get("status")
    status = status.get(key(id)) if isinstance(status, dict) else None
    if status is not None:
        row["status"] = status
    return row


def entries(ids):
    rows = (entry(id) for id in ([] if ids is None else ids))
    return [row for row in rows if row is not None]


# The answer being written: the chat, the words still to come, and when the
# next ones are due.
stream = None


def out():
    current = store.get("current")
    chat = chat_of(current) if current is not None else None

    def group(groups):
        return [{"id": g.get("id"), "title": g.get("title"), "chats": entries(g.get("chats"))}
                for g in ([] if groups is None else groups)]

    return {
        "available": True,
        "current": current if chat else None,
        "chat": {"id": current, "title": chat.get("title"), "messages": chat.get("messages")}
        if chat else {"id": None, "title": "New chat", "messages": []},
        "streaming": stream is not None,
        "pinned": entries(store.get("pinned")),
        "folders": group(store.get("folders")),
        "projects": group(store.get("projects")),
        "recents": entries(store.get("recents")),
        "dot": 0 if store.get("dot") is None else store["dot"],
    }


# The scripted answers, picked by the first pattern the message matches.
ANSWERS = [
    (re.compile(r"\b(code|rust|function|bug|error|panic|script|regex)\b", re.I), """\
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

Here is a small, safe shape for it:

```rust
fn parse(line: &str) -> Result<Entry, ParseError> {
    let (date, rest) = line.split_once(' ').ok_or(ParseError::NoDate)?;
    Ok(Entry { date: date.parse()?, memo: rest.trim().to_string() })
}
```

A few notes:

- Return a `Result` instead of calling `unwrap`, so one bad line does not stop the run.
- Keep parsing apart from printing; it makes **tests** easy to write.
- `split_once` reads better than indexing into the string.
"""),
    (re.compile(r"\b(name|call|term|word|title)\b", re.I), """\
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

A few directions for the name:

| Name | Why it works |
|---|---|
| Short and plain | Easy to say, easy to search |
| A metaphor | Carries the idea without explaining it |
| A made-up word | Free to own, but needs a story |

My pick: start from the **metaphor** and test it out loud with three people.
"""),
    (re.compile(r"\b(plan|trip|ideas?|list|schedule|week|steps?)\b", re.I), """\
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

A simple plan to start from:

1. **Decide what done looks like** — one sentence you could check.
2. **Block the time** — two short sessions beat one long one.
3. **Start with the smallest step** — it carries the rest.
4. **Review on Friday** — keep what worked, drop one thing.

Want me to turn this into a checklist?
"""),
    (re.compile(r"."), """\
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

Good question. Here is how I would think about it:

- **What you know** — the facts you can check today.
- **What you assume** — write these down; they are where surprises hide.
- **What would change your mind** — decide it before you look.

| Approach | Good for |
|---|---|
| Quick estimate | Deciding whether it is worth more time |
| Careful comparison | Choices you cannot easily undo |

The key point: **pick the approach by how hard the decision is to reverse.**
"""),
]


def answer_for(text):
    for pattern, answer in ANSWERS:
        if pattern.search(text):
            return answer.rstrip("\n")
    return ""


def title_for(text):
    words = [w for w in re.split(r"\s+", text) if w]
    title = re.sub(r"[?.!,:;]+$", "", " ".join(words[:6]))
    return title[:1].title() + title[1:] if title else "New chat"


def begin_answer(id, text):
    global stream
    chat = store["chats"][id]
    chat.setdefault("messages", []).append({"role": "assistant", "text": ""})
    # Words with the spaces after them, so the text grows as it was written.
    pieces = re.findall(r"\s*\S+\s*", answer_for(key(text)))
    # A short pause first, as if it were thinking.
    stream = {"id": id, "pieces": pieces, "due": time.time() + 0.6}


def step():
    global stream
    if stream is None:
        return
    now = time.time()
    if now < stream["due"]:
        return
    chat = chat_of(stream["id"])
    messages = chat.get("messages") if chat else None
    last = messages[-1] if messages else None
    if not (isinstance(last, dict) and last.get("role") == "assistant"):
        stream = None
        return
    pieces = stream["pieces"]
    count = 1 + random.randrange(3)
    while count > 0 and pieces:
        last["text"] = key(last.get("text")) + pieces.pop(0)
        count -= 1
    if pieces:
        stream["due"] = now + 0.03 + random.random() * 0.04
    else:
        last["text"] = key(last.get("text")).rstrip()
        stream = None
        save()


def stop():
    global stream
    if stream is None:
        return
    chat = chat_of(stream["id"])
    messages = chat.get("messages") if chat else None
    if messages and isinstance(messages[-1], dict) and messages[-1].get("role") == "assistant":
        last = messages[-1]
        last["text"] = key(last.get("text")).rstrip()
        if not last["text"]:
            messages.pop()
    stream = None
    save()


def receive(line):
    try:
        value = json.loads(line.decode("utf-8"))
        encode(value).encode("utf-8")  # no lone surrogates
    except ValueError:
        return
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    if scalar(msg.get("open")) and chat_of(msg["open"]):
        if stream is not None and stream["id"] != key(msg["open"]):
            stop()
        store["current"] = key(msg["open"])
    elif truthy(msg.get("new")):
        stop()
        store["current"] = None
    elif truthy(msg.get("stop")):
        stop()
        return
    elif scalar(msg.get("send")) and re.search(r"\S", key(msg["send"])) and stream is None:
        text = key(msg["send"])[:8000].strip()
        id = store.get("current")
        if id is None or chat_of(id) is None:
            number = store.get("next")
            count = key(number)
            store["next"] = (int(count) if re.match(r"-?[0-9]+\Z", count) else 0) + 1
            id = "n" + key(0 if number is None else number)
            store["chats"][id] = {"title": title_for(text), "messages": []}
            store.setdefault("recents", []).insert(0, id)
            store["current"] = id
        id = key(id)
        store["chats"][id].setdefault("messages", []).append({"role": "user", "text": text})
        begin_answer(id, text)
    elif truthy(msg.get("regenerate")) and stream is None:
        id = store.get("current")
        chat = chat_of(id) if id is not None else None
        messages = chat.get("messages") if chat else None
        if not (messages and len(messages) >= 2 and isinstance(messages[-1], dict)
                and messages[-1].get("role") == "assistant"):
            return
        messages.pop()
        asked = messages[-1]
        begin_answer(key(id), asked.get("text") if isinstance(asked, dict) else None)
    else:
        return
    save()


def main():
    load()
    last = ""
    buffer = b""
    stdin = sys.stdin.fileno()
    while True:
        step()
        line = encode(out())
        if line != last:
            sys.stdout.buffer.write(line.encode("utf-8") + b"\n")
            sys.stdout.buffer.flush()
            last = line
        wait = 0.02 if stream is not None else 60
        ready, _, _ = select.select([stdin], [], [], wait)
        if ready:
            try:
                got = os.read(stdin, 65536)
            except OSError:
                got = b""
            if not got:
                sys.exit(0)
            buffer += got
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                receive(line)


main()
