//! The `library` native provider: a folder of markdown documents, one file
//! each, for a writing app (`examples/folio`). It works on the user's own
//! files, so it needs the user's access, which Wasm has not.
//!
//! Publishes `app.library`:
//!
//! - `documents`: every document, newest first: `{ id, title, excerpt,
//!   words, modified ("Today, 14:02"), favorite, trashed }`
//! - `current`: the open document, `{ id, text, opened }`. `text` is the file
//!   as it was when opened (and stays so while it is edited), so the editor
//!   never gets its own keystrokes back late; `opened` identifies an opening
//!   across restarts too. One too large to send has `too_large = true` and
//!   no text.
//! - `folder`: where the files are
//! - `hidden`: documents left out past [`MAX_DOCUMENTS`], the oldest
//!
//! Messages, one key each: `open = id`; `save = { id, opened, text }`, its
//! new text, written once typing stops; `new = true`; `favorite = id`;
//! `trash = id` and `restore = id`; `trash_current = true`; `purge = id`,
//! which only ever takes a document from the trash.
//!
//! The folder is `$XDG_DATA_HOME/moonframe/<profile>/documents` (by default
//! under `~/.local/share`), started the first time with the documents
//! `[settings] samples` names, newest first, from the provider's `samples`
//! folder. Each edit is journaled before it is published, then recovered at
//! the next start if the app stopped before its debounced write.

mod store;
mod text;

use {
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, profile, to_json},
    serde_json::{Value, json},
    std::{
        env, fs, io,
        path::PathBuf,
        time::{Duration, Instant},
    },
    store::{Folder, atomic_write, digest, digest_text, read, unique, valid},
};

/// How long without a keystroke before an edit is written.
const QUIET: Duration = Duration::from_millis(600);

/// How often the folder is looked at, for files changed elsewhere.
const LOOK: Duration = Duration::from_secs(30);

/// Newest documents listed; the rest are counted in `hidden`.
const MAX_DOCUMENTS: usize = 500;

/// Bytes of what is published, under the host's limit of 1 MiB.
const MAX_PUBLISHED: usize = 900 * 1024;

/// Whether `wait` has passed since `since`.
fn quiet(since: Instant, wait: Duration) -> bool {
    since.elapsed() >= wait
}

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

#[derive(Default)]
struct Provider {
    library: Option<Library>,
    /// What was published last.
    last: Option<String>,
    looked: Option<Instant>,
}

impl Provider {
    fn publish(&mut self, host: &Handle) {
        let Some(library) = &mut self.library else {
            return;
        };
        self.looked = Some(Instant::now());
        let (value, line) = library.published();
        if self.last.as_ref() != Some(&line) {
            host.publish_json(&value);
            self.last = Some(line);
        }
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let samples = samples(host);
        let library = Library::open(folder(host), &samples)
            .map_err(|problem| format!("the documents folder: {problem}"))?;
        self.library = Some(library);
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        let Some(library) = &mut self.library else {
            return;
        };
        let written = library.pending.is_some() && quiet(library.typed, QUIET);
        if written && let Err(problem) = library.flush() {
            // The journal keeps the edit; try again after another wait.
            library.typed = Instant::now();
            host.fail(format!("saving: {problem}"));
            return;
        }
        if written || self.looked.is_none_or(|looked| quiet(looked, LOOK)) {
            self.publish(host);
        }
    }

    fn receive(&mut self, host: &Handle, _from: &str, message: Data) {
        let Some(library) = &mut self.library else {
            return;
        };
        if let Err(problem) = library.receive(&to_json(&message)) {
            host.fail(problem.to_string());
        }
        self.publish(host);
    }
}

/// `$XDG_DATA_HOME/moonframe/<profile>/documents`, as an app's own providers
/// keep theirs. The profile may hold anything; a folder name may not.
fn folder(host: &Handle) -> PathBuf {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"));
            PathBuf::from(home.unwrap_or_default()).join(".local/share")
        });
    let name: String = profile()
        .unwrap_or(host.name())
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' => c,
            _ => '-',
        })
        .collect();
    data.join("moonframe").join(name).join("documents")
}

/// The documents `[settings] samples` names, from the `samples` folder.
fn samples(host: &Handle) -> Vec<(String, String)> {
    let settings = to_json(host.settings());
    let names = settings.get("samples").and_then(Value::as_array);
    names
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|name| {
            let body = fs::read_to_string(host.dir().join("samples").join(format!("{name}.md")));
            Some((name.to_owned(), body.ok()?))
        })
        .collect()
}

