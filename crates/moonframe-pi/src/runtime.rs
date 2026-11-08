//! Finding pi, and the environment it should run in. A window opened from
//! the Dock has a bare environment, so PATH and provider keys come from the
//! user's login shell: what pi's tools run needs them as much as pi does.

use {
    crate::{Config, run::run},
    moonframe_providers::program::{default_search_path, search_path},
    std::{
        collections::HashSet,
        env,
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    },
};

const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(6);
const VERSION_TIMEOUT: Duration = Duration::from_secs(15);

/// What every program started here runs with, beyond this process's own
/// environment, which a process of many threads must not change.
#[derive(Clone, Debug, Default)]
pub(crate) struct Runtime {
    path: OsString,
    vars: Vec<(String, String)>,
}

/// Whether a login shell's variable is a credential pi may need.
fn credential(name: &str) -> bool {
    const ENDINGS: [&str; 5] = [
        "_API_KEY",
        "_AUTH_TOKEN",
        "_OAUTH_TOKEN",
        "_GITHUB_TOKEN",
        "_BASE_URL",
    ];
    name == "HF_TOKEN"
        || name.starts_with("AWS_")
        || name.starts_with("GOOGLE_")
        || ENDINGS.iter().any(|ending| name.ends_with(ending))
}

impl Runtime {
    /// This process's PATH and the usual install places, or the PATH
    /// `config` sets.
    pub(crate) fn new(config: &Config) -> Self {
        let set = config.env.iter().find(|(name, _)| name == "PATH");
        Self {
            path: set.map_or_else(default_search_path, |(_, path)| path.into()),
            vars: config.env.clone(),
        }
    }

    /// Adds what the user's login shell has. Its PATH goes after the one
    /// this process has, and both before the places searched by default.
    pub(crate) fn adopt_login(&mut self, config: &Config) {
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
        let mut command = Command::new(shell);
        command.args(["-ilc", "env -0"]);
        let ran = run(command, None, LOGIN_SHELL_TIMEOUT);
        if !ran.ok {
            return;
        }
        let text = String::from_utf8_lossy(&ran.stdout);
        let pairs = text.split('\0').filter_map(|line| line.split_once('='));
        let mut login_path = None;
        for (name, value) in pairs.filter(|(name, _)| !name.is_empty()) {
            if name == "PATH" {
                login_path = Some(value.to_owned());
            } else if credential(name) && self.var(name).is_none() {
                self.vars.push((name.to_owned(), value.to_owned()));
            }
        }
        let own = env::var_os("PATH").unwrap_or_default();
        let dirs: Vec<PathBuf> = env::split_paths(&own)
            .chain(login_path.iter().flat_map(env::split_paths))
            .collect();
        let joined = env::join_paths(dirs).unwrap_or(own);
        self.path = search_path(Some(&joined), Some(&config.home));
    }

    /// A variable, as what runs here sees it.
    pub(crate) fn var(&self, name: &str) -> Option<String> {
        let set = self.vars.iter().find(|(key, _)| key == name);
        set.map(|(_, value)| value.clone())
            .or_else(|| env::var(name).ok())
    }

    /// A command for `program` in this environment.
    pub(crate) fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command.env("PATH", &self.path);
        command.envs(self.vars.iter().map(|(name, value)| (name, value)));
        command
    }

    fn path_dirs(&self) -> Vec<PathBuf> {
        env::split_paths(&self.path)
            .filter(|dir| !dir.as_os_str().is_empty())
            .collect()
    }
}

fn is_shim(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str() == "shims")
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    path.is_file()
}

