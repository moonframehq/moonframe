//! A JSON document kept outside the app, such as herdr-core's document
//! store, which keeps it beside the terminals it runs. The app decides
//! what goes in it; a [`Store`] only holds it.

use serde_json::Value;

/// Where a document lives. Calls may block: use them off the UI thread.
pub trait Store: Send + Sync {
    /// Its short name, for logs: `herdr-core`.
    fn name(&self) -> &'static str;

    /// The saved document, or `None` when there is none yet.
    fn load(&self) -> Result<Option<Value>, String>;

    fn save(&self, document: &Value) -> Result<(), String>;
}
