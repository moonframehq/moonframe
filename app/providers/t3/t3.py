"""Read T3 Code's v2 shell snapshot, authenticated with a Sheprd pairing file."""

import http.client
import json
import os
import queue
import secrets
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from urllib.parse import urlsplit


class Unavailable(Exception):
    """A connection or configuration problem fit to show in the overview."""


class SetupNeeded(Unavailable):
    def __init__(self, step, message):
        super().__init__(message)
        self.step = step


def cli_path():
    # Desktop launches often lack the shell PATH that T3's installer updated.
    return shutil.which("t3") or shutil.which("t3", path=str(Path.home() / ".local" / "bin"))


def require_cli():
    cli = cli_path()
    if not cli:
        raise SetupNeeded("install", "Install the T3 Code command-line tool on this computer.")
    return cli


def pairing_problem(message):
    if os.getenv("MOONFRAME_T3_TOKEN_FILE"):
        return Unavailable(message + " Check the pairing file named by MOONFRAME_T3_TOKEN_FILE.")
    require_cli()
    return SetupNeeded("pair", message)


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
            raise pairing_problem("T3 Code refused access. Pair again with orchestration read access.")
        raise Unavailable(f"T3 Code returned HTTP {response.status}.")
    return read_json(response, 32 << 20)


def local_server(home):
    try:
        with (home / "userdata/server-runtime.json").open("rb") as stream:
            origin = urlsplit(read_json(stream, 65536)["origin"])
    except FileNotFoundError:
        require_cli()
        raise SetupNeeded("start", "Run t3 in a terminal to start your local server.") from None
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
            require_cli(),
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
        identity = environment(connection)
        try:
            with path.open("rb") as stream:
                saved = read_json(stream, 16384)
        except FileNotFoundError:
            raise pairing_problem("Connect Moonframe to read sessions from your local T3 server.") from None
        if not saved["environmentId"] or saved["environmentId"] != identity:
            raise pairing_problem("The pairing file belongs to a different T3 environment.")
        token = saved["token"]
        if not valid_token(token):
            raise pairing_problem("Invalid T3 pairing file.")
        state = overview(
            get(
                connection,
                "/api/orchestration/shell",
                {
                    "Authorization": "Bearer " + token,
                    "x-t3-orchestration-protocol": "2",
                },
            )
        )
        state["environmentId"] = identity
        return state
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
    # Reserve room for the project picker and write receipts.
    choices = []
    remaining = 990000
    for project in snapshot["projects"]:
        if project.get("archivedAt"):
            continue
        choice = {"id": project["id"], "label": project["title"]}
        size = len(json.dumps(choice)) + 1
        if len(choices) >= 200 or size > remaining // 2:
            break
        choices.append(choice)
        remaining -= size
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
    return {"available": True, "threads": rows, "total": len(threads), "projects": choices}


def snapshot(home):
    step = None
    try:
        return load(home)
    except SetupNeeded as error:
        step, problem = error.step, str(error)
    except ConnectionRefusedError:
        step = "start" if cli_path() else "install"
        problem = "The local T3 server is offline. Start T3 Code and refresh."
    except Unavailable as error:
        problem = str(error)
    except (OSError, http.client.HTTPException):
        problem = "Cannot read the pairing file or reach T3 Code. Check the server and retry."
    except (ValueError, KeyError, TypeError, AttributeError):
        problem = "Unrecognized T3 Code runtime, pairing file, or shell snapshot."
    return {"available": False, "error": problem, "setup": step, "threads": [], "projects": [], "total": 0}


def configured_writer():
    profile, sep, name = os.getenv("MOONFRAME_T3_WRITER", "").partition("/")
    if sep and profile and profile == os.getenv("MOONFRAME_PROFILE"):
        return name or None
    return None


