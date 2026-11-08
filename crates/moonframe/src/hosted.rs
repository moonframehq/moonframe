//! Views the host app writes in Rust, placed by its Luau with
//! `ui.view { name = "...", id = "...", props = {...} }`.
//!
//! The app registers each by name ([`crate::Moonframe::view`]); a view is
//! made the first time a mod draws it, kept by the mod and `id` as a
//! terminal is (across reloads, and while its pane is hidden), and told
//! when the mod's `props` change. What the user does in it goes back to the
//! mod's `update` through its [`Messenger`].
//!
//! Views are made and updated after the frame that drew them, since
//! drawing has no `&mut App`: the first frame shows the empty space, the
//! next the view.
//!
//! The main window draws them over the app rather than inside it ([`layers`]),
//! so that one drawing again by itself does not draw the window around it.

mod layers;

pub(crate) use layers::Layers;

use {
    futures::channel::mpsc::UnboundedSender,
    gpui_kit::{AnyView, App, AppContext as _, Context, Entity, Render, Window},
    moonframe_core::Data,
    std::{collections::BTreeMap, rc::Rc},
};

/// The most views one app keeps: each mod and id has one, so this is many
/// more than a layout needs.
const MAX_VIEWS: usize = 512;

/// A view of the host app's, drawn where its Luau puts a `ui.view`.
///
/// It fills the box the `view` rule styles: its root element is laid out at
/// that box's size, so give it `size_full()`. In the main window, notifying
/// it, or a view drawn inside it notifying itself, draws it again and nothing
/// else of the window.
pub trait HostView: Render + Sized {
    /// The mod gave other `props`. The view was made with the first ones
    /// ([`ViewInit::props`]); it draws again after this.
    fn set_props(&mut self, props: &Data, cx: &mut Context<Self>) {
        let _ = (props, cx);
    }
}

/// What a view is made with.
pub struct ViewInit {
    owner: String,
    id: String,
    props: Data,
    messenger: Messenger,
}

impl ViewInit {
    /// The mod that drew it.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Its `id`, unique within the mod (`"view"` when the mod gave none).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The `props` the mod gave, `Data::Nil` without any.
    pub fn props(&self) -> &Data {
        &self.props
    }

    /// Sends messages to the mod's `update`; keep it in the view.
    pub fn messenger(&self) -> Messenger {
        self.messenger.clone()
    }
}

/// Sends messages from a view to the mod that drew it. It may be cloned
/// into other threads.
#[derive(Clone)]
pub struct Messenger {
    owner: String,
    sender: UnboundedSender<(String, Data)>,
}

impl Messenger {
    /// Delivers `msg` to the mod's `update`, as a click of its own would.
    pub fn send(&self, msg: Data) {
        let _ = self.sender.unbounded_send((self.owner.clone(), msg));
    }

    /// The mod its messages go to.
    pub fn owner(&self) -> &str {
        &self.owner
    }
}

/// A made view, whatever its type.
trait Live {
    fn view(&self) -> AnyView;
    fn set_props(&self, props: &Data, cx: &mut App);
}

impl<T: HostView> Live for Entity<T> {
    fn view(&self) -> AnyView {
        self.clone().into()
    }

    fn set_props(&self, props: &Data, cx: &mut App) {
        self.update(cx, |view, cx| {
            view.set_props(props, cx);
            cx.notify();
        });
    }
}

type Factory = Rc<dyn Fn(ViewInit, &mut Window, &mut App) -> Box<dyn Live>>;

/// The views the app registered, by name.
#[derive(Clone, Default)]
pub(crate) struct Registry(BTreeMap<String, Factory>);

