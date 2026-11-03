#!/usr/bin/env python3
# The projects on this machine, for apps that work across many repositories:
# every git repository under the project folders, with its branch and its
# changes, and for the one an app has open, its changed files, its folders
# and one file's text. Prints one JSON line whenever that changes; reads the
# app's messages, one `{"message": ...}` per line, on stdin. Exits when the
# app closes stdin.
#
# Output (`app.projects`):
#   now       the time, in Unix seconds (views have no clock)
#   roots     the project folders looked in, as ~/...
#   projects  up to MAX_PROJECTS, most recently active first:
#             { path, name, home (its project folder, as ~/...), branch,
#               files, added, deleted, ahead, behind, time, subject,
#               worktrees }
#             worktrees: the repository's work trees, the main one first:
#             { path, branch, primary } (at most MAX_WORKTREES)
#   current   the open project (the most recently active one until a
#             message selects another), or absent:
#             { path, name, home, branch, scope, added, deleted, count,
#               truncated, files = { "<status>\t<added>\t<deleted>\t<path>" },
#               log = { { sha, subject, author, time } } }
#             status is M, A, D or R; untracked files are A; log is its
#             last MAX_LOG commits
#   dirs      the open project's folders that are open, by path from its
#             root ("" is the root): { "<kind><name>" }, where kind is d (a
#             folder), f (a file), and D or F when git ignores it
#   file      the file being read: { path, lines, total, truncated, binary,
#             missing }
#   forge     what `gh` says of the open project, once asked (`forge`):
#             { path, prs = { { number, title, branch, author, draft,
#               updated } }, runs = { { id, title, workflow, branch, status,
#               conclusion, created } }, error? }
#   action    the last write's outcome: { id, action, ok, message | error }
#   running   the write under way, by its action's name, or absent
#
# Messages (any combination of keys in one table):
#   select = "<path>"   open that project
#   scope = "uncommitted" | "branch" | "last"   which changes `current` lists
#   open = "a\na/b"     the open project's open folders, one per line
#   complete = "a/b", cwd = "<project>"  also list this folder for a composer;
#                       independent of `open`, discarded on project changes
#   read = "<path>"     read that file of the open project; false stops
#   refresh = true      look again now
#   forge = true        ask `gh` for the open project's pull requests and
#                       workflow runs, again each minute while asked;
#                       false stops
#
# Writes, only from the mods named in MOONFRAME_PROJECTS_WRITERS (comma-
# separated; default `source-control,navigator`), one at a time and in the
# background, so a signing key waiting for a tap holds nothing up:
#   commit = "<message>", id = ...   stage everything in the open project
#                                    and commit it
#   pull = true, id = ...            pull the open project (fast-forward)
#   push = true, id = ...            push its branch to origin
#   worktree = "<name>", repo = "<path>", id = ...
#                                    add a work tree of that repository on
#                                    a new branch <name>, in
#                                    ~/.moonframe/worktrees/<repo>/<name>
#
# The project folders are MOONFRAME_PROJECTS (colon-separated), else those
# of ~/code ~/dev ~/src ~/Projects ~/projects ~/work ~/.herdr/worktrees
# that exist. Each is searched two levels deep. It runs git with your
# access, and reads only inside the open project.
#
# Python 3.9 or later and its standard library; values are capped to stay
# well under the host's limits (10,000 values and 1 MiB a line).

import calendar
import datetime
import glob
import json
import os
import re
import select
import subprocess
import sys
import threading
import time

MAX_PROJECTS = 40
MAX_FILES = 300  # entries in current.files
MAX_ENTRIES = 400  # entries listed per folder
MAX_DIRS = 60  # open folders listed
MAX_LINES = 3000  # lines of a file read
MAX_LINE = 400  # characters a line keeps
MAX_BYTES = 2 * 1024 * 1024
MAX_WORKTREES = 12  # work trees listed per project
MAX_LOG = 30  # commits in current.log
FORGE_EVERY = 60  # seconds between asks to gh
SCAN_EVERY = 15  # seconds between looks for projects
LOOK_EVERY = 3  # seconds between looks at the open project
HIDDEN = {".git", ".DS_Store"}

