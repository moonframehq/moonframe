//! What the host keeps by an id and draws itself: terminals, browsers
//! and registered views, and the addresses a browser takes.

use super::*;

tokens! {
    /// The emulator a terminal runs on; both are built by default.
    TerminalEngine {
        /// `alacritty_terminal`, pure Rust.
        Alacritty = "alacritty",
        /// Ghostty's emulator, libghostty-vt.
        Ghostty = "ghostty",
    }
}

tokens! {
    /// What keeps a terminal's shell running.
    TerminalBackend {
        /// The best this machine has: herdr-core, then tmux, then a pty.
        Auto = "auto",
        /// herdr-core (`hc`): the shell outlives the app.
        HerdrCore = "herdr-core",
        /// A tmux session on the app's own server: the shell outlives the app.
        Tmux = "tmux",
        /// A local pseudo-terminal: the shell ends with the app.
        Pty = "pty",
    }
}

/// A shell the host runs. The host keeps it by the mod and `id`, so it
/// lives across reloads and while its pane is hidden, and with herdr-core
/// or tmux across launches; a mod picks where it starts, never what runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub look: Look,
    pub id: String,
    pub cwd: Option<String>,
    pub engine: TerminalEngine,
    pub backend: TerminalBackend,
    /// Sent to the mod once, with `exited = true` added, as the shell
    /// exits (Ctrl-D, `exit`). Without it the terminal says the shell
    /// exited and Enter starts another.
    pub exit: Option<Data>,
}

/// A web page the host shows in a native web view (WebKit on macOS,
/// WebView2 on Windows), kept by the mod and `id` like a terminal. The mod
/// gives the address; it never runs scripts in the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    pub look: Look,
    pub id: String,
    /// Where to go: loaded when it differs from the last one the mod gave,
    /// so the user's own navigation stands until the mod asks again.
    pub url: Option<String>,
    /// The page's zoom factor; 1 is its own size.
    pub zoom: f32,
    /// Sent to the mod as the page goes somewhere or its title changes,
    /// with `url` and `title` added.
    pub navigate: Option<Data>,
    /// Sent as a load starts and finishes, with `loading` and `url` added.
    pub loading: Option<Data>,
}

/// A view the app on Moonframe writes in Rust and registers by `name`
/// (`Moonframe::view`), kept by the mod and `id` like a terminal; the host
/// hands it `props` and tells it when they change. Only the app's own
/// views: Luau never names a Rust type.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub look: Look,
    pub name: String,
    pub id: String,
    pub props: Data,
}

/// Whether `url` is an `http` or `https` address: what may go to the system
/// browser or open from a page's popup. A `file` address handed to the
/// system opens whatever program the file is, so it never goes there.
pub fn is_remote_url(url: &str) -> bool {
    is_web_url(url)
        && url.split_once("://").is_some_and(|(scheme, _)| {
            matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
        })
}

/// Whether `url` is a `file` address.
pub fn is_file_url(url: &str) -> bool {
    url.split_once("://")
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
}

/// Whether a browser may show `url`: an `http`, `https` or `file` address,
/// one line without spaces, as a mod writes it or a page navigates.
pub fn is_web_url(url: &str) -> bool {
    const MAX: usize = 8192;
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    let host_given = match scheme.as_str() {
        "http" | "https" => rest
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|host| !host.is_empty()),
        // `file:///path`, or `file://localhost/path`.
        "file" => rest.starts_with('/') || rest.starts_with("localhost/"),
        _ => false,
    };
    host_given && url.len() <= MAX && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Where text typed in a browser's address field goes, as a browser does
/// it: an address as written, a host (`example.com`, `localhost:3000`)
/// over https (http for the machine itself), and anything else as a web
/// search. None for blank text.
pub fn typed_address(text: &str) -> Option<String> {
    const SEARCH: &str = "https://duckduckgo.com/?q=";
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("://") {
        return is_web_url(text).then(|| text.to_owned());
    }
    if !text.chars().any(char::is_whitespace) {
        let host = text.split(['/', '?', '#']).next().unwrap_or_default();
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        let local =
            matches!(name, "localhost" | "127.0.0.1" | "[::1]") || name.ends_with(".localhost");
        if local || name.contains('.') && !name.starts_with('.') && !name.ends_with('.') {
            let scheme = if local {
                "http"
            } else {
                "https"
            };
            let url = format!("{scheme}://{text}");
            if is_web_url(&url) {
                return Some(url);
            }
        }
    }
    let mut query = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                query.push(byte as char);
            },
            b' ' => query.push('+'),
            _ => query.push_str(&format!("%{byte:02X}")),
        }
    }
    Some(format!("{SEARCH}{query}")).filter(|url| is_web_url(url))
}
