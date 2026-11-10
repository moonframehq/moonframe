//! A worker's message route, cancellation, and staged predecessor.

use {
    crate::{Message, fingerprint::Fingerprint},
    moonframe_core::entries::Entry,
    std::{
        sync::{
            Arc, Mutex, PoisonError,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Duration,
    },
};

pub(super) struct Running {
    pub(super) fallback: Arc<Mutex<Option<Box<Running>>>>,
    pub(super) entry: Option<Entry>,
    pub(super) messages: mpsc::Sender<Message>,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) ready: Arc<AtomicBool>,
    /// None once explicitly asked to restart, whatever the folder holds.
    pub(super) fingerprint: Option<Fingerprint>,
    pub(super) follows_project: bool,
    /// How long shutdown allows it, and whether its worker has left.
    pub(super) leaving: Option<(Duration, Arc<AtomicBool>)>,
}

impl Running {
    pub(super) fn collect_leaving(&self, leaving: &mut Vec<(Duration, Arc<AtomicBool>)>) {
        if let Some(wait) = &self.leaving {
            leaving.push(wait.clone());
        }
        if let Some(old) = self
            .fallback
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            old.collect_leaving(leaving);
        }
    }

    pub(super) fn healthy(&self) -> bool {
        self.ready.load(Ordering::Acquire)
            || self
                .fallback
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .is_some_and(|old| old.healthy())
    }

    pub(super) fn send(&self, message: Message) -> Result<(), mpsc::SendError<Message>> {
        let fallback = self.fallback.lock().unwrap_or_else(PoisonError::into_inner);
        match fallback.as_ref() {
            Some(old) => old.send(message),
            None => self.messages.send(message),
        }
    }

    /// Remove finished failed candidates from the chain when retrying. Live
    /// candidates must remain until cancelled so their original worker stops.
    pub(super) fn live(mut previous: Option<Self>) -> Option<Self> {
        while previous.as_ref().is_some_and(|old| {
            old.leaving
                .as_ref()
                .is_some_and(|(_, left)| left.load(Ordering::Acquire))
        }) {
            let next = previous
                .as_ref()
                .and_then(|old| {
                    old.fallback
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take()
                })
                .map(|old| *old);
            previous = next;
        }
        previous
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.fallback
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}
