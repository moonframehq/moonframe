//! What runs off the provider's thread, because it waits on the network,
//! a keyring or a CLI: each job sends one or more [`Done`]s back, which the
//! provider takes in on its next tick.

use {
    crate::{
        Target,
        credentials::{self, Credential, Remembered, Saved, Source},
        device::{self, Poll},
        github, gitlab,
        http::Failure,
        kind::Kind,
        shape::Avatars,
    },
    serde_json::Value,
    std::{
        ffi::OsString,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::Sender,
        },
        thread,
        time::Duration,
    },
};

/// Who asked for an action, to answer them.
#[derive(Clone, Debug)]
pub struct Asker {
    pub from: String,
    pub id: moonframe_core::Data,
    pub action: &'static str,
}

pub enum Done {
    /// What the provider found at start (or for a new host).
    Loaded {
        host: String,
        credential: Option<Credential>,
        cli: bool,
        error: Option<String>,
    },
    /// A device code to show.
    Code {
        user: String,
        url: String,
        complete_url: Option<String>,
        expires: u64,
    },
    /// A sign-in checked and saved.
    SignedIn {
        host: String,
        credential: Credential,
        login: String,
        asker: Asker,
    },
    /// A sign-in that did not happen.
    Refused { error: String, asker: Asker },
    Fetched {
        target: Target,
        result: Result<Value, Failure>,
        /// The credential, renewed if it had to be.
        credential: Option<Credential>,
    },
}

/// What every job needs.
#[derive(Clone)]
pub struct Context {
    pub kind: Kind,
    pub sender: Sender<Done>,
    pub agent: ureq::Agent,
    pub saved: Arc<Saved>,
    pub path: OsString,
    pub images: PathBuf,
}

impl Context {
    fn send(&self, done: Done) {
        // The provider stopped: nothing is waiting.
        let _ = self.sender.send(done);
    }

    fn spawn(&self, job: impl FnOnce(Self) + Send + 'static) {
        let context = self.clone();
        let spawned = thread::Builder::new()
            .name(format!("moonframe-{}", self.kind.name()))
            .spawn(move || job(context));
        if let Err(error) = spawned {
            tracing::error!("{}: cannot start a thread: {error}", self.kind.name());
        }
    }

    /// Who `credential` belongs to.
    fn login(&self, host: &str, credential: &Credential) -> Result<String, Failure> {
        match self.kind {
            Kind::GitHub => github::login(&self.agent, &credential.token),
            Kind::GitLab => gitlab::login(&self.agent, host, &credential.token),
        }
    }

    /// Finds the sign-in for `host`: saved, else the environment. Also
    /// whether the CLI has one to offer.
    pub fn load(&self, host: String) {
        self.spawn(move |context| {
            let mut error = None;
            let cli = credentials::cli(context.kind, &host, &context.path);
            let credential = match context.saved.load(&host) {
                Ok(Some(Remembered::Token(credential))) => Some(credential),
                Ok(Some(Remembered::Cli)) => cli.clone(),
                Ok(None) => None,
                Err(problem) => {
                    error = Some(format!("cannot read the saved sign-in: {}", problem.0));
                    None
                },
            }
            .or_else(|| {
                // The environment's token goes only to the host it is for.
                context
                    .kind
                    .env_serves(&host)
                    .then(|| credentials::environment(context.kind))
                    .flatten()
            });
            context.send(Done::Loaded {
                host,
                credential,
                cli: cli.is_some(),
                error,
            });
        });
    }

    /// Checks `credential` with the forge, saves it, and says so.
    fn keep(&self, host: &str, credential: Credential, asker: Asker) {
        match self.login(host, &credential) {
            Ok(login) => {
                if let Err(problem) = self.saved.save(host, &credential) {
                    self.send(Done::Refused {
                        error: format!("signed in as {login}, but cannot save it: {}", problem.0),
                        asker,
                    });
                    return;
                }
                self.send(Done::SignedIn {
                    host: host.to_owned(),
                    credential,
                    login,
                    asker,
                });
            },
            Err(Failure::Unauthorized) => self.send(Done::Refused {
                error: format!("{} refused that token", self.kind.title()),
                asker,
            }),
            Err(failure) => self.send(Done::Refused {
                error: failure.to_string(),
                asker,
            }),
        }
    }

