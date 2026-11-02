//! The provider's own thread: where the sign-in stands, when to read the
//! forge again, what mods asked, and what goes out as `app.<forge>`.

use {
    crate::{
        Target,
        credentials::{Credential, Saved, Source},
        http::Failure,
        kind::Kind,
        repo::{self, Checkout},
        work::{Asker, Context, Done},
    },
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native},
    secrecy::SecretString,
    serde_json::{Value, json},
    std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{Receiver, channel},
        },
        time::{Duration, Instant},
    },
};

/// How often git is read for a new branch or worktree, in ticks.
const LOOK_EVERY: u32 = 5;
/// The least time between two reads a message asks for.
const MIN_ASKED: Duration = Duration::from_secs(5);
/// How long to wait after the forge limited requests, or could not be
/// reached.
const RATE_LIMITED: Duration = Duration::from_secs(300);
const UNREACHABLE: Duration = Duration::from_secs(60);

/// Where the sign-in stands, as `auth.status` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Looking for a saved sign-in.
    Checking,
    SignedOut,
    /// A device code is up; waiting on the user.
    Pending,
    /// Checking a token or the CLI's sign-in.
    Verifying,
    SignedIn,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::SignedOut => "signed_out",
            Self::Pending => "pending",
            Self::Verifying => "verifying",
            Self::SignedIn => "signed_in",
        }
    }
}

/// The device code shown while [`Status::Pending`].
struct Shown {
    user: String,
    url: String,
    complete_url: Option<String>,
    expires: u64,
}

pub(crate) struct Forge {
    kind: Kind,
    context: Option<Context>,
    done: Option<Receiver<Done>>,
    root: Option<PathBuf>,
    accounts: Mods,
    settings: Value,
    /// Every this long, the forge is read again.
    every: Duration,
    checkout: Checkout,
    ticks: u32,
    /// The host signed in to: the remote's, else the forge's own.
    host: String,
    loaded: Option<String>,
    status: Status,
    credential: Option<Credential>,
    login: Option<String>,
    cli: bool,
    error: Option<String>,
    code: Option<Shown>,
    cancel: Option<Arc<AtomicBool>>,
    /// The action waiting on a worker, to answer.
    waiting: Option<Asker>,
    /// Identifies the current sign-in attempt and credential generation.
    attempt: u64,
    epoch: u64,
    fetching: bool,
    fetched_at: Option<Instant>,
    asked_at: Option<Instant>,
    next: Option<Instant>,
    /// No read before this, whatever asks: the forge limited requests or
    /// could not be reached.
    blocked_until: Option<Instant>,
    data: Value,
    published: Value,
}

