import os
import re
import sys
import time

import state
from archive import read_archive, read_sample, read_thread
from attachments import FILE_KINDS, file_card, file_kind, find_file, read_files, sample_files
from formatting import (STATE, clean, clock, display_kind, display_label, full_date,
                        html_to_text, hue, initials, label_color, list_date, local, person,
                        plural, size_text)
from page import page, page_height, remote_content
from state import db_path, view
from values import ROWS, has_text, number, text, truthy

# ---- the model -----------------------------------------------------------

def load():
    if os.path.isfile(db_path):
        try:
            archive = read_archive()
        except Exception as error:  # a locked, partial or foreign file: show the sample
            print("vault: reading %s failed: %s" % (db_path, error), file=sys.stderr, flush=True)
        else:
            try:
                archive["files"] = read_files()
            except Exception:
                archive["files"] = []
            state.data, state.mode, state.read_at = archive, "archive", time.time()
            decorate(state.data)
            return
    state.data, state.mode, state.read_at = read_sample(), "sample", time.time()
    if not state.data:
        return
    decorate(state.data)
    state.data["files"] = sample_files(state.data)


def has_label(m, *names):
    want = set(names)
    return sum(1 for name in m.get("labels") or [] if text(name).upper() in want)


def decorate(d):
    for m in d["messages"]:
        if m.get("labels") is None:
            m["labels"] = []
        m["unread"] = 1 if has_label(m, "UNREAD") or truthy(m.get("imap_unread")) else 0
        m["starred"] = 1 if has_label(m, "STARRED") or truthy(m.get("imap_starred")) else 0
        m["email"] = 1 if text(m.get("kind")) in ("", "email") else 0
        m["snippet"] = clean(m.get("snippet"))


SENT = ("SENT", "SENT ITEMS", "SENT MAIL", "SENT MESSAGES", "[GMAIL]/SENT MAIL")
DRAFTS = ("DRAFT", "DRAFTS", "[GMAIL]/DRAFTS")
SPAM = ("SPAM", "JUNK", "JUNK E-MAIL", "JUNK EMAIL", "[GMAIL]/SPAM")
TRASH = ("TRASH", "BIN", "DELETED ITEMS", "DELETED MESSAGES", "[GMAIL]/TRASH", "[GMAIL]/BIN")


def message_kind(m):
    return m["kind"] if truthy(m.get("kind")) else "email"


def in_mailbox(m, box):
    if box == "inbox":
        return has_label(m, "INBOX")
    if box == "starred":
        return truthy(m.get("starred"))
    if box == "unread":
        return truthy(m.get("unread"))
    if box == "important":
        return has_label(m, "IMPORTANT")
    if box == "sent":
        return has_label(m, *SENT) or (truthy(m.get("mine")) and truthy(m.get("email")))
    if box == "drafts":
        return has_label(m, *DRAFTS)
    if box == "all":
        return truthy(m.get("email"))
    if box == "attachments":
        return truthy(m.get("email")) and truthy(m.get("att"))
    if box == "spam":
        return has_label(m, *SPAM)
    if box == "trash":
        return has_label(m, *TRASH)
    if box == "removed":
        return truthy(m.get("removed"))
    if box.startswith("label:"):
        return box[len("label:"):] in m["labels"]
    if box.startswith("kind:"):
        return message_kind(m) == box[len("kind:"):]
    return False


TITLES = {
    "inbox": "Inbox", "starred": "Starred", "unread": "Unread", "important": "Important",
    "sent": "Sent", "drafts": "Drafts", "all": "All Mail", "attachments": "Attachments",
    "spam": "Spam", "trash": "Trash", "removed": "Removed Upstream",
}


def mailbox_title(box):
    if box in TITLES:
        return TITLES[box]
    if box.startswith("label:"):
        return display_label(box[len("label:"):])
    if box.startswith("kind:"):
        return display_kind(box[len("kind:"):])
    return box


def local_day(t):
    l = local(t)
    return "%04d-%02d-%02d" % (l.tm_year, l.tm_mon, l.tm_mday)


