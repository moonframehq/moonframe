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
# Python 3.9 or later, standard library only.
import hashlib
import json
import math
import os
import re
import select
import sqlite3
import stat
import subprocess
import sys
import time
import urllib.parse
import zlib

LIMIT = 2000     # newest messages read from an archive
ROWS = 400       # rows published for one list
REFRESH = 60     # seconds between archive reads


def encode(value):
    """One line of JSON, keys sorted."""
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# ---- Loose values -----------------------------------------------------------
#
# The messages and the saved view are JSON, so a field may come as a number,
# a string, a boolean or null; these read one the way the views always have.

def truthy(value):
    """null, false, 0, "" and "0" are false; any list or map is true."""
    if isinstance(value, (list, dict)):
        return True
    return value not in (None, False, 0, "", "0")


def text(value):
    """A value as text: null as "", true and false as "1" and "0"."""
    if value is None:
        return ""
    if isinstance(value, bool):
        return "1" if value else "0"
    if isinstance(value, float):
        if value.is_integer() and abs(value) < 1e15:
            return str(int(value))
        return "%.15g" % value
    return str(value)


_NUMBER = re.compile(r"\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?)")


def number(value):
    """A value as a number, its leading digits for text, 0 when it has none."""
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, str):
        found = _NUMBER.match(value)
        if not found:
            return 0
        n = float(found.group(1))
        return int(n) if n.is_integer() else n
    return 0


def integer(value):
    n = number(value)
    try:
        return int(n)
    except (OverflowError, ValueError):
        return 0


def has_text(value):
    return value is not None and re.search(r"\S", text(value)) is not None


# ---- where things are --------------------------------------------------

home = os.environ.get("HOME", "")
vault_home = os.environ.get("MSGVAULT_HOME") or home + "/.msgvault"
db_path = vault_home + "/msgvault.db"
cache_root = os.environ.get("XDG_CACHE_HOME") or home + "/.cache"
cache_dir = cache_root + "/moonframe/msgvault"
view_file = cache_dir + "/view.json"
html_dir = cache_dir + "/html"
# Pictures go where ui.image may read them: the app's image cache.
images_dir = cache_root + "/moonframe/images/msgvault"
blob_root = vault_home + "/attachments"
OPEN_LIMIT = 512 * 1024 * 1024   # largest attachment read, as msgvault-gpui
THUMB_LIMIT = 8 * 1024 * 1024    # largest picture shown inline

# What was read (the archive or the sample), and what the user looks at.
data, mode, read_at = None, "", 0
view = {"mailbox": "inbox", "account": 0, "query": "", "open": 0, "fkind": "all",
        "fquery": "", "file": "", "remote": {}}


def tilde(path):
    if home and path.startswith(home):
        return "~" + path[len(home):]
    return path


def make_dir(path):
    try:
        os.makedirs(path, exist_ok=True)
    except OSError:
        pass


def write_file(path, content):
    """Writes beside it then renames, so a reader never sees half a file."""
    try:
        with open(path + ".tmp", "wb") as out:
            out.write(content)
    except OSError:
        return False
    try:
        os.rename(path + ".tmp", path)
    except OSError:
        pass
    return True


# ---- formatting, as msgvault-gpui's format.rs --------------------------

WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def local(t):
    return time.localtime(math.floor(number(t)))


def weekday(l):
    # struct_time counts from Monday; the tables from Sunday.
    return (l.tm_wday + 1) % 7


def clock(t):
    l = local(t)
    return "%d:%02d %s" % (l.tm_hour % 12 or 12, l.tm_min, "AM" if l.tm_hour < 12 else "PM")


def day_number(t):
    l = local(t)
    # Local noon of that day, so daylight saving changes never move it.
    return math.floor(time.mktime((l.tm_year, l.tm_mon, l.tm_mday, 12, 0, 0, 0, 0, -1)) / 86400)


def list_date(t, now):
    if not truthy(t):
        return ""
    days = day_number(now) - day_number(t)
    l = local(t)
    n = local(now)
    if days == 0:
        return clock(t)
    if days == 1:
        return "Yesterday"
    if 2 <= days < 7:
        return WEEKDAYS[weekday(l)]
    if l.tm_year == n.tm_year:
        return "%s %d" % (MONTHS[l.tm_mon - 1], l.tm_mday)
    return "%s %d, %d" % (MONTHS[l.tm_mon - 1], l.tm_mday, l.tm_year)