    /// A pasted token.
    pub fn paste(&self, host: String, credential: Credential, asker: Asker) {
        self.spawn(move |context| context.keep(&host, credential, asker));
    }

    /// The CLI's sign-in, once the user chose it.
    pub fn use_cli(&self, host: String, asker: Asker) {
        self.spawn(
            move |context| match credentials::cli(context.kind, &host, &context.path) {
                Some(credential) => context.keep(&host, credential, asker),
                None => {
                    let (program, _) = context.kind.cli(&host);
                    context.send(Done::Refused {
                        error: format!("{program} is not installed or not signed in to {host}"),
                        asker,
                    });
                },
            },
        );
    }

    /// A device sign-in, until it ends or `cancel` is set.
    pub fn device(&self, host: String, client: String, cancel: Arc<AtomicBool>, asker: Asker) {
        self.spawn(move |context| {
            let code = match device::start(&context.agent, context.kind, &host, &client) {
                Ok(code) => code,
                Err(failure) => {
                    context.send(Done::Refused {
                        error: failure.to_string(),
                        asker,
                    });
                    return;
                },
            };
            context.send(Done::Code {
                user: code.user.clone(),
                url: code.url.clone(),
                complete_url: code.complete_url.clone(),
                expires: code.expires,
            });
            let mut wait = code.interval;
            loop {
                // Sleep in short steps, so a cancel ends it at once.
                let mut slept = Duration::ZERO;
                while slept < wait {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(250));
                    slept += Duration::from_millis(250);
                }
                if credentials::now() >= code.expires {
                    context.send(Done::Refused {
                        error: "the code expired; start again".into(),
                        asker,
                    });
                    return;
                }
                match device::poll(&context.agent, context.kind, &host, &client, &code) {
                    Poll::Wait(next) => wait = next,
                    Poll::Done(credential) => {
                        if !cancel.load(Ordering::Acquire) {
                            context.keep(&host, credential, asker);
                        }
                        return;
                    },
                    Poll::Failed(error) => {
                        context.send(Done::Refused { error, asker });
                        return;
                    },
                }
            }
        });
    }

    /// Reads the forge for `target`, renewing the token first when it is
    /// about to run out (and once more if the forge refuses it).
    pub fn fetch(&self, target: Target, mut credential: Credential, client: Option<String>) {
        self.spawn(move |context| {
            let avatars = Avatars {
                folder: &context.images,
                prefix: match context.kind {
                    Kind::GitHub => String::new(),
                    Kind::GitLab => format!("{}-", target.host),
                },
            };
            let renew = |credential: &Credential| -> Option<Credential> {
                let client = client.as_deref()?;
                let renewed = device::refresh(
                    &context.agent,
                    context.kind,
                    &target.host,
                    client,
                    credential,
                )
                .ok()?;
                if let Err(problem) = context.saved.save(&target.host, &renewed) {
                    tracing::warn!(
                        "{}: cannot save the renewed sign-in: {}",
                        context.kind.name(),
                        problem.0
                    );
                }
                Some(renewed)
            };
            let mut renewed = false;
            if credential.expiring()
                && let Some(fresh) = renew(&credential)
            {
                credential = fresh;
                renewed = true;
            }
            let read = |credential: &Credential| match context.kind {
                Kind::GitHub => github::fetch(&context.agent, &credential.token, &target, &avatars),
                Kind::GitLab => gitlab::fetch(&context.agent, &credential.token, &target, &avatars),
            };
            let mut result = read(&credential);
            if result == Err(Failure::Unauthorized)
                && !renewed
                && credential.refresh.is_some()
                && let Some(fresh) = renew(&credential)
            {
                credential = fresh;
                renewed = true;
                result = read(&credential);
            }
            context.send(Done::Fetched {
                target,
                result,
                credential: renewed.then_some(credential),
            });
        });
    }
}

impl Source {
    /// Whether signing out can forget it: the environment's cannot be.
    pub fn forgettable(self) -> bool {
        !matches!(self, Self::Env)
    }
}
