//! Terminal/state backends, absent in an app with neither capability.

#[cfg(any(feature = "terminal", feature = "herdr-core"))]
pub(crate) use moonframe_mux::Backends;

#[cfg(not(any(feature = "terminal", feature = "herdr-core")))]
pub(crate) struct Backends;

#[cfg(not(any(feature = "terminal", feature = "herdr-core")))]
impl Backends {
    pub(crate) fn for_app(_name: &str) -> Self {
        Self
    }
}
