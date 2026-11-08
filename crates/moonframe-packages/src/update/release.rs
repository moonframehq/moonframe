//! Asking GitHub for the newest release, and reading what it answers: its
//! version, compared with the running one, and the build for this platform.

use {
    super::Source,
    serde::Deserialize,
    std::{cmp::Ordering, time::Duration},
};

/// A release newer than the running version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Its version, without the tag's `v`.
    pub version: String,
    /// Its page on GitHub, with the notes and every download.
    pub page: String,
    /// The download for this platform, if it has one.
    pub asset: Option<Asset>,
    /// Its `SHA256SUMS`, if it has one.
    pub(super) sums: Option<String>,
}

/// A file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub(crate) name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

/// What GitHub says of a release.
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
    #[serde(default)]
    draft: bool,
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The running version is the newest (or newer: a build from source).
    UpToDate {
        latest: String,
    },
    /// The repository has published no release yet.
    NoRelease,
    Newer(Release),
}

impl Release {
    /// Whether it can replace the running app: on macOS, from its `.app`,
    /// with a disk image or zip to install from.
    pub fn installable(&self, source: &Source) -> bool {
        source.bundle.is_some()
            && self
                .asset
                .as_ref()
                .is_some_and(|asset| asset.name.ends_with(".dmg") || asset.name.ends_with(".zip"))
    }
}

/// A client whose whole answer must arrive within `body`: short for
/// GitHub's answers, long for a download, never unbounded.
pub(super) fn agent(body: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(body))
        .user_agent(format!(
            "{}/{} (+https://moonframe.dev)",
            crate::identity().name,
            crate::identity().version
        ))
        .build()
        .new_agent()
}

/// How long GitHub's answer, or `SHA256SUMS`, may take to arrive.
pub(super) const ANSWER: Duration = Duration::from_secs(60);

/// How long a download may take: a disk image over a slow line, but not a
/// connection that stalled for good.
pub(super) const DOWNLOAD: Duration = Duration::from_secs(30 * 60);

/// Asks GitHub for `source`'s newest release: the newest published one,
/// or with `beta`, the newest pre-release too. Blocks on the network.
pub fn check(source: &Source, beta: bool) -> Result<Found, String> {
    let release = if beta {
        match newest(every_release(source)?) {
            Some(release) => release,
            None => return Ok(Found::NoRelease),
        }
    } else {
        let Some(text) = ask(&format!("{}/latest", releases(source)))? else {
            return Ok(Found::NoRelease);
        };
        serde_json::from_str(&text)
            .map_err(|error| format!("unexpected answer from GitHub: {error}"))?
    };
    Ok(found(release, &source.version))
}

/// How many releases a page of GitHub's list holds, at most.
const PER_PAGE: usize = 100;

/// How many pages of releases a beta check reads at most: a thousand
/// releases, so a repository with years of nightlies still answers in a
/// few requests.
const PAGES: usize = 10;

/// `source`'s releases, every page of them (up to [`PAGES`]): GitHub lists
/// them newest first by date, not by version, so a backport or nightly
/// published after a higher version can push it onto a later page.
fn every_release(source: &Source) -> Result<Vec<GithubRelease>, String> {
    let mut all = Vec::new();
    for page in 1..=PAGES {
        let url = format!("{}?per_page={PER_PAGE}&page={page}", releases(source));
        let Some(text) = ask(&url)? else {
            break;
        };
        let releases: Vec<GithubRelease> = serde_json::from_str(&text)
            .map_err(|error| format!("unexpected answer from GitHub: {error}"))?;
        let last = releases.len() < PER_PAGE;
        all.extend(releases);
        if last {
            break;
        }
    }
    Ok(all)
}

/// The API address of `source`'s releases.
fn releases(source: &Source) -> String {
    format!("https://api.github.com/repos/{}/releases", source.repo)
}

/// GitHub's answer at `url`, or `None` when it has none (404: no published
/// release yet).
fn ask(url: &str) -> Result<Option<String>, String> {
    let response = agent(ANSWER)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();
    match response {
        Ok(mut response) => response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_string()
            .map(Some)
            .map_err(|error| format!("cannot read GitHub's answer: {error}")),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(ureq::Error::StatusCode(403 | 429)) => {
            Err("GitHub is limiting requests; try again later".to_owned())
        },
        Err(error) => Err(format!("cannot reach GitHub: {error}")),
    }
}

/// The release with the highest version among `all`, pre-releases
/// included, drafts and tags that read as no version left out.
fn newest(all: Vec<GithubRelease>) -> Option<GithubRelease> {
    all.into_iter()
        .filter(|release| !release.draft && parse(&release.tag_name).is_some())
        .reduce(|best, release| {
            if newer(&release.tag_name, &best.tag_name) {
                release
            } else {
                best
            }
        })
}

