//! Links to the app's own URL schemes (`schemes` in its `moonframe.toml`,
//! [`Moonframe::url_scheme`](crate::Moonframe::url_scheme)), opened
//! anywhere: a browser, another app, `open notes:/new`. macOS hands them to
//! the app whose bundle claims the scheme, starting it if need be. Each
//! brings the app forward and reaches every module in the front main
//! window that subscribes to `app/url`, read:
//! `{ url, scheme, host?, path, query, fragment? }`.
//!
//! Any web page can ask to open a link, so one is input like any other: a
//! scheme the app did not claim, an address too long or one that does not
//! parse is dropped, and only a few are taken in a burst. Those that come
//! before a main window is open (the link started the app) wait for it.

use {
    super::{MoonframeApp, hub::front},
    futures::{StreamExt as _, channel::mpsc},
    gpui_kit::{App, Application, Context},
    moonframe_core::Data,
    std::{
        collections::{BTreeMap, VecDeque},
        sync::{Mutex, OnceLock},
        time::{Duration, Instant},
    },
};

/// The longest link taken, in bytes.
const MAX_URL: usize = 8 * 1024;
/// The most query parameters read from one; the rest are left in `url`.
const MAX_QUERY: usize = 64;
/// At most this many links in [`BURST_WINDOW`].
const BURST: usize = 5;
const BURST_WINDOW: Duration = Duration::from_secs(10);
/// How often a link waiting for a main window looks for one.
const WAIT: Duration = Duration::from_millis(100);

/// The schemes this run claims, set once at launch.
static SCHEMES: OnceLock<Vec<String>> = OnceLock::new();
/// The links the system opened, until the app starts taking them.
static OPENED: Mutex<Option<mpsc::UnboundedReceiver<String>>> = Mutex::new(None);

/// Hears the links the system opens with this app, from now on: call it
/// before the application runs, so the link that starts it is heard.
pub(crate) fn listen(application: &Application, schemes: Vec<String>) {
    if schemes.is_empty() {
        return;
    }
    let _ = SCHEMES.set(schemes);
    let (sender, opened) = mpsc::unbounded();
    if let Ok(mut slot) = OPENED.lock() {
        *slot = Some(opened);
    }
    application.on_open_urls(move |urls| {
        for url in urls {
            let _ = sender.unbounded_send(url);
        }
    });
}

/// Hands each link opened to the front main window, once there is one.
pub(super) fn deliver(cx: &mut App) {
    let Some(mut opened) = OPENED.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    let schemes = SCHEMES.get().cloned().unwrap_or_default();
    cx.spawn(async move |cx| {
        let mut taken = VecDeque::new();
        while let Some(url) = opened.next().await {
            let Some(link) = read(&url, &schemes) else {
                tracing::warn!("dropped a link the app does not open ({} bytes)", url.len());
                continue;
            };
            if !admit(&mut taken, Instant::now()) {
                tracing::warn!("dropped a link: more than {BURST} in {BURST_WINDOW:?}");
                continue;
            }
            while cx.update(|cx| front(cx).is_none()) {
                cx.background_executor().timer(WAIT).await;
            }
            cx.update(|cx| {
                if let Some(app) = front(cx) {
                    app.update(cx, |app, cx| app.open_link(link, cx));
                }
            });
        }
    })
    .detach();
}

impl MoonframeApp {
    /// A link to the app came: brings the app and this window forward and
    /// tells every module that subscribes to `app/url`.
    fn open_link(&mut self, link: Data, cx: &mut Context<Self>) {
        self.come_forward(cx);
        self.workspace.app_news("url", link);
        self.flush("url", cx);
    }
}

/// Keeps the times in `times` within [`BURST_WINDOW`] of `now` and adds
/// it, unless [`BURST`] of them are there already.
fn admit(times: &mut VecDeque<Instant>, now: Instant) -> bool {
    while times
        .front()
        .is_some_and(|at| now.duration_since(*at) >= BURST_WINDOW)
    {
        times.pop_front();
    }
    if times.len() >= BURST {
        return false;
    }
    times.push_back(now);
    true
}