/// Where pi might be, best first. A version manager's shim comes last:
/// mise's picks a version by the folder it runs in, so it would fail in any
/// project whose mise.toml it refuses, which is exactly where pi has to run.
fn candidates(config: &Config, runtime: &Runtime) -> Vec<PathBuf> {
    let home = &config.home;
    let on_path = runtime.path_dirs().into_iter().map(|dir| dir.join("pi"));
    let (shims, plain): (Vec<PathBuf>, Vec<PathBuf>) = on_path.partition(|path| is_shim(path));
    let mise_data = runtime
        .var("MISE_DATA_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home.join(".local/share/mise"), PathBuf::from);
    let mut list: Vec<PathBuf> = config.pi.iter().cloned().collect();
    list.extend(plain);
    list.push(mise_data.join("installs/pi/latest/pi/pi"));
    list.push("/opt/homebrew/bin/pi".into());
    list.push("/usr/local/bin/pi".into());
    list.push(home.join(".npm-global/bin/pi"));
    list.push(home.join(".bun/bin/pi"));
    list.extend(shims);
    list.push(home.join(".local/share/mise/shims/pi"));
    let mut seen = HashSet::new();
    list.retain(|path| executable(path) && seen.insert(path.clone()));
    list
}

/// A pi that answers.
#[derive(Clone, Debug)]
pub(crate) struct Found {
    pub(crate) path: PathBuf,
    pub(crate) version: String,
}

fn version_of(path: &Path, config: &Config, runtime: &Runtime) -> Result<String, String> {
    let mut command = runtime.command(path);
    command.arg("--version").current_dir(&config.home);
    let text = run(command, None, VERSION_TIMEOUT).output()?;
    let last = text.trim().lines().last().unwrap_or_default();
    Ok(last.to_owned())
}

/// The first pi that answers, or what to tell the user.
pub(crate) fn find_pi(config: &Config, runtime: &Runtime) -> Result<Found, String> {
    let mut refused = None;
    for path in candidates(config, runtime) {
        match version_of(&path, config, runtime) {
            Ok(version) => return Ok(Found { path, version }),
            Err(problem) => {
                tracing::warn!("pilot: {} --version: {problem}", path.display());
                refused.get_or_insert((path, problem));
            },
        }
    }
    Err(match refused {
        Some((path, problem)) => {
            let line = problem.lines().last().unwrap_or_default();
            format!("pi at {} did not run: {line}", path.display())
        },
        None => "pi was not found. Install pi, or set PILOT_PI to its path, then reopen Pilot."
            .to_owned(),
    })
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::fs};

    fn script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn config(home: &Path, pi: Option<PathBuf>) -> Config {
        Config {
            agent_dir: home.join("agent"),
            data_dir: home.join("data"),
            home: home.to_owned(),
            pi,
            login_shell: false,
            env: Vec::new(),
        }
    }

    #[test]
    fn a_shim_is_never_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("mise/shims/pi");
        let real = dir.path().join("bin/pi");
        script(&shim, "echo 1");
        script(&real, "echo 1");
        let config = config(dir.path(), None);
        let runtime = Runtime {
            path: env::join_paths([shim.parent().unwrap(), real.parent().unwrap()]).unwrap(),
            vars: Vec::new(),
        };
        let list = candidates(&config, &runtime);
        let at = |path: &Path| list.iter().position(|item| item == path).unwrap();
        assert!(at(&real) < at(&shim), "{list:?}");
    }

    #[test]
    fn pilot_pi_comes_first() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("chosen/pi");
        let other = dir.path().join("bin/pi");
        script(&chosen, "echo 1.2.3");
        script(&other, "echo 9.9.9");
        let config = config(dir.path(), Some(chosen.clone()));
        let runtime = Runtime {
            path: other.parent().unwrap().into(),
            vars: Vec::new(),
        };
        assert_eq!(candidates(&config, &runtime)[0], chosen);
        assert_eq!(find_pi(&config, &runtime).unwrap().version, "1.2.3");
    }

    #[test]
    fn a_pi_that_will_not_run_is_passed_over_and_named() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("broken/pi");
        script(&broken, "echo no such library >&2; exit 1");
        let config = config(dir.path(), Some(broken.clone()));
        // Whatever else this machine has, the broken one came first.
        if let Err(reason) = find_pi(&config, &Runtime::default()) {
            assert!(reason.contains("no such library"), "{reason}");
        }
    }

    #[test]
    fn credentials_are_recognised() {
        assert!(credential("OPENAI_API_KEY"));
        assert!(credential("HF_TOKEN"));
        assert!(credential("AWS_PROFILE"));
        assert!(!credential("EDITOR"));
    }
}
