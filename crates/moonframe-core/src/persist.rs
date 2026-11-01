//! What outlives the app: each mod's state and the topics mods publish. The
//! workspace saves through [`Persist`], so the core stays free of any
//! database; `moonframe-store` keeps it in SQLite.

use {crate::Data, std::collections::BTreeMap};

/// Somewhere to keep state between runs. Saves may be queued: a read right
/// after a save is allowed to see the older value.
pub trait Persist {
    /// The state saved at `key`, such as `mod:explorer`.
    fn state(&self, key: &str) -> Option<Data>;
    fn save_state(&self, key: &str, value: &Data);
    /// Every topic's last value.
    fn topics(&self) -> BTreeMap<String, Data>;
    fn save_topic(&self, name: &str, value: &Data);
}

/// Where a module's state is kept.
pub(crate) fn state_key(module: &str) -> String {
    format!("mod:{module}")
}
