#!/usr/bin/env python3
# What runs on this machine, for Lantern's process monitor. Prints one JSON
# line every two seconds (as `iostat` ticks) and after each message; reads
# the app's messages, one `{"from": ..., "message": ...}` per line, on stdin.
# Exits when the app closes stdin. macOS: ps, iostat, vm_stat, sysctl.
#
# Output (`app.procs`):
#   available   false with `reason` when the tools are missing
#   cpu         busy share of all cores, 0 to 100; `user` and `system` split it
#   history     the last 60 `cpu` readings, oldest first
#   load        [1m, 5m, 15m] load averages
#   memory      { used, total, active, wired, compressed, swap } in bytes,
#               and `pressure`: "normal", "warning" or "critical"
#   groups      processes grouped by name, the busiest first (or the largest
#               with sort = "memory"), at most 80, matching `query`:
#               { name, pid (the busiest member), count, cpu (% of one core,
#               summed), mem (bytes), open, children (when open: up to 12
#               { pid, name, cpu, mem }) }
#   total       how many processes match; `query`, `sort` as asked
#
# Messages (any combination of keys):
#   query = "<text>"        keep processes whose name or pid contains it
#   sort = "cpu" | "memory"
#   expand = "<name>"       show or hide that group's members

import json
import os
import re
import select
import subprocess
import sys

MAX_GROUPS = 80
MAX_CHILDREN = 12
HISTORY = 60

state = {"query": "", "sort": "cpu", "open": set()}
history = []
cpu = {"cpu": 0, "user": 0, "system": 0, "load": [0, 0, 0]}


def run(*command):
    """A command's output, or None when it can't run or fails."""
    try:
        done = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    except OSError:
        return None
    return done.stdout.decode("utf-8", "replace") if done.returncode == 0 else None


def number(value):
    """A float that holds a whole number as an int, so JSON shows no ".0"."""
    return int(value) if value == int(value) else value


def round1(value):
    return number(int(value * 10 + 0.5) / 10)


def as_int(text, default):
    try:
        return int(text.strip())
    except (AttributeError, ValueError):
        return default


def write(value):
    text = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    sys.stdout.buffer.write(text.encode("utf-8", "replace") + b"\n")
    sys.stdout.buffer.flush()


def memory():
    total = as_int(run("sysctl", "-n", "hw.memsize"), 0)
    vm = run("vm_stat") or ""
    found = re.search(r"page size of (\d+) bytes", vm)
    page = int(found.group(1)) if found else 0
    page = page or 16384
    pages = {}
    for found in re.finditer(r"^(.+?):\s+(\d+)\.?$", vm, re.M):
        pages[found.group(1)] = int(found.group(2))
    active = pages.get("Pages active", 0) * page
    wired = pages.get("Pages wired down", 0) * page
    compressed = pages.get("Pages occupied by compressor", 0) * page
    anonymous = pages.get("Anonymous pages", 0) * page
    purgeable = pages.get("Pages purgeable", 0) * page
    # As Activity Monitor counts it: app memory, wired, compressed.
    used = anonymous - purgeable + wired + compressed
    if total and used > total:
        used = total
    swap = 0
    found = re.search(r"used = ([\d.]+)M", run("sysctl", "-n", "vm.swapusage") or "")
    if found:
        swap = int(float(found.group(1)) * 1024 * 1024)
    level = as_int(run("sysctl", "-n", "kern.memorystatus_vm_pressure_level"), 1)
    pressure = "critical" if level >= 4 else "warning" if level >= 2 else "normal"
    return {
        "used": used,
        "total": total,
        "active": active,
        "wired": wired,
        "compressed": compressed,
        "swap": swap,
        "pressure": pressure,
    }


def processes():
    text = run("ps", "-Axo", "pid=,pcpu=,rss=,comm=") or ""
    found_all = []
    for line in text.split("\n"):
        found = re.match(r"^\s*(\d+)\s+([\d.]+)\s+(\d+)\s+(.+?)\s*$", line)
        if not found:
            continue
        pid, pcpu, rss, comm = found.groups()
        name = re.sub(r".*/", "", comm, count=1)
        if name == "":
            continue
        found_all.append({"pid": int(pid), "cpu": float(pcpu), "mem": int(rss) * 1024, "name": name})
    return found_all


