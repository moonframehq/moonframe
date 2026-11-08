//! A host app's view (`ui.view`), drawn over the app in a layer of its own:
//! drawing again by itself leaves the app as it was painted, and it follows
//! its place, keys, floats and the inspector as if it were inside the app.

use {
    super::*,
    crate::hosted::HostView,
    gpui_kit::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
        Styled as _, component::ActiveTheme as _, div, size,
    },
    moonframe_core::Data,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

/// A view that shows the props it was last given, and counts its frames.
struct Counter {
    props: String,
    frames: Rc<Cell<usize>>,
    focus: FocusHandle,
}

impl Render for Counter {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut Context<Self>) -> impl IntoElement {
        self.frames.set(self.frames.get() + 1);
        div()
            .id("counter")
            .debug_selector(|| "counter".into())
            .key_context("Counter")
            .track_focus(&self.focus)
            .size_full()
            // As a terminal does: what is under it takes no clicks.
            .occlude()
            .child(self.props.clone())
    }
}

impl HostView for Counter {
    fn set_props(&mut self, props: &Data, _: &mut Context<Self>) {
        self.props = format!("{props:?}");
    }
}

/// A shell showing the `counter` view in one of two places, under a float.
/// `bump` changes its props, `wide` the size of its place, `move` the place,
/// and `hide` hides it; anything else is logged.
const SHELL: &str = r#"
return mod {
	init = function() return { log = "", step = 1, wide = false, right = false, shown = true } end,
	update = function(state, msg)
		if msg == "bump" then state.step += 1
		elseif msg == "wide" then state.wide = not state.wide
		elseif msg == "move" then state.right = not state.right
		elseif msg == "hide" then state.shown = not state.shown
		else logged(state, msg) end
	end,
	view = function(state)
		local function place(here)
			if state.shown and here then
				return ui.column { class = "slot", grow = true,
					ui.view { name = "counter", props = { step = state.step } } }
			end
			return ui.text "empty"
		end
		return shell(state, ui.row { grow = true,
			ui.column { class = "left", width = if state.wide then 300 else 150,
				ui.text "one", ui.text "two", place(not state.right) },
			ui.column { class = "right", grow = true, place(state.right) },
			ui.row { class = "badge", float = { at = "bottom-left", inset = 0 },
				ui.button { "Badge", msg = "badge" } },
		})
	end,
}"#;

/// A window on [`SHELL`], the view once made, and its frames.
struct Hosting<'a> {
    window: Window<'a>,
    counter: Entity<Counter>,
    frames: Rc<Cell<usize>>,
    made: Rc<Cell<usize>>,
}

fn with_counter(cx: &mut TestAppContext) -> Hosting<'_> {
    let made = Rc::new(RefCell::new(None));
    let frames = Rc::new(Cell::new(0));
    let count = Rc::new(Cell::new(0));
    let mut views = Registry::default();
    {
        let (made, frames, count) = (made.clone(), frames.clone(), count.clone());
        views.insert("counter", move |init, _, cx| {
            *made.borrow_mut() = Some(cx.entity());
            count.set(count.get() + 1);
            Counter {
                props: format!("{:?}", init.props()),
                frames: frames.clone(),
                focus: cx.focus_handle(),
            }
        });
    }
    let window = open_with(cx, folder(SHELL), views);
    let counter = made.borrow().clone().expect("the view is made");
    Hosting {
        window,
        counter,
        frames,
        made: count,
    }
}

impl Hosting<'_> {
    /// How many frames drew the app.
    fn app_frames(&mut self) -> usize {
        let app = self.window.app.clone();
        self.window
            .cx
            .read_entity(&app, |app, _| app.views.layers().frames())
    }

    /// Sends `msg` to the shell, as a click would, and lets the frame draw.
    fn send(&mut self, msg: &str) {
        let app = self.window.app.clone();
        app.update(self.window.cx, |app, cx| {
            app.deliver("shell", msg.into(), cx)
        });
        self.window.cx.run_until_parked();
    }

    /// Whether the view lies exactly over its place, as last drawn.
    fn in_place(&mut self) -> bool {
        self.window.bounds("counter") == self.window.bounds("slot")
    }
}

