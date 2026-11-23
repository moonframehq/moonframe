"""Read T3 Code's v2 shell snapshot, authenticated with a Sheprd pairing file."""

import http.client
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from urllib.parse import urlsplit


class Unavailable(Exception):
    """A connection or configuration problem fit to show in the overview."""


def read_json(stream, limit):
    raw = stream.read(limit + 1)
    if len(raw) > limit:
        raise Unavailable("The T3 Code response exceeds the snapshot limit.")
    return json.loads(raw)


def get(connection, path, headers):
    connection.request("GET", path, headers=headers)
    response = connection.getresponse()
    if response.status != 200:
        if response.status in (401, 403):
            raise Unavailable("T3 Code refused access. Pair again with orchestration read access.")
        raise Unavailable(f"T3 Code returned HTTP {response.status}.")
    return read_json(response, 32 << 20)


def local_server(home):
    try:
        with (home / "userdata/server-runtime.json").open("rb") as stream:
            origin = urlsplit(read_json(stream, 65536)["origin"])
    except FileNotFoundError:
        raise Unavailable("Start T3 Code on this machine to see its sessions.") from None
    if origin.scheme != "http" or origin.hostname not in {"localhost", "127.0.0.1", "::1"}:
        raise Unavailable("Expected a local T3 Code server in server-runtime.json.")
    return http.client.HTTPConnection(origin.hostname, origin.port, timeout=6)


def credential_path(home):
    return Path(os.getenv("MOONFRAME_T3_TOKEN_FILE") or home / "moonframe-token.json").expanduser()


def environment(connection):
    descriptor = get(connection, "/.well-known/t3/environment", {})
    if descriptor["orchestrationProtocolVersion"] != 2:
        raise Unavailable("This provider requires T3 orchestration protocol v2.")
    return descriptor["environmentId"]


def connect(home):
    """Explicit one-time setup using T3 Code's own local auth CLI."""
    connection = local_server(home)
    try:
        identity = environment(connection)
    finally:
        connection.close()
    path = home / "moonframe-token.json"
    issued = subprocess.run(
        [
            "t3",
            "auth",
            "session",
            "issue",
            "--base-dir",
            str(home),
            "--scope",
            "orchestration:read",
            "--label",
            "Moonframe",
            "--token-only",
        ],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        timeout=30,
    )
    token = issued.stdout.strip()
    if not valid_token(token):
        raise Unavailable("T3 Code did not return a bearer token.")
    with tempfile.TemporaryDirectory(dir=home) as folder:
        # Restrict the staged file before writing the token or replacing an expired one.
        staged = Path(folder) / "token"
        with staged.open("x") as stream:
            staged.chmod(0o600)
            json.dump({"environmentId": identity, "token": token}, stream)
        staged.replace(path)


def valid_token(value):
    return isinstance(value, str) and bool(value) and all(33 <= ord(c) <= 126 for c in value)


def load(home):
    path = credential_path(home)
    connection = local_server(home)
    try:
        try:
            with path.open("rb") as stream:
                saved = read_json(stream, 16384)
        except FileNotFoundError:
            raise Unavailable(
                "Run the T3 provider setup (t3.py --connect) or set MOONFRAME_T3_TOKEN_FILE."
            ) from None
        identity = environment(connection)
        if not saved["environmentId"] or saved["environmentId"] != identity:
            raise Unavailable("The pairing file belongs to a different T3 environment.")
        token = saved["token"]
        if not valid_token(token):
            raise Unavailable("Invalid T3 pairing file.")
        return overview(
            get(
                connection,
                "/api/orchestration/shell",
                {
                    "Authorization": "Bearer " + token,
                    "x-t3-orchestration-protocol": "2",
                },
            )
        )
    finally:
        connection.close()


def group(thread):
    request = thread.get("pendingRuntimeRequest")
    needs_answer = request is not None and request.get("blocking", True)
    if (
        needs_answer
        or thread.get("hasActionableProposedPlan")
        or thread.get("pendingGuestMessageCount")
    ):
        return "waiting"
    if thread.get("settledOverride") == "settled":
        return "settled"
    if thread.get("activityRunStatus") or thread["status"] in {
        "preparing",
        "queued",
        "starting",
        "running",
        "waiting",
    }:
        return "running"
    return "active"


def overview(snapshot):
    if snapshot["schemaVersion"] != 2:
        raise Unavailable("Unsupported T3 shell snapshot version.")
    projects = {item["id"]: item["title"] for item in snapshot["projects"]}
    threads = sorted(
        (t for t in snapshot["threads"] if not t.get("archivedAt") and not t.get("deletedAt")),
        key=lambda t: (t["updatedAt"], t["id"]),
        reverse=True,
    )
    rows = []
    # Leave space under Moonframe's 1 MiB and 10,000-value publish limits.
    remaining = 1000000
    for thread in threads[:200]:
        prs = {}
        for pr in thread.get("pullRequests", []):
            if pr.get("source") != "stack-dismissed":
                prs[pr["url"]] = pr
        for key in ("linkedPullRequest", "branchPullRequest"):
            pr = thread.get(key)
            if pr:
                prs.setdefault(pr["url"], pr)
        row = {
            "id": thread["id"],
            "title": thread["title"],
            "project": projects.get(thread["projectId"], thread["projectId"]),
            "projectId": thread["projectId"],
            "status": thread["status"],
            "group": group(thread),
            "updatedAt": thread["updatedAt"],
            "branch": thread.get("branch"),
            "worktree": thread.get("worktreePath"),
            "provider": thread.get("providerInstanceId"),
            "pullRequests": [
                {"label": f'#{p["number"]}', "title": p["repository"], "url": p["url"]}
                for p in list(prs.values())[:4]
            ],
            "issues": [
                {"label": i["displayKey"], "title": i["title"], "url": i["url"]}
                for i in thread.get("issues", [])[:4]
            ],
        }
        remaining -= len(json.dumps(row)) + 1
        if remaining < 0:
            break
        rows.append(row)
    return {"available": True, "threads": rows, "total": len(threads)}


def main(home):
    # The reader only signals a refresh or EOF. Coalesce a burst of clicks.
    wake = threading.Event()
    closed = threading.Event()

    def listen():
        for _ in sys.stdin:
            wake.set()
        closed.set()
        wake.set()

    threading.Thread(target=listen, daemon=True).start()
    previous = None
    while not closed.is_set():
        wake.clear()
        try:
            state = load(home)
        except Unavailable as error:
            state = {"error": str(error)}
        except (OSError, http.client.HTTPException):
            state = {
                "error": "Cannot read the pairing file or reach T3 Code. Check the server and retry."
            }
        except (ValueError, KeyError, TypeError, AttributeError):
            state = {"error": "Unrecognized T3 Code runtime, pairing file, or shell snapshot."}
        if "error" in state:
            state.update(available=False, threads=[], total=0)
        if state != previous:
            print(json.dumps(state, separators=(",", ":")), flush=True)
            previous = state
        if not closed.is_set():
            wake.wait(15)


if __name__ == "__main__":
    home = Path(os.getenv("T3CODE_HOME") or "~/.t3").expanduser()
    if sys.argv[1:] == ["--connect"]:
        try:
            connect(home)
        except (OSError, Unavailable, subprocess.SubprocessError, ValueError, KeyError) as error:
            sys.exit(f"T3 setup failed: {error}")
        print("Connected Moonframe to T3 Code.")
    elif len(sys.argv) == 1:
        main(home)
    else:
        sys.exit("Usage: t3.py [--connect]")
