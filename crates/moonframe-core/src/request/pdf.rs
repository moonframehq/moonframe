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
    std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr},
    },
};

/// Longest a file's name can be.
const MAX_PATH: usize = 1024;
/// Longest address loaded.
const MAX_URL: usize = 2048;

/// What `app:pdf` renders, and where to.
#[derive(Clone, Debug, PartialEq)]
pub struct Pdf {
    pub source: PdfSource,
    /// The PDF to write: relative to the app's cache folder, or a
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
            if url.len() <= MAX_URL && crate::is_remote_url(url) =>
        {
            PdfSource::Url(public_url(url).ok_or_else(|| {
                fail(
                    "`url` is a web address (https:// or http://) on the internet, not this \
                     machine or its network",
                )
            })?)
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

/// `url` as the web view is given it, when it names a host on the
/// internet: parsed as browsers do (WHATWG), so the host checked is the
/// host loaded, and written back out in that form. `None` for this machine
/// or a private network by address or name, an address with credentials or
/// characters a browser would drop or reread (`\\`, spaces, controls), and
/// every IPv6 literal, as the page's content rules block them all. A name
/// that resolves to a private address is refused as it connects (the app's
/// PDF proxy).
fn public_url(url: &str) -> Option<String> {
    if url
        .chars()
        .any(|ch| ch == '\\' || ch.is_whitespace() || ch.is_control())
    {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    let public = match parsed.host()? {
        url::Host::Domain(name) => {
            // A name may end in a dot: `localhost.` is `localhost`.
            let name = name.trim_end_matches('.');
            !(name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local"))
        },
        url::Host::Ipv4(ip) => public_address(IpAddr::V4(ip)),
        url::Host::Ipv6(_) => false,
    };
    public.then(|| parsed.into())
}

/// Whether `ip` is on the public internet: not this machine, a private or
/// shared network (`10/8`, `100.64/10`, `fc00::/7`), a link, multicast, or
/// a range kept for documents, tests or the future. IPv6 addresses carrying
/// an IPv4 one (mapped, NAT64, 6to4) are judged by it; Teredo is refused.
pub fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_v4(v4);
            }
            let embedded = |high: u16, low: u16| {
                Ipv4Addr::new((high >> 8) as u8, high as u8, (low >> 8) as u8, low as u8)
            };
            match segments {
                // NAT64: the IPv4 address in the low 32 bits.
                [0x64, 0xff9b, 0, 0, 0, 0, high, low] => public_v4(embedded(high, low)),
                // 6to4: the IPv4 address after the prefix.
                [0x2002, high, low, ..] => public_v4(embedded(high, low)),
                // Teredo, and documentation.
                [0x2001, 0, ..] | [0x2001, 0xdb8, ..] => false,
                // Global unicast is `2000::/3`; the rest is local, multicast
                // or reserved.
                [first, ..] => first & 0xe000 == 0x2000,
            }
        },
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113))
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
        assert!(
            public_url("https://example.com:8443/a").is_some(),
            "https://example.com:8443/a"
        );
        assert!(public_url("https://8.8.8.8/").is_some(), "https://8.8.8.8/");
        assert!(
            public_url("https://example.com./").is_some(),
            "https://example.com./"
        );
        assert!(
            public_url("https://cdn.2024.example/").is_some(),
            "https://cdn.2024.example/"
        );
        assert_eq!(
            public_url("HTTPS://Example.COM:443/a b").as_deref(),
            None,
            "a space is refused, not reread"
        );
        assert_eq!(
            public_url("HTTPS://Example.COM:443/a").as_deref(),
            Some("https://example.com/a"),
            "the page loads the address as checked"
        );
        for private in [
            "http://localhost./",
            "http://LOCALHOST.:3000/",
            "http://app.localhost./",
            "http://10.0.0.1./",
            "http://[::ffff:192.168.0.1]/",
            "http://[2001:db8::1]/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://0177.0.0.1/",
            "http://127.1/",
            "http://100.64.0.1/",
            "http://198.18.0.1/",
            "http://224.0.0.1/",
            "http://255.255.255.255/",
            "http://example.com\\@127.0.0.1/",
            "http://127.0.0.1\\.example.com/",
            "http://exa\tmple.com/",
            "http:/\\127.0.0.1/",
            "ftp://example.com/",
        ] {
            assert!(public_url(private).is_none(), "{private}");
        }
    }

    #[test]
    fn only_the_public_internet_is_public() {
        for public in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
        ] {
            assert!(public_address(public.parse().unwrap()), "{public}");
        }
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "100.100.1.1",
            "172.31.255.255",
            "192.168.0.1",
            "169.254.169.254",
            "0.0.0.0",
            "239.1.1.1",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "64:ff9b::a00:1",
            "2002:c0a8:1::1",
            "2001::1",
            "2001:db8::1",
        ] {
            assert!(!public_address(private.parse().unwrap()), "{private}");
        }
    }
}