impl Forge {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            context: None,
            done: None,
            root: None,
            accounts: Mods::default(),
            settings: Value::Null,
            every: Duration::from_secs(60),
            checkout: Checkout::default(),
            ticks: 0,
            host: kind.default_host().to_owned(),
            loaded: None,
            status: Status::Checking,
            credential: None,
            login: None,
            cli: false,
            error: None,
            code: None,
            cancel: None,
            waiting: None,
            attempt: 0,
            epoch: 0,
            fetching: false,
            fetched_at: None,
            asked_at: None,
            next: None,
            blocked_until: None,
            data: Value::Null,
            published: Value::Null,
        }
    }

    /// GitLab hosts the settings name besides those called `gitlab`.
    fn hosts(&self) -> Vec<String> {
        self.settings["hosts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_ascii_lowercase)
            .collect()
    }

    /// The remote, when it is on this forge.
    fn remote(&self) -> Option<&(String, String)> {
        self.checkout
            .remote
            .as_ref()
            .filter(|(host, _)| self.kind.serves(host, &self.hosts()))
    }

    /// The public client id of the OAuth app device sign-ins use on
    /// `host`: the environment's, `client_ids.<host>`, else `client_id`
    /// for the forge's own host.
    fn client(&self, host: &str) -> Option<String> {
        let default = host == self.kind.default_host();
        let from_env = default
            .then(|| std::env::var(self.kind.client_env()).ok())
            .flatten();
        let from_hosts = self.settings["client_ids"][host]
            .as_str()
            .map(str::to_owned);
        let from_setting = default
            .then(|| self.settings["client_id"].as_str().map(str::to_owned))
            .flatten();
        from_env
            .or(from_hosts)
            .or(from_setting)
            .map(|id| id.trim().to_owned())
            .filter(|id| {
                !id.is_empty() && id.len() <= 256 && id.bytes().all(|b| b.is_ascii_graphic())
            })
    }

    fn target(&self) -> Option<Target> {
        let (host, path) = self.remote()?.clone();
        Some(Target {
            host,
            path,
            branch: self.checkout.branch.clone(),
            branches: self.checkout.branches.clone(),
        })
    }

    fn look(&mut self, handle: &Handle) {
        let checkout = self.root.as_deref().map(repo::read).unwrap_or_default();
        if checkout != self.checkout {
            self.checkout = checkout;
            self.data = Value::Null;
            self.next = Some(Instant::now());
        }
        let host = self.remote().map_or_else(
            || self.kind.default_host().to_owned(),
            |(host, _)| host.clone(),
        );
        if self.loaded.as_ref() != Some(&host) {
            self.stop_sign_in(handle, "the sign-in host changed");
            self.epoch += 1;
            self.host = host.clone();
            self.loaded = Some(host.clone());
            self.status = Status::Checking;
            self.credential = None;
            self.login = None;
            self.data = Value::Null;
            if let Some(context) = &self.context {
                context.load(host);
            }
        }
    }

    fn take_in(&mut self, host: &Handle, done: Done) {
        match done {
            Done::Loaded {
                host: loaded,
                credential,
                cli,
                error,
            } => {
                if loaded != self.host {
                    return;
                }
                self.cli = cli;
                self.error = error;
                if self.status == Status::Checking {
                    self.status = if credential.is_some() {
                        Status::SignedIn
                    } else {
                        Status::SignedOut
                    };
                    self.credential = credential;
                    self.epoch += 1;
                    self.next = Some(Instant::now());
                }
            },
            Done::Code {
                attempt,
                user,
                url,
                complete_url,
                expires,
            } => {
                if attempt != self.attempt || self.waiting.is_none() {
                    return;
                }
                self.status = Status::Pending;
                self.code = Some(Shown {
                    user: user.clone(),
                    url: url.clone(),
                    complete_url,
                    expires,
                });
                if let Some(asker) = &self.waiting {
                    reply(
                        host,
                        asker,
                        true,
                        json!({ "code": user, "url": url, "pending": true }),
                    );
                }
            },
            Done::SignedIn {
                host: signed_in,
                credential,
                login,
                asker,
            } => {
                if asker.attempt != self.attempt || self.waiting.is_none() {
                    return;
                }
                self.end_device();
                self.waiting = None;
                // Saved here, not by the worker: a sign-out that came while
                // it checked the token bumped `attempt`, and this is skipped.
                let saved = self.context.as_ref().map_or(Ok(()), |context| {
                    context.saved.save(&signed_in, &credential)
                });
                if let Err(problem) = saved {
                    let error = format!("signed in as {login}, but cannot save it: {}", problem.0);
                    self.status = if self.credential.is_some() {
                        Status::SignedIn
                    } else {
                        Status::SignedOut
                    };
                    self.error = Some(error.clone());
                    reply(host, &asker, false, json!({ "error": error }));
                    return;
                }
                if signed_in == self.host {
                    self.status = Status::SignedIn;
                    self.credential = Some(credential);
                    self.epoch += 1;
                    self.login = Some(login.clone());
                    self.error = None;
                    self.next = Some(Instant::now());
                }
                reply(
                    host,
                    &asker,
                    true,
                    json!({ "login": login, "message": format!("Signed in as {login}") }),
                );
            },
            Done::Refused { error, asker } => {
                if asker.attempt != self.attempt || self.waiting.is_none() {
                    return;
                }
                self.end_device();
                self.waiting = None;
                self.status = if self.credential.is_some() {
                    Status::SignedIn
                } else {
                    Status::SignedOut
                };
                self.error = Some(error.clone());
                reply(host, &asker, false, json!({ "error": error }));
            },
            Done::Fetched {
                target,
                epoch,
                result,
                renewed,
                credential,
            } => {
                self.fetching = false;
                self.fetched_at = Some(Instant::now());
                if epoch != self.epoch || self.target().as_ref() != Some(&target) {
                    return;
                }
                if let Some(credential) = credential
                    && self.credential.is_some()
                {
                    // Only a sign-in still current is saved: a sign-out
                    // during the fetch bumped `epoch`.
                    if renewed
                        && let Some(context) = &self.context
                        && let Err(problem) = context.saved.save(&target.host, &credential)
                    {
                        tracing::warn!(
                            "{}: cannot save the renewed sign-in: {}",
                            self.kind.name(),
                            problem.0
                        );
                    }
                    self.credential = Some(credential);
                }
                match result {
                    Ok(data) => {
                        self.login = data["viewer"]["login"].as_str().map(str::to_owned);
                        self.data = data;
                        self.error = None;
                        self.blocked_until = None;
                        // A refresh requested during this fetch still needs to run.
                        self.next.get_or_insert(Instant::now() + self.every);
                    },
                    Err(Failure::Unauthorized) => {
                        self.credential = None;
                        self.login = None;
                        self.data = Value::Null;
                        self.status = Status::SignedOut;
                        self.error = Some(format!(
                            "{} refused the sign-in; sign in again",
                            self.kind.title()
                        ));
                    },
                    Err(failure) => {
                        self.error = Some(failure.to_string());
                        let wait = if failure == Failure::RateLimited {
                            RATE_LIMITED
                        } else if matches!(failure, Failure::Network(_)) {
                            UNREACHABLE
                        } else {
                            self.every
                        };
                        if matches!(failure, Failure::RateLimited | Failure::Network(_)) {
                            self.blocked_until = Some(Instant::now() + wait);
                        }
                        self.next = Some(Instant::now() + wait);
                    },
                }
            },
        }
    }

    fn end_device(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.code = None;
    }

    fn stop_sign_in(&mut self, host: &Handle, error: &str) {
        self.end_device();
        self.attempt += 1;
        if let Some(asker) = self.waiting.take() {
            reply(host, &asker, false, json!({ "error": error }));
        }
        self.status = if self.credential.is_some() {
            Status::SignedIn
        } else {
            Status::SignedOut
        };
    }

    fn expire_device(&mut self, host: &Handle, now: u64) {
        if self.code.as_ref().is_some_and(|code| now >= code.expires) {
            let error = "the code expired; start again";
            self.stop_sign_in(host, error);
            self.error = Some(error.into());
        }
    }

    /// Whether a read is asked for and allowed at `now`: a request moves
    /// `next`, never `blocked_until`.
    fn due(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| now >= next)
            && self.blocked_until.is_none_or(|until| now >= until)
    }

    fn fetch_if_due(&mut self) {
        if !self.due(Instant::now()) || self.fetching || self.status != Status::SignedIn {
            return;
        }
        let (Some(target), Some(credential), Some(context)) =
            (self.target(), self.credential.clone(), &self.context)
        else {
            return;
        };
        self.fetching = true;
        self.next = None;
        context.fetch(
            target.clone(),
            credential,
            self.client(&target.host),
            self.epoch,
        );
    }

    fn snapshot(&self) -> Value {
        let mut auth = json!({
            "status": self.status.name(),
            "host": self.host,
            "device": self.client(&self.host).is_some(),
            "cli": self.cli,
        });
        if let Some(login) = &self.login {
            auth["login"] = json!(login);
        }
        if let Some(credential) = &self.credential {
            auth["source"] = json!(credential.source.name());
        }
        if let Some(error) = &self.error {
            auth["error"] = json!(error);
        }
        if let Some(code) = &self.code {
            auth["code"] = json!(code.user);
            auth["url"] = json!(code.url);
            auth["expires"] = json!(code.expires);
            if let Some(url) = &code.complete_url {
                auth["complete_url"] = json!(url);
            }
        }
        let reason = if self.root.is_none() {
            Some("not a git repository".to_owned())
        } else if self.remote().is_none() {
            Some(format!("no {} remote", self.kind.title()))
        } else if self.status != Status::SignedIn {
            Some(format!("not signed in to {}", self.kind.title()))
        } else if self.data.is_null() {
            Some(self.error.clone().unwrap_or_else(|| "loading".to_owned()))
        } else {
            None
        };
        let mut out = match reason {
            Some(reason) => json!({ "available": false, "reason": reason }),
            None => {
                let mut data = self.data.clone();
                data["available"] = json!(true);
                data
            },
        };
        out["host"] = json!(self.host);
        out["matches_remote"] = json!(self.remote().is_some());
        out["auth"] = auth;
        out
    }

    fn publish(&mut self, host: &Handle) {
        let snapshot = self.snapshot();
        if snapshot != self.published {
            host.publish_json(&snapshot);
            self.published = snapshot;
        }
    }

    fn asker(&self, from: &str, message: &Data, action: &'static str) -> Asker {
        Asker {
            from: from.to_owned(),
            id: message.get("id").cloned().unwrap_or(Data::Nil),
            action,
            attempt: self.attempt,
        }
    }

    fn act(&mut self, host: &Handle, from: &str, message: &Data, action: &'static str) {
        let mut asker = self.asker(from, message, action);
        if !self.accounts.allows(from) {
            let words = format!(
                "`{from}` may not {}: only the mods in providers/{}'s `accounts` may",
                action.replace('_', " "),
                self.kind.name()
            );
            reply(host, &asker, false, json!({ "error": words }));
            return;
        }
        let Some(context) = self.context.clone() else {
            return;
        };
        match action {
            "cancel" => {
                self.stop_sign_in(host, "the sign-in was cancelled");
                reply(host, &asker, true, json!({ "message": "Cancelled" }));
                return;
            },
            "sign_out" => {
                self.stop_sign_in(host, "the sign-in was cancelled by signing out");
                let forgotten = match &self.credential {
                    Some(credential) if !credential.source.forgettable() => Err(format!(
                        "the token comes from the environment ({}); unset it to sign out",
                        self.kind.env().join(" or ")
                    )),
                    _ => context
                        .saved
                        .forget(&self.host)
                        .map_err(|problem| problem.0),
                };
                match forgotten {
                    Ok(()) => {
                        self.credential = None;
                        self.epoch += 1;
                        self.login = None;
                        self.data = Value::Null;
                        self.error = None;
                        self.status = Status::SignedOut;
                        reply(host, &asker, true, json!({ "message": "Signed out" }));
                    },
                    Err(error) => reply(host, &asker, false, json!({ "error": error })),
                }
                return;
            },
            _ => {},
        }
        if self.waiting.is_some() || self.status == Status::Verifying {
            // A device code already up is answered again; anything else
            // waits its turn.
            if action == "sign_in"
                && let Some(code) = &self.code
            {
                reply(
                    host,
                    &asker,
                    true,
                    json!({ "code": code.user, "url": code.url, "pending": true }),
                );
            } else {
                reply(
                    host,
                    &asker,
                    false,
                    json!({ "error": "a sign-in is already under way" }),
                );
            }
            return;
        }
        self.attempt += 1;
        asker.attempt = self.attempt;
        match action {
            "sign_in" => {
                let Some(client) = self.client(&self.host) else {
                    let words = format!(
                        "no OAuth client id for {}: set `client_id` in providers/{}/provider.toml \
                         (or {}) to an OAuth app with the device flow enabled",
                        self.host,
                        self.kind.name(),
                        self.kind.client_env()
                    );
                    reply(host, &asker, false, json!({ "error": words }));
                    return;
                };
                let cancel = Arc::new(AtomicBool::new(false));
                self.cancel = Some(Arc::clone(&cancel));
                self.status = Status::Pending;
                context.device(self.host.clone(), client, cancel, asker.clone());
            },
            "token" => {
                let token = match (message.get("token"), message.get("value")) {
                    (Some(Data::String(token)), _)
                    | (Some(Data::Bool(true)), Some(Data::String(token))) => {
                        token.trim().to_owned()
                    },
                    _ => String::new(),
                };
                if !crate::credentials::plausible(&token) {
                    reply(
                        host,
                        &asker,
                        false,
                        json!({ "error": "that does not look like a token" }),
                    );
                    return;
                }
                self.status = Status::Verifying;
                context.paste(
                    self.host.clone(),
                    Credential::new(SecretString::from(token), Source::Saved),
                    asker.clone(),
                );
            },
            _ => {
                self.status = Status::Verifying;
                context.use_cli(self.host.clone(), asker.clone());
            },
        }
        self.error = None;
        self.waiting = Some(asker);
    }
}

