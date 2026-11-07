//! Native providers written with `async fn`, on tokio (the `tokio`
//! feature), and the runtime they and the app's views share.
//!
//! An [`AsyncNative`] keeps a native provider's order: `start` first, then
//! one `tick` or `receive` at a time, each awaited on the provider's own
//! thread with [`tokio::runtime::Handle::block_on`], while the runtime's
//! workers drive its sockets and timers. Work that should outlive a call
//! goes on the runtime with `tokio::spawn` and a clone of the [`Handle`].
//!
//! The runtime is the app's own when it gives one (or runs inside a
//! multi-thread one), else one made the first time something needs it.

use {
    crate::native::{Handle, Native},
    moonframe_core::Data,
    std::{future::Future, sync::OnceLock},
    tokio::runtime::{Builder, Handle as Runtime, Runtime as Owned, RuntimeFlavor},
};

/// A native provider written with `async fn`: the same calls as
/// [`Native`], each a future. Register it with `Moonframe::native_async`.
///
/// ```ignore
/// impl AsyncNative for Weather {
///     async fn tick(&mut self, host: &Handle) {
///         if let Ok(report) = self.client.get(URL).send().await {
///             host.publish_json(&report.json().await.unwrap_or_default());
///         }
///     }
/// }
/// ```
pub trait AsyncNative: Send + 'static {
    /// Called once, before anything else.
    fn start(&mut self, host: &Handle) -> impl Future<Output = Result<(), String>> + Send {
        let _ = host;
        async { Ok(()) }
    }

    /// Called every `interval` seconds of provider.toml, once the last call
    /// has finished.
    fn tick(&mut self, host: &Handle) -> impl Future<Output = ()> + Send {
        let _ = host;
        async {}
    }

    /// A message from the mod `from`.
    fn receive(
        &mut self,
        host: &Handle,
        from: &str,
        message: Data,
    ) -> impl Future<Output = ()> + Send {
        let _ = (host, from, message);
        async {}
    }
}

/// An [`AsyncNative`] as the host runs every native provider.
pub fn native<N: AsyncNative>(provider: N) -> Box<dyn Native> {
    Box::new(OnRuntime(provider))
}

struct OnRuntime<N>(N);

impl<N: AsyncNative> Native for OnRuntime<N> {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        runtime().block_on(self.0.start(host))
    }

    fn tick(&mut self, host: &Handle) {
        runtime().block_on(self.0.tick(host));
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        runtime().block_on(self.0.receive(host, from, message));
    }
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static OWNED: OnceLock<Owned> = OnceLock::new();

/// Uses `runtime` for async providers and [`on_runtime`], unless one was
/// set or made already; says whether it was taken. A current-thread
/// runtime is refused: the window holds the main thread, so it would
/// never run.
pub fn set_runtime(runtime: Runtime) -> bool {
    runtime.runtime_flavor() != RuntimeFlavor::CurrentThread && RUNTIME.set(runtime).is_ok()
}

/// The runtime async providers and [`on_runtime`] use: the app's, else a
/// multi-thread one made the first time it is needed.
pub fn runtime() -> Runtime {
    RUNTIME
        .get_or_init(|| {
            OWNED
                .get_or_init(|| {
                    Builder::new_multi_thread()
                        .enable_all()
                        .thread_name("moonframe-tokio")
                        .build()
                        .unwrap_or_else(|error| panic!("cannot start a tokio runtime: {error}"))
                })
                .handle()
                .clone()
        })
        .clone()
}

/// Runs `future` on the runtime and waits for it from any executor, such
/// as a GPUI view's `cx.spawn`, so a view can await what needs tokio:
///
/// ```ignore
/// cx.spawn(async move |this, cx| {
///     let page = moonframe::on_runtime(fetch(url)).await?;
///     this.update(cx, |view, cx| view.show(page, cx))
/// })
/// ```
///
/// Construct runtime-dependent futures inside an `async` block: functions
/// such as `tokio::time::sleep` need a reactor when called, before polling.
/// Passing `on_runtime(tokio::time::sleep(...))` from GPUI would panic.
///
/// ```
/// # futures::executor::block_on(async {
/// moonframe_providers::on_runtime(async {
///     tokio::time::sleep(std::time::Duration::from_millis(1)).await;
/// }).await.unwrap();
/// # });
/// ```
///
/// It fails only when the future panicked or the runtime shut down.
pub async fn on_runtime<F>(future: F) -> Result<F::Output, tokio::task::JoinError>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::*,
        crate::Event,
        std::{
            sync::{Arc, Mutex},
            time::Duration,
        },
    };

    /// Publishes what it was asked, after awaiting a tokio timer.
    #[derive(Default)]
    struct Echo {
        calls: Vec<String>,
    }

    impl AsyncNative for Echo {
        async fn start(&mut self, _: &Handle) -> Result<(), String> {
            self.calls.push("start".to_owned());
            Ok(())
        }

        async fn tick(&mut self, _: &Handle) {
            self.calls.push("tick".to_owned());
        }

        async fn receive(&mut self, host: &Handle, from: &str, message: Data) {
            tokio::time::sleep(Duration::from_millis(5)).await;
            self.calls.push(format!("receive from {from}"));
            host.publish(message);
        }
    }

    #[test]
    fn calls_are_awaited_in_order_on_the_providers_thread() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let events: crate::Events = {
            let seen = Arc::clone(&seen);
            Arc::new(move |event| seen.lock().unwrap().push(event))
        };
        let host = Handle::new("echo", std::path::Path::new("."), Data::Nil, events);
        let mut echo = OnRuntime(Echo::default());
        echo.start(&host).unwrap();
        echo.tick(&host);
        echo.receive(&host, "mine", Data::from("hello"));
        assert_eq!(echo.0.calls, ["start", "tick", "receive from mine"]);
        let seen = seen.lock().unwrap();
        assert!(
            matches!(&seen[..], [Event::Published { data, .. }] if data.as_str() == Some("hello"))
        );
    }

    #[test]
    fn a_current_thread_runtime_is_refused() {
        let runtime = Builder::new_current_thread().build().unwrap();
        assert!(!set_runtime(runtime.handle().clone()));
    }

    #[test]
    fn views_await_work_on_the_runtime_from_another_executor() {
        // A plain executor that knows nothing of tokio, as GPUI's.
        let answer = futures::executor::block_on(on_runtime(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            42
        }));
        assert_eq!(answer.unwrap(), 42);
    }
}