def full_date(t):
    if not truthy(t):
        return ""
    l = local(t)
    return "%s, %s %d, %d at %s" % (DAYS[weekday(l)], MONTHS[l.tm_mon - 1], l.tm_mday,
                                    l.tm_year, clock(t))


def size_text(n):
    units = ["B", "KB", "MB", "GB", "TB"]
    value, unit = number(n), 0
    while value >= 1024 and unit < len(units) - 1:
        value /= 1024
        unit += 1
    if unit == 0:
        return "%s B" % text(n)
    if value >= 100:
        return "%.0f %s" % (value, units[unit])
    return "%.1f %s" % (value, units[unit])


def count(n):
    n = text(n)
    while True:
        grouped = re.sub(r"^(\d+)(\d{3})", r"\1,\2", n)
        if grouped == n:
            return n
        n = grouped


def plural(n, one, many):
    return count(n) + " " + (one if number(n) == 1 else many)


def fnv(value):
    h = 2166136261
    for byte in value.encode("utf-8"):
        h = ((h ^ byte) * 16777619) & 0xFFFFFFFF
    return h


# A label's colour when it has none stored: format::label_color.
PALETTE = ["#4a86e8", "#8e63ce", "#16a766", "#f2a60c", "#e66550", "#2da2bb", "#d5608c",
           "#3dc789", "#7a8cff", "#c28b3c"]


def label_color(name, stored=None):
    if stored is not None and re.match(r"^#[0-9a-fA-F]{6}$", text(stored)):
        return text(stored).lower()
    return PALETTE[fnv(text(name)) % len(PALETTE)]


SYSTEM = {"INBOX", "SENT", "TRASH", "SPAM", "DRAFT", "DRAFTS", "UNREAD", "STARRED",
          "IMPORTANT", "CHAT"}
STATE = {"UNREAD", "STARRED", "INBOX", "SENT", "DRAFT", "IMPORTANT"}


def title_case(word):
    word = text(word).lower()
    return word[:1].title() + word[1:]


def display_label(name):
    name = text(name)
    upper = name.upper()
    category = re.match(r"^CATEGORY_(.+)$", upper)
    if category:
        return title_case(category.group(1))
    if upper in SYSTEM:
        return title_case(upper)
    return name


KINDS = {
    "": "Email", "email": "Email", "imessage": "iMessage", "sms": "SMS", "mms": "MMS",
    "rcs": "RCS", "whatsapp": "WhatsApp", "fbmessenger": "Messenger", "teams": "Teams",
    "slack": "Slack", "discord": "Discord", "beeper": "Beeper", "google_chat": "Google Chat",
    "calendar_event": "Calendar", "meeting_transcript": "Meetings", "matrix": "Matrix",
}


def display_kind(kind):
    kind = text(kind)
    if kind in KINDS:
        return KINDS[kind]
    return title_case(kind.replace("_", " "))


def kind_icon(kind):
    kind = text(kind)
    if kind == "calendar_event":
        return "calendar-days"
    if kind == "meeting_transcript":
        return "message-square"
    if re.match(r"^(slack|discord|teams|google_chat|matrix)$", kind):
        return "hash"
    return "messages-square"


def account_icon(kind):
    kind = text(kind)
    if re.match(r"^(gmail|imap|fastmail|microsoft|outlook|mbox|emlx|pst)$", kind):
        return "mail"
    if re.match(r"^(whatsapp|imessage|apple_messages|google_messages|sms|beeper|google_voice)$", kind):
        return "message-square"
    if re.match(r"^(slack|discord|teams|google_chat|matrix)$", kind):
        return "hash"
    if re.match(r"^(gcal|calendar)$", kind):
        return "calendar-days"
    return "database"


# Initials and a hue (0..11) for an avatar, as gpui-kit's Avatar draws one.
def initials(name):
    words = [w for w in text(name).split(" ") if w]
    letters = "".join(w[:1] for w in words[:2])
    if len(letters) == 1:
        letters = ("?" if name is None else text(name))[:2]
    if letters == "":
        letters = "?"
    return letters.upper()


def hue(letters):
    return fnv(letters) % 12