/// The open document.
#[derive(Clone)]
struct Current {
    id: String,
    opened: String,
    /// `None` when it is too large to send.
    text: Option<String>,
    /// Its file: saves go here, never by name.
    path: PathBuf,
    /// The hash of that file as this run last read or wrote it.
    base: Option<String>,
}

impl Current {
    fn json(&self) -> Value {
        match &self.text {
            Some(text) => json!({ "id": self.id, "opened": self.opened, "text": text }),
            None => json!({ "id": self.id, "opened": self.opened, "too_large": true }),
        }
    }
}

struct Library {
    folder: Folder,
    session: String,
    /// How many documents this run opened, for `Current::opened`.
    opened: u64,
    current: Option<Current>,
    /// The file and text not written yet.
    pending: Option<(PathBuf, String)>,
    /// When the last edit came.
    typed: Instant,
    /// This run's lock, held as long as it runs.
    lock: Option<fs::File>,
}

impl Library {
    fn open(root: PathBuf, samples: &[(String, String)]) -> io::Result<Self> {
        let session = unique();
        let folder = Folder::new(root, &session);
        folder.ensure(samples)?;
        let lock = Some(folder.recover()?);
        Ok(Self {
            folder,
            session,
            opened: 0,
            current: None,
            pending: None,
            typed: Instant::now(),
            lock,
        })
    }

    /// Writes the pending edit. A document deleted meanwhile is not recreated.
    fn flush(&mut self) -> io::Result<()> {
        let Some((path, body)) = &self.pending else {
            return Ok(());
        };
        if path.is_file() {
            atomic_write(path, body)?;
            if let Some(current) = self.current.as_mut().filter(|c| c.path == *path) {
                current.base = Some(digest_text(body));
            }
        }
        // Kept until now, so recovery lasts until the document is replaced.
        self.folder.clear_journal()?;
        self.pending = None;
        Ok(())
    }

    fn open_doc(&mut self, doc: &str) -> io::Result<()> {
        self.flush()?;
        let Some((path, _)) = self.folder.find(doc) else {
            return Ok(());
        };
        self.opened += 1;
        let size = fs::metadata(&path).map_or(0, |file| file.len());
        self.current = Some(Current {
            id: doc.to_owned(),
            opened: format!("{}-{}", self.session, self.opened),
            text: (size <= store::MAX_TEXT).then(|| read(&path)),
            base: digest(&path),
            path,
        });
        Ok(())
    }

    /// Moves a document into or out of the trash, renamed if it must be,
    /// and its star with it; the open document follows its file.
    fn relocate(&mut self, doc: &str, from: &PathBuf, trashed: bool) -> io::Result<()> {
        let name = self.folder.free(doc, trashed);
        let to = self.folder.path_of(&name, trashed);
        fs::rename(from, &to)?;
        if let Some(current) = self.current.as_mut().filter(|c| c.path == *from) {
            current.id.clone_from(&name);
            current.path = to;
        }
        let mut stars = self.folder.favorites();
        if name != doc && stars.remove(doc) {
            stars.insert(name);
            self.folder.save_favorites(&stars)?;
        }
        Ok(())
    }

    /// An edit of the open document goes to its file, whatever it is called
    /// now; one from an earlier opening is dropped.
    fn save(&mut self, save: &Value) -> io::Result<()> {
        let Some(body) = save.get("text").and_then(Value::as_str) else {
            return Ok(());
        };
        let Some(current) = &self.current else {
            return Ok(());
        };
        let mine = match save.get("opened").filter(|opened| !opened.is_null()) {
            Some(opened) => opened.as_str() == Some(current.opened.as_str()),
            None => save.get("id").and_then(Value::as_str) == Some(current.id.as_str()),
        };
        if !mine || !current.path.is_file() {
            return Ok(());
        }
        let path = current.path.clone();
        if self.pending.as_ref().is_some_and(|(file, _)| *file != path) {
            self.flush()?;
        }
        // After the flush, which may have moved the base on.
        let base = self.current.as_ref().and_then(|c| c.base.as_deref());
        self.folder.journal(&path, body, base)?;
        self.pending = Some((path, body.to_owned()));
        self.typed = Instant::now();
        Ok(())
    }

