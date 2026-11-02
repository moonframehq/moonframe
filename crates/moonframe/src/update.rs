//! **Check for Updates…** in the app menu: asks GitHub for the newest
//! release of the app's repository (`owner/repo`), compares it with the
//! running version, and offers it. On macOS, an app running from its own
//! `.app` installs the release's disk image (or zip) in place of itself and
//! starts again; anywhere else, the release's page opens to download from.
//!
//! The repository is the one a library app names with
//! [`Moonframe::updates`](crate::Moonframe::updates) (the starter's is
//! `penso/moonframe`), or in a profile its manifest's `updates`, with its
//! `version`. Releases follow GitHub's usual path: a tag `v1.2.3` (the `v`
//! is optional), published (not a draft or pre-release), with the builds as
//! assets named for their platform, such as
//! `App-1.2.3-universal-apple-darwin.dmg`, and optionally a `SHA256SUMS`
//! file the download is checked against.

use {
    serde::Deserialize,
    std::{
        cmp::Ordering,
        fs,
        io::{self, Read as _},
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
        time::{Duration, SystemTime},
    },
};

/// Where this run's updates come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    /// The GitHub repository, `owner/repo`.
    pub(crate) repo: String,
    /// The version running now.
    pub(crate) version: String,
    /// The `.app` this run can replace with a newer one: set only on macOS,
    /// when the bundle running is the app being updated.
    pub(crate) bundle: Option<PathBuf>,
}

/// This run's source of updates, set once at launch.
pub(crate) static SOURCE: OnceLock<Option<Source>> = OnceLock::new();

/// Where this run's updates come from, if anywhere: the menu shows Check
/// for Updates only then.
pub(crate) fn source() -> Option<&'static Source> {
    SOURCE.get().and_then(Option::as_ref)
}

/// Whether `repo` names a GitHub repository: `owner/repo`, of letters,
/// digits, `-`, `_` and `.`.
pub(crate) fn valid_repo(repo: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!(repo.split_once('/'), Some((owner, name)) if part(owner) && part(name))
}

/// The `.app` the running executable is in, on macOS.
pub(crate) fn running_bundle() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe()
        .and_then(|exe| exe.canonicalize())
        .ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_owned())
}

/// A release newer than the running version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Release {
    /// Its version, without the tag's `v`.
    pub(crate) version: String,
    /// Its page on GitHub, with the notes and every download.
    pub(crate) page: String,
    /// The download for this platform, if it has one.
    pub(crate) asset: Option<Asset>,
    /// Its `SHA256SUMS`, if it has one.
    sums: Option<String>,
}

/// A file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct Asset {
    pub(crate) name: String,
    #[serde(rename = "browser_download_url")]
    pub(crate) url: String,
}

/// What GitHub says of a release.
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Found {
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
    pub(crate) fn installable(&self, source: &Source) -> bool {
        source.bundle.is_some()
            && self
                .asset
                .as_ref()
                .is_some_and(|asset| asset.name.ends_with(".dmg") || asset.name.ends_with(".zip"))
    }
}

/// A client whose whole answer must arrive within `body`: short for
/// GitHub's answers, long for a download, never unbounded.
fn agent(body: Duration) -> ureq::Agent {
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
const ANSWER: Duration = Duration::from_secs(60);

/// How long a download may take: a disk image over a slow line, but not a
/// connection that stalled for good.
const DOWNLOAD: Duration = Duration::from_secs(30 * 60);

/// Asks GitHub for `source`'s newest release. Blocks on the network.
pub(crate) fn check(source: &Source) -> Result<Found, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        source.repo
    );
    let response = agent(ANSWER)
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();
    let text = match response {
        Ok(mut response) => response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_string()
            .map_err(|error| format!("cannot read GitHub's answer: {error}"))?,
        // A repository with no published release answers 404.
        Err(ureq::Error::StatusCode(404)) => return Ok(Found::NoRelease),
        Err(ureq::Error::StatusCode(403 | 429)) => {
            return Err("GitHub is limiting requests; try again later".to_owned());
        },
        Err(error) => return Err(format!("cannot reach GitHub: {error}")),
    };
    let release: GithubRelease = serde_json::from_str(&text)
        .map_err(|error| format!("unexpected answer from GitHub: {error}"))?;
    Ok(found(release, &source.version))
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

