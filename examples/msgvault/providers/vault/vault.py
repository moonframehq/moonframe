#!/usr/bin/env python3
# msgvault's archive for the msgvault app, published as `app.vault`.
#
# Where the data comes from:
# - a msgvault archive, when $MSGVAULT_HOME/msgvault.db (else
#   ~/.msgvault/msgvault.db) exists: the newest messages are read with
#   Python's sqlite3 module, opened read-only (mode=ro, PRAGMA query_only),
#   never written, and read again every minute;
# - else sample.json beside this file: SAMPLE data, a snapshot of
#   msgvault-gpui's demo archive, with times kept relative to now.
#
# What it publishes is shaped for the views: the sidebar's counts, labels
# and accounts, the list of the mailbox in view, and the open thread. Mods
# change the view by message: { mailbox = "inbox" | "label:Work" |
# "kind:whatsapp" | "files" | ... }, { account = id (0 for all) },
# { query = "text" }, { open = message id }, { remote = message id } (load
# its remote pictures), and in the attachments browser { file = id },
# { kind = "image" | ... }, { fquery = true, value = "text" }, { show =
# message id }. The view is kept in ~/.cache/moonframe/msgvault, with the
# HTML pages it writes for the reader. Only the reader and the list may ask
# for { open_file = id } (a temporary copy, opened in the system's app) and
# { save_file = id } (into ~/Downloads), each on a click.
#
# `python3 vault.py --dump` prints a sample.json from the archive it finds
# (msgvault-fixture's demo archive made this one).
#
# Python 3.9 or later, standard library only. This file publishes the view
# and reads the app's messages; the modules beside it hold the rest: values
# (loose JSON values), state (paths and what was read), formatting, archive
# (reading msgvault.db and sample.json), attachments, page (HTML mail) and
# model (mailboxes, the list, the thread).
import json
import os
import re
import select
import sys
import time

# The modules beside this one are imported from its folder (Python puts a
# script's folder first on its path); compiling them would leave a binary
# __pycache__ there, which a package may not hold.
sys.dont_write_bytecode = True

import state
from archive import read_archive, read_thread
from attachments import deliver
from formatting import (SYSTEM, account_icon, display_kind, display_label, kind_icon,
                        label_color, plural, size_text)
from model import (files_view, in_mailbox, load, mailbox_title, matches_query,
                   message_kind, row, thread_view)
from state import cache_dir, db_path, make_dir, view, view_file, write_file
from values import REFRESH, ROWS, encode, has_text, integer, number, text, truthy


# ---- the view ------------------------------------------------------------

def read_view():
    try:
        with open(view_file, "rb") as source:
            saved = json.loads(source.read())
    except (OSError, ValueError):
        return
    if isinstance(saved, dict):
        view.update(saved)


def save_view():
    make_dir(cache_dir)
    write_file(view_file, encode(view).encode("utf-8"))


def emit(value):
    sys.stdout.buffer.write(encode(value).encode("utf-8") + b"\n")
    sys.stdout.buffer.flush()


def publish():
    if not state.data:
        emit({"available": False, "reason": "No msgvault archive found and sample.json is missing"})
        return
    now = int(time.time())
    every = state.data["messages"]
    if truthy(view["account"]):
        scoped = [m for m in every if number(m.get("src") or 0) == number(view["account"])]
    else:
        scoped = list(every)

    counts = {"inbox": 0, "starred": 0, "drafts": 0, "all": 0, "attachments": 0, "removed": 0}
    label_count, kind_count = {}, {}
    for m in scoped:
        if truthy(m["unread"]) and in_mailbox(m, "inbox"):
            counts["inbox"] += 1
        if truthy(m["starred"]):
            counts["starred"] += 1
        if in_mailbox(m, "drafts"):
            counts["drafts"] += 1
        if truthy(m["email"]):
            counts["all"] += 1
        if truthy(m["email"]) and truthy(m.get("att")):
            counts["attachments"] += 1
        if truthy(m.get("removed")):
            counts["removed"] += 1
        for name in m["labels"]:
            label_count[name] = label_count.get(name, 0) + 1
        kind = message_kind(m)
        kind_count[kind] = kind_count.get(kind, 0) + 1
    labels, categories = [], []
    for label in state.data["labels"]:
        n = label_count.get(label["name"])
        if not n:
            continue
        upper = text(label["name"]).upper()
        entry = {"name": label["name"], "title": display_label(label["name"]), "count": n,
                 "color": label_color(label["name"], label.get("color"))}
        if upper.startswith("CATEGORY_"):
            categories.append(entry)
        elif upper not in SYSTEM and not re.match(r"^system$", text(label.get("type")), re.I):
            labels.append(entry)
    archive = [{"id": "kind:" + kind, "title": display_kind(kind), "icon": kind_icon(kind),
                "count": kind_count[kind]}
               for kind in sorted((k for k in kind_count if k != "email"),
                                  key=lambda k: (-kind_count[k], k))]
    if counts["removed"]:
        archive.append({"id": "removed", "title": "Removed Upstream", "icon": "cloud-off",
                        "count": counts["removed"]})

    accounts = [{
        "id": number(a["id"]),
        "title": a["name"] if text(a.get("name")) != "" else a.get("identifier"),
        "kind": display_kind(a.get("kind")),
        "identifier": a.get("identifier"),
        "icon": account_icon(a.get("kind")),
        "count": number(a.get("count")),
    } for a in state.data["accounts"]]
    current = next((a for a in accounts if a["id"] == number(view["account"])), None)
    if current is None:
        view["account"] = 0
    if current:
        header = {"title": current["title"],
                  "subtitle": "%s · %s" % (current["kind"], text(current["identifier"])),
                  "icon": current["icon"]}
    else:
        header = {"title": "All Accounts", "subtitle": plural(len(accounts), "source", "sources"),
                  "icon": "archive"}

    query = text(view.get("query"))
    searching = has_text(query)
    if searching:
        rows = [m for m in scoped if matches_query(m, query)]
    else:
        rows = [m for m in scoped if in_mailbox(m, text(view["mailbox"]))]
    # One row per conversation: its newest message, with the older ones'
    # unread, starred and attachment state folded in (merge_page).
    threads, row_of = [], {}
    for m in rows:
        conv = m.get("conv")
        if conv is None:
            threads.append(dict(m))
            continue
        folded = row_of.get(conv)
        if folded is not None:
            for key in ("unread", "starred", "att"):
                if not truthy(folded.get(key)):
                    folded[key] = m.get(key)
            continue
        row_of[conv] = dict(m)
        threads.append(row_of[conv])
    rows = threads
    title = "Search" if searching else mailbox_title(text(view["mailbox"]))
    if searching:
        subtitle = plural(len(rows), "result", "results") + " for “%s”" % query
    else:
        subtitle = plural(len(rows), "conversation", "conversations")
    if current:
        subtitle += " · " + text(current["title"])
    more = len(rows) > ROWS
    if more:
        rows = rows[:ROWS]

    stats = state.data["stats"]
    files = files_view(now) if view["mailbox"] == "files" else None
    open_id = number(view["open"])
    emit({
        "available": True,
        "sample": truthy(state.data.get("sample")),
        "backend": state.data["backend"],
        "location": state.data["location"],
        "stats": {
            "messages": plural(stats.get("messages"), "message", "messages"),
            "threads": plural(stats.get("threads"), "thread", "threads"),
            "attachments": plural(stats.get("attachments"), "attachment", "attachments"),
            "size": size_text(stats.get("size") or 0),
        },
        "header": header,
        "accounts": accounts,
        "account": number(view["account"]),
        "counts": counts,
        "labels": labels,
        "categories": categories,
        "archive": archive,
        "mailbox": view["mailbox"],
        "query": view["query"] if view.get("query") is not None else "",
        "list": {"title": title, "subtitle": subtitle + ("+" if more else ""),
                 "rows": [row(m, now) for m in rows]},
        "selected": open_id,
        "thread": thread_view(open_id, now) if truthy(view["open"]) else None,
        "total_files": len(state.data.get("files") or []),
        "files": files,
        "notice": state.notice,
    })


