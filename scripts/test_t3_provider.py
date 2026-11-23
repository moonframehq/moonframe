"""Drive the process provider through its normal stdin/stdout interface."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock

PROVIDER = Path(__file__).resolve().parents[1] / "app/providers/t3/t3.py"


def thread(name, status, **fields):
    return dict(id=name, title=name, status=status, projectId="p", updatedAt=name, **fields)


class T3ProviderTest(unittest.TestCase):
    def test_cli_writes_are_scoped_correlated_and_not_replayed(self):
        spec = importlib.util.spec_from_file_location("provider", PROVIDER)
        provider = importlib.util.module_from_spec(spec)
        with mock.patch.object(sys, "dont_write_bytecode", True):
            spec.loader.exec_module(provider)
        home = Path("/fixture/t3")
        state = {
            "available": True,
            "environmentId": "local",
            "threads": [{"id": "t"}],
            "projects": [{"id": "p"}],
        }
        prompt = "Review the change.\nKeep the prompt off the command line."

        def event(sender, action, target, key, ticket, **fields):
            return {
                "from": sender,
                "message": {
                    "action": action,
                    "target": target,
                    "id": key,
                    "ticket": ticket,
                    "text": prompt,
                    "environmentId": "local",
                    **fields,
                },
            }

        start = event("shell", "start", "p", "a", "initial")
        events = [
            event("other", "start", "p", "forged", "initial"),
            start,
            start,
            event("shell", "start", "p", "stale", "initial"),
            event("shell", "send", "t", "b", "next"),
            event("shell", "send", "missing", "c", "after-send"),
            event("shell", "send", "t", "d", "after-missing", environmentId="another"),
            event("shell", "send", "t", "timeout", "after-switch"),
            event("shell", "send", "t", "unknown-delivery", "after-timeout"),
        ]
        replies = [
            subprocess.CompletedProcess([], 0, '{"threadId":"t"}'),
            subprocess.CompletedProcess([], 0, '{"threadId":"t","delivery":"queued"}'),
            subprocess.TimeoutExpired("t3", 30),
            subprocess.CompletedProcess([], 0, '{"threadId":"t","delivery":"unknown"}'),
        ]

        def run(profile):
            out = io.StringIO()
            with (
                mock.patch.dict(
                    os.environ, MOONFRAME_PROFILE=profile, MOONFRAME_T3_WRITER="overview/shell"
                ),
                mock.patch.object(provider, "snapshot", return_value=state),
                mock.patch.object(
                    provider.secrets,
                    "token_hex",
                    side_effect=[
                        "initial",
                        "next",
                        "after-send",
                        "after-missing",
                        "after-switch",
                        "after-timeout",
                        "done",
                    ],
                ),
                mock.patch.object(provider.subprocess, "run", side_effect=replies) as cli,
                mock.patch("sys.stdin", io.StringIO("".join(json.dumps(e) + "\n" for e in events))),
                contextlib.redirect_stdout(out),
            ):
                provider.main(home)
            self.assertNotIn(prompt.splitlines()[0], out.getvalue())
            return json.loads(out.getvalue().splitlines()[-1]), cli

        disabled, cli = run("another-profile")
        cli.assert_not_called()
        self.assertIsNone(disabled["writer"])
        published, cli = run("overview")
        results = published["results"]
        self.assertNotIn("forged", results)
        self.assertEqual(results["a"], {"status": "started", "threadId": "t"})
        self.assertEqual(results["b"], {"status": "queued", "threadId": "t"})
        for key in ("stale", "c", "d", "timeout", "unknown-delivery"):
            self.assertEqual(results[key]["status"], "error")
        self.assertEqual(cli.call_count, 4)
        for call, (action, target) in zip(
            cli.call_args_list, [("start", "p"), ("send", "t"), ("send", "t"), ("send", "t")]
        ):
            self.assertEqual(
                call.args[0],
                ["t3", "thread", action, "--base-dir", str(home), "--json", target, "-"],
            )
            self.assertEqual(call.kwargs["input"], prompt)

    def test_discovery_pairing_refresh_and_recovery(self):
        calls = []

        class Server(BaseHTTPRequestHandler):
            def do_GET(self):
                calls.append(
                    (
                        self.path,
                        self.headers.get("Authorization"),
                        self.headers.get("x-t3-orchestration-protocol"),
                    )
                )
                if self.path == "/.well-known/t3/environment":
                    value = {"environmentId": "local", "orchestrationProtocolVersion": 2}
                else:
                    value = {
                        "schemaVersion": 2,
                        "projects": [{"id": "p", "title": "Moonframe"}],
                        "threads": [
                            thread("a", "completed"),
                            thread("b", "completed", settledOverride="settled"),
                            thread("c", "running", pendingRuntimeRequest={"kind": "approval"}),
                            thread(
                                "d",
                                "starting",
                                pendingRuntimeRequest={"blocking": False},
                                linkedPullRequest={
                                    "repository": "mf/core",
                                    "number": 4,
                                    "url": "https://example.com/pr/4",
                                },
                                issues=[
                                    {
                                        "displayKey": "UI-2",
                                        "title": "Issue",
                                        "url": "https://example.com/issue/2",
                                    }
                                ],
                            ),
                            thread("e", "idle", archivedAt="yesterday"),
                        ],
                    }
                self.send_response(200)
                self.end_headers()
                self.wfile.write(json.dumps(value).encode())

            def log_message(self, *_):
                pass

        with tempfile.TemporaryDirectory() as folder, ThreadingHTTPServer(
            ("127.0.0.1", 0), Server
        ) as server:
            threading.Thread(target=server.serve_forever, daemon=True).start()
            home = Path(folder)
            (home / "userdata").mkdir()
            runtime = home / "userdata/server-runtime.json"
            token = home / "token.json"
            env = dict(os.environ, T3CODE_HOME=folder, MOONFRAME_T3_TOKEN_FILE=str(token))
            with subprocess.Popen(
                [sys.executable, "-I", str(PROVIDER)],
                env=env,
                text=True,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
            ) as child:

                def refresh():
                    child.stdin.write('{"from":"overview","message":"refresh"}\n')
                    child.stdin.flush()
                    return json.loads(child.stdout.readline())

                try:
                    self.assertFalse(json.loads(child.stdout.readline())["available"])
                    token.write_text(json.dumps({"environmentId": "different", "token": "private"}))
                    runtime.write_text(
                        json.dumps({"origin": f"http://127.0.0.1:{server.server_port}"})
                    )
                    self.assertIn("different T3 environment", refresh()["error"])
                    self.assertEqual(calls, [("/.well-known/t3/environment", None, None)])
                    token.write_text(json.dumps({"environmentId": "local", "token": "private"}))
                    snapshot = refresh()
                    self.assertTrue(snapshot["available"])
                    rows = snapshot["threads"]
                    self.assertEqual(
                        [t["group"] for t in rows], ["running", "waiting", "settled", "active"]
                    )
                    self.assertEqual(rows[0]["project"], "Moonframe")
                    self.assertEqual(rows[0]["pullRequests"][0]["label"], "#4")
                    self.assertEqual(rows[0]["issues"][0]["label"], "UI-2")
                    self.assertEqual(calls[-1], ("/api/orchestration/shell", "Bearer private", "2"))
                finally:
                    child.stdin.close()
                    child.wait(timeout=5)
                    server.shutdown()


if __name__ == "__main__":
    unittest.main()