/// When the last automatic check ran, kept in the state folder per app and
/// profile.
fn stamp(profile: Option<&str>) -> PathBuf {
    crate::paths::state(profile).with_file_name(match profile {
        Some(profile) => format!("update-checked-{profile}"),
        None => "update-checked".to_owned(),
    })
}

/// How often the app checks by itself.
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// How often an open app asks whether the daily check is due: it may stay
/// open for days, and sleep through some of them.
pub(crate) const POLL: Duration = Duration::from_secs(60 * 60);

/// Whether this run checks by itself: an installed app (a bundle, or a
/// Linux prefix), or an app on the library built for release. A run from
/// source checks only when asked, and Developer Mode changes neither.
pub(crate) fn checks_by_itself() -> bool {
    crate::paths::in_bundle() || (crate::identity().hosted && !cfg!(debug_assertions))
}

/// Whether the automatic check is due: a day after the last check that
/// reached GitHub.
pub(crate) fn due(profile: Option<&str>) -> bool {
    !fs::metadata(stamp(profile))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < EVERY)
}

/// Notes that a check reached GitHub, so the next is a day away.
pub(crate) fn checked(profile: Option<&str>) {
    let file = stamp(profile);
    let written = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, b""));
    if let Err(error) = written {
        tracing::warn!(
            "cannot note the update check in {}: {error}",
            file.display()
        );
    }
}

/// The new app, copied beside the running one, ready to take its place.
#[derive(Debug)]
pub(crate) struct Staged {
    new: PathBuf,
    target: PathBuf,
    /// Held until the swap is done: the helper that makes it removes it.
    lock: Lock,
}

/// One update of an app at a time, whichever copy of it asks: a folder
/// beside the app, made atomically, which also holds the staged app (so
/// no two updates share a path). Removed when dropped, unless handed to
/// the helper that swaps the app in.
#[derive(Debug)]
struct Lock(Option<PathBuf>);

/// A lock older than this was left by an update that died half-way.
const STALE: Duration = Duration::from_secs(60 * 60);

