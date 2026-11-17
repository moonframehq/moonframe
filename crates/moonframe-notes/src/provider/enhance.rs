//! `enhance = { path }`: gives a note's web links their page titles and
//! their hosts' favicons, on a thread of its own as an import is. Bare
//! urls (and links whose text is their url) become `[Title](url)`,
//! written through the vault with the note as it was as the base, so an
//! edit made meanwhile is merged. Each host's favicon is kept in
//! Moonframe's images cache (`favicons/<host>.<ext>`), where `ui.image`
//! may read it, and the `note` view names the ones its links use.
//!
//! Answered `{ reply, action = "enhance", ok, path, titled, icons,
//! conflict? }` or `{ ok = false, error }`. A url that cannot be fetched
//! is skipped.

mod fetch;
mod html;
#[cfg(test)]
mod tests;
mod urls;

use {
    super::{Provider, send},
    crate::{Saved, parse::WebLink},
    fetch::{Found, Job},
    moonframe_providers::Handle,
    serde_json::{Map, Value, json},
    std::{
        collections::{BTreeSet, HashMap, HashSet},
        path::PathBuf,
        sync::Arc,
        time::{Duration, SystemTime},
    },
};

/// Most hosts one run asks.
const MAX_HOSTS: usize = 40;

/// Most pages one run reads for titles.
const MAX_PAGES: usize = 80;

/// Most hosts a view looks up a favicon for.
const MAX_VIEW_HOSTS: usize = 100;

/// A favicon older than this is fetched again.
const FRESH: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The formats a favicon is kept in, by extension.
const EXTENSIONS: &[&str] = &["png", "svg", "ico", "jpg", "gif", "webp"];

/// The enhancements running, and those done but not yet answered.
#[derive(Default)]
pub(super) struct Enhancing {
    running: BTreeSet<String>,
    finished: Vec<Finished>,
}

struct Finished {
    owner: String,
    reply: Value,
    root: PathBuf,
    path: String,
    /// The note as it was when asked: what titles are written into, and
    /// the base the write merges from.
    base: String,
    links: Vec<WebLink>,
    found: Found,
}

/// The favicons folder, where each host's is `<host>.<ext>`.
pub(super) struct Favicons {
    folder: PathBuf,
}

impl Favicons {
    fn new() -> Self {
        Self {
            folder: moonframe_packages::paths::images().join("favicons"),
        }
    }

    /// `host`'s favicon, when one is saved.
    fn cached(&self, host: &str) -> Option<PathBuf> {
        let host = urls::safe_host(host)?;
        EXTENSIONS
            .iter()
            .map(|extension| self.folder.join(format!("{host}.{extension}")))
            .find(|file| file.metadata().is_ok_and(|meta| meta.len() > 0))
    }

    /// Whether `host`'s favicon is saved and recent.
    fn fresh(&self, host: &str) -> bool {
        self.cached(host)
            .and_then(|file| file.metadata().ok()?.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age < FRESH)
    }
}

/// `{ [host] = path }` for the hosts `text` links to whose favicon is
/// saved. Only looks at files, so a view may ask each time.
pub(super) fn favicons(text: &str) -> Value {
    let favicons = Favicons::new();
    let mut seen = HashSet::new();
    let mut found = Map::new();
    for host in hosts(text) {
        if seen.len() >= MAX_VIEW_HOSTS {
            break;
        }
        if seen.insert(host.clone())
            && let Some(file) = favicons.cached(&host)
        {
            found.insert(host, json!(file.to_string_lossy()));
        }
    }
    Value::Object(found)
}

/// The host of each `http://` or `https://` url in `text`, read from the
/// raw text (code included: a view only shows what it finds).
fn hosts(text: &str) -> impl Iterator<Item = String> + '_ {
    text.match_indices("://").filter_map(|(at, _)| {
        // The scheme's letters are ASCII: start after the last other
        // character, by its length, so an accent before it stays whole.
        let start = text[..at]
            .char_indices()
            .rfind(|(_, c)| !c.is_ascii_alphabetic())
            .map_or(0, |(before, c)| before + c.len_utf8());
        let rest = &text[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || "()[]<>\"'`|*,".contains(c))
            .unwrap_or(rest.len());
        urls::host(&rest[..end])
    })
}

