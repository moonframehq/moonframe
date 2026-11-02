import json
import os
import re
import sqlite3
import time
import urllib.parse

from state import db_path, tilde
from values import LIMIT, has_text, integer, number, text, truthy

# ---- reading an archive ------------------------------------------------

def _cell(value):
    # A text cell that isn't UTF-8 (or a blob) reads as text all the same.
    if isinstance(value, bytes):
        return value.decode("utf-8", "replace")
    return value


def sql(query, params=()):
    """Rows as maps, from the archive opened read-only for this one query."""
    uri = "file:%s?mode=ro" % urllib.parse.quote(db_path)
    db = sqlite3.connect(uri, uri=True, timeout=2.0)
    try:
        db.text_factory = lambda raw: raw.decode("utf-8", "replace")
        db.execute("PRAGMA query_only=1")
        cursor = db.execute(query, params)
        names = [column[0] for column in cursor.description or ()]
        return [{name: _cell(value) for name, value in zip(names, row)} for row in cursor]
    finally:
        db.close()


def has_table(name):
    return len(sql("SELECT 1 AS ok FROM sqlite_master WHERE type = 'table' AND name = ?", (name,))) > 0


TS = """(CASE WHEN typeof(COALESCE(m.sent_at, m.received_at, m.internal_date)) IN ('integer', 'real')
    THEN CAST(COALESCE(m.sent_at, m.received_at, m.internal_date) / (CASE WHEN COALESCE(m.sent_at, m.received_at, m.internal_date) > 100000000000 THEN 1000 ELSE 1 END) AS INTEGER)
    ELSE CAST(strftime('%s', COALESCE(m.sent_at, m.received_at, m.internal_date)) AS INTEGER) END)"""

SUMMARY = """SELECT m.id AS id, m.conversation_id AS conv, m.source_id AS src,
    COALESCE(NULLIF(TRIM(m.subject), ''), conv.title, '') AS subject,
    COALESCE(NULLIF(TRIM(m.snippet), ''), SUBSTR(mb.body_text, 1, 240), '') AS snippet,
    COALESCE(NULLIF(TRIM(mr_from.display_name), ''), NULLIF(TRIM(p.display_name), ''), '') AS name,
    COALESCE(NULLIF(p.email_address, ''), NULLIF(p.phone_number, ''), '') AS address,
    """ + TS + """ AS ts,
    COALESCE(m.has_attachments, 0) AS att, COALESCE(m.message_type, '') AS kind,
    conv.message_count AS thread, conv.title AS conv_title,
    m.deleted_from_source_at IS NOT NULL AS removed, COALESCE(m.is_from_me, 0) AS mine
FROM messages m
LEFT JOIN message_recipients mr_from ON mr_from.id = (
    SELECT mr.id FROM message_recipients mr
    WHERE mr.message_id = m.id AND mr.recipient_type = 'from' ORDER BY mr.id LIMIT 1)
LEFT JOIN participants p ON p.id = COALESCE(mr_from.participant_id, m.sender_id)
LEFT JOIN conversations conv ON conv.id = m.conversation_id
LEFT JOIN message_bodies mb ON mb.message_id = m.id"""

# Attachments that are part of a message's look, not files sent with it.
HIDDEN_ROLE = re.compile(r"^(inline|avatar|thumbnail|preview|ui_asset)$")


def hidden(row):
    return row.get("role") is not None and HIDDEN_ROLE.match(text(row["role"])) is not None


def role_column(prefix):
    columns = sql("PRAGMA table_info(attachments)")
    return prefix + "attachment_role" if any(c["name"] == "attachment_role" for c in columns) else "NULL"