/// `url` as mods hear it, if it is a link to one of `schemes`:
/// `{ url, scheme, host?, path, query, fragment? }`, the query's
/// parameters decoded, the first of each name kept.
fn read(url: &str, schemes: &[String]) -> Option<Data> {
    if url.len() > MAX_URL {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    if !schemes.iter().any(|scheme| scheme == parsed.scheme()) {
        return None;
    }
    let mut query = BTreeMap::new();
    for (name, value) in parsed.query_pairs().take(MAX_QUERY) {
        query
            .entry(name.into_owned())
            .or_insert_with(|| Data::String(value.into_owned()));
    }
    let mut link = BTreeMap::from([
        ("url".to_owned(), Data::from(url)),
        ("scheme".to_owned(), Data::from(parsed.scheme())),
        ("path".to_owned(), Data::from(parsed.path())),
        ("query".to_owned(), Data::Map(query)),
    ]);
    if let Some(host) = parsed.host_str().filter(|host| !host.is_empty()) {
        link.insert("host".to_owned(), Data::from(host));
    }
    if let Some(fragment) = parsed.fragment() {
        link.insert("fragment".to_owned(), Data::from(fragment));
    }
    Some(Data::Map(link))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn field<'a>(link: &'a Data, name: &str) -> Option<&'a Data> {
        match link {
            Data::Map(map) => map.get(name),
            _ => None,
        }
    }

    fn text(link: &Data, name: &str) -> Option<String> {
        match field(link, name)? {
            Data::String(text) => Some(text.clone()),
            other => panic!("{name}: {other:?}"),
        }
    }

    #[test]
    fn a_link_is_read_for_mods() {
        let schemes = ["notes".to_owned()];
        let link = read(
            "notes:/capture?url=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&title=A+page&title=again#top",
            &schemes,
        )
        .unwrap();
        assert_eq!(text(&link, "scheme").as_deref(), Some("notes"));
        assert_eq!(text(&link, "path").as_deref(), Some("/capture"));
        assert_eq!(text(&link, "host"), None);
        assert_eq!(text(&link, "fragment").as_deref(), Some("top"));
        let query = field(&link, "query").unwrap();
        assert_eq!(
            text(query, "url").as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(
            text(query, "title").as_deref(),
            Some("A page"),
            "the first of a name"
        );

        // With an authority, the host is its own.
        let link = read("NOTES://open/day/2?", &schemes).unwrap();
        assert_eq!(text(&link, "scheme").as_deref(), Some("notes"));
        assert_eq!(text(&link, "host").as_deref(), Some("open"));
        assert_eq!(text(&link, "path").as_deref(), Some("/day/2"));
        assert_eq!(field(&link, "query"), Some(&Data::Map(BTreeMap::new())));
    }

    #[test]
    fn only_links_to_the_apps_schemes_are_read() {
        let schemes = ["notes".to_owned()];
        assert!(read("https://example.com", &schemes).is_none());
        assert!(read("notesx:/new", &schemes).is_none());
        assert!(read("not a url", &schemes).is_none());
        assert!(read("notes:/new", &[]).is_none());
        let long = format!("notes:/new?title={}", "a".repeat(MAX_URL));
        assert!(read(&long, &schemes).is_none());
    }

    #[test]
    fn links_are_taken_a_few_at_a_time() {
        let start = Instant::now();
        let mut taken = VecDeque::new();
        for _ in 0..BURST {
            assert!(admit(&mut taken, start));
        }
        assert!(!admit(&mut taken, start));
        assert!(admit(&mut taken, start + BURST_WINDOW));
        assert_eq!(taken.len(), 1, "the old ones were forgotten");
    }
}
