//! An inspector pick as its report says it, for an agent or the mod that
//! asked for it: the element, where it was built, what a click on it sends
//! ([`Sends`]), and the live values its mod drew it from
//! ([`ViewContext::report`]). Each window's backend says what it knows of
//! the pick ([`PickReport`]); the report is written here for all of them.

use {
    crate::{Action, Data, Input, Node, ViewContext, Workspace},
    std::{fmt::Write as _, path::Path},
};

/// A message an element sends, at most.
const MESSAGE: usize = 300;
/// What an input's message holds in place of its text and caret.
const TEXT: &str = "<text>";
const CARET: &str = "<caret byte>";

/// What clicking, or typing in, an element sends: kept as it is drawn,
/// and written out only for the pick a report is made of.
#[derive(Clone, Debug, PartialEq)]
pub enum Sends {
    Click(Action),
    /// An input's messages as it is typed in, submitted and left.
    Typing {
        to: Option<String>,
        change: Option<Data>,
        submit: Option<Data>,
        blur: Option<Data>,
    },
}

impl Sends {
    /// What `node` sends, when it sends anything.
    pub fn of(node: &Node) -> Option<Self> {
        let action = match node {
            Node::Stack(stack) => stack.action.as_ref(),
            Node::Button(button) => button.action.as_ref(),
            Node::Item(item) => item.action.as_ref(),
            Node::Input(input) => return typing(input),
            _ => None,
        }?;
        Some(Self::Click(action.clone()))
    }

    /// The mod it goes to, when not the one that drew it.
    pub fn to(&self) -> Option<&str> {
        match self {
            Self::Click(Action::Send { to, .. } | Action::Browser { to, .. })
            | Self::Typing { to, .. } => to.as_deref(),
            Self::Click(Action::Command(_) | Action::Window(_)) => None,
        }
    }

    /// As the report says it.
    pub fn describe(&self) -> String {
        match self {
            Self::Click(action) => click(action),
            Self::Typing {
                to,
                change,
                submit,
                blur,
            } => typed(to.as_deref(), [
                ("change", change.as_ref()),
                ("submit", submit.as_ref()),
                ("blur", blur.as_ref()),
            ])
            .unwrap_or_default(),
        }
    }
}

fn typing(input: &Input) -> Option<Sends> {
    let any = input.change.is_some() || input.submit.is_some() || input.blur.is_some();
    any.then(|| Sends::Typing {
        to: input.to.clone(),
        change: input.change.clone(),
        submit: input.submit.clone(),
        blur: input.blur.clone(),
    })
}

/// What an input sends, in the shape `update` gets: the host adds the
/// text as `value` (wrapping a message that is not a table as `{ msg,
/// value }`), and on change where the caret is.
fn typed(to: Option<&str>, events: [(&str, Option<&Data>); 3]) -> Option<String> {
    let sends: Vec<String> = events
        .into_iter()
        .filter_map(|(on, msg)| {
            let mut sent = Input::message(msg?, TEXT);
            if on == "change"
                && let Data::Map(fields) = &mut sent
            {
                fields.insert("caret".to_owned(), Data::from(CARET));
            }
            Some(format!("on {on} sends {}", send(to, &sent)))
        })
        .collect();
    (!sends.is_empty()).then(|| {
        format!(
            "{} ({TEXT} and {CARET} stand for the field's own)",
            sends.join("; ")
        )
    })
}

fn click(action: &Action) -> String {
    match action {
        Action::Send { to, msg } => format!("a click sends {}", send(to.as_deref(), msg)),
        Action::Command(command) => format!("a click runs the app command {command:?}"),
        Action::Browser { id, command, .. } => {
            format!("a click does {command:?} in the browser {id:?}")
        },
        Action::Window(name) => format!("a click opens the mod {name} in its own window"),
    }
}

fn send(to: Option<&str>, msg: &Data) -> String {
    let to = to.map_or_else(
        || "its mod's update".to_owned(),
        |to| format!("{to}'s update"),
    );
    format!("{} to {to}", msg.to_luau_line(MESSAGE))
}

/// What a backend knows of a pick, for [`Workspace::pick_report`].
#[derive(Default)]
pub struct PickReport<'a> {
    /// The element, as the inspector names it.
    pub element: &'a str,
    /// The innermost mod that drew it.
    pub owner: &'a str,
    /// The file and line that built it.
    pub at: &'a str,
    /// The elements around it, outermost first.
    pub inside: &'a [&'a str],
    /// Where it was drawn: x, y, width and height in pixels.
    pub drawn: Option<[f32; 4]>,
    pub sends: Option<&'a Sends>,
    /// Where to copy the file that built it when that ships signed.
    pub copy_to: Option<&'a Path>,
}

impl Workspace {
    /// The report on `pick` for `asker`, the mod it is handed to, or the
    /// user's own (none). What the element sends and its mod's values are
    /// read only as [`Workspace::may_read_context`] allows.
    pub fn pick_report(&self, pick: &PickReport, asker: Option<&str>) -> String {
        let mut out = String::from("Selected in Moonframe (View > Inspect Element):\n");
        let _ = writeln!(out, "- element: {}", pick.element);
        let _ = writeln!(out, "- mod: {}, drawn by {}", pick.owner, pick.at);
        if !pick.inside.is_empty() {
            let _ = writeln!(out, "- inside: {}", pick.inside.join(" > "));
        }
        if let Some([x, y, width, height]) = pick.drawn {
            let _ = writeln!(out, "- drawn at {x:.0},{y:.0}, {width:.0}×{height:.0} px");
        }
        // What it sends can carry the state of the mod that drew it, or of
        // the one it goes to, so it is read as both are.
        if let Some(sends) = pick.sends
            && self.may_read_context(pick.owner, asker)
            && sends.to().is_none_or(|to| self.may_read_context(to, asker))
        {
            let _ = writeln!(out, "- {}", sends.describe());
        }
        if let Some(copy) = pick.copy_to {
            let _ = writeln!(
                out,
                "- read-only: that file ships in the signed app. Copy it to {} and change the \
                 copy, which replaces it at once",
                copy.display()
            );
        }
        if let Some(context) = self.view_context_for(pick.owner, asker) {
            out.push_str(&ViewContext::report(&context, pick.owner));
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "pick_report/tests.rs"]
mod tests;