# msgvault's Gmail-like search: free text, from:, to:, subject:, label:,
# has:attachment, is:unread, is:starred, before:, after:.
def matches_query(m, query):
    for term in re.findall(r'(\S+:"[^"]*"|"[^"]*"|\S+)', query):
        operator = re.match(r"^(\w+):(.*)$", term)
        op, value = (operator.group(1).lower(), operator.group(2)) if operator else ("", term)
        v = re.sub(r'^"|"$', "", value).lower()

        def has(field):
            return v in text(field).lower()

        if op == "from":
            if not (has(m.get("name")) or has(m.get("address"))):
                return False
        elif op == "subject":
            if not has(m.get("subject")):
                return False
        elif op == "label":
            if not any(text(l).lower() == v or text(l).lower() == "category_" + v
                       or display_label(l).lower() == v for l in m["labels"]):
                return False
        elif op == "has":
            if re.match(r"attach", v) and not truthy(m.get("att")):
                return False
        elif op == "is":
            if v == "unread" and not truthy(m.get("unread")):
                return False
            if v == "starred" and not truthy(m.get("starred")):
                return False
        elif op in ("before", "after"):
            if not re.match(r"^(\d{4})-(\d\d)-(\d\d)$", v):
                continue
            day = local_day(m.get("ts") or 0)
            if op == "before" and day >= v:
                return False
            if op == "after" and day < v:
                return False
        elif op == "to":
            continue    # recipients are only read with a thread
        elif not (has(m.get("subject")) or has(m.get("snippet")) or has(m.get("name"))
                  or has(m.get("address")) or has(m.get("conv_title"))):
            return False
    return True


def chips(m, most=0):
    out = []
    for name in m.get("labels") or []:
        if text(name).upper() in STATE:
            continue
        out.append({"title": display_label(name), "color": label_color(name)})
        if most and len(out) >= most:
            break
    return out



def row(m, now):
    sender = person(m)
    if truthy(m.get("email")):
        title = sender
        subject = m["subject"] if has_text(m.get("subject")) else "(no subject)"
    else:
        title = m["subject"] if has_text(m.get("subject")) else display_kind(m.get("kind"))
        subject = sender
    letters = initials(title)
    thread = number(m.get("thread") or 0)
    return {
        "id": number(m["id"]),
        "title": title,
        "subject": subject,
        "snippet": m["snippet"],
        "date": list_date(m.get("ts"), now),
        "unread": truthy(m.get("unread")),
        "starred": truthy(m.get("starred")),
        "attachments": truthy(m.get("att")),
        "removed": truthy(m.get("removed")),
        "count": thread if truthy(m.get("email")) and thread > 1 else 0,
        "chips": chips(m, 2),
        "initials": letters,
        "hue": hue(letters),
    }