def clean(value):
    if value is None:
        return ""
    value = text(value)
    for entity, char in (("&#39;", "'"), ("&quot;", '"'), ("&lt;", "<"), ("&gt;", ">"),
                         ("&nbsp;", " "), ("&amp;", "&")):
        value = value.replace(entity, char)
    value = re.sub(r"\s+", " ", value)
    return re.sub(r"^ | $", "", value)


# A readable plain rendering of an HTML body: reader.rs's html_to_text.
def html_to_text(html):
    html = re.sub(r"<(style|script|head)\b.*?</\1\s*>", "", html, flags=re.I | re.S)
    html = re.sub(r"<li\b[^>]*>", "\n• ", html, flags=re.I)
    html = re.sub(r"</?(br|p|div|tr|li|h[1-4]|blockquote|table)\b[^>]*>", "\n", html, flags=re.I)
    html = re.sub(r"</?(td|th)\b[^>]*>", "\t", html, flags=re.I)
    html = re.sub(r"<[^>]*>", "", html)
    for entity, char in (("&nbsp;", " "), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", '"'),
                         ("&#39;", "'"), ("&amp;", "&")):
        html = html.replace(entity, char)
    lines = []
    for line in html.split("\n"):
        line = re.sub(r"\s+$", "", line)
        line = re.sub(r"^\t+", "", line)
        line = re.sub(r"\t+", "  ", line)
        if line == "" and lines and lines[-1] == "":
            continue
        lines.append(line)
    return re.sub(r"^\s+|\s+$", "", "\n".join(lines))


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


# ---- attachments -----------------------------------------------------------

# Every attachment in the archive (the newest LIMIT), with the message it
# came in, for the attachments browser.
def read_files():
    role = role_column("a.")
    files = sql("""SELECT a.id AS aid, a.message_id AS msg, COALESCE(a.filename, '') AS name,
        COALESCE(a.mime_type, '') AS mime, COALESCE(a.size, 0) AS size, %s AS role,
        a.content_hash AS hash, a.storage_path AS path
        FROM attachments a JOIN messages m ON m.id = a.message_id
        WHERE m.deleted_at IS NULL ORDER BY a.id DESC LIMIT %d""" % (role, LIMIT))
    files = [f for f in files if not hidden(f)]
    if not files:
        return []
    ids = ", ".join(str(integer(f["msg"])) for f in files)
    messages = {m["id"]: m for m in sql("%s WHERE m.id IN (%s)" % (SUMMARY, ids))}
    for f in files:
        f["aid"] = "a" + text(f["aid"])
        m = messages.get(f["msg"])
        if m is None:
            continue
        f["from"], f["subject"], f["ts"], f["conv"] = person(m), m["subject"], m["ts"], m["conv"]
        f["subject_for_content"] = m["subject"]
    return sorted(files, key=lambda f: -number(f.get("ts") or 0))


# The sample's attachments, from its threads: ids made from the message's.
def sample_files(d):
    files = []
    for thread in d["threads"].values():
        for m in thread:
            i = 0
            for f in m.get("files") or []:
                if f.get("aid") is None:
                    f["aid"] = "s%s-%d" % (text(m["id"]), i)
                    i += 1
                f["subject_for_content"] = m["conv_title"] if m.get("conv_title") is not None else m.get("subject")
                card = dict(f)
                card.update({"msg": m["id"], "from": person(m), "subject": m.get("subject"),
                             "ts": m.get("ts"), "conv": m.get("conv")})
                files.append(card)
    # Newest first, then by name (a stable sort, so sort by the second key first).
    files.sort(key=lambda f: text(f.get("name")))
    files.sort(key=lambda f: -number(f.get("ts") or 0))
    return files


# What kind of file it is, for the browser's filters and the icon.
FILE_KINDS = [
    ("image", "Images", "image"),
    ("pdf", "PDFs", "file-text"),
    ("document", "Documents", "file-text"),
    ("calendar", "Calendar", "calendar-days"),
    ("archive", "Archives", "file-archive"),
    ("other", "Other", "file"),
]
KIND_ICON = {kind: icon for kind, _, icon in FILE_KINDS}
KIND_TYPE = {kind: re.sub(r"s$", "", title) for kind, title, _ in FILE_KINDS}


