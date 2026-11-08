//! A host app's view (`ui.view`) drawing again by itself: the window replays
//! what it keeps around the view, and new props from the mod draw it all.

use {
    super::*,
    crate::hosted::HostView,
    gpui_kit::{Context, IntoElement, ParentElement as _, Render, div},
    moonframe_core::Data,
    std::{cell::RefCell, rc::Rc},
};

/// A view that shows the props it was last given.
struct Counter(String);

impl Render for Counter {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.0.clone())
    }
}

impl HostView for Counter {
    fn set_props(&mut self, props: &Data, _: &mut Context<Self>) {
        self.0 = format!("{props:?}");
    }
}

/// A shell showing the `counter` view beside plain nodes worth keeping;
/// `bump` changes the view's props.
const SHELL: &str = r#"
return mod {
	init = function() return { log = "", step = 1 } end,
	update = function(state, msg)
		if msg == "bump" then state.step += 1 end
	end,
	view = function(state)
		return shell(state, ui.row { grow = true,
			ui.column { ui.text "one", ui.text "two", ui.text "three" },
			ui.view { name = "counter", props = { step = state.step } },
		})
	end,
}"#;

/// A window on [`SHELL`], and the view once made.
fn with_counter(cx: &mut TestAppContext) -> (Window<'_>, Entity<Counter>) {
    let made = Rc::new(RefCell::new(None));
    let mut views = Registry::default();
    {
        let made = made.clone();
        views.insert("counter", move |init, _, cx| {
            *made.borrow_mut() = Some(cx.entity());
            Counter(format!("{:?}", init.props()))
        });
    }
    let window = open_with(cx, folder(SHELL), views);
    let counter = made.borrow().clone().expect("the view is made");
    (window, counter)
}

impl Window<'_> {
    /// `view` draws again by itself; whether that frame was a live one.
    fn notify(&mut self, view: &Entity<Counter>) -> bool {
        view.update(self.cx, |_, cx| cx.notify());
        self.cx.run_until_parked();
        let app = self.app.clone();
        self.cx.read_entity(&app, |app, _| app.drew_live())
    }
}

#[gpui_kit::test]
fn a_host_view_drawing_by_itself_replays_the_rest(cx: &mut TestAppContext) {
    let (mut window, counter) = with_counter(cx);
    // The first time makes it live; it keeps the rest from the next.
    assert!(!window.notify(&counter));
    assert!(window.notify(&counter));
    assert!(window.notify(&counter));
}

#[gpui_kit::test]
fn new_props_draw_the_whole_window(cx: &mut TestAppContext) {
    let (mut window, counter) = with_counter(cx);
    window.notify(&counter);
    assert!(window.notify(&counter));
    let app = window.app.clone();
    app.update(window.cx, |app, cx| app.deliver("shell", "bump".into(), cx));
    window.cx.run_until_parked();
    let shown = counter.read_with(window.cx, |counter, _| counter.0.clone());
    assert!(shown.contains('2'), "{shown}");
    let live = window.cx.read_entity(&app, |app, _| app.drew_live());
    assert!(!live, "the frame after new props is a whole one");
}