def thread_view(open_id, now):
    first = next((m for m in state.data["messages"] if number(m["id"]) == open_id), None)
    if first is None:
        return None
    messages = None
    if state.mode == "archive":
        if first.get("conv") is not None:
            try:
                messages = read_thread(first["conv"])
            except Exception:
                messages = None
        if not messages:
            messages = None
    else:
        conv = first.get("conv")
        messages = state.data["threads"].get("" if conv is None else text(conv))
    if messages is None:
        messages = [first]
    chat = sum(1 for m in messages if text(m.get("kind")) not in ("", "email"))
    # Labels from the list's copies, which carry them.
    listed = {m["id"]: m for m in state.data["messages"]}
    labels, seen = [], set()
    for m in messages:
        known = listed.get(m["id"], m)
        for chip in chips(known):
            if chip["title"] not in seen:
                seen.add(chip["title"])
                labels.append(chip)
    account = None
    for a in state.data["accounts"]:
        if first.get("src") is not None and number(a["id"]) == number(first["src"]):
            account = a["name"] if text(a.get("name")) != "" else a.get("identifier")
    subject = first["conv_title"] if first.get("conv_title") is not None else ""
    if truthy(first.get("email")) or not has_text(subject):
        subject = messages[0].get("subject") if messages else None
        subject = "" if subject is None else subject
    if not has_text(subject):
        subject = "(no subject)"
    meta = [plural(len(messages), "message", "messages")]
    if chat:
        meta.insert(0, display_kind(first.get("kind")))
    if account is not None and account not in meta:
        meta.append(account)
    out = []
    for m in messages:
        sender = person(m)
        letters = initials(sender)
        if m.get("text") is not None:
            body = m["text"]
        elif m.get("html") is not None:
            body = html_to_text(m["html"])
        else:
            body = m["snippet"] if m.get("snippet") is not None else ""
        body = re.sub(r"\s+$", "", text(body))
        known = listed.get(m["id"], m)
        address = text(m.get("address"))
        out.append({
            "id": number(m["id"]),
            "from": sender,
            "address": address if address != sender else "",
            "to": ", ".join(m.get("to") or []),
            "cc": ", ".join(m.get("cc") or []),
            "date": full_date(m.get("ts")),
            "time": clock(m["ts"]) if truthy(m.get("ts")) else "",
            "snippet": clean(m.get("snippet")),
            "body": body,
            "mine": truthy(m.get("mine")),
            "starred": truthy(known.get("starred")),
            "attachments": truthy(m.get("att")),
            "initials": letters,
            "hue": hue(letters),
            "files": [file_card(find_file(f["aid"]) or f) for f in m.get("files") or []],
            "html": html_view(m),
        })
    return {
        "id": number(first["id"]),
        "subject": subject,
        "chat": bool(chat),
        "meta": " · ".join(meta),
        "labels": labels,
        "messages": out,
    }


# The rendered form of a message with an HTML part: a page for each look,
# with remote pictures only when the user let them load for it.
def html_view(m):
    html = m.get("html")
    if html is None or text(m.get("kind")) not in ("", "email"):
        return None
    remote = remote_content(html)
    allowed = remote and truthy(view["remote"].get(text(m["id"])))
    return {
        "light": page(m["id"], html, False, allowed),
        "dark": page(m["id"], html, True, allowed),
        "height": page_height(html),
        "remote": remote,
        "allowed": allowed,
    }


# The attachments browser: every file, filtered by kind and a search.
def files_view(now):
    every = list(state.data.get("files") or [])
    if truthy(view["account"]):
        theirs = {number(m["id"]) for m in state.data["messages"]
                  if number(m.get("src") or 0) == number(view["account"])}
        every = [f for f in every if number(f.get("msg") or 0) in theirs]
    counted = {}
    for f in every:
        kind = file_kind(f)
        counted[kind] = counted.get(kind, 0) + 1
    kinds = [{"id": "all", "title": "All", "count": len(every)}]
    kinds += [{"id": kind, "title": title, "count": counted[kind]}
              for kind, title, _ in FILE_KINDS if counted.get(kind)]
    kind = view["fkind"] if view.get("fkind") is not None else "all"
    if not any(k["id"] == kind for k in kinds):
        kind = "all"
    query = text(view.get("fquery")).lower()
    rows = [f for f in every
            if (kind == "all" or file_kind(f) == kind)
            and (query == "" or query in " ".join(text(f.get(key)) for key in ("name", "from", "subject")).lower())]
    size = sum(number(f.get("size") or 0) for f in rows)
    more = len(rows) > ROWS
    if more:
        rows = rows[:ROWS]
    wanted = text(view.get("file"))
    selected = next((f for f in every if f["aid"] == wanted), None)
    return {
        "title": "All Attachments",
        "subtitle": plural(len(rows), "file", "files") + ("+" if more else "") + " · " + size_text(size),
        "kinds": kinds,
        "kind": kind,
        "query": view["fquery"] if view.get("fquery") is not None else "",
        "rows": [file_card(f, now) for f in rows],
        "selected": selected["aid"] if selected else "",
        "file": file_card(selected) if selected else None,
    }