impl Lock {
    fn take(target: &Path) -> Result<Self, String> {
        let parent = target.parent().ok_or("the app has no folder")?;
        let path = parent.join(format!(
            ".{}.updating",
            target.file_name().unwrap_or_default().to_string_lossy()
        ));
        for _ in 0..2 {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(Some(path))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age > STALE);
                    if !stale {
                        return Err("another copy of the app is installing an update".to_owned());
                    }
                    let _ = fs::remove_dir_all(&path);
                },
                Err(error) => {
                    return Err(format!(
                        "cannot write beside {} ({error}); download it from the release page",
                        target.display()
                    ));
                },
            }
        }
        Err(format!("cannot lock {}", path.display()))
    }

    fn path(&self) -> &Path {
        self.0.as_deref().unwrap_or(Path::new(""))
    }

    /// Hands the lock to whoever removes it later.
    fn release_to_helper(mut self) -> PathBuf {
        self.0.take().unwrap_or_default()
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

/// Downloads `release`'s build, checks it, and copies the `.app` inside
/// beside `source`'s bundle. Blocks for as long as the download takes.
pub(crate) fn stage(release: &Release, source: &Source) -> Result<Staged, String> {
    let target = source
        .bundle
        .clone()
        .ok_or("the app does not run from a bundle")?;
    let asset = release
        .asset
        .as_ref()
        .ok_or("the release has no download for this Mac")?;
    // Only a plain file name goes into a path.
    if asset.name.contains(['/', '\\']) || asset.name.starts_with('.') {
        return Err(format!("unexpected download name {:?}", asset.name));
    }
    let lock = Lock::take(&target)?;
    let work = tempfile::Builder::new()
        .prefix("update-")
        .tempdir_in(ensure(crate::paths::cache().join("updates"))?)
        .map_err(|error| format!("cannot make a download folder: {error}"))?;
    let file = work.path().join(&asset.name);
    tracing::info!("downloading {} to {}", asset.url, file.display());
    download(&asset.url, &file)?;
    if let Some(sums) = &release.sums {
        verify(sums, &asset.name, &file)?;
    } else {
        tracing::warn!("{} has no SHA256SUMS: not checked", release.page);
    }
    let unpacked = work.path().join("unpacked");
    let app = if asset.name.ends_with(".dmg") {
        let mount = work.path().join("mount");
        run(Command::new("hdiutil")
            .args([
                "attach",
                "-nobrowse",
                "-readonly",
                "-noautoopen",
                "-mountpoint",
            ])
            .arg(&mount)
            .arg(&file))?;
        // Copied out of the image, which is detached whatever happens.
        let copied = find_app(&mount, &target).and_then(|app| {
            let to = unpacked.join(app.file_name().unwrap_or_default());
            run(Command::new("ditto").arg(&app).arg(&to)).map(|()| to)
        });
        let _ = run(Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&mount));
        copied?
    } else {
        run(Command::new("ditto")
            .args(["-x", "-k"])
            .arg(&file)
            .arg(&unpacked))?;
        find_app(&unpacked, &target)?
    };
    // The same app, by its id, and intact.
    let want = bundle_id(&target)?;
    let got = bundle_id(&app)?;
    if want != got {
        return Err(format!("the download is {got}, not {want}"));
    }
    // Intact, and from whoever signed the running app: a Developer ID
    // app takes only an update its own team signed (never one signed ad
    // hoc). An app signed ad hoc names no signer to hold the update to;
    // the checksum and bundle id are all it has.
    let mut signature = Command::new("codesign");
    signature.args(["--verify", "--deep", "--strict"]);
    if let Some(team) = team(&target) {
        signature.arg(format!(
            "-R=anchor apple generic and certificate leaf[subject.OU] = \"{team}\""
        ));
    } else {
        tracing::warn!(
            "{} has no Developer ID: the update is not held to its signer",
            target.display()
        );
    }
    run(signature.arg(&app))
        .map_err(|problem| format!("the download's signature is refused: {problem}"))?;
    // Beside the old one (in the lock), on the same volume, so it moves
    // into place.
    let new = lock.path().join("new.app");
    run(Command::new("ditto").arg(&app).arg(&new)).map_err(|problem| {
        format!(
            "cannot write beside {} ({problem}); download it from the release page",
            target.display()
        )
    })?;
    Ok(Staged { new, target, lock })
}

/// The helper that swaps the app in once this process exits. The old app
/// is kept until the new one opens; if the new one cannot (a newer macOS
/// than this one, say), the old one goes back and opens instead. Either
/// opens with this run's arguments (`--user`, `--profile`), and the lock
/// goes last. Paths and arguments come in as its arguments, never into its
/// text.
const HELPER: &str = r#"
pid=$1 new=$2 target=$3 lock=$4
shift 4
launch() {
    if [ $# -gt 0 ]; then open "$target" --args "$@"; else open "$target"; fi
}
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
old="$lock/old.app"
if mv "$target" "$old"; then
    if mv "$new" "$target" && launch "$@"; then
        rm -rf "$old"
    else
        rm -rf "$target"
        mv "$old" "$target"
        launch "$@"
    fi
else
    launch "$@"
fi
rm -rf "$lock"
"#;

/// Swaps the staged app in once this process exits, then opens it. The
/// caller quits the app right after.
pub(crate) fn relaunch(staged: Staged) -> Result<(), String> {
    let Staged { new, target, lock } = staged;
    let spawned = helper(
        std::process::id(),
        &new,
        &target,
        lock.path(),
        &launch_args(),
    )
    .spawn()
    .map_err(|error| format!("cannot start the installer: {error}"))?;
    drop(spawned);
    // The helper removes it once the app is in place.
    lock.release_to_helper();
    Ok(())
}

fn helper(
    pid: u32,
    new: &Path,
    target: &Path,
    lock: &Path,
    args: &[std::ffi::OsString],
) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(HELPER)
        .arg("moonframe-update")
        .arg(pid.to_string())
        .arg(new)
        .arg(target)
        .arg(lock)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}