/// The hosts to ask about `links`: titles for bare links, favicons not
/// saved recently.
fn plan(links: &[WebLink], favicons: &Favicons) -> Vec<Job> {
    let mut jobs: Vec<Job> = Vec::new();
    let mut by_host: HashMap<String, usize> = HashMap::new();
    let mut titled = HashSet::new();
    for link in links {
        let Some(host) = urls::host(&link.url) else {
            continue;
        };
        let index = match by_host.get(&host) {
            Some(index) => *index,
            None if jobs.len() >= MAX_HOSTS => continue,
            None => {
                by_host.insert(host.clone(), jobs.len());
                jobs.push(Job {
                    icon: !favicons.fresh(&host),
                    host,
                    titles: Vec::new(),
                    page: link.url.clone(),
                });
                jobs.len() - 1
            },
        };
        if link.bare && titled.len() < MAX_PAGES && titled.insert(link.url.clone()) {
            jobs[index].titles.push(link.url.clone());
        }
    }
    jobs.retain(|job| job.icon || !job.titles.is_empty());
    jobs
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

impl Provider {
    /// The notes being enhanced, for the summary.
    pub(super) fn enhancing(&self) -> Vec<String> {
        self.enhance
            .lock()
            .map(|state| state.running.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn start_enhance(&mut self, host: &Handle, from: &str, reply: Value, ask: &Value) {
        let path = ask
            .as_str()
            .unwrap_or_else(|| text(ask, "path"))
            .to_string();
        let fail = |error: &str| {
            send(
                host,
                from,
                json!({ "reply": reply, "action": "enhance", "ok": false, "path": path, "error": error }),
            );
        };
        if path.is_empty() {
            return fail("say which note: enhance = { path = \"notes/Note.md\" }");
        }
        // The note as typed so far.
        self.flush(host, true);
        let Some(vault) = &self.vault else {
            return fail("no vault is open");
        };
        let base = match vault.read(&path) {
            Ok(base) => base,
            Err(problem) => return fail(&problem.to_string()),
        };
        let Ok(mut state) = self.enhance.lock() else {
            return fail("enhancing broke earlier; restart the app");
        };
        if !state.running.insert(path.clone()) {
            drop(state);
            return fail("that note is already being enhanced");
        }
        drop(state);
        let links = crate::parse::web_links(&base);
        let root = vault.root().to_path_buf();
        let state = Arc::clone(&self.enhance);
        let owner = from.to_string();
        std::thread::spawn(move || {
            let favicons = Favicons::new();
            let found = fetch::run(plan(&links, &favicons), &favicons);
            if let Ok(mut state) = state.lock() {
                state.finished.push(Finished {
                    owner,
                    reply,
                    root,
                    path,
                    base,
                    links,
                    found,
                });
            }
        });
        self.publish(host);
    }

    /// Writes and answers the enhancements that are done; true when any were.
    pub(super) fn enhance_finished(&mut self, host: &Handle) -> bool {
        let finished = match self.enhance.lock() {
            Ok(mut state) => {
                let finished = std::mem::take(&mut state.finished);
                for done in &finished {
                    state.running.remove(&done.path);
                }
                finished
            },
            Err(_) => return false,
        };
        if finished.is_empty() {
            return false;
        }
        // Edits typed meanwhile go first; the titles merge into them.
        self.flush(host, true);
        for done in finished {
            let answer = self.write_enhanced(&done);
            send(host, &done.owner, answer);
        }
        true
    }

    fn write_enhanced(&mut self, done: &Finished) -> Value {
        let (text, titled) = urls::rewrite(&done.base, &done.links, &done.found.titles);
        let mut answer = json!({ "reply": done.reply, "action": "enhance", "ok": true, "path": done.path, "titled": titled, "icons": done.found.icons });
        if titled == 0 {
            return answer;
        }
        let vault = self
            .vault
            .as_mut()
            .filter(|vault| vault.root() == done.root);
        let Some(vault) = vault else {
            answer["ok"] = json!(false);
            answer["error"] = json!("another vault was opened meanwhile; nothing was written");
            return answer;
        };
        match vault.write(
            &done.path,
            &text,
            Some(&done.base),
            &super::actions::stamp(),
        ) {
            Ok(Saved::Conflict { copy, .. }) => answer["conflict"] = json!(copy),
            Ok(_) => {},
            Err(problem) => {
                answer["ok"] = json!(false);
                answer["error"] = json!(format!("writing the titles: {problem}"));
            },
        }
        answer
    }
}
