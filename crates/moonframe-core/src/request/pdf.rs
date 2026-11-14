//! `app:pdf`: a page rendered to a PDF file, as a reader saves an article:
//! `{ html = "<h1>…", pdf = "articles/a.pdf", reply = { saved = true } }`.
//! The page is HTML the mod has (`html`, or an `.html` file of the app's,
//! `html_file`), shown with no script and nothing from the network, or a web
//! address (`url`), loaded in a session of its own. It is laid out on paper
//! pages (`paper`, else the system's), and written under the app's cache or
//! state folders. With `reply`, the mod hears the file's `path`, or `error`.

use {
    super::APP,
    crate::{Data, Error, with_fields},
    std::collections::BTreeMap,
};

/// Longest a file's name can be.
const MAX_PATH: usize = 1024;
/// Longest address loaded.
const MAX_URL: usize = 2048;

/// What `app:pdf` renders, and where to.
#[derive(Clone, Debug, PartialEq)]
pub struct Pdf {
    pub source: PdfSource,
    /// The PDF to write: relative to the app's cache folder's `pdf/`, or a
    /// path inside the app's cache or state folders. The app checks it.
    pub pdf: String,
    pub paper: Option<Paper>,
    pub reply: Option<Data>,
}

/// The page a PDF is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfSource {
    /// A whole page as text: no script, nothing loaded but `data:`.
    Html(String),
    /// An `.html` file of the app's, read as `Html` is.
    HtmlFile(String),
    /// An `https://` or `http://` page, loaded with its script, in a session
    /// of its own.
    Url(String),
}

tokens! {
    /// The paper a PDF's pages are.
    Paper {
        A4 = "a4",
        Letter = "letter",
    }
}

impl Pdf {
    /// The answer to send back, if the mod asked for one: the file written,
    /// or why not.
    pub fn answer(&self, written: Result<String, String>) -> Option<Data> {
        let reply = self.reply.as_ref()?;
        let field = match written {
            Ok(path) => ("path".to_owned(), Data::String(path)),
            Err(problem) => ("error".to_owned(), Data::String(problem)),
        };
        Some(with_fields(reply, BTreeMap::from([field])))
    }
}