/// This run's arguments, for the new app to open with; see
/// [`relaunch_args`].
fn launch_args() -> Vec<std::ffi::OsString> {
    relaunch_args(
        std::env::args_os().skip(1),
        std::env::current_dir().ok().as_deref(),
    )
}

/// `args` as the relaunched app takes them: but for the process serial
/// number old macOS adds to a Finder launch, and with the paths among them
/// (`--user DIR`, the app folder) made absolute, as `open` starts the app
/// in `/`. Names (`--profile NAME`, `--component NAME`) and flags stay as
/// they are, whatever files the folder holds.
fn relaunch_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
    cwd: Option<&Path>,
) -> Vec<std::ffi::OsString> {
    /// What the next argument is, after a flag that takes one.
    enum Next {
        Path,
        Name,
    }
    let absolute = |arg: std::ffi::OsString| match cwd {
        Some(cwd) if Path::new(&arg).is_relative() => cwd.join(arg).into_os_string(),
        _ => arg,
    };
    let mut next = None;
    let mut out = Vec::new();
    for arg in args {
        let text = arg.to_string_lossy().into_owned();
        match next.take() {
            Some(Next::Path) => out.push(absolute(arg)),
            Some(Next::Name) => out.push(arg),
            None if text.starts_with("-psn_") => {},
            None => {
                match text.as_str() {
                    "--user" => next = Some(Next::Path),
                    "--profile" | "--component" => next = Some(Next::Name),
                    _ => {},
                }
                // Anything else not a flag is the app folder.
                out.push(if text.starts_with('-') {
                    arg
                } else {
                    absolute(arg)
                });
            },
        }
    }
    out
}

fn ensure(dir: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&dir)
        .map(|()| dir.clone())
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let response = agent(DOWNLOAD)
        .get(url)
        .call()
        .map_err(|error| format!("cannot download the update: {error}"))?;
    let mut reader = response.into_body().into_reader();
    let mut file =
        fs::File::create(to).map_err(|error| format!("cannot write the update: {error}"))?;
    io::copy(&mut reader, &mut file)
        .map(drop)
        .map_err(|error| format!("the download failed: {error}"))
}

/// Checks `file` against its line in the release's `SHA256SUMS`.
fn verify(sums: &str, name: &str, file: &Path) -> Result<(), String> {
    let mut text = String::new();
    agent(ANSWER)
        .get(sums)
        .call()
        .map_err(|error| format!("cannot download SHA256SUMS: {error}"))?
        .into_body()
        .into_reader()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|error| format!("cannot read SHA256SUMS: {error}"))?;
    let want = expected_sum(&text, name)
        .ok_or_else(|| format!("SHA256SUMS does not list {name}"))?
        .to_ascii_lowercase();
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(file)
        .output()
        .map_err(|error| format!("cannot run shasum: {error}"))?;
    let got = String::from_utf8_lossy(&output.stdout);
    let got = got.split_whitespace().next().unwrap_or_default();
    if got != want {
        return Err(format!("{name} does not match its checksum"));
    }
    Ok(())
}

