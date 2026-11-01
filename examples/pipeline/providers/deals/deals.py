#!/usr/bin/env python3
# Tally's deals, kept in a JSON file, and the figures the board shows.
# Prints one JSON line whenever that changes; reads the app's messages, one
# `{"message": ...}` per line, on stdin. Exits when the app closes stdin.
#
# Output (`app.deals`):
#   stages    { id, label, color, count, total }, in order
#   deals     { id, company, summary, amount, date, owner, stage, chance,
#               status (risk, won or absent), touched (days since contact) }
#   quota     { target, won, commit, days_left }
#   figures   { weighted, open, win_rate, win_delta, cycle_days, cycle_delta,
#               weighted_delta }
#   months    closed revenue by month, { label, value, future }
#   owners    { initials, name, color }
#
# Messages (one key each):
#   move = { id, stage }      put a deal in another stage
#   shift = { id, by = 1|-1 } the next or previous stage
#   won = id                  mark won (moves it to Closed won)
#   risk = id                 flag or unflag it as at risk
#   add = { stage, company? } a new deal in a stage
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/deals.json (by default under
# ~/.local/share), started with sample deals the first time.

import json
import os
import re
import select
import sys

HOME = os.environ.get("HOME", "")
_xdg = os.environ.get("XDG_DATA_HOME", "")
DATA = (
    (_xdg if _xdg.startswith("/") else HOME + "/.local/share")
    + "/moonframe/"
    + re.sub(r"[^A-Za-z0-9_-]", "-", os.environ.get("MOONFRAME_PROFILE", "pipeline"))
)
FILE = DATA + "/deals.json"

STAGES = [
    {"id": "lead", "label": "Lead", "color": "#9a9aa2"},
    {"id": "qualified", "label": "Qualified", "color": "#4b8df8"},
    {"id": "proposal", "label": "Proposal", "color": "#a78bfa"},
    {"id": "negotiation", "label": "Negotiation", "color": "#f5a524"},
    {"id": "won", "label": "Closed won", "color": "#22c55e"},
]
INDEX = {stage["id"]: i for i, stage in enumerate(STAGES)}


def sample():
    deals = []

    def deal(company, summary, amount, date, owner, stage, chance, **more):
        deals.append(dict(id="d%d" % (len(deals) + 1), company=company, summary=summary,
                          amount=amount, date=date, owner=owner, stage=stage, chance=chance, **more))

    deal("Bluefin Studio", "Site rebuild and hosting", 22000, "Dec 11", "MK", "lead", 10)
    deal("Cobalt Clinics", "Booking portal trial", 41500, "Jan 9", "SL", "lead", 15)
    deal("Granary Foods", "Loyalty app scoping", 14000, "Jan 21", "TO", "lead", 10)
    deal("Harbor Credit", "Reporting add-on", 49000, "Feb 3", "RD", "lead", 20)
    deal("Pinewood Labs", "Team rollout, 3 sites", 68000, "Dec 5", "TO", "qualified", 30)
    deal("Solstice Power", "Field crew licences", 43000, "Nov 27", "SL", "qualified", 35)
    deal("Marlow & Daughters", "Brand refresh retainer", 26500, "Dec 16", "MK", "qualified", 25)
    deal("Wren Supply", "Seats upgrade", 17200, "Dec 23", "RD", "qualified", 30)
    deal("Kestrel Airways", "Crew rostering suite", 132000, "Nov 29", "RD", "proposal", 50)
    deal("Ashby College", "Campus licence", 91000, "Dec 10", "TO", "proposal", 45)
    deal("Quarry Data", "Warehouse migration", 61000, "Nov 21", "SL", "proposal", 55)
    deal("Lumen Retail", "Platform renewal", 98000, "Nov 13", "TO", "negotiation", 70,
         status="risk", touched=12)
    deal("Ferro Logistics", "Fleet tracking expansion", 47500, "Nov 25", "RD", "negotiation", 80,
         touched=11)
    deal("Vesper Hotels", "Point of sale integration", 70000, "Nov 18", "MK", "negotiation", 75,
         touched=14)
    deal("Inkwell Press", "Enterprise plan", 118000, "Oct 20", "SL", "won", 100, status="won")
    deal("Fernhill Co-op", "Annual renewal", 52000, "Oct 17", "TO", "won", 100, status="won")
    deal("Orbit Works", "Seats expansion", 33500, "Oct 8", "MK", "won", 100, status="won")
    return {"next": 100, "deals": deals}


# JSON as deals.json holds it: no NaN or
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


store = None


def load():
    global store
    try:
        with open(FILE, encoding="utf-8", errors="replace") as fh:
            store = decode(fh.read())
    except (OSError, ValueError):
        store = None
    if not isinstance(store, dict) or not isinstance(store.get("deals"), list):
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
    if id is None or isinstance(id, (bool, dict, list)):
        return None
    for deal in store["deals"]:
        if isinstance(deal, dict) and text(deal.get("id")) == text(id):
            return deal
    return None


def place(deal, stage):
    if not isinstance(stage, str) or stage not in INDEX:
        return
    deal["stage"] = stage
    if stage == "won":
        deal["status"] = "won"
        deal["chance"] = 100
    elif text(deal.get("status")) == "won":
        del deal["status"]
        deal["chance"] = 50


def out():
    deals = store["deals"]
    stages = []
    for stage in STAGES:
        inside = [deal for deal in deals if text(deal.get("stage")) == stage["id"]]
        total = sum(number(deal.get("amount")) for deal in inside)
        stages.append(dict(stage, count=len(inside), total=total))
    won, weighted, open_ = 0, 0, 0
    for deal in deals:
        if text(deal.get("stage")) == "won":
            won += number(deal.get("amount"))
        else:
            weighted += number(deal.get("amount")) * number(deal.get("chance")) / 100
            open_ += 1
    target = 2_000_000
    # Earlier wins this quarter, beside the ones on the board.
    won += 1_160_000
    return {
        "available": True,
        "stages": stages,
        "deals": deals,
        "quota": {"target": target, "won": won, "commit": 280_000, "days_left": 34},
        "figures": {
            "weighted": int(weighted + 2_640_000), "open": open_ + 25, "weighted_delta": "+9.2%",
            "win_rate": "31%", "win_delta": "+4 pts", "cycle_days": 38, "cycle_delta": "+3 days",
        },
        "months": [
            {"label": "Jul", "value": 180, "future": False},
            {"label": "Aug", "value": 260, "future": False},
            {"label": "Sep", "value": 210, "future": False},
            {"label": "Oct", "value": 330, "future": False},
            {"label": "Nov", "value": 410, "future": False},
            {"label": "Dec", "value": 380, "future": True},
        ],
        "owners": [
            {"initials": "MK", "name": "Maya Kim", "color": "#4b8df8"},
            {"initials": "SL", "name": "Sami Lund", "color": "#ec4899"},
            {"initials": "RD", "name": "Ravi Desai", "color": "#22c55e"},
            {"initials": "TO", "name": "Tess Okafor", "color": "#f59e0b"},
        ],
    }


def receive(line):
    try:
        value = decode(line)
    except ValueError:
        return
    msg = value.get("message") if isinstance(value, dict) else None
    if not isinstance(msg, dict):
        return
    move, shift, add = msg.get("move"), msg.get("shift"), msg.get("add")
    if isinstance(move, dict) and find(move.get("id")):
        place(find(move.get("id")), move.get("stage"))
    elif isinstance(shift, dict) and find(shift.get("id")):
        moved = find(shift.get("id"))
        by = number(shift.get("by"))
        to = INDEX.get(text(moved.get("stage")), 0) + (by > 0) - (by < 0)
        if 0 <= to < len(STAGES):
            place(moved, STAGES[to]["id"])
    elif msg.get("won") is not None and find(msg["won"]):
        place(find(msg["won"]), "won")
    elif msg.get("risk") is not None and find(msg["risk"]):
        risky = find(msg["risk"])
        if text(risky.get("status")) == "risk":
            del risky["status"]
        elif text(risky.get("status")) != "won":
            risky["status"] = "risk"
    elif isinstance(add, dict) and isinstance(add.get("stage"), str) and add["stage"] in INDEX:
        company = add.get("company")
        if company is None or isinstance(company, (bool, dict, list)) or not re.search(r"\S", text(company)):
            company = "New deal"
        store["deals"].append({
            "id": "d%s" % text(store.get("next")), "company": text(company)[:80],
            "summary": "Added from the board", "amount": 10000, "date": "Dec 31", "owner": "MK",
            "stage": add["stage"], "chance": 10,
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
