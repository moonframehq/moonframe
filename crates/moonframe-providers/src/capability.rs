//! Host-defined, manifest-gated calls from sandboxed providers.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

/// Identity supplied by the loader, never by guest arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub provider: String,
    pub folder: PathBuf,
    pub root: PathBuf,
    pub namespace: Option<String>,
    pub capabilities: Vec<String>,
}

pub trait HostCapability: Send + Sync + 'static {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String>;
}

#[cfg(feature = "tokio")]
pub trait AsyncHostCapability: Send + Sync + 'static {
    fn call(
        &self,
        caller: &Caller,
        method: &str,
        args: &str,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

/// Host-call limits are independent of the guest's Wasm fuel.
#[derive(Clone, Copy, Debug)]
pub struct HostCallLimits {
    pub max_bytes: usize,
    pub timeout: Duration,
    /// Bounds outstanding synchronous calls, including ones that timed out.
    pub max_in_flight: usize,
}

impl Default for HostCallLimits {
    fn default() -> Self {
        Self {
            max_bytes: 1024 * 1024,
            timeout: Duration::from_secs(5),
            max_in_flight: 32,
        }
    }
}

#[derive(Clone, Default)]
pub struct Capabilities {
    handlers: BTreeMap<String, Handler>,
    pub limits: HostCallLimits,
    active: Arc<AtomicUsize>,
}

type Handler = Arc<dyn Fn(&Caller, &str, &str, Duration) -> Result<String, String> + Send + Sync>;

impl Capabilities {
    /// Registers a namespaced capability, for example `mshub.entities`.
    /// Unprefixed names and the `moonframe` namespace belong to the host.
    pub fn capability(&mut self, name: &str, cap: impl HostCapability) -> Result<(), String> {
        self.insert(
            name,
            Arc::new(move |caller, method, args, _| cap.call(caller, method, args)),
        )
    }

    fn insert(&mut self, name: &str, handler: Handler) -> Result<(), String> {
        if !valid_name(name) {
            return Err(format!(
                "capability `{name}` needs a non-reserved namespace, e.g. mshub.entities"
            ));
        }
        if self.handlers.contains_key(name) {
            return Err(format!("capability `{name}` is already registered"));
        }
        self.handlers.insert(name.to_owned(), handler);
        Ok(())
    }

    #[cfg(feature = "tokio")]
    pub fn capability_async(
        &mut self,
        name: &str,
        cap: impl AsyncHostCapability,
    ) -> Result<(), String> {
        self.insert(
            name,
            Arc::new(move |caller, method, args, timeout| {
                crate::runtime().block_on(async {
                    tokio::time::timeout(timeout, cap.call(caller, method, args))
                        .await
                        .map_err(|_| "host-call timed out".to_owned())?
                })
            }),
        )
    }

    pub fn contains(&self, name: &str) -> bool {
        name == "system" || self.handlers.contains_key(name)
    }

    /// Dispatch used by the Wasm import. Each instance has its own `busy`
    /// flag, so a timed-out guest cannot keep spawning host work.
    pub(crate) fn call(
        &self,
        caller: &Caller,
        busy: &Arc<AtomicBool>,
        capability: &str,
        method: &str,
        args: &str,
    ) -> Result<String, String> {
        if !caller.capabilities.iter().any(|name| name == capability) {
            return Err(format!("undeclared capability `{capability}`"));
        }
        let handler = self
            .handlers
            .get(capability)
            .cloned()
            .ok_or_else(|| format!("unknown host capability `{capability}`"))?;
        if args.len() > self.limits.max_bytes || method.len() > self.limits.max_bytes {
            return Err(format!(
                "host-call arguments exceed {} bytes",
                self.limits.max_bytes
            ));
        }
        serde_json::from_str::<serde_json::Value>(args)
            .map_err(|e| format!("host-call arguments are not JSON: {e}"))?;
        if busy.swap(true, Ordering::AcqRel) {
            return Err("previous host call is still running".into());
        }
        let count = self
            .active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.limits.max_in_flight).then_some(n + 1)
            });
        if count.is_err() {
            busy.store(false, Ordering::Release);
            return Err("too many host calls in flight".into());
        }
        let permit = Permit {
            active: self.active.clone(),
            busy: busy.clone(),
        };
        let (caller, method, args) = (caller.clone(), method.to_owned(), args.to_owned());
        let (send, receive) = mpsc::sync_channel(1);
        let limits = self.limits;
        std::thread::Builder::new()
            .name("host-capability".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handler(&caller, &method, &args, limits.timeout)
                }))
                .unwrap_or_else(|_| Err("host capability panicked".into()));
                let result = match result {
                    Ok(text) if text.len() > limits.max_bytes => Err(format!(
                        "host-call result exceeds {} bytes",
                        limits.max_bytes
                    )),
                    Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
                        .map(|_| text)
                        .map_err(|e| format!("host-call result is not JSON: {e}")),
                    Err(text) if text.len() > limits.max_bytes => {
                        Err("host-call error exceeds size limit".into())
                    },
                    Err(text) => Err(text),
                };
                drop(permit);
                let _ = send.send(result);
            })
            .map_err(|e| format!("cannot start host call: {e}"))?;
        receive.recv_timeout(limits.timeout).map_err(|e| match e {
            mpsc::RecvTimeoutError::Timeout => "host-call timed out".to_owned(),
            mpsc::RecvTimeoutError::Disconnected => "host-call worker stopped".to_owned(),
        })?
    }
}

struct Permit {
    active: Arc<AtomicUsize>,
    busy: Arc<AtomicBool>,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        self.busy.store(false, Ordering::Release);
    }
}

fn valid_name(name: &str) -> bool {
    name.len() <= 128
        && name.contains('.')
        && !name.starts_with("moonframe.")
        && name.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b))
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
