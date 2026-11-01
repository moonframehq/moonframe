//! The mods a provider's settings name (`writers`, `starters`): who may do
//! what only some mods may. A message comes from a mod (`editor`), one of
//! its instances (`editor#2`) or a component it drew
//! (`editor/markdown-editor#main`); each counts as that mod.

use moonframe_core::Data;

/// Mod names from a provider's `[settings]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mods(Vec<String>);

impl Mods {
    /// The names listed under `key` in `settings`; none when it is missing
    /// or not a list. Entries that are not strings are left out.
    pub fn from_settings(settings: &Data, key: &str) -> Self {
        let Data::Map(settings) = settings else {
            return Self::default();
        };
        let Some(Data::List(names)) = settings.get(key) else {
            return Self::default();
        };
        Self(
            names
                .iter()
                .filter_map(|name| match name {
                    Data::String(name) => Some(name.clone()),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Whether the sender `from` is, or belongs to, a mod in the list.
    pub fn allows(&self, from: &str) -> bool {
        let owner = owner(from);
        self.0.iter().any(|name| name == owner)
    }
}

impl<S: Into<String>> FromIterator<S> for Mods {
    fn from_iter<I: IntoIterator<Item = S>>(names: I) -> Self {
        Self(names.into_iter().map(Into::into).collect())
    }
}

/// The mod a sender is or belongs to: `editor/markdown-editor#main` and
/// `editor#2` are both `editor`.
pub fn owner(from: &str) -> &str {
    let module = from.split('/').next().unwrap_or(from);
    module.split('#').next().unwrap_or(module)
}
