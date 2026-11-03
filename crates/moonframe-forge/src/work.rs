//! What runs off the provider's thread, because it waits on the network,
//! a keyring or a CLI: each job sends one or more [`Done`]s back, which the
//! provider takes in on its next tick.

use {
    crate::{
        Target,
        credentials::{self, Credential, Remembered, Renewal, Saved, Source},
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

mod fetch;

/// Who asked for an action, to answer them.
#[derive(Clone, Debug)]
pub struct Asker {
    pub from: String,
    pub id: moonframe_core::Data,
    pub action: &'static str,
    pub attempt: u64,
}

pub enum Done {
    // keep-sorted start
    /// Whether a CLI sign-in is available now, without selecting it.
    Cli {
        host: String,
        epoch: u64,
        available: bool,
    },
    /// A device code to show.
    Code {
        attempt: u64,
        user: String,
        url: String,
        complete_url: Option<String>,
        expires: u64,
    },
    Fetched {
        target: Target,
        epoch: u64,
        result: Result<Value, Failure>,
        /// The credential, renewed (and saved) or read from the CLI again.
        credential: Option<Credential>,
    },
    /// What the provider found at start (or for a new host).
    Loaded {
        host: String,
        epoch: u64,
        credential: Option<Credential>,
        cli: bool,
        error: Option<String>,
        retry: bool,
    },
    /// A sign-in that did not happen.
    Refused { error: String, asker: Asker },
    /// A sign-in checked and saved.
    SignedIn {
        host: String,
        credential: Credential,
        login: String,
        asker: Asker,
    },
    // keep-sorted end
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
    pub fn load(&self, host: String, epoch: u64) {
        self.spawn(move |context| {
            let mut error = None;
            let mut retry = false;
            let cli = credentials::cli(context.kind, &host, &context.path);
            let credential = match context.saved.load(&host) {
                Ok(Some(Remembered::Token(credential))) => Some(credential),
                Ok(Some(Remembered::Cli)) => {
                    match credentials::cli_ready(context.kind, &host, &context.path) {
                        Ok(credential) => credential,
                        Err(failure) => {
                            retry = true;
                            error = Some(failure.to_string());
                            None
                        },
                    }
                },
                Ok(None) => None,
                Err(problem) => {
                    error = Some(format!("cannot read the saved sign-in: {}", problem.0));
                    None
                },
            }
            .or_else(|| {
                // The environment's token goes only to the host it is for.
                (!retry)
                    .then(|| {
                        context
                            .kind
                            .env_serves(&host)
                            .then(|| credentials::environment(context.kind))
                            .flatten()
                    })
                    .flatten()
            });
            context.send(Done::Loaded {
                host,
                epoch,
                credential,
                cli: cli.is_ok_and(|credential| credential.is_some()),
                error,
                retry,
            });
        });
    }

    pub fn probe_cli(&self, host: String, epoch: u64) {
        self.spawn(move |context| {
            let available = credentials::cli(context.kind, &host, &context.path)
                .is_ok_and(|credential| credential.is_some());
            context.send(Done::Cli {
                host,
                epoch,
                available,
            });
        });
    }

    /// Checks `credential` with the forge and says so. Saving it is the
    /// provider's, once it knows no sign-out came in the meantime.
    fn keep(&self, host: &str, credential: Credential, asker: Asker) {
        match self.login(host, &credential) {
            Ok(login) => {
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
        self.spawn(move |context| {
            match credentials::cli_ready(context.kind, &host, &context.path) {
                Ok(Some(credential)) => context.keep(&host, credential, asker),
                Ok(None) => {
                    let (program, _) = context.kind.cli(&host);
                    context.send(Done::Refused {
                        error: format!("{program} is not installed or not signed in to {host}"),
                        asker,
                    });
                },
                Err(failure) => context.send(Done::Refused {
                    error: failure.to_string(),
                    asker,
                }),
            }
        });
    }

    /// A device sign-in, until it ends or `cancel` is set.
    pub fn device(&self, host: String, client: String, cancel: Arc<AtomicBool>, asker: Asker) {
        self.spawn(move |context| {
            let mut code = match device::start(&context.agent, context.kind, &host, &client) {
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
                attempt: asker.attempt,
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
                match device::poll(&context.agent, context.kind, &host, &client, &mut code) {
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
    pub fn fetch(
        &self,
        target: Target,
        credential: Credential,
        client: Option<String>,
        epoch: u64,
    ) {
        self.spawn(move |context| {
            let credential =
                match credentials::for_fetch(context.kind, &target.host, &context.path, credential)
                {
                    Ok(credential) => credential,
                    Err(failure) => {
                        context.send(Done::Fetched {
                            target,
                            epoch,
                            result: Err(failure),
                            credential: None,
                        });
                        return;
                    },
                };
            let avatars = Avatars {
                folder: &context.images,
                prefix: match context.kind {
                    Kind::GitHub => String::new(),
                    Kind::GitLab => format!("{}-", target.host),
                },
            };
            // Renewing goes through the store, in turn with other apps
            // sharing this sign-in. Adopting another app's pair is not a
            // renewal: if refused, it can still be renewed once.
            let renew = |credential: &Credential| {
                let Some(client) = client.as_deref() else {
                    return Renewal::Failed(Failure::Status(
                        400,
                        "no OAuth client id to renew the sign-in".into(),
                    ));
                };
                context.saved.renew(&target.host, credential, |credential| {
                    device::refresh(
                        &context.agent,
                        context.kind,
                        &target.host,
                        client,
                        credential,
                    )
                })
            };
            let read = |credential: &Credential| match context.kind {
                Kind::GitHub => github::fetch(&context.agent, &credential.token, &target, &avatars),
                Kind::GitLab => gitlab::fetch(&context.agent, &credential.token, &target, &avatars),
            };
            let (result, credential) = fetch::read(credential, renew, read);
            context.send(Done::Fetched {
                target,
                epoch,
                result,
                credential,
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