/// What `release` means for a run of `current`.
fn found(release: GithubRelease, current: &str) -> Found {
    let version = strip_v(&release.tag_name).to_owned();
    if !newer(&version, current) {
        return Found::UpToDate { latest: version };
    }
    let asset = pick(
        &release.assets,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
    .cloned();
    let sums = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .map(|asset| asset.url.clone());
    Found::Newer(Release {
        version,
        page: release.html_url,
        asset,
        sums,
    })
}

fn strip_v(tag: &str) -> &str {
    let tag = tag.trim();
    tag.strip_prefix(['v', 'V']).unwrap_or(tag)
}

/// A version's numbers and pre-release, as semver orders them; build
/// metadata (`+…`) is left out.
fn parse(version: &str) -> Option<(Vec<u64>, Option<&str>)> {
    let version = strip_v(version);
    let version = version.split_once('+').map_or(version, |(core, _)| core);
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let numbers = core
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u64>>>()?;
    Some((numbers, pre))
}

/// Whether `latest` is a newer version than `current`. A version that does
/// not read as one is never newer: no offer beats a wrong one.
pub(crate) fn newer(latest: &str, current: &str) -> bool {
    let (Some((mut a, a_pre)), Some((mut b, b_pre))) = (parse(latest), parse(current)) else {
        return false;
    };
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    match a.cmp(&b) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match (a_pre, b_pre) {
            // 1.0.0 is newer than 1.0.0-beta.
            (None, Some(_)) => true,
            (Some(a), Some(b)) => compare_pre(a, b) == Ordering::Greater,
            _ => false,
        },
    }
}

/// Orders two pre-releases as semver does: dot by dot, numbers below words.
fn compare_pre(a: &str, b: &str) -> Ordering {
    let mut a = a.split('.');
    let mut b = b.split('.');
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            },
        }
    }
}

