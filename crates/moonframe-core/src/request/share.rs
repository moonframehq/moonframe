//! `app:share`: the system's share menu (macOS's share picker) for a web
//! address, some text, or files, at the pointer in the window that asked:
//! `{ url = "https://…", text = "Its title" }` or `{ files = { "out.pdf" } }`.
//! Nothing leaves until the user picks a service and goes on in it. With
//! `reply`, the mod hears which service they picked (`service`) or that they
//! closed it (`cancelled = true`).

use {
    super::APP,
    crate::{Data, Error, with_fields},
    std::collections::BTreeMap,
};

/// Longest text shared beside the rest, in bytes.
const MAX_TEXT: usize = 4096;
/// Longest address shared.
const MAX_URL: usize = 2048;
/// Most files shared at once, and longest a file's name can be.
const MAX_FILES: usize = 20;
const MAX_PATH: usize = 1024;

/// What `app:share` offers to share.
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    /// An `https://` or `http://` address.
    pub url: Option<String>,
    pub text: Option<String>,
    /// Files, as `ui.image` names pictures: relative to the mod's folder,
    /// `~/...` or absolute. The app checks each may be shared.
    pub files: Vec<String>,
    /// What the mod hears back, if anything.
    pub reply: Option<Data>,
}

impl Share {
    /// The answer to send back, if the mod asked for one: the service the
    /// user picked, or `cancelled`.
    pub fn answer(&self, service: Option<String>) -> Option<Data> {
        let reply = self.reply.as_ref()?;
        let field = match service {
            Some(service) => ("service".to_owned(), Data::String(service)),
            None => ("cancelled".to_owned(), Data::Bool(true)),
        };
        Some(with_fields(reply, BTreeMap::from([field])))
    }
}

/// `{ url, text, files, reply }`: at least one of `url`, `text` and `files`.
pub(super) fn share(msg: &Data) -> Result<Share, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}share`: {problem}"));
    let usage =
        "takes { url = \"https://...\", text = \"...\", files = { \"...\" }, reply = <msg> }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "url" | "text" | "files" | "reply"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes url, text, files and reply"
        )));
    }
    let url = match fields.get("url") {
        None => None,
        Some(Data::String(url)) if url.len() <= MAX_URL && crate::is_remote_url(url) => {
            Some(url.clone())
        },
        Some(_) => {
            return Err(fail(&format!(
                "`url` is a web address (https:// or http://, at most {MAX_URL} bytes)"
            )));
        },
    };
    let text = match fields.get("text") {
        None => None,
        Some(Data::String(text)) if text.len() <= MAX_TEXT => {
            Some(text.clone()).filter(|text| !text.trim().is_empty())
        },
        Some(_) => {
            return Err(fail(&format!("`text` is text of at most {MAX_TEXT} bytes")));
        },
    };
    let files = match fields.get("files") {
        None => Vec::new(),
        Some(Data::List(files)) if files.len() <= MAX_FILES => files
            .iter()
            .map(|file| match file {
                Data::String(file)
                    if !file.is_empty() && file.len() <= MAX_PATH && !file.contains("://") =>
                {
                    Ok(file.clone())
                },
                _ => Err(fail("each of `files` is a file's path, never an address")),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err(fail(&format!(
                "`files` is a list of at most {MAX_FILES} files"
            )));
        },
    };
    if url.is_none() && text.is_none() && files.is_empty() {
        return Err(fail("needs a `url`, some `text` or `files` to share"));
    }
    Ok(Share {
        url,
        text,
        files,
        reply: fields
            .get("reply")
            .filter(|reply| **reply != Data::Nil)
            .cloned(),
    })
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
    fn a_page_or_files_are_shared() {
        let page = share(&map(&[
            ("url", Data::from("https://example.com/a")),
            ("text", Data::from("A page")),
        ]))
        .unwrap();
        assert_eq!(page.url.as_deref(), Some("https://example.com/a"));
        assert_eq!(page.text.as_deref(), Some("A page"));
        assert!(page.files.is_empty() && page.reply.is_none());
        assert_eq!(page.answer(Some("Mail".to_owned())), None, "no reply asked");

        let files = share(&map(&[
            ("files", Data::List(vec![Data::from("out/news.pdf")])),
            ("reply", map(&[("shared", Data::Bool(true))])),
        ]))
        .unwrap();
        assert_eq!(files.files, ["out/news.pdf"]);
        assert_eq!(
            files.answer(Some("AirDrop".to_owned())),
            Some(map(&[
                ("shared", Data::Bool(true)),
                ("service", Data::from("AirDrop")),
            ]))
        );
        assert_eq!(
            files.answer(None),
            Some(map(&[
                ("shared", Data::Bool(true)),
                ("cancelled", Data::Bool(true)),
            ]))
        );
    }

    #[test]
    fn what_cannot_be_shared_is_the_senders_error() {
        for (bad, says) in [
            (map(&[]), "needs a `url`"),
            (map(&[("text", Data::from("  "))]), "needs a `url`"),
            (
                map(&[("url", Data::from("file:///etc/passwd"))]),
                "web address",
            ),
            (
                map(&[("url", Data::from("javascript:alert(1)"))]),
                "web address",
            ),
            (
                map(&[("files", Data::List(vec![Data::from("https://x/y")]))]),
                "never an address",
            ),
            (map(&[("files", Data::from("a.pdf"))]), "a list"),
            (
                map(&[("files", Data::List(vec![Data::from("a"); MAX_FILES + 1]))]),
                "at most 20",
            ),
            (
                map(&[("text", Data::from("x".repeat(MAX_TEXT + 1)))]),
                "4096",
            ),
            (map(&[("title", Data::from("x"))]), "unknown key"),
            (Data::from("https://example.com"), "takes"),
        ] {
            let problem = share(&bad).unwrap_err().to_string();
            assert!(problem.contains(says), "{bad:?}: {problem}");
        }
    }
}