/// Answers `asker` with `{ reply, action, ok }` and `fields`.
fn reply(host: &Handle, asker: &Asker, ok: bool, fields: Value) {
    let mut answer = vec![
        ("reply".to_owned(), match &asker.id {
            Data::Nil => Data::from(asker.action),
            id => id.clone(),
        }),
        ("action".to_owned(), Data::from(asker.action)),
        ("ok".to_owned(), Data::from(ok)),
    ];
    if let Value::Object(fields) = fields {
        for (key, value) in fields {
            if let Ok(value) = moonframe_providers::from_json(&value) {
                answer.push((key, value));
            }
        }
    }
    host.send(&asker.from, Data::map(answer));
}

impl Native for Forge {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.settings = moonframe_providers::to_json(host.settings());
        self.accounts = Mods::from_settings(host.settings(), "accounts");
        if let Some(seconds) = self.settings["poll"].as_u64() {
            self.every = Duration::from_secs(seconds.clamp(15, 3600));
        }
        let plaintext = self.settings["plaintext"].as_bool().unwrap_or(false);
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(".local/state")))
            .ok_or("no home folder to keep the sign-in in")?
            .join("moonframe");
        let (sender, receiver) = channel();
        self.context = Some(Context {
            kind: self.kind,
            sender,
            agent: crate::http::agent(),
            saved: Arc::new(Saved::new(self.kind, state, plaintext)),
            path: moonframe_providers::program::default_search_path(),
            images: moonframe_packages::paths::images().join("avatars"),
        });
        self.done = Some(receiver);
        self.root = repo::root(host.dir());
        self.look(host);
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.ticks += 1;
        if self.ticks >= LOOK_EVERY {
            self.ticks = 0;
            self.look(host);
        }
        let done: Vec<Done> = self
            .done
            .as_ref()
            .map(|done| done.try_iter().collect())
            .unwrap_or_default();
        for done in done {
            self.take_in(host, done);
        }
        self.expire_device(host, crate::credentials::now());
        self.fetch_if_due();
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let flag = |key: &str| {
            message
                .get(key)
                .is_some_and(|value| !matches!(value, Data::Nil | Data::Bool(false)))
        };
        let action = if flag("sign_in") {
            Some("sign_in")
        } else if flag("token") {
            Some("token")
        } else if flag("use_cli") {
            Some("use_cli")
        } else if flag("cancel") {
            Some("cancel")
        } else if flag("sign_out") {
            Some("sign_out")
        } else {
            None
        };
        match action {
            Some(action) => self.act(host, from, &message, action),
            None => {
                // Read again, but not faster than every few seconds.
                let now = Instant::now();
                if self
                    .asked_at
                    .is_none_or(|at| now.duration_since(at) >= MIN_ASKED)
                {
                    self.asked_at = Some(now);
                    self.look(host);
                    self.next = Some(now);
                }
            },
        }
        self.fetch_if_due();
        self.publish(host);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