/// `name`'s checksum in a `sha256sum` listing (`<hex>  <name>`, or
/// `<hex> *<name>` for binary mode).
fn expected_sum<'a>(listing: &'a str, name: &str) -> Option<&'a str> {
    listing.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start();
        let file = file.strip_prefix('*').unwrap_or(file);
        (file == name && sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit()))
            .then_some(sum)
    })
}

/// The `.app` at the top of `folder`: the one named like `target` if there
/// are several.
fn find_app(folder: &Path, target: &Path) -> Result<PathBuf, String> {
    let apps: Vec<PathBuf> = fs::read_dir(folder)
        .map_err(|error| format!("cannot read {}: {error}", folder.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "app")
                && !fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink())
        })
        .collect();
    apps.iter()
        .find(|app| app.file_name() == target.file_name())
        .or_else(|| apps.first().filter(|_| apps.len() == 1))
        .cloned()
        .ok_or_else(|| "the download holds no app".to_owned())
}

/// The team that signed `app` with a Developer ID, if one did.
fn team(app: &Path) -> Option<String> {
    let output = Command::new("codesign")
        .args(["-d", "--verbose=2"])
        .arg(app)
        .output()
        .ok()?;
    // codesign describes the signature on stderr.
    team_in(&String::from_utf8_lossy(&output.stderr)).map(str::to_owned)
}

/// The `TeamIdentifier` in `codesign -dv`'s description: ten letters and
/// digits, never "not set".
fn team_in(description: &str) -> Option<&str> {
    let team = description
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))?
        .trim();
    (team.len() == 10 && team.chars().all(|c| c.is_ascii_alphanumeric())).then_some(team)
}

fn bundle_id(app: &Path) -> Result<String, String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .map_err(|error| format!("cannot read {}: {error}", app.display()))?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || id.is_empty() {
        return Err(format!("{} has no bundle id", app.display()));
    }
    Ok(id)
}

