#!/usr/bin/env python3
# Storefront's audit log, SAMPLE DATA: a few hundred store activities made
# up from templates with a fixed seed, so every launch shows the same log.
# Publishes them as one JSON line, then answers `{ "export": [ids] }` from
# the audit mod by writing those rows to a CSV file (in ~/Downloads when it
# exists, else ~/.cache/moonframe/storefront) and publishing where it went.
# Exits when the app closes stdin.

import calendar
import json
import os
import sys
import time

HOME = os.environ.get("HOME", "")

# "Now" for the sample: the log ends here, whatever the clock says.
NOW = calendar.timegm((2026, 10, 30, 17, 20, 0))  # 2026-10-30 17:20 UTC

KINDS = [
    {"id": "orders",    "label": "Orders",        "icon": "file-text"},
    {"id": "products",  "label": "Products",      "icon": "layers"},
    {"id": "customers", "label": "Customers",     "icon": "user"},
    {"id": "users",     "label": "Users & roles", "icon": "shield"},
    {"id": "apps",      "label": "Apps",          "icon": "plug"},
    {"id": "content",   "label": "Content",       "icon": "palette"},
    {"id": "discounts", "label": "Discounts",     "icon": "tag"},
]

AUTHORS = [
    {"id": "nadia",  "name": "Nadia Okafor",   "color": "#7fc8a9"},
    {"id": "theo",   "name": "Theo Lindqvist", "color": "#f2a7b8"},
    {"id": "priya",  "name": "Priya Raman",    "color": "#9ec5f0"},
    {"id": "marcus", "name": "Marcus Hale",    "color": "#f4b183"},
]

# kind, subject pattern, verb, tags. %d is an order number, %s a name.
TEMPLATES = [
    ("orders",    'Order #%d',        "marked as",         ["Fulfilled", "Refunded", "Paid", "Cancelled"]),
    ("orders",    'Order #%d',        "moved to archived", ["Archived"]),
    ("products",  'Product "%s"',     "marked as",         ["Active", "Draft"]),
    ("products",  'Product "%s"',     "updated with",      ["New variant", "New price", "New images"]),
    ("customers", 'Customer "%s"',    "marked as",         ["Added"]),
    ("customers", 'Customer "%s"',    "updated with",      ["New customer tags", "New email"]),
    ("users",     'User "%s"',        "updated with",      ["New permissions", "New role"]),
    ("users",     'Role preset "%s"', "updated with",      ["New access rule"]),
    ("apps",      'App "%s"',         "marked as",         ["Installed"]),
    ("apps",      'App "%s"',         "updated with",      ["Removed", "New scopes"]),
    ("content",   'Theme "%s"',       "marked as",         ["Published"]),
    ("content",   'Page "%s"',        "updated with",      ["New content"]),
    ("discounts", 'Discount "%s"',    "marked as",         ["Active", "Expired"]),
]

NAMES = {
    "products":  ["Harbor Hoodie", "Summer Tee", "Canvas Tote", "Linen Shirt", "Trail Cap", "Wool Beanie"],
    "customers": ["Ada Moreno", "Kenji Sato", "Lena Fischer", "Omar Haddad", "Sofia Rossi", "Yuki Tanaka"],
    "users":     ["Ines Duarte", "Rafael Costa", "Maya Brooks", "Elio Marchetti"],
    "roles":     ["Support Agent", "Warehouse", "Store Manager"],
    "apps":      ["Mailwise", "Paydock", "Shipwell", "Reviewly"],
    "content":   ["Autumn Collection", "Coastline", "About us", "Shipping policy"],
    "discounts": ["AUTUMN20", "WELCOME10", "FREESHIP", "VIP15"],
}

# A small fixed-seed generator, so the sample never changes.
seed = 20261030


def rnd(n):
    global seed
    seed = (seed * 1103515245 + 12345) % 2147483648
    return seed // 65536 % n


def pick(items):
    return items[rnd(len(items))]


