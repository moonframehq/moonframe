//! Edits from the editors: held while typing goes on, then written, merged
//! with whatever changed the note meanwhile. Each editor's edits are kept
//! apart, with the text that editor started from, so two layouts showing
//! one note never take each other's words for their own. A write that
//! fails stays for another try, and whatever waits is written when the
//! provider stops.

use {
    super::{Provider, QUIET, send},
    moonframe_providers::Handle,
    serde_json::{Value, json},
    std::time::{Duration, Instant},
};

/// The longest wait between two tries of a write that keeps failing.
const MOST_DELAY: Duration = Duration::from_secs(60);

/// An edit waiting for typing to stop.
pub(super) struct Pending {
    pub text: String,
    /// The text the editor started from, merged against what is on disk.
    pub base: Option<String>,
    pub reply: Value,
    pub since: Instant,
    /// Writes of it that failed, for waiting longer before each next try.
    pub failures: u32,
}

impl Pending {
    /// Typing stopped long enough ago, or, after a failure, the wait
    /// before the next try is over.
    fn due(&self) -> bool {
        self.since.elapsed() >= retry_delay(self.failures)
    }
}

/// How long an edit waits before its next try: `QUIET`, then doubling
/// with each failure, up to a minute.
pub(super) fn retry_delay(failures: u32) -> Duration {
    QUIET
        .saturating_mul(1_u32 << failures.min(8))
        .min(MOST_DELAY)
}

/// Who an edit is from and which note: `(editor, path)`.
pub(super) type Key = (String, String);

impl Provider {
    /// Holds `text` as `from`'s edit of `path`, to be written once typing
    /// stops. `base` is the text the editor changed (its own, when it says);
    /// edits coming before the last one was written keep the first base.
    pub(super) fn hold(
        &mut self,
        from: &str,
        path: String,
        text: String,
        base: Option<String>,
        reply: Value,
    ) {
        let key = (from.to_string(), path);
        // The editor's own base is exact: it is the text this keystroke
        // changed. Without one, the text this editor was last known to have.
        let base = base.or_else(|| self.known.get(&key).cloned());
        let base = match self.pending.get(&key) {
            Some(earlier) => earlier.base.clone().or(base),
            None => base,
        };
        let failures = self.pending.get(&key).map_or(0, |earlier| earlier.failures);
        self.known.insert(key.clone(), text.clone());
        self.pending.insert(key, Pending {
            text,
            base,
            reply,
            since: Instant::now(),
            failures,
        });
    }

    /// Writes edits that have waited long enough (all of them with `all`),
    /// answering each editor; true when any was written.
    pub(super) fn flush(&mut self, host: &Handle, all: bool) -> bool {
        let answers = self.write_pending(all);
        let wrote = answers.iter().any(|(_, answer)| answer["ok"] == true);
        for (to, answer) in answers {
            send(host, &to, answer);
        }
        wrote
    }

    /// Writes the edits that are due, keeping those that fail for another
    /// try; the answer for each editor.
    pub(super) fn write_pending(&mut self, all: bool) -> Vec<(String, Value)> {
        let ready: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, pending)| all || pending.due())
            .map(|(key, _)| key.clone())
            .collect();
        let mut answers = Vec::new();
        for key in ready {
            let Some(mut pending) = self.pending.remove(&key) else {
                continue;
            };
            let (from, path) = key.clone();
            let Some(vault) = &mut self.vault else {
                // No vault to write to: keep it, and say so.
                answers.push((from, failed(&pending, &path, "no vault is open")));
                pending.failures += 1;
                pending.since = Instant::now();
                self.pending.insert(key, pending);
                continue;
            };
            let stamp = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
            match vault.write(&path, &pending.text, pending.base.as_deref(), &stamp) {
                Ok(saved) => {
                    let hash = match &saved {
                        // The editor gets the merged text in this answer.
                        crate::Saved::Merged { text, hash } => {
                            self.known.insert(key.clone(), text.clone());
                            hash.clone()
                        },
                        crate::Saved::Written { hash }
                        | crate::Saved::Conflict { hash, .. }
                        | crate::Saved::Unchanged { hash } => {
                            self.known.insert(key.clone(), pending.text.clone());
                            hash.clone()
                        },
                    };
                    // Coming back from the watcher, this write is news to
                    // every editor but this one.
                    self.written.insert(path.clone(), (hash, from.clone()));
                    let mut answer = json!({ "reply": pending.reply, "action": "save", "ok": true, "path": path });
                    if let (Some(answer), Ok(Value::Object(saved))) =
                        (answer.as_object_mut(), serde_json::to_value(&saved))
                    {
                        answer.extend(saved);
                    }
                    answers.push((from, answer));
                },
                Err(problem) => {
                    tracing::warn!(target: "commonplace", %path, %problem, failures = pending.failures + 1, "save failed; kept for another try");
                    answers.push((from, failed(&pending, &path, &problem.to_string())));
                    pending.failures += 1;
                    pending.since = Instant::now();
                    self.pending.insert(key, pending);
                },
            }
        }
        answers
    }
}

fn failed(pending: &Pending, path: &str, error: &str) -> Value {
    json!({
        "reply": pending.reply,
        "action": "save",
        "ok": false,
        "path": path,
        "error": error,
        // The edit is still held: the next try is in `retry` seconds.
        "kept": true,
        "retry": retry_delay(pending.failures + 1).as_secs_f64(),
    })
}

impl Drop for Provider {
    /// The app quitting or the provider stopping: edits still waiting for
    /// typing to stop are written now, not lost.
    fn drop(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        for (_, answer) in self.write_pending(true) {
            if answer["ok"] != true {
                tracing::error!(target: "commonplace", path = %answer["path"], error = %answer["error"], "an edit could not be written as the provider stopped");
            }
        }
    }
}