    /// Into the trash, or back out. Each looks only in the folder it moves
    /// out of: the same name may be in both.
    fn trash_or_restore(&mut self, doc: &str, to_trash: bool) -> io::Result<()> {
        let from = self.folder.path_of(doc, !to_trash);
        if !valid(doc) || !from.is_file() {
            return Ok(());
        }
        self.flush()?;
        let was_open = self.current.as_ref().is_some_and(|c| c.path == from);
        self.relocate(doc, &from, to_trash)?;
        if to_trash && was_open {
            self.current = None;
        }
        Ok(())
    }

    fn receive(&mut self, message: &Value) -> io::Result<()> {
        let word = |key: &str| message.get(key).and_then(Value::as_str);
        let flag = |key: &str| message.get(key).and_then(Value::as_bool) == Some(true);
        if let Some(doc) = word("open") {
            return self.open_doc(doc);
        }
        if let Some(save) = message.get("save").filter(|save| save.is_object()) {
            return self.save(save);
        }
        if flag("new") {
            self.flush()?;
            let doc = self.folder.fresh_name();
            fs::write(self.folder.path_of(&doc, false), "# ")?;
            return self.open_doc(&doc);
        }
        if let Some(doc) = word("favorite").filter(|doc| self.folder.find(doc).is_some()) {
            let mut stars = self.folder.favorites();
            if !stars.remove(doc) {
                stars.insert(doc.to_owned());
            }
            return self.folder.save_favorites(&stars);
        }
        if flag("trash_current") {
            // Not one open from the trash: a live document may share its name.
            let live = self.current.as_ref().map(|current| current.id.clone());
            let live = live.filter(|id| {
                self.current.as_ref().map(|current| &current.path)
                    == Some(&self.folder.path_of(id, false))
            });
            return live.map_or(Ok(()), |id| self.trash_or_restore(&id, true));
        }
        if let Some(doc) = word("trash") {
            return self.trash_or_restore(doc, true);
        }
        if let Some(doc) = word("restore") {
            return self.trash_or_restore(doc, false);
        }
        if let Some(doc) = word("purge") {
            // Only ever from the trash, even when a live document shares the name.
            let path = self.folder.path_of(doc, true);
            if valid(doc) && path.is_file() {
                self.flush()?;
                fs::remove_file(path)?;
                if self.current.as_ref().is_some_and(|c| !c.path.is_file()) {
                    self.current = None;
                }
            }
        }
        Ok(())
    }

    fn documents(&mut self) -> Vec<Value> {
        let documents = self.folder.listing(self.pending.as_ref());
        if self.current.is_none() {
            let first = documents
                .iter()
                .find(|doc| doc["trashed"] == false)
                .or(documents.first());
            if let Some(id) = first.and_then(|doc| doc["id"].as_str()) {
                // Nothing is pending with no document open.
                let _ = self.open_doc(id);
            }
        }
        documents
    }

    /// What to publish, and as text, to compare with the last. A text that
    /// escapes to more than fits is left out like a large one.
    fn published(&mut self) -> (Value, String) {
        let mut documents = self.documents();
        let hidden = documents.len().saturating_sub(MAX_DOCUMENTS);
        documents.truncate(MAX_DOCUMENTS);
        let value = |current: Option<&Current>| {
            json!({
                "documents": documents,
                "hidden": hidden,
                "current": current.map(Current::json),
                "folder": self.folder.root.to_string_lossy(),
            })
        };
        let published = value(self.current.as_ref());
        let line = published.to_string();
        if line.len() <= MAX_PUBLISHED {
            return (published, line);
        }
        if let Some(current) = &mut self.current {
            current.text = None;
        }
        let published = value(self.current.as_ref());
        let line = published.to_string();
        (published, line)
    }

    /// A run killed: nothing pending is written, and its lock goes.
    #[cfg(test)]
    fn kill(mut self) {
        self.pending = None;
        self.lock = None;
    }
}

impl Drop for Library {
    /// A clean stop writes what is pending; the journal covers any other.
    /// The lock is held until that is done.
    fn drop(&mut self) {
        let lock = self.lock.take();
        if lock.is_some() && self.flush().is_ok() {
            self.folder.release();
        }
    }
}

#[cfg(test)]
mod tests;