def stamp(t):
    tm = time.gmtime(t)
    ampm = "AM" if tm.tm_hour < 12 else "PM"
    h12 = tm.tm_hour % 12 or 12
    return ("%02d/%02d/%04d" % (tm.tm_mon, tm.tm_mday, tm.tm_year),
            "%02d:%02d:%02d %s" % (h12, tm.tm_min, tm.tm_sec, ampm))


activities = []
today = NOW - NOW % 86400
# 145 in the last seven days (the sample's default filter), the rest over
# the seven weeks before.
for i in range(1, 233):
    day = rnd(7) if i <= 145 else 7 + rnd(49)
    t = today - day * 86400 + 8 * 3600 + rnd(11 * 3600)
    if t > NOW:
        t -= 86400
    kind, pattern, verb, tags = pick(TEMPLATES)
    if "%d" in pattern:
        subject = pattern % (1000 + rnd(9000))
    else:
        pool = "roles" if pattern.startswith("Role") else kind
        subject = pattern % pick(NAMES[pool])
    date, clock = stamp(t)
    activities.append({
        "kind": kind,
        "subject": subject,
        "verb": verb,
        "tag": pick(tags),
        "author": pick(AUTHORS)["id"],
        "time": t,
        "date": date,
        "clock": clock,
    })
activities.sort(key=lambda activity: activity["time"])
for n, activity in enumerate(activities, 1):
    activity["id"] = "a%d" % n
by_id = {activity["id"]: activity for activity in activities}
author_name = {author["id"]: author["name"] for author in AUTHORS}

export = None
exports = 0


def publish():
    data = {
        "available": True,
        "sample": True,
        "now": NOW,
        "store": {"name": "Fernhill Supply", "detail": "fernhill.shop"},
        "kinds": KINDS,
        "authors": AUTHORS,
        "activities": activities,
    }
    if export:
        data["export"] = export
    line = json.dumps(data, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    sys.stdout.buffer.write(line.encode("utf-8", "surrogateescape") + b"\n")
    sys.stdout.buffer.flush()


def csv(value):
    return '"' + value.replace('"', '""') + '"'


def write_csv(ids):
    global export, exports
    downloads = HOME + "/Downloads"
    folder = downloads if os.path.isdir(downloads) else HOME + "/.cache/moonframe/storefront"
    try:
        os.makedirs(folder, exist_ok=True)
    except OSError:
        pass
    path = folder + "/storefront-audit-log.csv"
    try:
        fh = open(path, "w", encoding="utf-8", errors="surrogateescape", newline="")
    except OSError as error:
        exports += 1
        export = {"ok": False, "error": "Could not write %s: %s" % (path, os.strerror(error.errno)),
                  "seq": exports}
        return
    count = 0
    with fh:
        fh.write("Activity,Author,Timestamp\n")
        for id in ids:
            activity = by_id.get(id) if isinstance(id, str) else None
            if not activity:
                continue
            fh.write(",".join([
                csv("%s %s %s" % (activity["subject"], activity["verb"], activity["tag"])),
                csv(author_name[activity["author"]]),
                csv("%s %s" % (activity["date"], activity["clock"])),
            ]) + "\n")
            count += 1
    shown = "~" + path[len(HOME):] if path.startswith(HOME) else path
    exports += 1
    export = {"ok": True, "path": shown, "count": count, "seq": exports}


def receive(line):
    try:
        incoming = json.loads(line)
    except ValueError:
        return
    if not isinstance(incoming, dict):
        return
    msg = incoming.get("message")
    # Only the audit page may write files.
    if incoming.get("from") == "audit" and isinstance(msg, dict) and isinstance(msg.get("export"), list):
        write_csv(msg["export"])
    publish()


def main():
    publish()
    buffer = b""
    fd = sys.stdin.fileno()
    while True:
        got = os.read(fd, 65536)
        if not got:
            break
        buffer += got
        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            receive(line.decode("utf-8", "replace"))
    # A last line without its newline still counts.
    if buffer:
        receive(buffer.decode("utf-8", "replace"))


main()