def file_kind(f):
    mime = text(f.get("mime")).lower()
    name = text(f.get("name")).lower()
    if re.match(r"image/", mime) or re.search(r"\.(png|jpe?g|gif|webp|heic|bmp|tiff?)$", name):
        return "image"
    if "pdf" in mime or re.search(r"\.pdf$", name):
        return "pdf"
    if mime == "text/calendar" or re.search(r"\.ics$", name):
        return "calendar"
    if re.search(r"zip|compressed|tar|gzip|7z|rar", mime) or re.search(r"\.(zip|tar|gz|tgz|7z|rar)$", name):
        return "archive"
    if (re.search(r"^text/|document|sheet|presentation|msword|excel|powerpoint|rtf|json|xml|csv", mime)
            or re.search(r"\.(txt|md|csv|docx?|xlsx?|pptx?|pages|numbers|key|rtf|odt|ods|json)$", name)):
        return "document"
    return "other"


def remote_path(f):
    return re.match(r"https?://", text(f.get("path"))) is not None


# Filenames from mail headers are untrusted: keep a safe last component.
def safe_filename(raw):
    parts = re.split(r"[/\\]", text(raw))
    while parts and parts[-1] == "":
        parts.pop()
    name = parts[-1] if parts else ""
    name = re.sub(r'[\x00-\x1f\x7f-\x9f:*?"<>|]', "_", name)
    name = re.sub(r"^[\s.]+|\s+$", "", name)
    return "attachment" if name == "" else name


# The SHA-256 the bytes are filed under: content_hash, else storage_path's name.
def content_hash(f):
    path = f.get("path")
    for candidate in (f.get("hash"), text(path).split("/")[-1] if path is not None else None):
        if candidate is not None and re.match(r"^[0-9a-fA-F]{64}$", text(candidate)):
            return text(candidate).lower()
    return None


def slurp(path, offset=None, length=None):
    try:
        with open(path, "rb") as source:
            if truthy(offset):
                source.seek(integer(offset))
            return source.read() if length is None else source.read(max(integer(length), 0))
    except OSError:
        return None


def find_program(name):
    for directory in os.environ.get("PATH", "").split(":"):
        if os.access(directory + "/" + name, os.X_OK):
            return directory + "/" + name
    return None


# An attachment's bytes, read the way msgvault stores them (internal/store):
# a sealed pack found through attachment_pack_index (a zstd frame when flag
# bit 0 is set), else a loose file at attachments/<sha[:2]>/<sha>; checked
# against their SHA-256. The sample's are made as msgvault-gpui's fixture
# makes them. Returns (bytes, None), or (None, why).
def file_bytes(f):
    if mode != "archive":
        return synthesize(f), None
    if remote_path(f):
        return None, "it is stored remotely"
    digest = content_hash(f)
    if not digest:
        return None, "msgvault has no content for it"
    if number(f.get("size") or 0) > OPEN_LIMIT:
        return None, "it is larger than 512 MB"
    content = None
    if has_table("attachment_pack_index"):
        entries = sql("""SELECT pack_id AS pack, pack_offset AS offset, stored_len AS stored,
            raw_len AS raw, flags FROM attachment_pack_index WHERE blob_hash = ?""", (digest,))
        entry = entries[0] if entries else None
        if entry and re.match(r"^[0-9A-Za-z_-]{2,}$", text(entry.get("pack"))):
            pack_id = text(entry["pack"])
            pack = "%s/packs/%s/%s.mvpack" % (blob_root, pack_id[:2], pack_id)
            stored = slurp(pack, entry.get("offset"), entry.get("stored"))
            if stored is None:
                return None, "its pack is missing: " + tilde(pack)
            if integer(entry.get("flags") or 0) & 1:
                zstd = find_program("zstd")
                if not zstd:
                    return None, "it is compressed with zstd, which is not installed"
                try:
                    content = subprocess.run([zstd, "-dcq"], input=stored, stdout=subprocess.PIPE).stdout
                except OSError as error:
                    return None, "zstd: " + (error.strerror or str(error))
            else:
                content = stored
    if content is None:
        candidates = ["%s/%s/%s" % (blob_root, digest[:2], digest)]
        path = f.get("path")
        if path is not None and not re.search(r"\.\.|^/", text(path)):
            candidates.append("%s/%s" % (blob_root, text(path)))
        for path in candidates:
            if not os.path.isfile(path):
                continue
            content = slurp(path)
            break
    if content is None:
        return None, "its content is not in the archive"
    if hashlib.sha256(content).hexdigest() != digest:
        return None, "its content does not match its checksum"
    return content, None


