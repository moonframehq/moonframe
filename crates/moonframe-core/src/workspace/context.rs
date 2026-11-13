//! What a mod's view was drawn from, as it is now: its state, the props
//! it was given, and the keys of `app` it read with what they hold. An
//! inspector pick hands it to an agent with the element's file and line
//! ([`ViewContext::report`]), so the agent knows the values behind what it
//! sees, not only the code.

use {
    super::{render::BUILT, *},
    std::fmt::Write as _,
};

/// What a mod's last view was drawn from.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewContext {
    /// Its state; none before it loads, and an error when the state holds
    /// what data cannot (a function).
    pub state: Option<Result<Data, Error>>,
    /// What it was drawn with as an instance; nil for a mod in its own place.
    pub props: Data,
    /// The keys of `app` its last view read, each with what it holds now
    /// (nil for a key nothing shares); none when it went through all of
    /// `app`.
    pub reads: Option<Vec<(String, Data)>>,
    /// What is wrong with it: its load, update or view error, then each
    /// child its view could not draw.
    pub problems: Vec<String>,
}

impl Workspace {
    /// What the module `name` (as [`Workspace::slot`] finds it) drew its
    /// last view from.
    pub fn view_context(&self, name: &str) -> Option<ViewContext> {
        let slot = self.slot(name)?;
        let reads = match &slot.reads {
            Reads::All => None,
            Reads::Keys(keys) => {
                // What a provider shares is most of `app`: build the rest
                // of it only for a key `app` sets itself.
                let mut app = None;
                // A mod in a window of its own, and what it mounts, read
                // that window as `app.window`, as `Slot::render` gives it.
                let window = slot.drawn.as_ref().and_then(|drawn| drawn.window);
                Some(
                    keys.iter()
                        .map(|key| {
                            let value = match &window {
                                Some(window) if key == "window" => Some(window.data()),
                                // What `app` sets over the host's, as the view got it.
                                _ if BUILT.contains(&key.as_str()) => {
                                    app.get_or_insert_with(|| self.app()).get(key).cloned()
                                },
                                _ => self.host.get(key).cloned(),
                            };
                            (key.clone(), value.unwrap_or_default())
                        })
                        .collect(),
                )
            },
        };
        let problems = slot
            .error()
            .into_iter()
            .chain(slot.broken())
            .map(ToString::to_string)
            .collect();
        Some(ViewContext {
            state: slot.state(),
            props: slot.props.clone(),
            reads,
            problems,
        })
    }

    /// [`Workspace::view_context`] of `owner`, for a reader: the user's own
    /// (none), or the module `asker`, which gets it only when it drew the
    /// element itself or is one of the app's own (the built-in app's, the
    /// app's or the user's, never a package's). A mod that asks for a pick
    /// must not read another mod's state through it.
    pub fn view_context_for(&self, owner: &str, asker: Option<&str>) -> Option<ViewContext> {
        self.may_read_context(owner, asker)
            .then(|| self.view_context(owner))
            .flatten()
    }

    /// Whether `asker` (none for the user's own reader) may read what
    /// `owner` drew from, as [`Workspace::view_context_for`] decides: also
    /// what its elements send, which can carry its state. A mod drew what
    /// its own instances and components drew (`tally#b`, `tally/tabs#main`).
    pub fn may_read_context(&self, owner: &str, asker: Option<&str>) -> bool {
        asker.is_none_or(|asker| module_of(asker) == module_of(owner) || self.own_module(asker))
    }

    /// Whether the module `name` comes from one of the app's own folders.
    fn own_module(&self, name: &str) -> bool {
        self.slot(name)
            .and_then(Slot::source)
            .is_some_and(|source| self.roots.in_app_layer(source))
    }
}

/// The mod whose code made the module `name`: `tally` for itself, its
/// instance `tally#b`, and the component `tally/tabs#main` it mounted.
fn module_of(name: &str) -> &str {
    name.split(['/', '#']).next().unwrap_or(name)
}

/// The most text one value takes in a report.
const STATE: usize = 4000;
const PROPS: usize = 1500;
/// Each key of `app` the view read, and all of them together.
const READ: usize = 600;
const READS: usize = 3000;

impl ViewContext {
    /// The context as lines of an inspector's report on an element `owner`
    /// drew, values written as Luau and cut to fit.
    pub fn report(&self, owner: &str) -> String {
        let mut out = String::new();
        match &self.state {
            Some(Ok(Data::Nil)) | None => {},
            Some(Ok(state)) => {
                let _ = writeln!(
                    out,
                    "- state of {owner}, as it is now:\n{}",
                    block(state, STATE)
                );
            },
            Some(Err(error)) => {
                let _ = writeln!(out, "- state of {owner}: cannot be shown ({error})");
            },
        }
        if self.props != Data::Nil {
            let _ = writeln!(
                out,
                "- props it was drawn with:\n{}",
                block(&self.props, PROPS)
            );
        }
        match &self.reads {
            None => {
                let _ = writeln!(
                    out,
                    "- reads from app: all of it (it goes through every key)"
                );
            },
            Some(reads) if reads.is_empty() => {},
            Some(reads) => {
                let _ = writeln!(out, "- reads from app (key = value now):");
                let mut left = READS;
                for (index, (key, value)) in reads.iter().enumerate() {
                    if left == 0 {
                        let _ = writeln!(out, "  -- {} more keys cut", reads.len() - index);
                        break;
                    }
                    let text = value.to_luau_line(READ.min(left));
                    left = left.saturating_sub(text.len());
                    let _ = writeln!(out, "  app.{key} = {text}");
                }
            },
        }
        for problem in &self.problems {
            let _ = writeln!(out, "- problem: {problem}");
        }
        out
    }
}

/// `data` as an indented Luau block under a report line.
fn block(data: &Data, budget: usize) -> String {
    data.to_luau(budget)
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_app_sets_itself_are_built() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(dir.path(), false);
        let app = workspace.app();
        for key in BUILT {
            assert!(app.contains_key(*key), "{key}");
        }
        for key in app.keys().filter(|key| !workspace.host.contains_key(*key)) {
            assert!(BUILT.contains(&key.as_str()), "{key}");
        }
    }

    #[test]
    fn a_module_belongs_to_the_mod_whose_code_made_it() {
        assert_eq!(module_of("tally"), "tally");
        assert_eq!(module_of("tally#b"), "tally");
        assert_eq!(module_of("tally#b/tabs#main"), "tally");
    }
}
