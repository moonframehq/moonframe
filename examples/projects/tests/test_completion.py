"""The composer's listings must not change the inspector's open folders."""
import importlib.util
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


class CompletionTests(unittest.TestCase):
    def test_completion_and_inspector_listings_are_independent(self):
        path = Path(__file__).parents[1] / "providers/projects/projects.py"
        spec = importlib.util.spec_from_file_location("projects", path)
        provider = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(provider)
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            for folder in (".git", "src", "docs"):
                (base / folder).mkdir()
            (base / "src/main.rs").write_text("fn main() {}")
            provider.state.update(select=root, open=["docs"])
            provider.keep = lambda: None
            provider.find_projects = lambda: []
            provider.summarise_all = lambda _: [{"name": "repo", "branch": "main", "time": 0}]
            provider.changes = lambda *_: {}
            provider.commit_log = lambda _: []
            provider.ask_forge = lambda *_: None

            def send(message):
                provider.receive(json.dumps({"message": message}).encode())

            def published():
                output = io.BytesIO()
                with patch.object(provider.sys, "stdout", SimpleNamespace(buffer=output)):
                    provider.emit(True)
                return json.loads(output.getvalue())

            send({"complete": "src", "cwd": root})
            self.assertEqual(provider.state["open"], ["docs"])
            self.assertIn("fmain.rs", published()["dirs"]["src"])
            send({"open": ""})
            self.assertIn("src", published()["dirs"])
            self.assertNotIn("docs", published()["dirs"])
            send({"complete": "other", "cwd": root + "/different"})
            self.assertEqual(provider.completion, "src")
            send({"select": root + "/another"})
            self.assertIsNone(provider.completion)


if __name__ == "__main__":
    unittest.main()