#[gpui_kit::test]
fn a_host_view_drawing_by_itself_leaves_the_app_as_it_was(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    for step in 0..4 {
        if step == 2 {
            // Replayed frames keep the places the last drawn one noted.
            hosting.send("wide");
            hosting.window.cx.simulate_resize(size(px(900.), px(500.)));
            hosting.window.cx.run_until_parked();
        }
        let (app, view) = (hosting.app_frames(), hosting.frames.get());
        hosting
            .counter
            .update(hosting.window.cx, |_, cx| cx.notify());
        hosting.window.cx.run_until_parked();
        assert_eq!(hosting.frames.get(), view + 1, "the view drew again");
        assert_eq!(hosting.app_frames(), app, "the app was replayed");
        assert!(hosting.in_place());
    }
}

#[gpui_kit::test]
fn a_host_view_follows_its_place_in_the_same_frame(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    assert!(hosting.in_place());
    let narrow = hosting.window.bounds("slot");
    // Each of these draws once; the view lands where its place now is.
    hosting.send("wide");
    assert_ne!(hosting.window.bounds("slot"), narrow);
    assert!(hosting.in_place(), "a new layout");
    hosting.window.cx.simulate_resize(size(px(900.), px(500.)));
    hosting.window.cx.run_until_parked();
    assert!(hosting.in_place(), "a resized window");
    hosting.send("move");
    assert!(
        hosting
            .window
            .bounds("right")
            .contains(&hosting.window.bounds("counter").center())
    );
    assert!(hosting.in_place(), "another place");
    hosting.send("hide");
    assert!(!hosting.window.shows("counter"), "hidden by its mod");
    hosting.send("hide");
    assert!(hosting.in_place(), "shown again");
    assert_eq!(hosting.made.get(), 1, "one view, wherever it goes");
}

#[gpui_kit::test]
fn new_props_reach_the_view(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    hosting.send("bump");
    hosting.window.settle();
    let shown = hosting
        .counter
        .read_with(hosting.window.cx, |counter, _| counter.props.clone());
    assert!(shown.contains('2'), "{shown}");
}

#[gpui_kit::test]
fn what_floats_over_a_host_view_stays_over_it(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    let badge = hosting.window.bounds("badge");
    assert!(hosting.window.bounds("counter").contains(&badge.center()));
    hosting.window.click("badge");
    assert!(
        hosting.window.log().contains("badge"),
        "{}",
        hosting.window.log()
    );
}

#[gpui_kit::test]
fn keys_typed_in_a_host_view_reach_the_window(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    let focus = hosting
        .counter
        .read_with(hosting.window.cx, |counter, _| counter.focus.clone());
    hosting
        .window
        .cx
        .update(|window, cx| focus.focus(window, cx));
    hosting.window.settle();
    let contexts = hosting.window.cx.update(|window, _| window.context_stack());
    let names: Vec<String> = contexts
        .iter()
        .map(|context| format!("{context:?}"))
        .collect();
    let app = names.iter().position(|name| name.contains("MoonframeApp"));
    let view = names.iter().position(|name| name.contains("Counter"));
    assert!(app.is_some() && app < view, "{names:?}");
    let dark = |window: &mut Window| window.cx.update(|_, cx| cx.theme().is_dark());
    let before = dark(&mut hosting.window);
    hosting.window.keys("cmd-shift-l");
    assert_ne!(dark(&mut hosting.window), before, "the app's shortcut ran");
}

#[gpui_kit::test]
fn the_inspector_picks_a_host_view(cx: &mut TestAppContext) {
    let mut hosting = with_counter(cx);
    let app = hosting.window.app.clone();
    app.update(hosting.window.cx, |app, cx| app.toggle_inspect(cx));
    hosting.window.settle();
    hosting.window.click("counter");
    let picked = app.read_with(hosting.window.cx, |app, _| {
        app.inspector
            .selected
            .as_ref()
            .map(|placed| placed.crumb.label.clone())
    });
    assert!(
        picked
            .as_deref()
            .is_some_and(|label| label.starts_with("view")),
        "{picked:?}"
    );
}