def receive(line):
    try:
        envelope = json.loads(line)
    except ValueError:
        return
    if not isinstance(envelope, dict):
        return
    msg = envelope.get("message")
    if not isinstance(msg, dict):
        return
    if msg.get("mailbox") is not None:
        view["mailbox"] = text(msg["mailbox"])
        view["query"] = ""
    if msg.get("account") is not None:
        view["account"] = integer(msg["account"])
    if msg.get("query") is not None:
        view["query"] = text(msg["query"])
        # A search is over messages: leave the attachments browser for it.
        if has_text(view["query"]) and view["mailbox"] == "files":
            view["mailbox"] = "all"
    if msg.get("open") is not None:
        view["open"] = integer(msg["open"])
    if msg.get("kind") is not None:
        view["fkind"] = text(msg["kind"])
    # The browser's search field sends its text as `value`.
    if msg.get("fquery") is not None:
        view["fquery"] = text(msg["value"]) if msg.get("value") is not None else ""
    if truthy(msg.get("fquery_clear")):
        view["fquery"] = ""
    if msg.get("file") is not None:
        view["file"] = text(msg["file"])
    if msg.get("show") is not None:
        # From the attachments browser to the message a file came in.
        view["mailbox"] = "all"
        view["query"] = ""
        view["open"] = integer(msg["show"])
    if truthy(msg.get("remote")):
        view["remote"][str(integer(msg["remote"]))] = 1
    if envelope.get("from") in ("reader", "list"):
        if msg.get("open_file") is not None:
            deliver(text(msg["open_file"]), False)
        if msg.get("save_file") is not None:
            deliver(text(msg["save_file"]), True)
    if truthy(msg.get("refresh")):
        load()
    save_view()


# ---- main ----------------------------------------------------------------

def dump():
    if not os.path.isfile(db_path):
        sys.exit("no archive at %s" % db_path)
    archive = read_archive()
    now = int(time.time())
    convs = {m["conv"] for m in archive["messages"] if m.get("conv") is not None}
    threads = {text(conv): read_thread(conv) for conv in convs}
    for m in archive["messages"] + [m for thread in threads.values() for m in thread]:
        m["ago"] = now - number(m.pop("ts", None))
    archive["threads"] = threads
    del archive["location"], archive["backend"]
    # The pretty form sample.json here is written in.
    pretty = json.dumps(archive, sort_keys=True, ensure_ascii=False, indent=3, separators=(",", " : "))
    sys.stdout.buffer.write(pretty.encode("utf-8") + b"\n")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--dump":
        dump()
        return
    read_view()
    load()
    publish()
    stdin = sys.stdin.fileno()
    buffer = b""
    while True:
        ready, _, _ = select.select([stdin], [], [], REFRESH)
        if not ready:
            if state.mode == "archive":
                load()
                publish()
            continue
        try:
            got = os.read(stdin, 65536)
        except OSError:
            got = b""
        if not got:
            break    # the app closed stdin
        buffer += got
        *lines, buffer = buffer.split(b"\n")
        for line in lines:
            receive(line)
        publish()


if __name__ == "__main__":
    main()