fn run(command: &mut Command) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run {command:?}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .unwrap_or("it failed")
        .trim()
        .to_owned())
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
                "html_url": "https://github.com/penso/moonframe/releases/tag/v0.3.0",
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
        assert_eq!(found(release, "0.1.0"), Found::UpToDate {
            latest: "0.1.0".to_owned()
        });
    }

    #[test]
    fn reads_checksums() {
        let sum = "a".repeat(64);
        let listing = format!("{sum}  App.dmg\n{} *App.zip\n", "b".repeat(64));
        assert_eq!(expected_sum(&listing, "App.dmg"), Some(sum.as_str()));
        assert_eq!(
            expected_sum(&listing, "App.zip"),
            Some("b".repeat(64).as_str())
        );
        assert_eq!(expected_sum(&listing, "App"), None);
        assert_eq!(expected_sum("short  App.dmg", "App.dmg"), None);
    }

    #[test]
    fn reads_the_signing_team() {
        let signed =
            "Executable=/A.app/Contents/MacOS/a\nIdentifier=so.pen.a\nTeamIdentifier=VJAHQVZ96V\n";
        assert_eq!(team_in(signed), Some("VJAHQVZ96V"));
        assert_eq!(team_in("Signature=adhoc\nTeamIdentifier=not set\n"), None);
        assert_eq!(team_in("TeamIdentifier=AB\"CD\" or 1\n"), None);
        assert_eq!(team_in("code object is not signed at all\n"), None);
    }

    #[test]
    fn the_relaunch_keeps_names_and_makes_paths_absolute() {
        let cwd = Path::new("/home/me/work");
        let args = |list: &[&str]| -> Vec<std::ffi::OsString> {
            list.iter().map(std::ffi::OsString::from).collect()
        };
        // A profile named like a folder beside it stays a name.
        assert_eq!(
            relaunch_args(
                args(&[
                    "--profile",
                    "notes",
                    "--user",
                    "conf",
                    "--component",
                    "app",
                    "my-app"
                ]),
                Some(cwd)
            ),
            // Joined as the platform joins them (`\` on Windows).
            vec![
                "--profile".into(),
                "notes".into(),
                "--user".into(),
                cwd.join("conf").into_os_string(),
                "--component".into(),
                "app".into(),
                cwd.join("my-app").into_os_string(),
            ]
        );
        assert_eq!(
            relaunch_args(
                args(&["-psn_0_1234", "--background", "--user", "/abs"]),
                Some(cwd)
            ),
            args(&["--background", "--user", "/abs"])
        );
    }

    #[test]
    fn one_update_at_a_time() {
        let dir = tempfile::tempdir().expect("a folder");
        let target = dir.path().join("App.app");
        let lock = Lock::take(&target).expect("the first takes it");
        assert!(lock.path().ends_with(".App.app.updating"));
        let problem = Lock::take(&target).expect_err("the second waits");
        assert!(problem.contains("another copy"), "{problem}");
        drop(lock);
        let lock = Lock::take(&target).expect("free again once dropped");
        // Handed to the helper, it stays until the helper removes it.
        let path = lock.release_to_helper();
        assert!(path.is_dir());
    }

    /// Runs the helper as the app would, with `open` replaced by a stub
    /// that notes its arguments and fails while `fail` exists.
    #[cfg(unix)]
    fn swap(dir: &Path, fail_open: bool) -> (String, String) {
        use std::os::unix::fs::PermissionsExt as _;
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let opened = dir.join("opened");
        let fail = dir.join("fail");
        let stub = bin.join("open");
        fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s|' \"$@\" >> '{}'\necho >> '{}'\n[ ! -e '{}' ]\n",
                opened.display(),
                opened.display(),
                fail.display()
            ),
        )
        .expect("stub");
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod");
        if fail_open {
            fs::write(&fail, "").expect("fail");
        }
        let target = dir.join("App.app");
        fs::create_dir_all(&target).expect("old");
        fs::write(target.join("version"), "old").expect("old");
        let lock = Lock::take(&target).expect("lock");
        let new = lock.path().join("new.app");
        fs::create_dir_all(&new).expect("new");
        fs::write(new.join("version"), "new").expect("new");
        // A process that has already exited stands for the app.
        let mut gone = Command::new("/usr/bin/true").spawn().expect("true");
        let pid = gone.id();
        gone.wait().expect("exited");
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let args = [
            "--profile".into(),
            "notes".into(),
            "--user".into(),
            "/a b".into(),
        ];
        let status = helper(pid, &new, &target, lock.path(), &args)
            .env("PATH", path)
            .status()
            .expect("the helper runs");
        assert!(status.success());
        assert!(
            !lock.release_to_helper().exists(),
            "the helper removes the lock"
        );
        (
            fs::read_to_string(target.join("version")).expect("an app is in place"),
            fs::read_to_string(&opened).unwrap_or_default(),
        )
    }

    #[cfg(unix)]
    #[test]
    fn the_helper_swaps_and_opens_with_the_same_arguments() {
        let dir = tempfile::tempdir().expect("a folder");
        let (version, opened) = swap(dir.path(), false);
        assert_eq!(version, "new");
        let target = dir.path().join("App.app");
        assert_eq!(
            opened,
            format!("{}|--args|--profile|notes|--user|/a b|\n", target.display())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_new_app_that_cannot_open_gives_the_old_one_back() {
        let dir = tempfile::tempdir().expect("a folder");
        let (version, opened) = swap(dir.path(), true);
        assert_eq!(version, "old");
        // Tried the new one, then opened the old one again.
        assert_eq!(opened.lines().count(), 2, "{opened}");
    }

    #[test]
    fn repositories() {
        assert!(valid_repo("penso/moonframe"));
        assert!(valid_repo("my-org/my.app_2"));
        assert!(!valid_repo("penso"));
        assert!(!valid_repo("penso/moonframe/extra"));
        assert!(!valid_repo("../x"));
        assert!(!valid_repo("https://github.com/penso/moonframe"));
    }
}