def groups(found_all):
    query = as_text(state["query"]).lower()
    by = {}
    total = 0
    for proc in found_all:
        if query and query not in proc["name"].lower() and query not in str(proc["pid"]):
            continue
        total += 1
        group = by.get(proc["name"])
        if group is None:
            group = by[proc["name"]] = {"name": proc["name"], "count": 0, "cpu": 0, "mem": 0, "members": []}
        group["count"] += 1
        group["cpu"] += proc["cpu"]
        group["mem"] += proc["mem"]
        group["members"].append(proc)
    key = "mem" if state["sort"] == "memory" else "cpu"
    ordered = sorted(by.values(), key=lambda g: (-g[key], g["name"]))[:MAX_GROUPS]
    out = []
    for group in ordered:
        members = sorted(group["members"], key=lambda p: -p[key])
        is_open = group["name"] in state["open"] and group["count"] > 1
        row = {
            "name": group["name"],
            "pid": members[0]["pid"],
            "count": group["count"],
            "cpu": round1(group["cpu"]),
            "mem": group["mem"],
            "open": is_open,
        }
        if is_open:
            row["children"] = [
                {"pid": p["pid"], "name": p["name"], "cpu": round1(p["cpu"]), "mem": p["mem"]}
                for p in members[:MAX_CHILDREN]
            ]
        out.append(row)
    return out, total


def emit():
    rows, total = groups(processes())
    write(
        {
            "available": True,
            "cpu": cpu["cpu"],
            "user": cpu["user"],
            "system": cpu["system"],
            "load": cpu["load"],
            "history": list(history),
            "memory": memory(),
            "groups": rows,
            "total": total,
            "query": state["query"],
            "sort": state["sort"],
        }
    )


def as_text(value):
    """A JSON scalar as a string, whole numbers without ".0"."""
    if isinstance(value, float) and value == int(value):
        return str(int(value))
    return str(value)


def receive(line):
    try:
        msg = json.loads(line)
    except ValueError:
        return
    if not msg:
        return
    if isinstance(msg, dict) and "message" in msg:
        msg = msg["message"]
    if not isinstance(msg, dict):
        return
    query = msg.get("query")
    if isinstance(query, (str, int, float)) and not isinstance(query, bool):
        state["query"] = query if isinstance(query, str) else number(query)
    if msg.get("sort") in ("cpu", "memory"):
        state["sort"] = msg["sort"]
    name = msg.get("expand")
    if isinstance(name, (str, int, float)) and not isinstance(name, bool):
        name = as_text(name)
        if name in state["open"]:
            state["open"].discard(name)
        else:
            state["open"].add(name)


def main():
    stdin = sys.stdin.fileno()
    # iostat prints a line every two seconds: "us sy id 1m 5m 15m".
    try:
        iostat = subprocess.Popen(["iostat", "-n0", "-w2"], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE)
    except OSError:
        write({"available": False, "reason": "iostat is not available"})
        # Never exit on an error: wait for the app to close stdin.
        while os.read(stdin, 65536):
            pass
        return

    watched = [stdin, iostat.stdout.fileno()]
    data, stats = b"", b""
    first = True
    emit()
    while True:
        changed = False
        readable, _, _ = select.select(watched, [], [])
        for fd in readable:
            if fd == stdin:
                got = os.read(stdin, 65536)
                if not got:
                    iostat.terminate()
                    sys.exit(0)
                data += got
                while b"\n" in data:
                    line, data = data.split(b"\n", 1)
                    receive(line)
                    changed = True
            else:
                got = os.read(fd, 4096)
                if not got:
                    watched.remove(fd)
                    continue
                stats += got
                while b"\n" in stats:
                    line, stats = stats.split(b"\n", 1)
                    found = re.match(
                        rb"^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)", line
                    )
                    if not found:
                        continue
                    # The first reading averages since boot: skip it.
                    if first:
                        first = False
                        continue
                    user, system, idle = (int(v) for v in found.groups()[:3])
                    load = [number(float(v)) for v in found.groups()[3:]]
                    cpu.update(cpu=100 - idle, user=user, system=system, load=load)
                    history.append(100 - idle)
                    del history[:-HISTORY]
                    changed = True
        if changed:
            emit()


if __name__ == "__main__":
    main()
