#!/usr/bin/env python3
# Daybook's tasks, events and notes, kept in a JSON file, and the dates the
# views need (views have no clock). Prints one JSON line whenever that
# changes; reads the app's messages, one `{"message": ...}` per line, on
# stdin. Exits when the app closes stdin.
#
# Output (`app.planner`):
#   today, selected   dates as YYYY-MM-DD; `selected` is the day shown
#   title, subtitle   "Today" (or the weekday) and "2026. October 31."
#   greeting          "Good evening"
#   days              two weeks from the selected day, for `day-strip`:
#                     { id, day, weekday, today, dot }
#   items             every task, event and note: { id, kind (task, event,
#                     note), title, date?, time?, when (the date as "Sat,
#                     Oct 31"), state (open, important, progress, done),
#                     due, overdue, habit, tags, emoji?, favorite }
#   tags              { name, color }
#   folders           folder names
#   month             the selected day's month, Monday first: { title,
#                     weeks = { { { id, day, today, outside, count } x7 } } }
#   look              the window's look: classic, sepia or light
#
# Messages (one key each):
#   select = "YYYY-MM-DD" | "today"   show that day
#   shift = 1 | -1                    the next or previous day
#   month = 1 | -1                    the same day of the next or previous month
#   toggle = id                       a task done or open again
#   cycle = id                        open → important → in progress → open
#   add = "title"                     a task for the day shown
#   remove = id
#   look = "classic" | "sepia" | "light"
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/planner.json (by default
# under ~/.local/share), started with sample items the first time.

import json
import locale
import os
import re
import select
import signal
import sys
import time

# Day and month names in the user's language, and quiet death when the
# app stops reading.
try:
    locale.setlocale(locale.LC_TIME, "")
except locale.Error:
    pass
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
# The profile is cleaned byte by byte, so a non-ASCII name always maps to
# the same folder.
_profile = re.sub(rb"[^A-Za-z0-9_-]", b"-", os.environb.get(b"MOONFRAME_PROFILE", b"daybook"))
DATA = (_xdg if _xdg.startswith("/") else HOME + "/.local/share") + "/moonframe/" + _profile.decode()
FILE = DATA + "/planner.json"
LOOK = 30  # seconds between looks at the clock and the file


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# Messages come from Luau as JSON, so a value may be a string, number or
# boolean where a string is meant; these read any of them.
def text(value):
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
    return "\0ref"  # a list or table matches no id


def truthy(value):
    if isinstance(value, str):
        return value not in ("", "0")
    if isinstance(value, (list, dict)):
        return True
    return bool(value)


def integer(value):
    if value is None or isinstance(value, (list, dict)):
        return None
    t = text(value)
    return int(t) if re.match(r"-?[0-9]+\n?\Z", t) else None


DATE = re.compile(r"([0-9]{4})-([0-9]{2})-([0-9]{2})\n?\Z")

# ---------------------------------------------------------------- dates ---


def noon(y, m, d):
    # The local time at noon that day; out-of-range days and months roll
    # over, as mktime does.
    try:
        return time.localtime(time.mktime((y, m, d, 12, 0, 0, 0, 0, -1)))
    except (OverflowError, ValueError):
        return time.localtime(0)


# YYYY-MM-DD of the day `offset` days after `date` (today if None).
def day(date=None, offset=0):
    found = DATE.match(text(date)) if date is not None else None
    if found:
        y, m, d = int(found.group(1)), int(found.group(2)), int(found.group(3))
    else:
        now = time.localtime()
        y, m, d = now.tm_year, now.tm_mon, now.tm_mday
    return time.strftime("%Y-%m-%d", noon(y, m, d + (offset or 0)))


def parts(date):
    found = DATE.match(text(date))
    if not found:
        return None
    return noon(int(found.group(1)), int(found.group(2)), int(found.group(3)))


def label(date, format):
    t = parts(date)
    return time.strftime(format, t) if t else ""


# ----------------------------------------------------------------- data ---


def sample():
    today = day()
    items = []

    def item(**fields):
        fields["id"] = "i%d" % (len(items) + 1)
        items.append(fields)

    item(kind="task", title="Water the plants", date=today, done=True)
    item(kind="task", title="Finish the quarterly review", date=today, time="17:00",
         state="progress", tags=["Work"])
    item(kind="task", title="Call Sam about the offsite", date=today, time="16:00",
         state="important", tags=["Work"])
    item(kind="task", title="Morning run", date=today, time="07:30", habit=True,
         tags=["Health"])
    item(kind="event", title="Dinner with Lea", date=today, time="19:00")
    item(kind="event", title="Design critique", date=today, time="10:00", tags=["Work"])
    item(kind="task", title="Book the train to Porto", date=day(today, -1), state="important",
         tags=["Home"])
    item(kind="task", title="Sketch the onboarding flow", tags=["Ideas"])
    item(kind="task", title="Read the research notes")
    item(kind="event", title="Team standup", date=day(today, 1), time="09:30", tags=["Work"])
    item(kind="task", title="Renew the passport", date=day(today, 3), tags=["Home"])
    item(kind="event", title="Climbing", date=day(today, 4), time="18:30", tags=["Health"])
    item(kind="note", title="Getting started", emoji="⭐", favorite=True)
    item(kind="note", title="Trip to Porto", emoji="\U0001f9f3")
    item(kind="note", title="Launch plan", emoji="\U0001f680")
    item(kind="note", title="App ideas", emoji="\U0001f4a1")
    return {
        "tags": [
            {"name": "Work", "color": "#3b6fd8"},
            {"name": "Health", "color": "#2f9e5b"},
            {"name": "Ideas", "color": "#8b5cf6"},
            {"name": "Home", "color": "#d97706"},
        ],
        "folders": ["Work", "Personal"],
        "items": items,
        "next": len(items) + 1,
    }


store = None


def load():
    global store
    try:
        with open(FILE, encoding="utf-8") as fh:
            store = json.load(fh)
    except (OSError, ValueError):
        store = None
    if not isinstance(store, dict) or not isinstance(store.get("items"), list):
        store = sample()
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


def find(id):
    id = text(id)
    for item in store["items"]:
        if text(item.get("id")) == id:
            return item
    return None


# ----------------------------------------------------------------- loop ---

selected = None


def out():
    global selected
    today = day()
    if selected is None:
        selected = today
    hour = time.localtime().tm_hour
    now = time.strftime("%H:%M")
    busy = set(text(i.get("date")) for i in store["items"]
               if i.get("kind") != "note" and not truthy(i.get("done")))
    days = []
    for offset in range(14):
        date = day(selected, offset)
        days.append({"id": date, "day": label(date, "%-d"), "weekday": label(date, "%a"),
                     "today": date == today, "dot": date in busy})
    items = []
    for original in store["items"]:
        item = dict(original)
        done = truthy(original.get("done"))
        date = item.get("date")
        when = text(date) if truthy(date) else None
        clock = text(item.get("time")) if truthy(item.get("time")) else None
        item["state"] = "done" if done else ("open" if item.get("state") is None else item["state"])
        item["done"] = done
        item["when"] = label(date, "%a, %b %-d") if when else None
        late = when is not None and (when < today or (when == today and clock is not None and clock <= now))
        item["overdue"] = item.get("kind") == "task" and not done and when is not None and when < today
        item["due"] = item.get("kind") != "note" and not done and clock is not None and late
        if item.get("time") is not None:
            item["time"] = re.sub(r"^0", "", text(item["time"]), count=1)
        if item.get("tags") is None:
            item["tags"] = []
        items.append(item)
    count = {}
    for i in store["items"]:
        if truthy(i.get("date")) and i.get("kind") != "note" and not truthy(i.get("done")):
            count[text(i["date"])] = count.get(text(i["date"]), 0) + 1
    month_key = selected[:7]
    first = month_key + "-01"
    back = parts(first).tm_wday  # 0 Monday, so the days back to Monday
    start = day(first, -back)
    weeks = []
    for week in range(6):
        row = []
        for weekday in range(7):
            date = day(start, week * 7 + weekday)
            row.append({"id": date, "day": label(date, "%-d"), "today": date == today,
                        "outside": date[:7] != month_key, "count": count.get(date, 0)})
        if week >= 4 and row[0]["outside"]:
            break
        weeks.append(row)
    if hour < 5:
        greeting = "Good night"
    elif hour < 12:
        greeting = "Good morning"
    elif hour < 18:
        greeting = "Good afternoon"
    else:
        greeting = "Good evening"
    return {
        "available": True,
        "look": "classic" if store.get("look") is None else store["look"],
        "month": {"title": label(first, "%B %Y"), "weeks": weeks},
        "today": today,
        "selected": selected,
        "title": "Today" if selected == today else label(selected, "%A"),
        "subtitle": label(selected, "%Y. %B %-d."),
        "greeting": greeting,
        "days": days,
        "items": items,
        "tags": [] if store.get("tags") is None else store["tags"],
        "folders": [] if store.get("folders") is None else store["folders"],
    }


def receive(line):
    global selected
    try:
        value = json.loads(line.decode("utf-8"))
        encode(value).encode("utf-8")  # no lone surrogates
    except ValueError:
        return
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    changed = False
    scalar = (str, int, float)
    pick = msg.get("select")
    if isinstance(pick, scalar) and not isinstance(pick, bool):
        selected = day() if text(pick) == "today" else day(pick)
    elif integer(msg.get("shift")) is not None:
        selected = day(selected, integer(msg["shift"]))
    elif integer(msg.get("month")) is not None:
        found = DATE.match(selected or day())
        y, m, d = int(found.group(1)), int(found.group(2)), int(found.group(3))
        months = y * 12 + m - 1 + integer(msg["month"])
        y, m = int(months / 12), months % 12 + 1
        # The 31st of a shorter month is its last day.
        if d > 28 and not day("%04d-%02d-%02d" % (y, m, d)).startswith("%04d-%02d" % (y, m)):
            d = 28
        selected = "%04d-%02d-%02d" % (y, m, d)
    elif msg.get("toggle") is not None and find(msg["toggle"]):
        item = find(msg["toggle"])
        item["done"] = not truthy(item.get("done"))
        changed = True
    elif msg.get("cycle") is not None and find(msg["cycle"]):
        task = find(msg["cycle"])
        following = {"open": "important", "important": "progress", "progress": "open"}
        state = task.get("state")
        task["state"] = following.get("open" if state is None else text(state), "open")
        changed = True
    elif (isinstance(msg.get("add"), scalar) and not isinstance(msg["add"], bool)
          and re.search(r"\S", text(msg["add"]))):
        title = text(msg["add"]).strip()[:200]
        number = store.get("next")
        store["next"] = (integer(number) or 0) + 1
        task = {"id": "i" + text(0 if number is None else number), "kind": "task", "title": title}
        # From the inbox: no date; else the day shown.
        if not truthy(msg.get("inbox")):
            task["date"] = selected or day()
        store["items"].append(task)
        changed = True
    elif msg.get("look") in ("classic", "sepia", "light"):
        store["look"] = msg["look"]
        changed = True
    elif msg.get("remove") is not None:
        gone = text(msg["remove"])
        store["items"] = [i for i in store["items"] if text(i.get("id")) != gone]
        changed = True
    if changed:
        save()


def main():
    load()
    last = ""
    buffer = b""
    stdin = sys.stdin.fileno()
    while True:
        line = encode(out())
        if line != last:
            sys.stdout.buffer.write(line.encode("utf-8") + b"\n")
            sys.stdout.buffer.flush()
            last = line
        ready, _, _ = select.select([stdin], [], [], LOOK)
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
