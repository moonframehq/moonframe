#!/usr/bin/env python3
# Tally's meetings, kept in a JSON file, with today's date and the time now
# (a quarter hour at a time), which views can't read themselves. Prints one
# JSON line whenever that changes; reads the app's messages, one
# `{"message": ...}` per line, on stdin. Exits when the app closes stdin.
#
# Output (`app.meetings`):
#   today     "YYYY-MM-DD"
#   now       { date, hour } (hour as 13.25 for 1:15 PM)
#   me        the signed-in person's name
#   meetings  { id, title, date, start, finish (hours), mark (video, meet,
#               chat, call, grid), host, related, created = { date, time,
#               zone }, participants = { names }, agenda, link, notes },
#             by date and time
#
# Messages (one key each):
#   reschedule = id                      the same time, a day later
#   schedule = { date, start, title? }   a new hour-long meeting
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/meetings.json (by default
# under ~/.local/share), started the first time with sample meetings around
# the current week.

import datetime
import json
import os
import re
import select
import sys
import time

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
DATA = (
    (_xdg if _xdg.startswith("/") else HOME + "/.local/share")
    + "/moonframe/"
    + re.sub(r"[^A-Za-z0-9_-]", "-", os.environ.get("MOONFRAME_PROFILE", "pipeline"))
)
FILE = DATA + "/meetings.json"
ME = "Maya Kim"

MONTHS = ["January", "February", "March", "April", "May", "June", "July",
          "August", "September", "October", "November", "December"]
DATE = re.compile(r"(\d{4})-(\d\d)-(\d\d)\n?\Z", re.ASCII)


def parse_date(date):
    found = DATE.match(date) if isinstance(date, str) else None
    if not found:
        return None
    try:
        return datetime.date(*(int(part) for part in found.groups()))
    except ValueError:
        return None


# A date "YYYY-MM-DD" moved by some days (whole days on the calendar: no DST).
def shift_date(date, days):
    day = parse_date(date)
    if day is None:
        return date
    return (day + datetime.timedelta(days=days)).isoformat()


# "November 3, 2026", in English whatever the locale.
def long_date(day):
    return "%s %d, %d" % (MONTHS[day.month - 1], day.day, day.year)


def today():
    return time.strftime("%Y-%m-%d", time.localtime())


def sample():
    now = time.localtime()
    sunday = shift_date(today(), -((now.tm_wday + 1) % 7))
    meetings = []

    def meeting(day, start, finish, title, mark, related, people, host=ME, at="11:58 PM",
                agenda=None, link=None, notes=None):
        meetings.append({
            "id": "m%d" % (len(meetings) + 1), "title": title, "date": shift_date(sunday, day),
            "start": start, "finish": finish, "mark": mark, "host": host,
            "related": related,
            "created": {"date": long_date(parse_date(shift_date(sunday, day - 3))),
                        "time": at, "zone": "PST"},
            "participants": people,
            "agenda": agenda or "Walk through where %s stands, agree on the next step and who owns it." % related,
            "link": link or "https://meet.tally.example/" + re.sub(r"\W+", "-", related).lower(),
            "notes": notes or "Share the latest numbers before the call.",
        })

    meeting(0, 10, 12, "Platform roadmap review", "grid", "Lumen Retail",
            ["Tess Okafor", "Jonah Price", "Lena Brooks"],
            agenda="Align on the renewal scope for 2027: which modules stay, the timeline, and what procurement needs before signing.",
            notes="Bring the usage report and the revised pricing deck.")
    meeting(1, 8, 9, "Product demo - Kestrel Airways", "video", "Kestrel Airways",
            ["Ravi Desai", "Owen Hart"])
    meeting(1, 12, 13, "Discovery call - new lead", "grid", "Bluefin Studio",
            ["Owen Hart", "Priya Nair"], at="9:14 AM")
    meeting(2, 10, 11, "Quarterly business review", "meet", "Fernhill Co-op",
            ["Tess Okafor", "Marco Silva"])
    meeting(2, 14, 15, "Security audit discussion", "video", "Quarry Data",
            ["Sami Lund", "Ada Fenwick"], host="Sami Lund")
    meeting(3, 12, 13, "Discovery call - Granary Foods", "grid", "Granary Foods",
            ["Tess Okafor", "Lena Brooks"])
    meeting(3, 12.5, 13.5, "Pricing sync", "call", "Vesper Hotels",
            ["Ravi Desai"], host="Ravi Desai")
    meeting(4, 9, 10, "Follow-up call - discussion", "meet", "Ferro Logistics",
            ["Ravi Desai", "Marco Silva"])
    meeting(4, 15, 17, "Schedule business review", "chat", "Ashby College",
            ["Tess Okafor", "Priya Nair", "Owen Hart"])
    meeting(5, 11, 12, "Contract redlines", "video", "Lumen Retail",
            ["Jonah Price", "Lena Brooks"])
    meeting(6, 9.5, 10.5, "Weekly pipeline sync", "chat", "Tally team",
            ["Sami Lund", "Ravi Desai", "Tess Okafor"],
            link="https://chat.tally.example/pipeline")
    meeting(8, 10, 12, "Product roadmap review", "grid", "Pinewood Labs",
            ["Sebastian Gray", "Nathan Phillips", "Ben Scott"],
            agenda="This meeting will focus on aligning the team on Q1-Q2 2027 product priorities, timelines, and dependencies to prepare for the rollout to three sites.",
            link="https://chat.tally.example/pinewood",
            notes="Finalize roadmap priorities before the rollout, share the latest mockups and metrics.")
    meeting(8, 14, 15, "Onboarding kickoff", "meet", "Inkwell Press",
            ["Sami Lund", "Ada Fenwick"])
    meeting(9, 9, 10, "Field licences demo", "video", "Solstice Power",
            ["Sami Lund", "Priya Nair"])
    meeting(10, 13, 14, "Renewal check-in", "call", "Wren Supply",
            ["Ravi Desai", "Marco Silva"])
    meeting(11, 10, 11, "Campus licence Q&A", "meet", "Ashby College",
            ["Tess Okafor", "Owen Hart"])
    return {"next": 100, "meetings": meetings}


# JSON as meetings.json holds it: no NaN or
# Infinity, no lone surrogates, and whole floats written as integers.

def _reject(constant):
    raise ValueError(constant)


def decode(text):
    value = json.loads(text, parse_constant=_reject)
    encode(value).encode("utf-8")  # a lone surrogate raises here
    return value


def tidy(value):
    if isinstance(value, float):
        value = float("%.15g" % value)
        return int(value) if value.is_integer() and abs(value) < 1e15 else value
    if isinstance(value, dict):
        return {key: tidy(item) for key, item in value.items()}
    if isinstance(value, list):
        return [tidy(item) for item in value]
    return value


def encode(value, pretty=False):
    if pretty:
        return json.dumps(tidy(value), sort_keys=True, ensure_ascii=False,
                          indent=3, separators=(",", " : ")) + "\n"
    return json.dumps(tidy(value), sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# A value from a message, which may be a string, number or boolean: as
# text, and as a number.

def text(value):
    if value is None:
        return ""
    if isinstance(value, bool):
        return "1" if value else "0"
    if isinstance(value, float):
        return "%.15g" % value
    return str(value)


def number(value):
    if isinstance(value, (bool, int, float)):
        return value
    if isinstance(value, str):
        found = re.match(r"\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?", value)
        return float(found.group(0)) if found else 0
    # A reference counts as its (positive) address.
    return 1 if isinstance(value, (dict, list)) else 0


def scalar(value):
    return value is not None and not isinstance(value, (bool, dict, list))


store = None


def load():
    global store
    try:
        with open(FILE, encoding="utf-8", errors="replace") as fh:
            store = decode(fh.read())
    except (OSError, ValueError):
        store = None
    if not isinstance(store, dict) or not isinstance(store.get("meetings"), list):
        store = sample()
        save()


def save():
    try:
        os.makedirs(DATA, exist_ok=True)
        fh = open(FILE + ".tmp", "w", encoding="utf-8")
    except OSError:
        return
    try:
        with fh:
            fh.write(encode(store, pretty=True))
        os.rename(FILE + ".tmp", FILE)
    except OSError:
        pass


def find(id):
    if not scalar(id):
        return None
    for meeting in store["meetings"]:
        if isinstance(meeting, dict) and text(meeting.get("id")) == text(id):
            return meeting
    return None


def out():
    now = time.localtime()
    hour = now.tm_hour + (now.tm_min // 15) / 4
    meetings = sorted(store["meetings"],
                      key=lambda meeting: (text(meeting.get("date")), number(meeting.get("start"))))
    return {
        "available": True,
        "today": today(),
        "now": {"date": today(), "hour": hour},
        "me": ME,
        "meetings": meetings,
    }


def receive(line):
    try:
        value = decode(line)
    except ValueError:
        return
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    schedule = msg.get("schedule")
    if msg.get("reschedule") is not None and find(msg["reschedule"]):
        meeting = find(msg["reschedule"])
        meeting["date"] = shift_date(meeting.get("date"), 1)
    elif isinstance(schedule, dict) and DATE.match(text(schedule.get("date"))):
        start = schedule.get("start")
        if not (scalar(start) and re.match(r"\d+(\.\d+)?\n?\Z", text(start), re.ASCII)
                and number(start) < 23):
            start = 9
        start = number(start)
        title = schedule.get("title")
        if not (scalar(title) and re.search(r"\S", text(title))):
            title = "New meeting"
        now = time.localtime()
        store["meetings"].append({
            "id": "m%s" % text(store.get("next")), "title": text(title)[:80],
            "date": schedule["date"], "start": start, "finish": start + 1,
            "mark": "video", "host": ME, "related": "Tally team",
            "created": {"date": long_date(datetime.date(now.tm_year, now.tm_mon, now.tm_mday)),
                        "time": "%d:%02d %s" % ((now.tm_hour + 11) % 12 + 1, now.tm_min,
                                                "AM" if now.tm_hour < 12 else "PM"),
                        "zone": time.strftime("%Z", now)},
            "participants": [], "agenda": "Added from the calendar.",
            "link": "https://meet.tally.example/new", "notes": "",
        })
        store["next"] = number(store.get("next")) + 1
    else:
        return
    save()


def main():
    load()
    last = ""
    buffer = b""
    fd = sys.stdin.fileno()
    while True:
        line = encode(out())
        if line != last:
            sys.stdout.buffer.write(line.encode("utf-8") + b"\n")
            sys.stdout.buffer.flush()
            last = line
        if select.select([fd], [], [], 60)[0]:
            got = os.read(fd, 65536)
            if not got:
                sys.exit(0)
            buffer += got
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                receive(line.decode("utf-8", "replace"))


main()