def perform_write(home, request, state):
    action = request.get("action")
    target = request.get("target")
    prompt = request.get("text")
    if not state["available"] or state["environmentId"] != request.get("environmentId"):
        raise Unavailable("The T3 environment changed or is offline. Refresh before sending.")
    if action not in ("start", "send"):
        raise Unavailable("Unknown T3 write action.")
    choices = state["projects" if action == "start" else "threads"]
    if not any(item["id"] == target for item in choices) or target.startswith("-"):
        raise Unavailable("The selected project or thread is no longer available.")
    if not isinstance(prompt, str) or not prompt.strip() or len(prompt.encode("utf-8")) > 65536:
        raise Unavailable("Enter a message of up to 64 KiB.")
    try:
        result = subprocess.run(
            [require_cli(), "thread", action, "--base-dir", str(home), "--json", target, "-"],
            input=prompt,
            encoding="utf-8",
            capture_output=True,
            timeout=30,
        )
    except FileNotFoundError:
        raise Unavailable("Moonframe could not find the t3 CLI on PATH.") from None
    except (OSError, subprocess.TimeoutExpired, UnicodeError):
        raise Unavailable(
            "Delivery could not be confirmed. Check T3 Code before retrying."
        ) from None
    if result.returncode:
        # CLI diagnostics can contain the prompt. Only the outcome is shared with other mods.
        raise Unavailable(
            "T3 did not confirm delivery. Check the thread and the CLI's project and provider settings."
        )
    try:
        reply = json.loads(result.stdout)
        thread_id = reply["threadId"]
        if not isinstance(thread_id, str) or not 0 < len(thread_id) <= 256:
            raise ValueError()
        if action == "send" and thread_id != target:
            raise ValueError()
        if action == "start":
            return {"status": "started", "threadId": thread_id}
        if reply["delivery"] not in ("queued", "started", "steered", "restarted"):
            raise ValueError()
        return {
            "status": "queued" if reply["delivery"] == "queued" else "sent",
            "threadId": thread_id,
        }
    except (KeyError, ValueError, TypeError):
        raise Unavailable("Unrecognized CLI reply. Check T3 Code before retrying.") from None


def main(home):
    # One consumer runs CLI calls in order. Bounded stdin backpressure keeps bursts bounded.
    inbox = queue.Queue(maxsize=32)

    def listen():
        for line in sys.stdin:
            try:
                envelope = json.loads(line)
            except ValueError:
                continue
            if isinstance(envelope, dict):
                inbox.put(envelope)
        inbox.put(None)

    threading.Thread(target=listen, daemon=True).start()
    writer = configured_writer()
    ticket = secrets.token_hex(16)
    receipts = {}
    previous = None
    state = snapshot(home)

    def publish():
        nonlocal previous
        value = dict(state, writer=writer, ticket=ticket if writer else None, results=receipts)
        line = json.dumps(value, separators=(",", ":"))
        if previous != line:
            print(line, flush=True)
            previous = line

    while True:
        publish()
        try:
            envelope = inbox.get(timeout=15)
        except queue.Empty:
            envelope = {"message": "refresh"}
        if envelope is None:
            return
        request = envelope.get("message")
        if request == "connect":
            if (
                state.get("setup") != "pair"
                or os.getenv("MOONFRAME_T3_TOKEN_FILE")
                or os.getenv("MOONFRAME_PROFILE") != "t3-overview"
                or envelope.get("from") != "shell"
            ):
                continue
            try:
                connect(home)
            except (OSError, http.client.HTTPException, Unavailable, ValueError, KeyError, subprocess.SubprocessError):
                state = dict(state, error="Pairing failed. Check the local t3 command and server, then try again.")
                continue
            state = snapshot(home)
            continue
        if not isinstance(request, dict):
            state = snapshot(home)
            continue
        if not writer or envelope.get("from") != writer:
            continue
        key = request.get("id")
        if not isinstance(key, str) or not 1 <= len(key) <= 80 or key in receipts:
            continue
        if len(receipts) >= 16:
            del receipts[next(iter(receipts))]
        if request.get("ticket") != ticket:
            receipts[key] = {
                "status": "error",
                "error": "The composer has changed. Review the draft and retry.",
            }
            continue
        # Consume the ticket even if the CLI times out after accepting a message.
        ticket = secrets.token_hex(16)
        receipts[key] = {"status": "sending"}
        publish()
        state = snapshot(home)
        try:
            receipts[key] = perform_write(home, request, state)
        except Unavailable as error:
            receipts[key] = {"status": "error", "error": str(error)}
        publish()
        state = snapshot(home)


if __name__ == "__main__":
    home = Path(os.getenv("T3CODE_HOME") or "~/.t3").expanduser()
    if sys.argv[1:] == ["--connect"]:
        try:
            connect(home)
        except (OSError, Unavailable, subprocess.SubprocessError, ValueError, KeyError) as error:
            sys.exit(f"T3 setup failed: {error}")
        print("Connected Moonframe to T3 Code.")
    elif len(sys.argv) == 1:
        sys.stdin.reconfigure(encoding="utf-8")
        try:
            main(home)
        except BrokenPipeError:
            # Moonframe closed its end of the process-provider stream.
            sys.stdout = open(os.devnull, "w")
    else:
        sys.exit("Usage: t3.py [--connect]")
