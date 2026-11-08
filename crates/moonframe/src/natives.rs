//! The native providers this build has: Rust linked into the app, each
//! behind its own cargo feature. A provider folder with `kind = "native"`
//! runs the one registered under its name; an app on the library adds its
//! own with `Moonframe::native`.
//!
//! A built-in one a feature leaves out is still registered, as
//! [`LeftOut`]: its `app.<name>` stays nil, as mods already allow, so an
//! app built without `git` shows no error for the starter's mods reading
//! `app.git`. Only a message to it fails, as that asks for something.

use {
    crate::{Data, Handle, Native},
    moonframe_providers::Natives,
    std::sync::Arc,
};

pub(crate) fn natives() -> Natives {
    let built_in: &[(&str, fn() -> Box<dyn Native>)] = &[
        // keep-sorted start
        ("acp", acp),
        ("chat", chat),
        // The demo gallery's (examples/demo): it opens apps on this binary.
        #[cfg(feature = "studio")]
        ("demos", crate::demos::native),
        ("docs", docs),
        ("git", git),
        ("github", github),
        ("gitlab", gitlab),
        ("library", library),
        ("pi", pi),
        ("procs", procs),
        ("projects", projects),
        ("repo", repo),
        ("shell", shell),
        ("speech", speech),
        ("usage", usage),
        // keep-sorted end
    ];
    built_in
        .iter()
        .map(|&(name, make)| {
            let factory: Arc<dyn Fn() -> Box<dyn Native> + Send + Sync> = Arc::new(make);
            (name.to_owned(), factory)
        })
        .collect()
}

#[cfg(feature = "acp")]
fn acp() -> Box<dyn Native> {
    moonframe_acp::native()
}

#[cfg(feature = "chat")]
fn chat() -> Box<dyn Native> {
    moonframe_chat::native()
}

#[cfg(feature = "docs")]
fn docs() -> Box<dyn Native> {
    moonframe_docs::native()
}

#[cfg(feature = "git")]
fn git() -> Box<dyn Native> {
    moonframe_git::native()
}

#[cfg(feature = "forge")]
fn github() -> Box<dyn Native> {
    moonframe_forge::github()
}

#[cfg(feature = "forge")]
fn gitlab() -> Box<dyn Native> {
    moonframe_forge::gitlab()
}

#[cfg(feature = "documents")]
fn library() -> Box<dyn Native> {
    moonframe_documents::native()
}

#[cfg(feature = "pi")]
fn pi() -> Box<dyn Native> {
    moonframe_pi::pi()
}

#[cfg(feature = "procs")]
fn procs() -> Box<dyn Native> {
    moonframe_procs::native()
}

#[cfg(feature = "projects")]
fn projects() -> Box<dyn Native> {
    moonframe_projects::native()
}

#[cfg(feature = "pi")]
fn repo() -> Box<dyn Native> {
    moonframe_pi::repo()
}

#[cfg(feature = "shell")]
fn shell() -> Box<dyn Native> {
    moonframe_shell::native()
}

#[cfg(feature = "speech")]
fn speech() -> Box<dyn Native> {
    moonframe_speech::native()
}

#[cfg(feature = "usage")]
fn usage() -> Box<dyn Native> {
    moonframe_usage::native()
}

#[cfg(not(feature = "acp"))]
fn acp() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "chat"))]
fn chat() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "shell"))]
fn shell() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "docs"))]
fn docs() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "git"))]
fn git() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "forge"))]
fn github() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "forge"))]
fn gitlab() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "documents"))]
fn library() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "pi"))]
fn pi() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "procs"))]
fn procs() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "projects"))]
fn projects() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "pi"))]
fn repo() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "speech"))]
fn speech() -> Box<dyn Native> {
    Box::new(LeftOut)
}

#[cfg(not(feature = "usage"))]
fn usage() -> Box<dyn Native> {
    Box::new(LeftOut)
}

/// A built-in provider this build leaves out: it publishes nothing.
// With every feature on, only the tests make one.
#[cfg_attr(
    all(
        feature = "acp",
        feature = "chat",
        feature = "docs",
        feature = "documents",
        feature = "forge",
        feature = "git",
        feature = "pi",
        feature = "procs",
        feature = "projects",
        feature = "shell",
        feature = "speech",
        feature = "usage"
    ),
    allow(dead_code)
)]
struct LeftOut;

impl Native for LeftOut {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        tracing::debug!("provider {}: left out of this build", host.name());
        Ok(())
    }

    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, host: &Handle, from: &str, _message: Data) {
        host.fail(format!(
            "{from} asked `{}`, which this build leaves out; enable its feature",
            host.name()
        ));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::*,
        moonframe_providers::Event,
        std::sync::{Arc, Mutex},
    };

    #[test]
    fn a_left_out_provider_stays_quiet_until_asked() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let events: moonframe_providers::Events = {
            let seen = Arc::clone(&seen);
            Arc::new(move |event| seen.lock().unwrap().push(event))
        };
        let host = Handle::new("git", std::path::Path::new("."), Data::Nil, events);
        let mut left_out = LeftOut;
        left_out.start(&host).unwrap();
        left_out.tick(&host);
        assert!(seen.lock().unwrap().is_empty(), "it published or failed");
        left_out.receive(&host, "status", Data::from("commit"));
        assert!(matches!(&seen.lock().unwrap()[..], [Event::Failed { .. }]));
    }

    #[test]
    fn every_built_in_native_is_registered() {
        let natives = natives();
        for name in [
            "acp", "chat", "docs", "git", "github", "gitlab", "library", "pi", "procs", "projects",
            "repo", "shell", "usage",
        ] {
            assert!(natives.contains_key(name), "{name}");
        }
    }
}