/// The download for `os` and `arch` (Rust's names: `macos`, `aarch64`)
/// among `assets`, by its name: the right kind of file, naming the platform
/// and not another architecture.
pub(crate) fn pick<'a>(assets: &'a [Asset], os: &str, arch: &str) -> Option<&'a Asset> {
    const ARCHES: &[(&str, &[&str])] = &[
        ("aarch64", &["aarch64", "arm64"]),
        ("x86_64", &["x86_64", "x64", "amd64", "intel"]),
    ];
    let own: &[&str] = ARCHES
        .iter()
        .find(|(name, _)| *name == arch)
        .map_or(&[], |(_, words)| words);
    let others: Vec<&str> = ARCHES
        .iter()
        .filter(|(name, _)| *name != arch)
        .flat_map(|(_, words)| words.iter().copied())
        .collect();
    let score = |asset: &Asset| -> Option<u32> {
        let name = asset.name.to_ascii_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
        let (kind, hinted) = match os {
            "macos" => {
                let hinted = has(&["darwin", "macos", "mac", "osx", "universal"]);
                match () {
                    () if name.ends_with(".dmg") => (3, hinted),
                    // A zip is the Mac's only when its name says so.
                    () if name.ends_with(".zip") && hinted => (2, true),
                    () => return None,
                }
            },
            "linux" => {
                let hinted = has(&["linux"]);
                match () {
                    () if name.ends_with(".appimage") => (3, hinted),
                    () if (name.ends_with(".tar.gz") || name.ends_with(".tgz")) && hinted => {
                        (2, true)
                    },
                    () => return None,
                }
            },
            "windows" => {
                let hinted = has(&["windows", "win64", "win32", "msvc"]);
                match () {
                    () if name.ends_with(".msi") => (3, hinted),
                    () if name.ends_with(".zip") && hinted => (2, true),
                    () if name.ends_with(".exe") && hinted => (1, true),
                    () => return None,
                }
            },
            _ => return None,
        };
        let universal = name.contains("universal");
        let ours = has(own);
        // Another architecture's build is no use here.
        if !ours && !universal && has(&others) {
            return None;
        }
        Some(kind * 10 + u32::from(hinted) * 4 + u32::from(ours || universal) * 2)
    };
    assets
        .iter()
        .filter_map(|asset| score(asset).map(|score| (score, asset)))
        .max_by_key(|(score, _)| *score)
        .map(|(_, asset)| asset)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.to_owned(),
            url: format!("https://example.com/{name}"),
        }
    }

    #[test]
    fn versions_order_as_semver() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(newer("1.10.0", "1.9.0"));
        assert!(newer("1.0.1", "1.0"));
        assert!(newer("1.0.0", "1.0.0-beta.2"));
        assert!(newer("1.0.0-beta.10", "1.0.0-beta.2"));
        assert!(newer("1.0.0-rc.1", "1.0.0-beta.9"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("1.0.0+build.5", "1.0.0"));
        assert!(!newer("0.9.0", "1.0.0"));
        assert!(!newer("1.0.0-beta", "1.0.0"));
        assert!(!newer("nightly", "1.0.0"));
        assert!(!newer("2.0.0", "dev"));
    }

    #[test]
    fn picks_the_build_for_the_platform() {
        let assets = [
            asset("Moonframe-0.2.0-universal-apple-darwin.dmg"),
            asset("Moonframe-0.2.0-universal-apple-darwin.dmg.sigstore.json"),
            asset("Moonframe-0.2.0-x86_64-unknown-linux-gnu.tar.gz"),
            asset("Moonframe-0.2.0-aarch64-unknown-linux-gnu.tar.gz"),
            asset("Moonframe-0.2.0-x86_64-pc-windows-msvc.zip"),
            asset("SHA256SUMS"),
        ];
        let name = |os, arch| pick(&assets, os, arch).map(|asset| asset.name.as_str());
        assert_eq!(
            name("macos", "aarch64"),
            Some("Moonframe-0.2.0-universal-apple-darwin.dmg")
        );
        assert_eq!(
            name("linux", "aarch64"),
            Some("Moonframe-0.2.0-aarch64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(
            name("linux", "x86_64"),
            Some("Moonframe-0.2.0-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(
            name("windows", "x86_64"),
            Some("Moonframe-0.2.0-x86_64-pc-windows-msvc.zip")
        );
        assert_eq!(name("windows", "aarch64"), None);
    }

    #[test]
    fn prefers_the_mac_build_for_this_architecture() {
        let assets = [
            asset("App-1.0-x86_64.dmg"),
            asset("App-1.0-arm64.dmg"),
            asset("App-1.0-windows.zip"),
        ];
        assert_eq!(
            pick(&assets, "macos", "aarch64").map(|a| a.name.as_str()),
            Some("App-1.0-arm64.dmg")
        );
        assert_eq!(
            pick(&assets, "macos", "x86_64").map(|a| a.name.as_str()),
            Some("App-1.0-x86_64.dmg")
        );
        // A zip with no Mac in its name is not taken for the Mac's.
        assert_eq!(pick(&[asset("App-1.0.zip")], "macos", "aarch64"), None);
        assert_eq!(
            pick(&[asset("App.dmg")], "macos", "aarch64").map(|a| a.name.as_str()),
            Some("App.dmg")
        );
    }

    #[test]
    fn reads_a_release() {
        let release: GithubRelease = serde_json::from_str(
            r#"{
                "tag_name": "v0.3.0",
                "html_url": "https://github.com/moonframehq/moonframe/releases/tag/v0.3.0",
                "draft": false,
                "assets": [
                    {"name": "Moonframe-0.3.0-universal-apple-darwin.dmg",
                     "browser_download_url": "https://github.com/d/Moonframe-0.3.0-universal-apple-darwin.dmg",
                     "size": 10},
                    {"name": "SHA256SUMS", "browser_download_url": "https://github.com/d/SHA256SUMS"}
                ]
            }"#,
        )
        .expect("a release");
        let Found::Newer(newer) = found(release, "0.1.0") else {
            panic!("0.3.0 is newer than 0.1.0");
        };
        assert_eq!(newer.version, "0.3.0");
        assert_eq!(
            newer.sums.as_deref(),
            Some("https://github.com/d/SHA256SUMS")
        );
        let release: GithubRelease =
            serde_json::from_str(r#"{"tag_name": "0.1.0", "html_url": "u"}"#).expect("a release");
        assert_eq!(
            found(release, "0.1.0"),
            Found::UpToDate {
                latest: "0.1.0".to_owned()
            }
        );
    }

    #[test]
    fn beta_takes_the_newest_release_with_pre_releases() {
        let all: Vec<GithubRelease> = serde_json::from_str(
            r#"[
                {"tag_name": "v1.2.0-beta.1", "html_url": "b1", "prerelease": true},
                {"tag_name": "v1.3.0-beta.1", "html_url": "draft", "draft": true},
                {"tag_name": "v1.1.0", "html_url": "stable"},
                {"tag_name": "nightly", "html_url": "nightly", "prerelease": true},
                {"tag_name": "v1.2.0-beta.2", "html_url": "b2", "prerelease": true}
            ]"#,
        )
        .expect("releases");
        let release = newest(all).expect("a release");
        assert_eq!(release.html_url, "b2");
        let Found::Newer(newer) = found(release, "1.1.0") else {
            panic!("1.2.0-beta.2 is newer than 1.1.0");
        };
        assert_eq!(newer.version, "1.2.0-beta.2");
        // Once the stable release is out, it beats its betas.
        let all: Vec<GithubRelease> = serde_json::from_str(
            r#"[
                {"tag_name": "v1.2.0", "html_url": "stable"},
                {"tag_name": "v1.2.0-rc.1", "html_url": "rc", "prerelease": true}
            ]"#,
        )
        .expect("releases");
        assert_eq!(newest(all).map(|r| r.html_url).as_deref(), Some("stable"));
        assert!(newest(Vec::new()).is_none());
    }
}
