//! State for apps without a terminal or herdr-core backend.

pub(crate) fn open(
    _backends: &crate::backends::Backends,
    profile: Option<&str>,
) -> Option<Box<dyn moonframe_core::Persist>> {
    moonframe_store::Store::open(&crate::paths::state(profile))
        .inspect_err(|error| tracing::warn!("{error}"))
        .ok()
        .map(|store| Box::new(store) as Box<dyn moonframe_core::Persist>)
}