# The sample's files: msgvault-gpui's fixture content (a one-page PDF, an
# event, a gradient picture, a line of text).
def synthesize(f):
    title = f.get("subject_for_content")
    if title is None:
        title = f.get("name")
    title = text(title)
    mime = text(f.get("mime"))
    if mime == "application/pdf":
        line = re.sub(r"[^\x20-\x7e]|[()\\]", "", title)
        stream = "BT /F1 18 Tf 72 720 Td (%s) Tj ET" % line
        objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
            "<< /Length %d >>\nstream\n%s\nendstream" % (len(stream), stream),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ]
        out = "%PDF-1.4\n"
        offsets = []
        for i, body in enumerate(objects):
            offsets.append(len(out))
            out += "%d 0 obj\n%s\nendobj\n" % (i + 1, body)
        xref = len(out)
        out += "xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
        out += "".join("%010d 00000 n \n" % offset for offset in offsets)
        out += "trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
        return out.encode("ascii")
    if mime == "text/calendar":
        return ("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//msgvault-gpui//fixture//EN\r\n"
                "BEGIN:VEVENT\r\nUID:%s\r\nSUMMARY:%s\r\nDTSTART:20240920T184500Z\r\n"
                "DTEND:20240921T140000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
                % (text(f.get("name")), title)).encode("utf-8")
    if mime.startswith("image/"):
        w, h = 320, 200
        raw = bytearray()
        for y in range(h):
            raw.append(0)
            for x in range(w):
                raw += bytes((int(40 + 180 * x / w), int(90 + 100 * y / h), int(200 - 90 * x / w)))

        def chunk(kind, body):
            return (len(body).to_bytes(4, "big") + kind + body
                    + (zlib.crc32(kind + body) & 0xFFFFFFFF).to_bytes(4, "big"))

        header = w.to_bytes(4, "big") + h.to_bytes(4, "big") + bytes((8, 2, 0, 0, 0))
        return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
                + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b""))
    return (title + "\n").encode("utf-8")


def find_file(aid):
    for f in (data or {}).get("files") or []:
        if f["aid"] == aid:
            return f
    return None


# A picture attachment as a file in the image cache, for ui.image; None
# when it is not a picture, is too large, or can't be read.
thumb_failed = set()


def thumbnail(f):
    if file_kind(f) != "image" or number(f.get("size") or 0) > THUMB_LIMIT:
        return None
    found = re.search(r"\.(png|jpe?g|gif|webp|bmp|tiff?|heic)$", text(f.get("name")).lower())
    if found:
        ext = found.group(1)
    else:
        found = re.match(r"image/(\w+)", text(f.get("mime")))
        ext = found.group(1) if found else "png"
    key = content_hash(f)
    key = re.sub(r"[^0-9A-Za-z_-]", "", text(f.get("aid")) if key is None else key)
    path = "%s/%s.%s" % (images_dir, key, ext)
    if os.path.isfile(path):
        return path
    if key in thumb_failed:
        return None
    content, _ = file_bytes(f)
    if content is None:
        thumb_failed.add(key)
        return None
    make_dir(images_dir)
    return path if write_file(path, content) else None


def file_card(f, now=None):
    kind = file_kind(f)
    return {
        "id": f["aid"],
        "name": f["name"] if text(f.get("name")) != "" else "attachment",
        "size": "Remote file" if remote_path(f) else size_text(f.get("size") or 0),
        "kind": kind,
        "type": KIND_TYPE[kind],
        "icon": KIND_ICON[kind],
        "thumb": thumbnail(f),
        "from": f["from"] if f.get("from") is not None else "",
        "subject": f["subject"] if f.get("subject") is not None else "",
        "message": number(f.get("msg") or 0),
        "date": list_date(f.get("ts"), now) if now else full_date(f.get("ts")),
    }


# Open (in the system's app for it) or save (to Downloads, then shown in the
# file manager) an attachment: only ever on the user's click.
notice_seq = 0
notice = None


def tell_user(ok, message):
    global notice_seq, notice
    notice_seq += 1
    notice = {"seq": notice_seq, "ok": bool(ok), "text": message}


