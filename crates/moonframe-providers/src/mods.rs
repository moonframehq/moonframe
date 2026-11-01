//! The mods a provider's settings name (`writers`, `starters`): who may do
//! what only some mods may. A message comes from a mod (`editor`) or a
//! component it drew (`editor/markdown-editor#main`); both count as that
//! mod. A mod instance (`editor#2`) does not: any mod may mount one, with
//! props of its choosing, under the same key.

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

/// The mod a sender is or belongs to: `editor/markdown-editor#main` is
/// `editor`. A mod instance stays itself (`editor#2`), so it matches no
/// mod's name.
pub fn owner(from: &str) -> &str {
    from.split('/').next().unwrap_or(from)
}
