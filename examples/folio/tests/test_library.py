"""Exercise the real provider over its JSON pipes in an isolated library.

Run with: python3 -m unittest discover -s examples/folio/tests
"""

import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
import unittest


PROVIDER = Path(__file__).resolve().parents[1] / "providers/library/library.py"


class LibraryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.env = dict(os.environ, XDG_DATA_HOME=self.temp.name, MOONFRAME_PROFILE="folio")
        self.folder = Path(self.temp.name) / "moonframe/folio/documents"
        self.folder.mkdir(parents=True)
        (self.folder / "Draft.md").write_text("Original draft", encoding="utf-8")

    def start(self):
        process = subprocess.Popen(
            [sys.executable, str(PROVIDER)], env=self.env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        self.addCleanup(self.stop, process)
        self.process = process
        self.buffer = b""
        return self.output()

    @staticmethod
    def stop(process):
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        for pipe in (process.stdin, process.stdout, process.stderr):
            pipe.close()

    def output(self):
        deadline = time.monotonic() + 5
        while b"\n" not in self.buffer:
            ready, _, _ = select.select([self.process.stdout], [], [], max(0, deadline - time.monotonic()))
            self.assertTrue(ready, "provider did not publish")
            chunk = os.read(self.process.stdout.fileno(), 1 << 20)
            self.assertTrue(chunk, "provider exited before publishing")
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def journals(self):
        return sorted((self.folder / ".journals").glob("*.json"))

    def send(self, message):
        self.process.stdin.write((json.dumps({"message": message}) + "\n").encode())
        self.process.stdin.flush()
        return self.output()

    def test_killed_provider_recovers_latest_edit_before_debounce(self):
        self.start()
        self.send({"save": {"id": "Draft", "text": "First edit"}})
        self.send({"save": {"id": "Draft", "text": "Latest edit — café"}})
        # Publication acknowledges the edit, but the markdown is still old.
        self.assertEqual((self.folder / "Draft.md").read_text(), "Original draft")
        self.stop(self.process)
        restored = self.start()
        self.assertEqual(restored["current"]["text"], "Latest edit — café")
        self.assertEqual((self.folder / "Draft.md").read_text(), "Latest edit — café")
        self.assertEqual(self.journals(), [])

    def test_opening_tokens_change_on_reopen_and_restart(self):
        first = self.start()["current"]["opened"]
        second = self.send({"open": "Draft"})["current"]["opened"]
        self.assertNotEqual(first, second)
        self.stop(self.process)
        (self.folder / "Draft.md").write_text("Edited outside Folio")
        current = self.start()["current"]
        self.assertNotIn(current["opened"], (first, second))
        self.assertEqual(current["text"], "Edited outside Folio")

    def test_switching_documents_flushes_the_previous_edit(self):
        self.start()
        self.send({"save": {"id": "Draft", "text": "Saved before switching"}})
        new = self.send({"new": True})["current"]
        self.assertNotEqual(new["id"], "Draft")
        self.assertEqual((self.folder / "Draft.md").read_text(), "Saved before switching")
        self.send({"save": {"id": new["id"], "text": "New document edit"}})
        self.stop(self.process)
        self.start()
        self.assertEqual((self.folder / (new["id"] + ".md")).read_text(), "New document edit")
        self.assertEqual((self.folder / "Draft.md").read_text(), "Saved before switching")

    def test_purging_a_pending_document_does_not_recover_it(self):
        self.start()
        self.send({"trash": "Draft"})
        self.send({"save": {"id": "Draft", "text": "Edited in trash"}})
        self.assertIsNone(self.send({"purge": "Draft"})["current"])
        self.stop(self.process)
        restored = self.start()
        self.assertEqual(restored["documents"], [])
        self.assertEqual(self.journals(), [])

    def test_trashing_never_replaces_a_trashed_document_of_the_same_name(self):
        trash = self.folder / ".trash"
        trash.mkdir()
        (trash / "Draft.md").write_text("Trashed earlier", encoding="utf-8")
        self.start()
        documents = self.send({"trash": "Draft"})["documents"]
        self.assertEqual((trash / "Draft.md").read_text(), "Trashed earlier")
        self.assertEqual((trash / "Draft (2).md").read_text(), "Original draft")
        self.assertEqual(sorted(d["id"] for d in documents), ["Draft", "Draft (2)"])
        # Putting it back is just as careful.
        (self.folder / "Draft.md").write_text("Written meanwhile", encoding="utf-8")
        self.send({"restore": "Draft"})
        self.assertEqual((self.folder / "Draft.md").read_text(), "Written meanwhile")
        self.assertEqual((self.folder / "Draft (2).md").read_text(), "Trashed earlier")
        # Deleting for good takes the trashed one, never the live one.
        (trash / "Draft.md").write_text("Trashed again", encoding="utf-8")
        self.process.stdin.write(b'{"message": {"purge": "Draft"}}\n')
        self.process.stdin.flush()
        deadline = time.monotonic() + 5
        while (trash / "Draft.md").exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertFalse((trash / "Draft.md").exists())
        self.assertEqual((self.folder / "Draft.md").read_text(), "Written meanwhile")

    def test_a_document_too_large_to_send_opens_without_its_text(self):
        (self.folder / "Draft.md").write_text("é\n" * 200_000, encoding="utf-8")
        current = self.start()["current"]
        self.assertTrue(current["too_large"])
        self.assertNotIn("text", current)

    def test_a_large_library_lists_the_newest_and_counts_the_rest(self):
        for n in range(510):
            (self.folder / ("Note %d.md" % n)).write_text("A note", encoding="utf-8")
        published = self.start()
        self.assertEqual(len(published["documents"]), 500)
        self.assertEqual(published["hidden"], 11)

    def test_a_document_put_back_under_a_new_name_keeps_its_edits_to_itself(self):
        self.start()
        self.send({"trash": "Draft"})
        opened = self.send({"open": "Draft"})["current"]["opened"]
        (self.folder / "Draft.md").write_text("Another document", encoding="utf-8")
        current = self.send({"restore": "Draft"})["current"]
        self.assertEqual(current["id"], "Draft (2)")
        self.assertEqual(current["opened"], opened)
        # The editor still names the opening it was given, and its old name.
        self.send({"save": {"id": "Draft", "opened": opened, "text": "Edited after restoring"}})
        self.stop(self.process)
        self.start()
        self.assertEqual((self.folder / "Draft.md").read_text(), "Another document")
        self.assertEqual((self.folder / "Draft (2).md").read_text(), "Edited after restoring")

    def test_runs_of_one_profile_keep_their_own_recovery(self):
        first = self.start()["current"]["opened"]
        self.send({"save": {"id": "Draft", "opened": first, "text": "First run's edit"}})
        one = self.process
        self.start()  # a second run while the first still goes
        self.assertEqual((self.folder / "Draft.md").read_text(), "Original draft")
        self.assertEqual(len(self.journals()), 1)
        self.stop(self.process)
        # The first run is still alive and writes its edit itself.
        self.process = one
        deadline = time.monotonic() + 5
        while (self.folder / "Draft.md").read_text() != "First run's edit" and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertEqual((self.folder / "Draft.md").read_text(), "First run's edit")
        self.assertIsNone(one.poll())
        self.assertEqual(self.journals(), [])

    def test_a_long_name_stays_valid_when_it_must_change(self):
        name = "L" * 81
        (self.folder / (name + ".md")).write_text("Long", encoding="utf-8")
        trash = self.folder / ".trash"
        trash.mkdir()
        (trash / (name + ".md")).write_text("Older long", encoding="utf-8")
        self.start()
        documents = self.send({"trash": name})["documents"]
        ids = sorted(d["id"] for d in documents)
        self.assertEqual(len(ids), 3)
        self.assertTrue(all(len(i) <= 81 for i in ids))
        self.assertIn("L" * 77 + " (2)", ids)

    def test_a_stale_recovery_never_overwrites_newer_writing(self):
        opened = self.start()["current"]["opened"]
        self.send({"save": {"id": "Draft", "opened": opened, "text": "Run A, never written"}})
        self.stop(self.process)  # A stops before writing; its journal stays
        self.assertEqual(len(self.journals()), 1)
        # Meanwhile the file changes elsewhere.
        (self.folder / "Draft.md").write_text("Newer, saved by run B", encoding="utf-8")
        self.start()
        self.assertEqual((self.folder / "Draft.md").read_text(), "Newer, saved by run B")
        self.assertEqual((self.folder / "Draft (recovered).md").read_text(), "Run A, never written")
        self.assertEqual(self.journals(), [])

    def test_only_locked_files_are_published_as_locks(self):
        self.start()
        locks = list((self.folder / ".journals").glob("*.lock"))
        self.assertEqual(len(locks), 1)
        self.assertEqual(list((self.folder / ".journals").glob("*.locking")), [])
        # A second run's scan leaves a live run's lock in place.
        one = self.process
        self.start()
        self.assertTrue(locks[0].exists())
        self.assertIsNone(one.poll())


if __name__ == "__main__":
    unittest.main()
