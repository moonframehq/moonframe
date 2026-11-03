//! Delivering messages: from controls to modules, between modules, to
//! providers, and through topics.

use {
    super::*,
    crate::{APP, Action, Command, WindowRequest},
};

/// The one app request a preview may make: edits of its own fields.
const EDIT: &str = "app:edit";

impl Workspace {
    /// Who a send from `from` to `to` reaches: `parent` is the module that
    /// drew an instance, and `tabbed#main` names an instance the sender drew
    /// before an instance of the app's of that name.
    fn resolve(&self, from: &str, to: &str) -> String {
        if to == PARENT
            && let Some(mounted) = self.instances.get(from)
        {
            return mounted.owner.clone();
        }
        if to.contains('#') && !to.contains('/') {
            let own = format!("{from}/{to}");
            if self.instances.contains_key(&own) {
                return own;
            }
        }
        to.to_owned()
    }

    /// Runs what a control drawn by the module `from` asks for. Messages go
    /// to modules here; commands are the app's, so they are handed back.
    pub fn dispatch(&mut self, from: &str, action: &Action) -> Option<Command> {
        match action {
            // A preview never quits or closes the app.
            Action::Command(_) if from.starts_with(DEMO) => None,
            Action::Command(command) => Some(*command),
            // The app runs these on the browser it keeps, and opens windows.
            Action::Browser { .. } | Action::Window(_) => None,
            Action::Send { to, msg } => {
                self.send(from, to.as_deref().unwrap_or(from), msg.clone());
                None
            },
        }
    }

    /// Delivers `msg` to the module `to`, then whatever it sends on, and
    /// draws every module again.
    pub fn send(&mut self, from: &str, to: &str, msg: Data) {
        let (started, before) = (Instant::now(), self.views_run);
        self.process(VecDeque::from([(from.to_owned(), to.to_owned(), msg)]));
        let updated = started.elapsed();
        self.settle();
        let elapsed = started.elapsed();
        crate::perf!(
            elapsed,
            "message to {to}: updated in {updated:.1?}, {} views ran in {:.1?}",
            self.views_run - before,
            elapsed - updated,
        );
    }

    /// Runs the component demos only while the Components window shows them.
    pub fn show_demos(&mut self, shown: bool) {
        if shown != self.demos_shown {
            self.demos_shown = shown;
            if shown {
                self.render();
            }
        }
    }