HOME = os.environ.get("HOME", "")
# An app opened from the Dock has no shell's PATH; gh is often in Homebrew's.
os.environ["PATH"] = ":".join(
    p for p in (os.environ.get("PATH", "/usr/bin:/bin"), "/opt/homebrew/bin", "/usr/local/bin") if p
)
GIT_ENV = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
RUN_ENV = dict(os.environ, GIT_TERMINAL_PROMPT="0")


def encode(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def decode(data):
    # Names on disk and in git's output need not be UTF-8; they keep their
    # bytes, for the file system, and print with a replacement character.
    return data.decode("utf-8", "surrogateescape")


def expand(path):
    return re.sub(r"^~(?=/|$)", lambda _: HOME, path)


if "MOONFRAME_PROJECTS" in os.environ:
    _wanted = os.environ["MOONFRAME_PROJECTS"]
else:
    _wanted = ":".join(
        HOME + "/" + d for d in ("code", "dev", "src", "Projects", "projects", "work", ".herdr/worktrees")
    )
roots = [r for r in (expand(p) for p in _wanted.split(":")) if os.path.isdir(r)]

# ------------------------------------------------------------- helpers ----


def tilde(path):
    if HOME:
        return re.sub("^" + re.escape(HOME) + "(?=/|$)", "~", path)
    return path


def last_name(path):
    """The last part of a path, or None when it ends in a slash."""
    m = re.search(r"([^/]+)$", path)
    return m.group(1) if m else None


def truthy(value):
    """True as the app meant it: not false, nil, 0 or an empty string."""
    if value is None or value is False:
        return False
    if isinstance(value, (int, float)):
        return value != 0
    if isinstance(value, str):
        return value not in ("", "0")
    return True


def text_of(value):
    """A message's string (or number, as text); None for anything else."""
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return str(int(value)) if value.is_integer() and abs(value) < 1e15 else "%.15g" % value
    return None


def digits(text):
    return text is not None and re.match(r"^[0-9]+$", text) is not None


# Runs git in dir; its output, or None when it fails.
def git(dir, *args):
    try:
        done = subprocess.run(
            ["git", "-C", dir] + list(args),
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=GIT_ENV,
        )
    except OSError:
        return None
    return decode(done.stdout) if done.returncode == 0 else None


# The git folder of the work tree at dir: `.git`, or where a worktree's
# `.git` file points.
def git_dir(dir):
    dot = dir + "/.git"
    if os.path.isdir(dot):
        return dot
    if os.path.isfile(dot):
        try:
            with open(dot, "rb") as fh:
                line = decode(fh.readline())
        except OSError:
            return None
        m = re.match(r"^gitdir:\s*(.+?)\s*$", line)
        if m:
            target = m.group(1)
            if not target.startswith("/"):
                target = dir + "/" + target
            return target
    return None


def mtime(path):
    try:
        return int(os.stat(path).st_mtime)
    except OSError:
        return 0


# ------------------------------------------------------------ projects ----

summary = {}  # path -> { key, data }


def glob_sorted(pattern):
    # Sorted ignoring case, then by case.
    return sorted(glob.glob(pattern), key=lambda p: (p.lower(), p))


def find_projects():
    found = []
    seen = set()
    for root in roots:
        for depth in (1, 2):
            pattern = glob.escape(root) + ("/*" if depth == 1 else "/*/*")
            for dir in glob_sorted(pattern):
                if dir in seen or not os.path.isdir(dir) or not os.path.exists(dir + "/.git"):
                    continue
                if any(dir.startswith(s + "/") for s in seen):  # inside another
                    continue
                seen.add(dir)
                # A project two deep groups under its parent: the
                # worktrees of one repository, say.
                group = root if depth == 1 else re.sub(r"/[^/]+$", "", dir, count=1)
                found.append({"path": dir, "group": group})
    return found


# What a project's summary depends on: its git folder's activity, and the
# half minute, since edits to the work tree touch nothing there.
def activity(dir):
    gd = git_dir(dir) or dir + "/.git"
    return max(mtime(gd + "/index"), mtime(gd + "/HEAD"), mtime(gd + "/logs/HEAD"), 0)


# The script run in each project: status, line counts, and the last
# commit, separated by \001.
SUMMARY = r"""
    export GIT_OPTIONAL_LOCKS=0
    git -C "$1" status --porcelain=v2 --branch -z --untracked-files=normal
    printf '\001'
    git -C "$1" diff HEAD --numstat --no-renames
    printf '\001'
    git -C "$1" log -1 --format='%ct%x09%s'
    printf '\001'
    git -C "$1" worktree list --porcelain
"""


# Branch, ahead and behind, changed files and lines, and the last commit of
# each project, each project's git running at once with the others'.
def summarise_all(projects):
    running, out = [], []
    for project in projects:
        dir = project["path"]
        when = activity(dir)
        # A work tree added or removed touches the main git folder's list.
        gd = git_dir(dir) or dir + "/.git"
        key = ":".join(str(n) for n in (when, int(time.time()) // 30, mtime(gd + "/worktrees"), mtime(gd + "/..")))
        cached = summary.get(dir)
        if cached and cached["key"] == key:
            out.append(cached["data"])
            continue
        try:
            child = subprocess.Popen(
                ["sh", "-c", SUMMARY, "summary", dir],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            )
        except OSError:
            continue
        running.append((project, child, when, key))
    for project, child, when, key in running:
        text = decode(child.stdout.read())
        child.stdout.close()
        child.wait()
        dir = project["path"]
        parts = text.split("\001", 3)
        status, numstat, log, trees = parts + [""] * (4 - len(parts))
        data = {
            "path": dir, "name": last_name(dir), "home": tilde(project["group"]),
            "branch": "", "files": 0, "added": 0, "deleted": 0, "ahead": 0, "behind": 0,
            "time": when, "subject": "", "worktrees": [],
        }
        # The repository's work trees, the main one first, as git lists them.
        for block in trees.split("\n\n"):
            m = re.search(r"^worktree (.+)$", block, re.M)
            if not m or re.search(r"^bare$", block, re.M):
                continue
            if len(data["worktrees"]) >= MAX_WORKTREES:
                break
            ref = re.search(r"^branch refs/heads/(.+)$", block, re.M)
            data["worktrees"].append({
                "path": m.group(1), "branch": ref.group(1) if ref else "HEAD",
                "primary": not data["worktrees"],
            })
        for entry in status.split("\0"):
            m = re.match(r"^# branch\.head (.+)$", entry)
            if m:
                data["branch"] = "HEAD" if m.group(1) == "(detached)" else m.group(1)
                continue
            m = re.match(r"^# branch\.ab \+(\d+) -(\d+)$", entry)
            if m:
                data["ahead"], data["behind"] = int(m.group(1)), int(m.group(2))
            elif re.match(r"^[12u?] ", entry):
                data["files"] += 1
        for line in numstat.split("\n"):
            fields = line.split("\t")
            if digits(fields[0]):
                data["added"] += int(fields[0])
            if len(fields) > 1 and digits(fields[1]):
                data["deleted"] += int(fields[1])
        m = re.match(r"^(\d+)\t(.*)", log)
        if m:
            data["subject"] = m.group(2)[:200]
            if int(m.group(1)) > data["time"]:
                data["time"] = int(m.group(1))
        summary[dir] = {"key": key, "data": data}
        out.append(data)
    return out


# ------------------------------------------------------------- current ----

state = {"select": None, "scope": "uncommitted", "open": [], "read": None}
# A composer's directory listing is independent of the inspector's folds.
completion = None
SCOPE = r"^(uncommitted|branch|last)$"

# What the app asked for, kept across restarts (an edit to this folder, an
# update), so a file being read or a project being shown stays so; the
# mods asked once and will not ask again. In the cache, not this folder,
# whose changes restart the provider. One file per app (profile).
_xdg = os.environ.get("XDG_CACHE_HOME", "")
CACHE = (_xdg if _xdg.startswith("/") else HOME + "/.cache") + "/moonframe/projects"
# The host names the profile it runs; outside one, `default`. Byte by byte,
# so a name's file is the one it always was.
_who = os.environ.get("MOONFRAME_PROFILE", "default").encode("utf-8", "surrogateescape")
KEPT = CACHE + "/" + re.sub(rb"[^A-Za-z0-9_-]", b"-", _who).decode("ascii") + ".json"


def restore():
    try:
        with open(KEPT, "rb") as fh:
            kept = json.loads(decode(fh.read()))
    except (OSError, ValueError):
        return
    if not isinstance(kept, dict):
        return
    if isinstance(kept.get("select"), str) and os.path.isdir(kept["select"]):
        state["select"] = kept["select"]
    if isinstance(kept.get("scope"), str) and re.match(SCOPE, kept["scope"]):
        state["scope"] = kept["scope"]
    if isinstance(kept.get("open"), list):
        state["open"] = [p for p in kept["open"] if not isinstance(p, (list, dict, bool))]
    if isinstance(kept.get("read"), str):
        state["read"] = kept["read"]


def keep():
    try:
        os.makedirs(CACHE, exist_ok=True)
        with open(KEPT + ".tmp", "wb") as fh:
            fh.write(encode(state).encode("utf-8", "surrogateescape"))
        os.rename(KEPT + ".tmp", KEPT)
    except OSError:
        pass


# The commit a branch's changes count from: where it left the default branch.
def branch_base(dir):
    for ref in ("origin/HEAD", "origin/main", "origin/master", "main", "master"):
        base = git(dir, "merge-base", "HEAD", ref)
        m = re.match(r"^([0-9a-f]+)", base or "")
        if m:
            return m.group(1)
    return None


def changes(dir, scope):
    if scope == "last":
        span = ["HEAD~1", "HEAD"]
    elif scope == "branch":
        span = [branch_base(dir) or "HEAD"]
    else:
        span = ["HEAD"]

    files, order = {}, []
    names = git(dir, "diff", *span, "--name-status", "-z", "--no-renames")
    if names is not None:
        parts = names.split("\0")
        while len(parts) >= 2:
            status, path = parts[0][:1], parts[1]
            del parts[:2]
            files[path] = {"status": status, "added": 0, "deleted": 0}
            order.append(path)
    numstat = git(dir, "diff", *span, "--numstat", "-z", "--no-renames")
    if numstat is not None:
        for record in numstat.split("\0"):
            fields = record.split("\t", 2)
            if len(fields) < 3 or fields[2] not in files:
                continue
            added, deleted, path = fields
            files[path]["added"] = int(added) if digits(added) else 0
            files[path]["deleted"] = int(deleted) if digits(deleted) else 0
    if scope != "last":
        untracked = git(dir, "ls-files", "--others", "--exclude-standard", "-z") or ""
        for path in untracked.split("\0"):
            if path == "" or path in files:
                continue
            lines = 0
            full = dir + "/" + path
            try:
                if os.path.isfile(full) and os.path.getsize(full) < 512 * 1024:
                    with open(full, "rb") as fh:
                        data = fh.read()
                    lines = data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0)
            except OSError:
                pass
            files[path] = {"status": "A", "added": lines, "deleted": 0}
            order.append(path)
    added = deleted = 0
    listed = []
    for path in sorted(order):
        f = files[path]
        added += f["added"]
        deleted += f["deleted"]
        if len(listed) < MAX_FILES:
            listed.append("\t".join((f["status"], str(f["added"]), str(f["deleted"]), path)))
    return {
        "files": listed, "count": len(order), "truncated": len(order) > MAX_FILES,
        "added": added, "deleted": deleted,
    }


# A path inside the project, by a relative path without `..`.
def inside(rel):
    return rel is not None and not re.search(r"(^|/)\.\.(/|$)", rel) and not rel.startswith("/")


def contained(root, full):
    """Whether `full`, links followed, is still inside the repository: a link
    in a cloned repository could point anywhere the user can read."""
    base = os.path.realpath(root)
    return os.path.commonpath([os.path.realpath(full), base]) == base


def list_dir(root, rel):
    base = root if rel == "" else root + "/" + rel
    try:
        names = [n for n in os.listdir(base) if n not in HIDDEN and "\n" not in n]
    except OSError:
        return None
    dirs, files = [], []
    for name in names:
        (dirs if os.path.isdir(base + "/" + name) else files).append(name)
    dirs.sort(key=str.lower)
    files.sort(key=str.lower)
    entries = ([(n, True) for n in dirs] + [(n, False) for n in files])[:MAX_ENTRIES]
    ignored = set()
    if entries:
        prefix = "" if rel == "" else rel + "/"
        given = "".join(prefix + name + "\0" for name, _ in entries)
        try:
            done = subprocess.run(
                ["git", "-C", root, "check-ignore", "-z", "--stdin"],
                input=given.encode("utf-8", "surrogateescape"),
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            )
            for path in decode(done.stdout).split("\0"):
                if path:
                    ignored.add(last_name(path))
        except OSError:
            pass
    kinds = []
    for name, is_dir in entries:
        kind = "d" if is_dir else "f"
        if name in ignored:
            kind = kind.upper()
        kinds.append(kind + name)
    return kinds


file_cache = {}  # path -> { mtime, data }


def read_file(root, rel):
    full = root + "/" + rel
    when = mtime(full)
    cached = file_cache.get(full)
    if cached and cached["mtime"] == when:
        return cached["data"]
    data = {"path": rel, "lines": [], "total": 0, "truncated": False, "binary": False, "missing": False}
    if not os.path.isfile(full) or not contained(root, full):
        data["missing"] = True
    else:
        try:
            with open(full, "rb") as fh:
                raw = fh.read(MAX_BYTES)
        except OSError:
            raw = None
        if raw is None:
            pass
        elif b"\0" in raw[:8192]:
            data["binary"] = True
        else:
            lines = re.split(r"\r?\n", raw.decode("utf-8", "replace"))
            if lines and lines[-1] == "":
                lines.pop()
            data["total"] = len(lines)
            if len(lines) > MAX_LINES:
                del lines[MAX_LINES:]
                data["truncated"] = True
            for i, line in enumerate(lines):
                line = re.sub(r"[\x00-\x08\x0b-\x1f\x7f]", "", line.replace("\t", "    "))
                lines[i] = line[:MAX_LINE]
            data["lines"] = lines
    file_cache[full] = {"mtime": when, "data": data}
    return data


# The last MAX_LOG commits of the open project.
def commit_log(dir):
    text = git(dir, "log", "-%d" % MAX_LOG, "--format=%h%x09%ct%x09%an%x09%s") or ""
    log = []
    for line in text.split("\n"):
        fields = line.split("\t", 3)
        if len(fields) < 4:
            continue
        sha, when, author, subject = fields
        log.append({"sha": sha, "time": int(when) if digits(when) else 0, "author": author[:60],
                    "subject": subject[:200]})
    return log


# ---------------------------------------------------------- background ----

# Writes and `gh`, which may wait (on the network, on a key to sign with),
# run in a thread each; the loop learns each is done when its pipe closes.
jobs = {}  # fd -> { result, done }


# Starts `run` in a thread; `done` gets what it returned, in the loop.
def start_job(run, done):
    try:
        r, w = os.pipe()
    except OSError:
        return False
    job = {"result": None, "done": done}

    def work():
        try:
            job["result"] = run()
        except Exception as e:  # noqa: BLE001 - any failure is the answer
            job["result"] = {"ok": False, "error": str(e)}
        os.close(w)

    jobs[r] = job
    threading.Thread(target=work, daemon=True).start()
    return True


# Hands on what a finished job returned. True when done.
def read_job(fd):
    job = jobs.get(fd)
    if job is None:
        return False
    if os.read(fd, 65536):
        return False
    del jobs[fd]
    os.close(fd)
    result = job["result"]
    job["done"](result if isinstance(result, dict) else {"ok": False, "error": "no answer"})
    return True


# Runs a command, stderr with stdout: its exit status and text.
def run(dir, *command):
    try:
        done = subprocess.run(
            list(command), cwd=dir, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=RUN_ENV,
        )
    except OSError:
        return 127, ""
    return (done.returncode if done.returncode >= 0 else 128 - done.returncode), \
        done.stdout.decode("utf-8", "replace").rstrip()


# --------------------------------------------------------------- writes ---

WRITERS = {w for w in re.split(r"\s*,\s*", os.environ.get("MOONFRAME_PROJECTS_WRITERS", "source-control,navigator")) if w}
action = None
running = None
rescan = False


def finish(name, id, result):
    global action, running, rescan
    running = None
    action = dict(result, action=name, id=name if id is None else id)
    action["ok"] = truthy(action.get("ok"))
    rescan = True  # look again: a commit or a work tree changes what shows


def write_action(sender, name, id, run):
    global running
    if (text_of(sender) or "") not in WRITERS:
        finish(name, id, {"ok": False, "error": "`%s` may not write; see MOONFRAME_PROJECTS_WRITERS"
                          % (text_of(sender) or "?")})
        return
    if running is not None:
        finish(name, id, {"ok": False, "error": "%s is still running" % running})
        return
    running = name
    if not start_job(run, lambda result: finish(name, id, result)):
        finish(name, id, {"ok": False, "error": "could not start"})


def commit(dir, message):
    status, text = run(dir, "git", "add", "-A")
    if status:
        return {"ok": False, "error": text or "git add failed"}
    status, text = run(dir, "git", "commit", "-m", message)
    if status:
        return {"ok": False, "error": text or "git commit failed"}
    return {"ok": True, "message": text.split("\n")[0] if text else "Committed"}


def sync(dir, what):
    command = ["git", "pull", "--ff-only"] if what == "pull" else ["git", "push", "-u", "origin", "HEAD"]
    status, text = run(dir, *command)
    lines = [line for line in text.split("\n") if re.search(r"\S", line)]
    if status:
        return {"ok": False, "error": lines[-1] if lines else "git %s failed" % what}
    return {"ok": True, "message": lines[-1] if lines else ("Pulled" if what == "pull" else "Pushed")}


def add_worktree(repo, name):
    where = "%s/.moonframe/worktrees/%s/%s" % (HOME, last_name(repo) or "", name)
    if os.path.exists(where):
        return {"ok": False, "error": "%s already exists" % where}
    status, text = run(repo, "git", "worktree", "add", "-b", name, where)
    if status:
        return {"ok": False, "error": text or "git worktree add failed"}
    return {"ok": True, "message": "Work tree %s at %s" % (name, tilde(where)), "path": where}


# ---------------------------------------------------------------- forge ---

# GitHub's pull requests and runs for the open project, through `gh`.
forge = None
forge_ask = {"want": False, "at": 0, "path": "", "busy": False}


def stamp(iso):
    m = re.match(r"^(\d+)-(\d+)-(\d+)T(\d+):(\d+):(\d+)", iso) if isinstance(iso, str) else None
    if not m:
        return 0
    try:
        return calendar.timegm(datetime.datetime(*(int(g) for g in m.groups())).timetuple())
    except (ValueError, OverflowError):
        return 0


def number(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def gh_list(text):
    try:
        items = json.loads(text)
    except ValueError:
        return []
    return [i for i in items if isinstance(i, dict)] if isinstance(items, list) else []


def ask_forge(dir, now):
    if not forge_ask["want"] or forge_ask["busy"]:
        return
    if forge_ask["path"] == dir and now - forge_ask["at"] < FORGE_EVERY:
        return
    forge_ask.update(path=dir, at=now, busy=True)

    def ask():
        result = {"path": dir, "prs": [], "runs": []}
        status, text = run(dir, "gh", "pr", "list", "--limit", "20",
                           "--json", "number,title,headRefName,author,isDraft,updatedAt")
        if status:
            result["error"] = text.split("\n")[0] if text else "gh failed"
            return result
        result["prs"] = [{
            "number": number(p.get("number")), "title": p.get("title") or "",
            "branch": p.get("headRefName") or "", "author": (p.get("author") or {}).get("login") or "",
            "draft": truthy(p.get("isDraft")), "updated": stamp(p.get("updatedAt")),
        } for p in gh_list(text)]
        status, text = run(dir, "gh", "run", "list", "--limit", "15",
                           "--json", "databaseId,displayTitle,workflowName,headBranch,status,conclusion,createdAt")
        result["runs"] = [{
            "id": number(r.get("databaseId")), "title": r.get("displayTitle") or "",
            "workflow": r.get("workflowName") or "", "branch": r.get("headBranch") or "",
            "status": r.get("status") or "", "conclusion": r.get("conclusion") or "",
            "created": stamp(r.get("createdAt")),
        } for r in ([] if status else gh_list(text))]
        return result

    def done(result):
        global forge
        forge_ask["busy"] = False
        if result.get("path") == dir:
            forge = result

    if not start_job(ask, done):
        forge_ask["busy"] = False


# --------------------------------------------------------------- loop -----

projects = []
last_scan = 0
last_out = ""


def emit(force):
    global projects, last_scan, last_out, rescan
    now = int(time.time())
    if force or rescan or now - last_scan >= SCAN_EVERY:
        rescan = False
        projects = sorted(summarise_all(find_projects()), key=lambda p: -p["time"])[:MAX_PROJECTS]
        last_scan = now
        # Until an app picks one, the most recently active project is open.
        if state["select"] is None and projects:
            state["select"] = projects[0]["path"]
    # To the minute: ages show no finer, and a clock that ticks every look
    # would publish, and redraw, with nothing else new.
    out = {"available": True, "now": now - now % 60, "roots": [tilde(r) for r in roots], "projects": projects}
    dir = state["select"]
    if dir is not None and os.path.isdir(dir) and os.path.exists(dir + "/.git"):
        parent = re.sub(r"/[^/]+$", "", dir, count=1)
        (info,) = summarise_all([{"path": dir, "group": parent}]) or [{}]
        out["current"] = dict(
            changes(dir, state["scope"]), path=dir, name=info.get("name"), home=info.get("home"),
            branch=info.get("branch"), scope=state["scope"], log=commit_log(dir),
        )
        ask_forge(dir, now)
        if forge and forge["path"] == dir:
            out["forge"] = forge
        dirs = {}
        count = 0
        for rel in dict.fromkeys([""] + ([completion] if completion else []) + [text_of(p) for p in state["open"]]):
            if not inside(rel):
                continue
            if count > MAX_DIRS:
                break
            count += 1
            entries = list_dir(dir, rel)
            if entries is not None:
                dirs[rel] = entries
        out["dirs"] = dirs
        if state["read"] is not None and inside(state["read"]):
            out["file"] = read_file(dir, state["read"])
    if action:
        out["action"] = action
    if running is not None:
        out["running"] = running
    text = encode(out)
    if force or text != last_out:
        sys.stdout.buffer.write((text + "\n").encode("utf-8", "replace"))
        sys.stdout.buffer.flush()
        last_out = text


def receive(line):
    global completion
    try:
        value = json.loads(line.decode("utf-8", "replace"))
    except ValueError:
        return
    if not isinstance(value, dict):
        return
    msg = value.get("message")
    if not isinstance(msg, dict):
        return
    path = text_of(msg.get("select"))
    if path is not None:
        path = expand(path)
        if state["select"] != path:
            state["select"] = path
            completion = None
            state["open"] = []
            state["read"] = None
    scope = text_of(msg.get("scope"))
    if scope is not None and re.match(SCOPE, scope):
        state["scope"] = scope
    if "open" in msg and not isinstance(msg["open"], (list, dict, bool)):
        state["open"] = [p for p in (text_of(msg["open"]) or "").split("\n") if p]
    if "complete" in msg and text_of(msg.get("cwd")) == state["select"]:
        completion = text_of(msg["complete"])
    if "read" in msg:
        state["read"] = text_of(msg["read"])
    if "forge" in msg:
        forge_ask["want"] = truthy(msg["forge"])
        if truthy(msg.get("refresh")):
            forge_ask["at"] = 0
    sender = value.get("from")
    id = msg.get("id") if text_of(msg.get("id")) is not None else None
    message = text_of(msg.get("commit"))
    if message is not None:
        dir = state["select"]
        if dir is None or not os.path.exists(dir + "/.git"):
            finish("commit", id, {"ok": False, "error": "No work tree open"})
        elif not re.search(r"\S", message):
            finish("commit", id, {"ok": False, "error": "Write a message to commit with"})
        else:
            write_action(sender, "commit", id, lambda dir=dir, message=message: commit(dir, message))
    for what in ("pull", "push"):
        if not truthy(msg.get(what)):
            continue
        dir = state["select"]
        if dir is None or not os.path.exists(dir + "/.git"):
            finish(what, id, {"ok": False, "error": "No work tree open"})
        else:
            write_action(sender, what, id, lambda dir=dir, what=what: sync(dir, what))
    name = text_of(msg.get("worktree"))
    if name is not None:
        repo = expand(text_of(msg.get("repo")) or "")
        if (not re.match(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$", name) or ".." in name
                or not os.path.exists(repo + "/.git")):
            finish("worktree", id, {"ok": False, "error": "A branch name (letters, digits, . _ / -) in a repository"})
        else:
            write_action(sender, "worktree", id, lambda: add_worktree(repo, name))
    keep()


def main():
    restore()
    stdin = sys.stdin.fileno()
    buffer = b""
    emit(True)
    while True:
        changed = False
        readable, _, _ = select.select([stdin] + list(jobs), [], [], LOOK_EVERY)
        for fd in readable:
            if fd == stdin:
                got = os.read(stdin, 65536)
                if not got:
                    sys.exit(0)
                buffer += got
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    receive(line)
                    changed = True
            elif read_job(fd):
                changed = True
        emit(changed)


if __name__ == "__main__":
    main()
