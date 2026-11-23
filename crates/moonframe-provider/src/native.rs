//! The host side of a provider compiled into the app. The app's adapter
//! (`moonframe_providers::Portable`) implements [`Host`] and wraps each call
//! to the provider in [`enter`], so [`crate::log`], [`crate::system_load`]
//! and [`crate::host_call`] reach the app on the provider's own thread, as
//! the WIT imports reach it from a component.

use std::{cell::RefCell, rc::Rc};

/// What the app does for a provider compiled into it.
pub trait Host {
    fn log(&self, message: &str);
    /// The `system` capability's sample, as JSON text.
    fn system_load(&self) -> Result<String, String>;
    /// A call to a capability declared in provider.toml, JSON in and out.
    fn host_call(&self, capability: &str, method: &str, args: &str) -> Result<String, String>;
}

thread_local! {
    static HOST: RefCell<Option<Rc<dyn Host>>> = const { RefCell::new(None) };
}

/// Runs `call` with `host` answering the provider's calls on this thread.
pub fn enter<R>(host: Rc<dyn Host>, call: impl FnOnce() -> R) -> R {
    /// Puts the outer host back, even after a panic.
    struct Restore(Option<Rc<dyn Host>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            HOST.set(self.0.take());
        }
    }
    let _restore = Restore(HOST.replace(Some(host)));
    call()
}

fn host() -> Result<Rc<dyn Host>, String> {
    HOST.with_borrow(Clone::clone)
        .ok_or_else(|| "no Moonframe host on this thread".to_owned())
}

pub(crate) fn log(message: &str) {
    match HOST.with_borrow(Clone::clone) {
        Some(host) => host.log(message),
        // A thread the provider started itself: the terminal still sees it.
        None => eprintln!("{message}"),
    }
}

pub(crate) fn system_load() -> Result<String, String> {
    host()?.system_load()
}

pub(crate) fn host_call(capability: &str, method: &str, args: &str) -> Result<String, String> {
    host()?.host_call(capability, method, args)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::cell::Cell};

    struct Answering(Cell<usize>);

    impl Host for Answering {
        fn log(&self, _: &str) {
            self.0.set(self.0.get() + 1);
        }

        fn system_load(&self) -> Result<String, String> {
            Ok(r#"{"cpu":0.5}"#.to_owned())
        }

        fn host_call(&self, capability: &str, method: &str, args: &str) -> Result<String, String> {
            Ok(format!(r#"["{capability}","{method}",{args}]"#))
        }
    }

    #[test]
    fn calls_reach_the_entered_host_and_no_other() {
        assert!(crate::system_load().is_err());
        let host = Rc::new(Answering(Cell::new(0)));
        let answer: serde_json::Value = enter(host.clone(), || {
            crate::log("hello");
            assert_eq!(crate::system_load().unwrap()["cpu"], 0.5);
            crate::host_call("example.echo", "echo", &1).unwrap()
        });
        assert_eq!(answer, serde_json::json!(["example.echo", "echo", 1]));
        assert_eq!(host.0.get(), 1);
        assert!(crate::host_call::<_, serde_json::Value>("example.echo", "echo", &1).is_err());
    }
}
