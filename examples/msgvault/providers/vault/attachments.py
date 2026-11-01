import hashlib
import os
import re
import stat
import subprocess
import sys
import time
import zlib

import state
from archive import SUMMARY, has_table, hidden, role_column, sql
from formatting import full_date, list_date, person, size_text
from state import (OPEN_LIMIT, THUMB_LIMIT, blob_root, home, images_dir, make_dir, tell_user,
                   tilde, write_file)
from values import LIMIT, integer, number, text, truthy

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
    if state.mode != "archive":
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
    for f in (state.data or {}).get("files") or []:
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
def system_open(*args):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.call([opener] + list(args), stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
    except OSError:
        pass


# Kinds of file that run something when opened: an attachment of one of
# these is shown in its folder, never opened, so a mail can't run code.
LAUNCHERS = {
    ".app", ".command", ".terminal", ".tool", ".sh", ".bash", ".zsh", ".csh", ".fish",
    ".pkg", ".mpkg", ".dmg", ".fileloc", ".webloc", ".inetloc", ".url", ".desktop",
    ".scpt", ".scptd", ".applescript", ".workflow", ".action", ".jar", ".py", ".pl",
    ".rb", ".js", ".jse", ".vbs", ".exe", ".bat", ".cmd", ".com", ".msi", ".ps1",
    ".appimage", ".run", ".bin", ".so", ".dylib", ".kext", ".prefpane", ".plugin",
}


def launcher(name):
    ext = os.path.splitext(name)[1].lower()
    return ext in LAUNCHERS


def quarantine(path):
    """Marks a written attachment as downloaded, as Mail does, so macOS
    asks before running it. Best effort: without `xattr`, nothing is set."""
    if sys.platform != "darwin":
        return
    flag = "0081;%x;msgvault;" % int(time.time())
    try:
        subprocess.call(["xattr", "-w", "com.apple.quarantine", flag, path],
                        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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
    if remote_path(f) and state.mode == "archive":
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
    quarantine(path)
    if save:
        if sys.platform == "darwin" and not truthy(os.environ.get("MSGVAULT_NO_OPEN")):
            system_open("-R", path)
        return tell_user(True, "Saved %s to Downloads" % re.sub(r".*/", "", path))
    if launcher(name):
        if sys.platform == "darwin" and not truthy(os.environ.get("MSGVAULT_NO_OPEN")):
            system_open("-R", path)
        return tell_user(False, "%s could run a program, so it is shown in its folder, not opened" % name)
    if not truthy(os.environ.get("MSGVAULT_NO_OPEN")):
        system_open(path)
    return tell_user(True, "Opened %s" % name)