# Everything the views need but the bodies: the newest messages with their
# labels and flags, the accounts, labels and totals.
def read_archive():
    newest = ("SELECT id FROM messages WHERE deleted_at IS NULL\n"
              "        ORDER BY COALESCE(sent_at, received_at, internal_date) DESC, id DESC LIMIT %d" % LIMIT)
    messages = sql("%s WHERE m.id IN (%s) ORDER BY ts DESC, m.id DESC" % (SUMMARY, newest))
    by_id = {m["id"]: m for m in messages}
    for row in sql("""SELECT ml.message_id AS id, l.name AS name FROM message_labels ml
            JOIN labels l ON l.id = ml.label_id WHERE ml.message_id IN (%s)
            ORDER BY l.name COLLATE NOCASE""" % newest):
        m = by_id.get(row["id"])
        if m is None:
            continue
        labels = m.setdefault("labels", [])
        if row["name"] not in labels:
            labels.append(row["name"])
    if has_table("imap_message_memberships"):
        for row in sql("""SELECT message_id AS id, MAX(flags LIKE '%%Seen%%') AS seen,
                MAX(flags LIKE '%%Flagged%%') AS flagged FROM imap_message_memberships
                WHERE message_id IN (%s) GROUP BY message_id""" % newest):
            m = by_id.get(row["id"])
            if m is None:
                continue
            m["imap_unread"] = 0 if truthy(row["seen"]) else 1
            m["imap_starred"] = 1 if truthy(row["flagged"]) else 0
    accounts = sql("""SELECT s.id AS id, s.source_type AS kind, s.identifier AS identifier,
        s.display_name AS name, (SELECT COUNT(*) FROM messages m WHERE m.source_id = s.id
        AND m.deleted_at IS NULL) AS count FROM sources s
        ORDER BY s.source_type, s.identifier COLLATE NOCASE""")
    labels = sql("""SELECT name, MAX(label_type) AS type, MAX(color) AS color FROM labels
        GROUP BY name ORDER BY name COLLATE NOCASE""")
    totals = sql("""SELECT (SELECT COUNT(*) FROM messages WHERE deleted_at IS NULL) AS messages,
        (SELECT COUNT(*) FROM conversations) AS threads,
        (SELECT COUNT(*) FROM attachments) AS attachments""")[0]
    size = 0
    for path in (db_path, db_path + "-wal"):
        try:
            size += os.path.getsize(path)
        except OSError:
            pass
    totals["size"] = size
    return {
        "messages": messages, "accounts": accounts, "labels": labels, "stats": totals,
        "location": tilde(db_path), "backend": "SQLite",
    }


# Every message of a conversation, oldest first, with bodies, recipients and
# attachments.
def read_thread(conv):
    conv = integer(conv)
    messages = sql("""%s WHERE m.conversation_id = %d AND m.deleted_at IS NULL
        ORDER BY ts ASC, m.id ASC""" % (SUMMARY, conv))
    if not messages:
        return []
    by_id = {m["id"]: m for m in messages}
    ids = ", ".join(str(integer(m["id"])) for m in messages)
    for row in sql("""SELECT mr.message_id AS id, mr.recipient_type AS type,
            COALESCE(NULLIF(TRIM(mr.display_name), ''), NULLIF(TRIM(p.display_name), ''),
                NULLIF(p.email_address, ''), NULLIF(p.phone_number, ''), '') AS name
            FROM message_recipients mr JOIN participants p ON p.id = mr.participant_id
            WHERE mr.message_id IN (%s) AND mr.recipient_type IN ('to', 'cc', 'bcc')
            ORDER BY mr.message_id, mr.id""" % ids):
        if row["id"] in by_id:
            by_id[row["id"]].setdefault(row["type"], []).append(row["name"])
    for row in sql("""SELECT message_id AS id, body_text AS text, body_html AS html
            FROM message_bodies WHERE message_id IN (%s)""" % ids):
        m = by_id.get(row["id"])
        if m is None:
            continue
        if has_text(row["text"]):
            m["text"] = row["text"]
        if has_text(row["html"]):
            m["html"] = row["html"]
    role = role_column("")
    for row in sql("""SELECT id AS aid, message_id AS id, COALESCE(filename, '') AS name,
            COALESCE(mime_type, '') AS mime, COALESCE(size, 0) AS size, %s AS role,
            content_hash AS hash, storage_path AS path
            FROM attachments WHERE message_id IN (%s) ORDER BY id""" % (role, ids)):
        if hidden(row) or row["id"] not in by_id:
            continue
        by_id[row["id"]].setdefault("files", []).append({
            "aid": "a" + text(row["aid"]), "name": row["name"], "mime": row["mime"],
            "size": row["size"], "hash": row["hash"], "path": row["path"],
        })
    return messages


# ---- the sample ----------------------------------------------------------

def read_sample():
    try:
        with open("sample.json", "rb") as source:
            d = json.loads(source.read())
    except (OSError, ValueError):
        return None
    if not truthy(d):
        return None
    now = int(time.time())
    for m in list(d["messages"]) + [m for thread in d["threads"].values() for m in thread]:
        if m.get("ago") is not None:
            m["ts"] = now - number(m["ago"])
    d["backend"] = "Sample"
    d["location"] = "demo archive"
    d["sample"] = 1
    return d
