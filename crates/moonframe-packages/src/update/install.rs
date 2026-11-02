//! Installing a release on macOS: downloading and checking its build,
//! staging the app beside the running one, and swapping it in on quit.

use {
    super::{
        Release, Source,
        release::{ANSWER, DOWNLOAD, agent},
    },
    std::{
        fs,
        io::{self, Read as _},
        path::{Path, PathBuf},
        process::Command,
        time::{Duration, SystemTime},
    },
};

/// The new app, copied beside the running one, ready to take its place.
#[derive(Debug)]
pub struct Staged {
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
pub fn stage(release: &Release, source: &Source) -> Result<Staged, String> {
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
pub fn relaunch(staged: Staged) -> Result<(), String> {
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
            // Joined as the platform joins them: `\` on Windows.
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
}