def system_open(*args):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.call([opener] + list(args), stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
    except OSError:
        pass


def private_dir(path):
    """`path` as a folder only this user can enter, or None when it is
    someone else's or a link: /tmp is shared on Linux, and a folder left
    there by another user could catch what is written in it."""
    try:
        os.mkdir(path, 0o700)
    except FileExistsError:
        pass
    except OSError:
        return None
    try:
        info = os.lstat(path)
    except OSError:
        return None
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid():
        return None
    if info.st_mode & 0o077:
        try:
            os.chmod(path, 0o700)
        except OSError:
            return None
    return path


def unique_path(directory, name):
    stem, ext = re.match(r"^(.*?)(\.[^.]*)?$", name).groups()
    ext = ext or ""
    path = "%s/%s" % (directory, name)
    n = 1
    while os.path.exists(path):
        path = "%s/%s (%d)%s" % (directory, stem, n, ext)
        n += 1
    return path


def deliver(aid, save):
    f = find_file(aid)
    if f is None:
        return tell_user(False, "That attachment is no longer in the archive")
    name = safe_filename(f.get("name"))
    if remote_path(f) and mode == "archive":
        system_open(f["path"])
        return tell_user(True, "Opened %s in the browser" % name)
    content, error = file_bytes(f)
    if content is None:
        return tell_user(False, "Can't open %s: %s" % (name, error))
    if save:
        directory = os.environ.get("MSGVAULT_DOWNLOADS") or home + "/Downloads"
    else:
        scratch = private_dir((os.environ.get("TMPDIR") or "/tmp") + "/msgvault-moonframe")
        if scratch is None:
            return tell_user(False, "Can't open %s: the temporary folder is not yours alone" % name)
        directory = scratch + "/" + f["aid"]
    make_dir(directory)
    path = unique_path(directory, name) if save else "%s/%s" % (directory, name)
    try:
        # Never through a link someone left in its place.
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o666)
        with os.fdopen(fd, "wb") as out:
            out.write(content)
    except OSError as error:
        return tell_user(False, "Can't write %s: %s" % (tilde(path), error.strerror or error))
    if save:
        if sys.platform == "darwin" and not truthy(os.environ.get("MSGVAULT_NO_OPEN")):
            system_open("-R", path)
        return tell_user(True, "Saved %s to Downloads" % re.sub(r".*/", "", path))
    if not truthy(os.environ.get("MSGVAULT_NO_OPEN")):
        system_open(path)
    return tell_user(True, "Opened %s" % name)


# ---- HTML mail as a page ---------------------------------------------------

# Whether the HTML loads anything from the network (pictures, backgrounds).
def remote_content(html):
    return bool(re.search(r"""(?:src|background|srcset)\s*=\s*["']?\s*(?:https?:)?//""", html, re.I)
                or re.search(r"""url\(\s*['"]?\s*(?:https?:)?//""", html, re.I))


PAGE = """<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src {images}; style-src 'unsafe-inline'; font-src data:; form-action 'none'; base-uri 'none'">
<meta name="color-scheme" content="{scheme}">
<style>
html, body {{ margin: 0; padding: 0; background: {bg}; color: {fg}; }}
body {{ font: 14px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; overflow-wrap: anywhere; }}
a {{ color: {link}; }}
h1 {{ font-size: 2.15em; line-height: 1.2; margin: 0.15em 0 0.35em; }}
h2, h3 {{ line-height: 1.3; margin: 0.5em 0 0.3em; }}
p {{ margin: 0 0 0.5em; }}
ul, ol {{ margin: 0 0 0.5em; padding-left: 1.1em; }}
blockquote {{ margin: 0.3em 0 0.5em; padding-left: 1.4em; border-left: 3px solid {rule}; color: {muted}; }}
img {{ max-width: 100%; height: auto; }}
table {{ border-collapse: collapse; max-width: 100%; }}
td, th {{ padding: 2px 12px 2px 0; vertical-align: top; text-align: left; }}
small {{ color: {muted}; }}
</style>
{kept}
</head><body>{body}</body></html>
"""


