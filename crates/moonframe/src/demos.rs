//! The `demos` native provider: the app packages beside the app it is in
//! (for the demo gallery in `examples/demo`, every other example app), each
//! with a picture, and each opened as a window of its own on request. An
//! app opens as a profile linked from its folder, in the user's folder this
//! run uses, and runs on this same binary, so any number can be open at
//! once and edits to their folders reload them live.
//!
//! Publishes `{ available, apps = { { name, title, description, folder,
//! picture?, open } }, user }`. `picture` is the app's screenshot
//! (`[settings] pictures`, by default `docs/screenshots/apps/<name>.png` in
//! the repository) or else its icon, copied into the image cache where
//! views may show it.
//!
//! Messages, only from the mods in `[settings] openers` (as shipped,
//! `gallery`): `{ open = "<name>" }` opens the app, or brings it forward
//! when it is open; `{ open_all = true }` opens every app not open yet;
//! `{ close = "<name>" }` and `{ close_all = true }` quit them.

use {
    crate::{packages, paths},
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    serde_json::{Value, json},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        time::UNIX_EPOCH,
    },
};

pub(crate) fn native() -> Box<dyn Native> {
    Box::new(Demos::default())
}

#[derive(Default)]
struct Demos {
    /// The app this provider is in, whose siblings it lists.
    app: PathBuf,
    /// Where the apps are: the folder holding `app`.
    folder: PathBuf,
    /// Screenshots, `<name>.png`.
    pictures: Option<PathBuf>,
    /// The mods that may open and close apps.
    openers: Vec<String>,
    /// The user's folder this run uses; profiles go in it.
    user: Option<PathBuf>,
    /// The apps this provider opened, by name, while they run.
    open: BTreeMap<String, Child>,
    last: Option<Value>,
}

/// An app package found beside the gallery.
struct App {
    name: String,
    title: String,
    description: String,
    folder: PathBuf,
    icon: PathBuf,
}

impl Native for Demos {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = to_json(host.settings());
        let app = host.dir().join("..").join("..");
        self.app = app.canonicalize().unwrap_or(app);
        // It links and opens apps without asking about their providers, so
        // only the apps of the checkout the gallery itself is in: the
        // folders of one repository, which the user linked to develop.
        let repo = self
            .app
            .ancestors()
            .find(|folder| folder.join(".git").exists())
            .map(Path::to_path_buf)
            .ok_or("the demo gallery opens the apps of the repository it is in, and is in none")?;
        let inside = |key: &str, default: PathBuf| -> Result<PathBuf, String> {
            let Some(path) = settings.get(key).and_then(Value::as_str) else {
                return Ok(default);
            };
            let path = self.app.join(path);
            let path = path.canonicalize().unwrap_or(path);
            if path.starts_with(&repo) {
                Ok(path)
            } else {
                Err(format!("`{key}` must be inside {}", repo.display()))
            }
        };
        self.folder = inside("apps", self.app.join(".."))?;
        self.folder = self.folder.canonicalize().unwrap_or_else(|_| self.folder.clone());
        self.pictures = Some(inside("pictures", repo.join("docs/screenshots/apps"))?);
        self.openers = settings
            .get("openers")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|| vec!["gallery".to_owned()]);
        self.user = paths::user_folder();
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.open
            .retain(|_, child| matches!(child.try_wait(), Ok(None)));
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        if !self.openers.iter().any(|opener| opener == from) {
            host.fail(format!(
                "`{from}` may not open apps: only the mods in providers/demos' `openers` may"
            ));
            return;
        }
        let message = to_json(&message);
        let apps = self.apps();
        let named = |key: &str| {
            let name = message.get(key).and_then(Value::as_str)?;
            apps.iter().find(|app| app.name == name)
        };
        if let Some(app) = named("open") {
            self.open_app(host, app);
        } else if message
            .get("open_all")
            .is_some_and(|all| all == &json!(true))
        {
            for app in &apps {
                if !self.open.contains_key(&app.name) {
                    self.open_app(host, app);
                }
            }
        } else if let Some(name) = message.get("close").and_then(Value::as_str) {
            if let Some(mut child) = self.open.remove(name) {
                let _ = child.kill();
                let _ = child.wait();
            }
        } else if message
            .get("close_all")
            .is_some_and(|all| all == &json!(true))
        {
            for (_, mut child) in std::mem::take(&mut self.open) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.publish(host);
    }
}

