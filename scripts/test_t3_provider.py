"""Drive the process provider through its normal stdin/stdout interface."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest

PROVIDER = Path(__file__).resolve().parents[1] / "app/providers/t3/t3.py"


def thread(name, status, **fields):
    return dict(id=name, title=name, status=status, projectId="p", updatedAt=name, **fields)


class T3ProviderTest(unittest.TestCase):
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
                    child.stdin.write('{"from":"overview","msg":"refresh"}\n')
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