    /// Delivers each message in `queue`, and what the modules send on.
    pub(super) fn process(&mut self, mut queue: VecDeque<(String, String, Data)>) {
        let (mut delivered, mut sent) = (0, 0);
        while let Some((from, to, msg)) = queue.pop_front() {
            sent += 1;
            if sent > MAX_SENDS {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!(
                        "more than {MAX_SENDS} messages from one action, counting those to providers and topics"
                    )));
                }
                break;
            }
            let to = self.resolve(&from, &to);
            // A preview, and every instance it drew at any depth, keeps to
            // itself: no other mod, provider or topic hears it. `app:edit`
            // touches only the sender's own fields, so a demo may edit them.
            let root = self.root_owner(&from);
            if root.starts_with(DEMO)
                && to != from
                && to != root
                && to != EDIT
                && !self.owned_by(&to, &root)
            {
                continue;
            }
            if let Some(provider) = to.strip_prefix(PROVIDER) {
                self.outbox.push((provider.to_owned(), from.clone(), msg));
                continue;
            }
            if let Some(what) = to.strip_prefix(APP) {
                // Modules ask; a provider's answer or a topic's news does not.
                if from.starts_with(PROVIDER) || from.starts_with(TOPIC) {
                    continue;
                }
                match Request::parse(what, &msg) {
                    Ok(request) => {
                        // What a window opens with reaches its mod now, in
                        // this cascade, so its first frame shows it.
                        if let Request::Window(WindowRequest::Open {
                            name,
                            send: Some(send),
                        }) = &request
                        {
                            queue.push_back((from.clone(), name.clone(), send.clone()));
                        }
                        self.requests.push((from, request));
                    },
                    Err(error) => {
                        if let Some(slot) = self.slot_mut(&from) {
                            slot.update_error = Some(error);
                        }
                    },
                }
                continue;
            }
            if let Some(topic) = to.strip_prefix(TOPIC) {
                match self.publish(topic, msg) {
                    Ok(deliveries) => queue.extend(deliveries),
                    Err(error) => {
                        if let Some(slot) = self.slot_mut(&from) {
                            slot.update_error = Some(error);
                        }
                    },
                }
                continue;
            }
            // What arrives as `provider/` or `app/` topic news comes from
            // the host only: a mod cannot pass one off to another.
            if !from.starts_with(TOPIC) && forged_topic(&msg) {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!(
                        "a message to `{to}` looks like `{PROVIDER_TOPIC}` or `{APP_TOPIC}` topic news, which only the app sends"
                    )));
                }
                continue;
            }
            delivered += 1;
            if delivered > MAX_MESSAGES {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!(
                        "more than {MAX_MESSAGES} messages from one action; are mods sending in a loop?"
                    )));
                }
                break;
            }
            let Some(Slot {
                module: Some(module),
                update_error,
                dirty,
                ..
            }) = self.slot_mut(&to)
            else {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!("no mod named `{to}`")));
                }
                continue;
            };
            // `update` may change the state in place, so the view runs again.
            *dirty = true;
            match module.update(&msg) {
                Ok(sends) if sends.len() > MAX_SENDS_PER_UPDATE => {
                    *update_error = Some(Error::new(format!(
                        "update sent {} messages; it may send up to {MAX_SENDS_PER_UPDATE}",
                        sends.len()
                    )));
                },
                Ok(sends) => {
                    *update_error = None;
                    queue.extend(
                        sends
                            .into_iter()
                            .map(|send| (to.clone(), send.to, send.msg)),
                    );
                },
                Err(error) => *update_error = Some(error),
            }
        }
    }

    /// Keeps `value` as the topic `name`'s, saves it, and returns a message
    /// for each module that hears it.
    fn publish(&mut self, name: &str, value: Data) -> Result<Vec<(String, String, Data)>, Error> {
        let plain = !name.is_empty()
            && name.len() <= MAX_TOPIC
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
        if !plain || name.starts_with(PROVIDER_TOPIC) || name.starts_with(APP_TOPIC) {
            return Err(Error::new(format!(
                "`topic:{name}`: a topic is 1 to {MAX_TOPIC} of a-z, A-Z, 0-9, - _ . /; provider/ topics are the providers' and app/ ones the app's"
            )));
        }
        let deliveries = self.deliveries(name, &value);
        if let Some(persist) = &self.persist {
            persist.save_topic(name, &value);
        }
        self.topics.insert(name.to_owned(), value);
        Ok(deliveries)
    }

    /// A message for each module subscribed to the topic `name`, carrying
    /// `value`. Previews hear nothing: they keep to themselves.
    pub(super) fn deliveries(&self, name: &str, value: &Data) -> Vec<(String, String, Data)> {
        self.listeners()
            .filter(|(_, slot)| {
                slot.module
                    .as_ref()
                    .is_some_and(|module| module.subscribes(name))
            })
            .map(|(key, _)| {
                (
                    format!("{TOPIC}{name}"),
                    key.to_owned(),
                    topic_message(name, value),
                )
            })
            .collect()
    }

    /// The last value published on the topic `name`.
    pub fn topic(&self, name: &str) -> Option<&Data> {
        self.topics.get(name)
    }

    /// Whether any module hears the topic `name`, such as a tab bar
    /// `tabs/main/open`.
    pub fn hears(&self, name: &str) -> bool {
        self.listeners().any(|(_, slot)| {
            slot.module
                .as_ref()
                .is_some_and(|module| module.subscribes(name))
        })
    }

    /// Every module that may subscribe: the shell, the mods, and the
    /// instances not drawn by a preview.
    pub(super) fn listeners(&self) -> impl Iterator<Item = (&str, &Slot)> {
        std::iter::once((SHELL, &self.shell))
            .chain(self.mods.iter().map(|(name, slot)| (name.as_str(), slot)))
            .chain(
                self.instances
                    .iter()
                    .filter(|(_, mounted)| !mounted.owner.starts_with(DEMO))
                    .map(|(key, mounted)| (key.as_str(), &mounted.slot)),
            )
    }

    /// The module at the top of `key`'s chain of owners: itself, for a mod.
    fn root_owner(&self, key: &str) -> String {
        let mut at = key;
        while let Some(mounted) = self.instances.get(at) {
            at = &mounted.owner;
        }
        at.to_owned()
    }

    /// Whether `key` is an instance drawn, at any depth, by `owner`.
    fn owned_by(&self, key: &str, owner: &str) -> bool {
        let mut at = key;
        while let Some(mounted) = self.instances.get(at) {
            if mounted.owner == owner {
                return true;
            }
            at = &mounted.owner;
        }
        false
    }
}

/// What a module hears on a topic: `{ topic = "<name>", value = ... }`.
/// A message with a `topic` the host alone publishes.
fn forged_topic(msg: &Data) -> bool {
    let Data::Map(fields) = msg else {
        return false;
    };
    matches!(fields.get("topic"), Some(Data::String(topic))
        if topic.starts_with(PROVIDER_TOPIC) || topic.starts_with(APP_TOPIC))
}

pub(super) fn topic_message(name: &str, value: &Data) -> Data {
    Data::Map(
        [
            ("topic".to_owned(), Data::from(name)),
            ("value".to_owned(), value.clone()),
        ]
        .into(),
    )
}