impl Registry {
    pub(crate) fn insert<T: HostView>(
        &mut self,
        name: &str,
        make: impl Fn(&ViewInit, &mut Window, &mut Context<T>) -> T + 'static,
    ) {
        let factory: Factory = Rc::new(move |init, window, cx| {
            let entity = cx.new(|cx| make(&init, window, cx));
            Box::new(entity)
        });
        self.0.insert(name.to_owned(), factory);
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

struct Placed {
    name: String,
    props: Data,
    live: Box<dyn Live>,
}

/// What the frame found a view needs, done after it.
pub(crate) enum Request {
    Make {
        key: String,
        owner: String,
        id: String,
        name: String,
        props: Data,
    },
    Props {
        key: String,
        props: Data,
    },
}

/// The views made so far, by mod and id.
pub(crate) struct Views {
    registry: Registry,
    placed: BTreeMap<String, Placed>,
    requests: Vec<Request>,
    sender: UnboundedSender<(String, Data)>,
    /// Where the main window draws them.
    layers: Rc<Layers>,
}

/// What a `ui.view` shows this frame.
pub(crate) enum Shown {
    View(AnyView),
    /// It is being made; it shows from the next frame.
    Pending,
    Missing(String),
}

impl Views {
    /// Views made from `registry`, whose messages go to `sender`.
    pub(crate) fn new(registry: Registry, sender: UnboundedSender<(String, Data)>) -> Self {
        Self {
            registry,
            placed: BTreeMap::new(),
            requests: Vec::new(),
            sender,
            layers: Rc::default(),
        }
    }

    /// Where the main window draws them.
    pub(crate) fn layers(&self) -> &Rc<Layers> {
        &self.layers
    }

    /// The view `name` the mod `owner` draws as `id`, with `props`; makes
    /// or updates it after the frame when it needs to.
    pub(crate) fn show(&mut self, owner: &str, id: &str, name: &str, props: &Data) -> Shown {
        if !self.registry.0.contains_key(name) {
            let known: Vec<&str> = self.registry.names().collect();
            return Shown::Missing(if known.is_empty() {
                format!("this app has no view `{name}`: it registers none")
            } else {
                format!("no view `{name}`; this app has {}", known.join(", "))
            });
        }
        let key = format!("{owner}/{id}");
        match self.placed.get(&key) {
            Some(placed) if placed.name == name => {
                if &placed.props != props {
                    self.requests.push(Request::Props {
                        key,
                        props: props.clone(),
                    });
                }
                Shown::View(placed.live.view())
            },
            _ if self.placed.len() >= MAX_VIEWS && !self.placed.contains_key(&key) => {
                Shown::Missing(format!("the app already has {MAX_VIEWS} views"))
            },
            _ => {
                self.requests.push(Request::Make {
                    key,
                    owner: owner.to_owned(),
                    id: id.to_owned(),
                    name: name.to_owned(),
                    props: props.clone(),
                });
                Shown::Pending
            },
        }
    }

    /// What the last frame asked for.
    pub(crate) fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Makes and updates views as `requests` say; true when one was made,
    /// so the window draws again to show it.
    pub(crate) fn apply(
        &mut self,
        requests: Vec<Request>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let mut made = false;
        for request in requests {
            match request {
                Request::Make {
                    key,
                    owner,
                    id,
                    name,
                    props,
                } => {
                    // Asked twice in one frame (a mod in two places).
                    if self
                        .placed
                        .get(&key)
                        .is_some_and(|placed| placed.name == name)
                    {
                        continue;
                    }
                    let Some(factory) = self.registry.0.get(&name).cloned() else {
                        continue;
                    };
                    let init = ViewInit {
                        messenger: Messenger {
                            owner: owner.clone(),
                            sender: self.sender.clone(),
                        },
                        owner,
                        id,
                        props: props.clone(),
                    };
                    let live = factory(init, window, cx);
                    self.placed.insert(key, Placed { name, props, live });
                    made = true;
                },
                Request::Props { key, props } => {
                    if let Some(placed) = self.placed.get_mut(&key)
                        && placed.props != props
                    {
                        placed.live.set_props(&props, cx);
                        placed.props = props;
                    }
                },
            }
        }
        made
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {super::*, futures::StreamExt as _, gpui_kit::IntoElement};

    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui_kit::div()
        }
    }

    impl HostView for Empty {}

    fn views() -> (
        Views,
        futures::channel::mpsc::UnboundedReceiver<(String, Data)>,
    ) {
        let mut registry = Registry::default();
        registry.insert("chart", |_, _, _| Empty);
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        (Views::new(registry, sender), receiver)
    }

    #[test]
    fn a_view_is_asked_for_once_per_frame_and_unknown_names_say_what_there_is() {
        let (mut views, _) = views();
        let props = Data::map([("step", 2.into())]);
        assert!(matches!(
            views.show("shell", "main", "chart", &props),
            Shown::Pending
        ));
        let requests = views.take_requests();
        assert!(matches!(
            requests.as_slice(),
            [Request::Make { key, name, .. }] if key == "shell/main" && name == "chart"
        ));
        assert!(views.take_requests().is_empty());
        match views.show("shell", "main", "graph", &props) {
            Shown::Missing(why) => assert!(why.contains("this app has chart"), "{why}"),
            _ => panic!("an unknown view showed"),
        }
    }

    #[test]
    fn a_messenger_sends_to_the_mod_that_drew_the_view() {
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let messenger = Messenger {
            owner: "sidebar".to_owned(),
            sender,
        };
        let copy = messenger.clone();
        std::thread::spawn(move || copy.send("clicked".into()))
            .join()
            .unwrap();
        let (owner, msg) = futures::executor::block_on(receiver.next()).unwrap();
        assert_eq!(owner, "sidebar");
        assert_eq!(msg, Data::from("clicked"));
    }
}