impl Demos {
    /// The app packages beside the gallery, by folder name.
    fn apps(&self) -> Vec<App> {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return Vec::new();
        };
        let mut folders: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        folders.sort();
        folders
            .into_iter()
            .filter(|folder| folder.canonicalize().ok().as_ref() != Some(&self.app))
            .filter_map(|folder| {
                let manifest = packages::App::read(&folder).ok()??;
                let text = fs::read_to_string(folder.join("moonframe.toml")).unwrap_or_default();
                Some(App {
                    title: manifest
                        .title
                        .clone()
                        .unwrap_or_else(|| manifest.name.clone()),
                    description: about(&text),
                    icon: folder.join(manifest.icon.as_deref().unwrap_or("icon.png")),
                    name: manifest.name,
                    folder,
                })
            })
            .collect()
    }

    fn publish(&mut self, host: &Handle) {
        let apps: Vec<Value> = self
            .apps()
            .iter()
            .map(|app| {
                json!({
                    "name": app.name,
                    "title": app.title,
                    "description": app.description,
                    "folder": app.folder,
                    "picture": self.picture(app),
                    "open": self.open.contains_key(&app.name),
                })
            })
            .collect();
        let value = match &self.user {
            Some(user) => json!({ "available": true, "apps": apps, "user": user }),
            None => json!({
                "available": false,
                "reason": "Apps open as profiles, which need the user's folder: run without --no-user.",
            }),
        };
        if self.last.as_ref() != Some(&value) {
            host.publish_json(&value);
            self.last = Some(value);
        }
    }

    /// A copy of the app's screenshot, else its icon, in the image cache,
    /// named by its time so a new screenshot shows at once.
    fn picture(&self, app: &App) -> Option<PathBuf> {
        let shot = self
            .pictures
            .as_ref()
            .map(|folder| folder.join(format!("{}.png", app.name)))
            .filter(|shot| shot.is_file());
        let source = shot.unwrap_or_else(|| app.icon.clone());
        let time = fs::metadata(&source)
            .and_then(|meta| meta.modified())
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs();
        let folder = paths::images().join("demos");
        let copy = folder.join(format!("{}-{time}.png", app.name));
        if !copy.is_file() {
            fs::create_dir_all(&folder).ok()?;
            for old in fs::read_dir(&folder).ok()?.flatten() {
                let name = old.file_name();
                if name
                    .to_string_lossy()
                    .starts_with(&format!("{}-", app.name))
                {
                    let _ = fs::remove_file(old.path());
                }
            }
            fs::copy(&source, &copy).ok()?;
        }
        Some(copy)
    }

    /// Opens `app` as its own window, or brings it forward when it is open.
    fn open_app(&mut self, host: &Handle, app: &App) {
        if let Some(child) = self.open.get(&app.name) {
            bring_forward(child.id());
            return;
        }
        let Some(user) = self.user.clone() else {
            return;
        };
        let result = link(&user, app).and_then(|()| {
            let log = paths::cache().join("demos");
            fs::create_dir_all(&log).map_err(|error| error.to_string())?;
            let log = fs::File::create(log.join(format!("{}.log", app.name)))
                .map_err(|error| error.to_string())?;
            let errors = log.try_clone().map_err(|error| error.to_string())?;
            Command::new(binary()?)
                .arg("--user")
                .arg(&user)
                .args(["--profile", &app.name])
                .stdin(Stdio::null())
                .stdout(log)
                .stderr(errors)
                .spawn()
                .map_err(|error| format!("cannot open {}: {error}", app.title))
        });
        match result {
            Ok(child) => {
                self.open.insert(app.name.clone(), child);
            },
            Err(problem) => host.fail(problem),
        }
    }
}

/// This binary, which runs every app.
fn binary() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| format!("cannot find the moonframe binary: {error}"))
}

/// Links `app`'s folder as its profile in `user`, unless it is linked from
/// there already; a profile of that name from elsewhere (another worktree)
/// is replaced.
fn link(user: &Path, app: &App) -> Result<(), String> {
    let profile = paths::profile(user, &app.name);
    let packages = fs::read_to_string(profile.join("packages.toml")).unwrap_or_default();
    let folder = app
        .folder
        .canonicalize()
        .unwrap_or_else(|_| app.folder.clone());
    if packages.contains(&format!("url = {:?}", folder.display().to_string())) {
        return Ok(());
    }
    let run = |args: &[&std::ffi::OsStr]| -> Result<(), String> {
        let output = Command::new(binary()?)
            .arg("--user")
            .arg(user)
            .arg("app")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
    };
    if profile.exists() {
        run(&["remove".as_ref(), app.name.as_ref(), "--force".as_ref()])?;
    }
    run(&["link".as_ref(), folder.as_os_str(), "--yes".as_ref()])
}

/// The comment under `[app]` in a moonframe.toml: what the app is.
fn about(manifest: &str) -> String {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[app]")
        .skip(1)
        .take_while(|line| line.trim_start().starts_with('#'))
        .map(|line| line.trim_start().trim_start_matches('#').trim())
        .collect::<Vec<_>>()
        .join(" ")
        // How to open it is the gallery's to say.
        .split_inclusive(". ")
        .filter(|sentence| !sentence.contains("--profile"))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[cfg(target_os = "macos")]
fn bring_forward(pid: u32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    let Ok(pid) = i32::try_from(pid) else {
        return;
    };
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    }
}

#[cfg(not(target_os = "macos"))]
fn bring_forward(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_comment_under_app_is_what_it_is() {
        let manifest = "[app]\n# A fund overview:\n# sample data.\nkeep = []\nname = \"ledger\"\n";
        assert_eq!(about(manifest), "A fund overview: sample data.");
        assert_eq!(about("[app]\nname = \"x\"\n"), "");
        let opens =
            "[app]\n# A planner. `moonframe --profile daybook` opens it.\nname = \"daybook\"\n";
        assert_eq!(about(opens), "A planner.");
    }
}
