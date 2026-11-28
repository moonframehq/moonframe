//! The network side of enhancing a note: each page's title, and each
//! host's favicon saved in the favicons folder. Every answer is untrusted:
//! only `http:` and `https:` are followed, bodies are capped, and a file
//! is written only under a host name checked to be safe.

use {
    super::{
        Favicons, html,
        urls::{origin, resolve},
    },
    crate::parse::is_web,
    std::{
        collections::{HashMap, HashSet},
        fs,
        io::Read,
        sync::Mutex,
        time::Duration,
    },
    ureq::{Agent, Body, http::Response},
};

/// Most of a page read for its title and icons.
const PAGE_LIMIT: u64 = 512 * 1024;

/// Largest favicon kept.
const ICON_LIMIT: u64 = 512 * 1024;

/// How long one request may take, redirects each counting anew.
const TIMEOUT: Duration = Duration::from_secs(8);

/// Most redirects followed.
const REDIRECTS: usize = 5;

/// How many hosts are asked at once.
const WORKERS: usize = 4;

/// What one host is asked: the pages wanting a title, and whether its
/// favicon is wanted (from `page`, the first of its urls).
pub(super) struct Job {
    pub host: String,
    pub titles: Vec<String>,
    pub icon: bool,
    pub page: String,
}

/// What a run found: titles by url, and how many favicons were saved.
#[derive(Default)]
pub(super) struct Found {
    pub titles: HashMap<String, String>,
    pub icons: usize,
}

/// Runs `jobs`, a few hosts at a time.
pub(super) fn run(jobs: Vec<Job>, favicons: &Favicons) -> Found {
    // No cookies are kept, and redirects are followed by hand (see `get`).
    let agent = Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent("Commonplace/0.1")
        .build()
        .new_agent();
    let workers = WORKERS.min(jobs.len());
    let queue = Mutex::new(jobs);
    let found = Mutex::new(Found::default());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(job) = queue.lock().ok().and_then(|mut queue| queue.pop()) {
                    let (titles, icon) = host(&agent, &job, favicons);
                    if let Ok(mut found) = found.lock() {
                        found.titles.extend(titles);
                        found.icons += usize::from(icon);
                    }
                }
            });
        }
    });
    found.into_inner().unwrap_or_default()
}

/// One host's job: its titles, and whether its favicon was saved.
fn host(agent: &Agent, job: &Job, favicons: &Favicons) -> (HashMap<String, String>, bool) {
    let mut titles = HashMap::new();
    let mut tried = HashSet::new();
    let mut icon_page = None;
    for url in &job.titles {
        tried.insert(url.as_str());
        let Some((at, html)) = page(agent, url) else {
            continue;
        };
        if let Some(title) = html::title(&html) {
            titles.insert(url.clone(), title);
        }
        if icon_page.is_none() {
            icon_page = Some((at, html));
        }
    }
    if !job.icon {
        return (titles, false);
    }
    if icon_page.is_none() && !tried.contains(job.page.as_str()) {
        icon_page = page(agent, &job.page);
    }
    let base = icon_page
        .as_ref()
        .map_or(job.page.as_str(), |(at, _)| at.as_str());
    let linked = icon_page
        .as_ref()
        .and_then(|(at, html)| resolve(at, &html::icon(html)?));
    let fallback = origin(base).map(|origin| format!("{origin}/favicon.ico"));
    let saved = linked
        .into_iter()
        .chain(fallback)
        .find_map(|url| image(agent, &url))
        .is_some_and(|(bytes, extension)| favicons.save(&job.host, &bytes, extension));
    (titles, saved)
}

/// GETs `url`, following redirects only to `http:` or `https:`; the
/// answer when it is a success, with the url it came from.
fn get(agent: &Agent, url: &str, accept: &str) -> Option<(String, Response<Body>)> {
    let mut url = url.to_string();
    for _ in 0..=REDIRECTS {
        if !is_web(&url) {
            return None;
        }
        let response = agent.get(&url).header("Accept", accept).call().ok()?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            let location = response.headers().get("location")?.to_str().ok()?;
            url = resolve(&url, location)?;
            continue;
        }
        return (200..300).contains(&status).then_some((url, response));
    }
    None
}

/// The first [`PAGE_LIMIT`] bytes of the HTML page at `url`, with where
/// it ended up.
fn page(agent: &Agent, url: &str) -> Option<(String, String)> {
    let (at, mut response) = get(agent, url, "text/html,application/xhtml+xml")?;
    let html = response
        .headers()
        .get("content-type")
        .and_then(|kind| kind.to_str().ok())
        .is_none_or(|kind| kind.to_ascii_lowercase().contains("html"));
    if !html {
        return None;
    }
    let mut bytes = Vec::new();
    let read = response
        .body_mut()
        .as_reader()
        .take(PAGE_LIMIT)
        .read_to_end(&mut bytes);
    if read.is_err() && bytes.is_empty() {
        return None;
    }
    Some((at, String::from_utf8_lossy(&bytes).into_owned()))
}

/// The picture at `url` and the extension its bytes say it has.
fn image(agent: &Agent, url: &str) -> Option<(Vec<u8>, &'static str)> {
    let (_, mut response) = get(agent, url, "image/*")?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(ICON_LIMIT)
        .read_to_vec()
        .ok()?;
    let extension = sniff(&bytes)?;
    Some((bytes, extension))
}

/// The format of a picture, from its first bytes (never from what the
/// server says): `png`, `gif`, `jpg`, `ico`, `webp` or `svg`.
pub(super) fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(&[0, 0, 1, 0]) && bytes.len() > 6 {
        return Some("ico");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    let start = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    (start.contains("<svg") && !start.contains("<html")).then_some("svg")
}

impl Favicons {
    /// Saves `host`'s favicon, replacing one in another format.
    fn save(&self, host: &str, bytes: &[u8], extension: &str) -> bool {
        let Some(host) = super::urls::safe_host(host) else {
            return false;
        };
        if fs::create_dir_all(&self.folder).is_err() {
            return false;
        }
        let file = self.folder.join(format!("{host}.{extension}"));
        let part = self.folder.join(format!("{host}.{extension}.part"));
        if fs::write(&part, bytes).is_err() || fs::rename(&part, &file).is_err() {
            let _ = fs::remove_file(&part);
            return false;
        }
        for other in super::EXTENSIONS
            .iter()
            .filter(|other| **other != extension)
        {
            let _ = fs::remove_file(self.folder.join(format!("{host}.{other}")));
        }
        true
    }
}