# A message's HTML as a page for the reader's web view: its body and styles
# kept; scripts, frames, forms and refreshes dropped; under a
# Content-Security-Policy that runs no script and loads nothing from the
# network (remote pictures neither, until the user asks); in the window's
# light or dark colours. Returns its file:// address.
def page(message_id, html, dark, remote):
    path = "%s/%s-%s%s.html" % (html_dir, text(message_id), "dark" if dark else "light",
                                "-remote" if remote else "")
    if os.path.isfile(path):
        return "file://" + path
    styles = [re.sub(r"@import[^;]*;", "", style, flags=re.I)
              for style in re.findall(r"(<style\b.*?</style\s*>)", html, re.I | re.S)]
    found = re.search(r"<body\b[^>]*>(.*)</body\s*>", html, re.I | re.S)
    body = found.group(1) if found else html
    body = re.sub(r"<(script|style|head|title|iframe|object|embed|form|noscript)\b.*?</\1\s*>", "",
                  body, flags=re.I | re.S)
    body = re.sub(r"<(script|meta|link|base|iframe|object|embed)\b[^>]*>", "", body, flags=re.I)
    body = re.sub(r"""\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)""", "", body, flags=re.I)
    if dark:
        bg, fg, muted, link, rule = "#0a0a0a", "#fafafa", "#a3a3a3", "#60a5fa", "#262626"
    else:
        bg, fg, muted, link, rule = "#ffffff", "#0a0a0a", "#737373", "#2563eb", "#e5e5e5"
    content = PAGE.format(
        images="data: cid: https: http:" if remote else "data: cid:",
        scheme="dark" if dark else "light",
        bg=bg, fg=fg, muted=muted, link=link, rule=rule,
        kept="\n".join(styles), body=body,
    )
    make_dir(html_dir)
    if not write_file(path, content.encode("utf-8")):
        return None
    return "file://" + path


# How tall the page draws, roughly: a view can't measure it.
def page_height(html):
    height = 6
    plain = html_to_text(html)
    for line in plain.split("\n") if plain else []:
        height += 23 * (1 + len(line) // 95) if re.search(r"\S", line) else 6
    height += 38 * len(re.findall(r"<h1\b", html, re.I))
    height += 12 * len(re.findall(r"<h[23]\b", html, re.I))
    # Pictures: their stated height, else a guess.
    for img in re.findall(r"<img\b[^>]*>", html, re.I):
        stated = re.search(r"""height\s*=\s*["']?([0-9]+)""", img, re.I)
        height += int(stated.group(1)) + 8 if stated else 160
    return min(max(height, 60), 1400)


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


# ---- the model -----------------------------------------------------------

def load():
    global data, mode, read_at
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
            data, mode, read_at = archive, "archive", time.time()
            decorate(data)
            return
    data, mode, read_at = read_sample(), "sample", time.time()
    if not data:
        return
    decorate(data)
    data["files"] = sample_files(data)


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


def person(m):
    if text(m.get("name")) != "":
        return m["name"]
    if text(m.get("address")) != "":
        return m["address"]
    return "Unknown"


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
    first = next((m for m in data["messages"] if number(m["id"]) == open_id), None)
    if first is None:
        return None
    messages = None
    if mode == "archive":
        if first.get("conv") is not None:
            try:
                messages = read_thread(first["conv"])
            except Exception:
                messages = None
        if not messages:
            messages = None
    else:
        conv = first.get("conv")
        messages = data["threads"].get("" if conv is None else text(conv))
    if messages is None:
        messages = [first]
    chat = sum(1 for m in messages if text(m.get("kind")) not in ("", "email"))
    # Labels from the list's copies, which carry them.
    listed = {m["id"]: m for m in data["messages"]}
    labels, seen = [], set()
    for m in messages:
        known = listed.get(m["id"], m)
        for chip in chips(known):
            if chip["title"] not in seen:
                seen.add(chip["title"])
                labels.append(chip)
    account = None
    for a in data["accounts"]:
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
    every = list(data.get("files") or [])
    if truthy(view["account"]):
        theirs = {number(m["id"]) for m in data["messages"]
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
    if not data:
        emit({"available": False, "reason": "No msgvault archive found and sample.json is missing"})
        return
    now = int(time.time())
    every = data["messages"]
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
    for label in data["labels"]:
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
    } for a in data["accounts"]]
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

    stats = data["stats"]
    files = files_view(now) if view["mailbox"] == "files" else None
    open_id = number(view["open"])
    emit({
        "available": True,
        "sample": truthy(data.get("sample")),
        "backend": data["backend"],
        "location": data["location"],
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
        "total_files": len(data.get("files") or []),
        "files": files,
        "notice": notice,
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
            if mode == "archive":
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