/// `{ html | html_file | url, pdf, paper, reply }`.
pub(super) fn pdf(msg: &Data) -> Result<Pdf, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}pdf`: {problem}"));
    let usage = "takes { html = \"...\" or html_file = \"page.html\" or url = \"https://...\", \
                 pdf = \"out.pdf\", paper = \"a4\" or \"letter\", reply = <msg> }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields.keys().find(|key| {
        !matches!(
            key.as_str(),
            "html" | "html_file" | "url" | "pdf" | "paper" | "reply"
        )
    }) {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes html, html_file, url, pdf, paper and reply"
        )));
    }
    let path = |key: &str, extensions: &[&str]| match fields.get(key) {
        Some(Data::String(path))
            if !path.is_empty()
                && path.len() <= MAX_PATH
                && !path.contains("://")
                && extensions.iter().any(|extension| {
                    path.to_ascii_lowercase()
                        .ends_with(&format!(".{extension}"))
                }) =>
        {
            Ok(path.clone())
        },
        _ => Err(fail(&format!(
            "`{key}` is a file's path ending in .{}",
            extensions.join(" or .")
        ))),
    };
    let source = match (
        fields.get("html"),
        fields.get("html_file"),
        fields.get("url"),
    ) {
        (Some(Data::String(html)), None, None) if !html.trim().is_empty() => {
            PdfSource::Html(html.clone())
        },
        (Some(_), None, None) => return Err(fail("`html` is a page's text")),
        (None, Some(_), None) => PdfSource::HtmlFile(path("html_file", &["html", "htm"])?),
        (None, None, Some(Data::String(url)))
            if url.len() <= MAX_URL && crate::is_remote_url(url) && !private_host(url) =>
        {
            PdfSource::Url(url.clone())
        },
        (None, None, Some(_)) => {
            return Err(fail(
                "`url` is a web address (https:// or http://) on the internet, not this machine \
                 or its network",
            ));
        },
        _ => return Err(fail("one of `html`, `html_file` and `url`")),
    };
    let paper = match fields.get("paper") {
        None => None,
        Some(Data::String(name)) => {
            Some(Paper::parse(name).ok_or_else(|| fail("`paper` is \"a4\" or \"letter\""))?)
        },
        Some(_) => return Err(fail("`paper` is \"a4\" or \"letter\"")),
    };
    Ok(Pdf {
        source,
        pdf: path("pdf", &["pdf"])?,
        paper,
        reply: fields
            .get("reply")
            .filter(|reply| **reply != Data::Nil)
            .cloned(),
    })
}

/// Whether `url` names this machine or a private network by its address
/// or name: a page made into a PDF reaches only the internet. A name that
/// resolves to one is refused as the page loads (see the app's rules).
pub fn private_host(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return true;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Credentials in an address are refused too.
    if authority.contains('@') {
        return true;
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    }
    .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(ip) => {
                ip.is_loopback()
                    || ip.is_private()
                    || ip.is_link_local()
                    || ip.is_unspecified()
                    || ip.is_broadcast()
            },
            std::net::IpAddr::V6(ip) => {
                ip.is_loopback()
                    || ip.is_unspecified()
                    || (ip.segments()[0] & 0xfe00) == 0xfc00
                    || (ip.segments()[0] & 0xffc0) == 0xfe80
            },
        };
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn map(fields: &[(&str, Data)]) -> Data {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[test]
    fn a_page_is_made_into_a_pdf() {
        let request = pdf(&map(&[
            ("html", Data::from("<h1>Hi</h1>")),
            ("pdf", Data::from("articles/hi.PDF")),
            ("paper", Data::from("letter")),
            ("reply", map(&[("saved", Data::Bool(true))])),
        ]))
        .unwrap();
        assert_eq!(request.source, PdfSource::Html("<h1>Hi</h1>".to_owned()));
        assert_eq!(request.pdf, "articles/hi.PDF");
        assert_eq!(request.paper, Some(Paper::Letter));
        assert_eq!(
            request.answer(Ok("/cache/pdf/articles/hi.PDF".to_owned())),
            Some(map(&[
                ("saved", Data::Bool(true)),
                ("path", Data::from("/cache/pdf/articles/hi.PDF")),
            ]))
        );
        assert_eq!(
            request.answer(Err("timed out".to_owned())),
            Some(map(&[
                ("saved", Data::Bool(true)),
                ("error", Data::from("timed out")),
            ]))
        );
        let page = pdf(&map(&[
            ("url", Data::from("https://example.com/a")),
            ("pdf", Data::from("a.pdf")),
        ]))
        .unwrap();
        assert_eq!(page.answer(Ok("x".to_owned())), None, "no reply asked");
        let file = pdf(&map(&[
            ("html_file", Data::from("mail/1.html")),
            ("pdf", Data::from("mail/1.pdf")),
        ]))
        .unwrap();
        assert_eq!(file.source, PdfSource::HtmlFile("mail/1.html".to_owned()));
    }

    #[test]
    fn what_cannot_be_made_is_the_senders_error() {
        let html = ("html", Data::from("<p>x</p>"));
        let out = ("pdf", Data::from("x.pdf"));
        for (bad, says) in [
            (map(std::slice::from_ref(&out)), "one of"),
            (
                map(&[
                    html.clone(),
                    ("url", Data::from("https://a.b")),
                    out.clone(),
                ]),
                "one of",
            ),
            (map(std::slice::from_ref(&html)), "`pdf` is a file's path"),
            (map(&[html.clone(), ("pdf", Data::from("x.png"))]), ".pdf"),
            (
                map(&[html.clone(), ("pdf", Data::from("https://x/y.pdf"))]),
                ".pdf",
            ),
            (
                map(&[("html", Data::from("  ")), out.clone()]),
                "page's text",
            ),
            (
                map(&[("html_file", Data::from("x.txt")), out.clone()]),
                ".html",
            ),
            (
                map(&[("url", Data::from("file:///etc/passwd")), out.clone()]),
                "web address",
            ),
            (
                map(&[("url", Data::from("http://127.0.0.1:8080/")), out.clone()]),
                "not this machine",
            ),
            (
                map(&[("url", Data::from("http://192.168.1.4/admin")), out.clone()]),
                "not this machine",
            ),
            (
                map(&[("url", Data::from("http://localhost/")), out.clone()]),
                "not this machine",
            ),
            (
                map(&[("url", Data::from("http://[::1]/")), out.clone()]),
                "not this machine",
            ),
            (
                map(&[
                    ("url", Data::from("https://me:pw@example.com/")),
                    out.clone(),
                ]),
                "not this machine",
            ),
            (
                map(&[html.clone(), out.clone(), ("paper", Data::from("a3"))]),
                "\"a4\" or \"letter\"",
            ),
            (map(&[html, out, ("title", Data::from("x"))]), "unknown key"),
            (Data::from("<p>x</p>"), "takes"),
        ] {
            let problem = pdf(&bad).unwrap_err().to_string();
            assert!(problem.contains(says), "{bad:?}: {problem}");
        }
        assert!(!private_host("https://example.com:8443/a"));
        assert!(!private_host("https://8.8.8.8/"));
    }
}
