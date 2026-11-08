//! End-to-end tests: the real app in a headless window, driven as a user
//! drives it, by clicks, keys and the wheel through GPUI's own hit testing
//! and key dispatch. Each test runs a small shell over the real components;
//! the shell logs what reaches its `update`, and the test reads the log.
//! Elements are found by `debug_selector`: a row or column's first class.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod host_views;
mod regressions;

use {
    crate::{
        app::{MoonframeApp, Parts},
        backends::Backends,
        hosted::Registry,
    },
    gpui_kit::{
        AppContext as _, Bounds, Entity, Modifiers, Pixels, Point, ScrollDelta, ScrollEvent,
        TestAppContext, TouchPhase, VisualTestContext, component::Root, point, px,
    },
    moonframe_core::Workspace,
    moonframe_packages::paths::Folders,
    moonframe_providers::Natives,
    std::{path::Path, sync::Arc},
    tempfile::TempDir,
};

/// What every shell shares: `log(...)` keeps a line, and `shell { ... }`
/// shows the lines where the test can read them.
const PRELUDE: &str = r#"
local function logged(state, line)
	state.log = state.log .. "|" .. line
end
local function shell(state, body)
	return ui.column { grow = true, ui.text { "log:" .. state.log }, body }
end
"#;

/// A window on a shell, and the folder it runs from.
struct Window<'a> {
    cx: &'a mut VisualTestContext,
    app: Entity<MoonframeApp>,
    _folder: TempDir,
}

/// An app folder holding `main`, with the real components and stylesheet.
fn folder(main: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    std::fs::write(dir.path().join("main.luau"), format!("{PRELUDE}{main}")).unwrap();
    std::fs::copy(app.join("style.luau"), dir.path().join("style.luau")).unwrap();
    std::os::unix::fs::symlink(app.join("components"), dir.path().join("components")).unwrap();
    dir
}

fn open<'a>(cx: &'a mut TestAppContext, main: &str) -> Window<'a> {
    open_in(cx, folder(main))
}

fn open_in(cx: &mut TestAppContext, dir: TempDir) -> Window<'_> {
    open_with(cx, dir, Registry::default())
}

/// A window on the app in `dir`, whose host app registers `views`.
fn open_with(cx: &mut TestAppContext, dir: TempDir, views: Registry) -> Window<'_> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::actions::bind_keys(cx);
    });
    let workspace = Workspace::open(dir.path(), false);
    let chrome = workspace.chrome();
    let folders = Folders {
        builtin: dir.path().to_owned(),
        host: None,
        user: None,
        profile: None,
        bundled: None,
    };
    let backends = Arc::new(Backends::for_app("e2e"));
    let mut app = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            MoonframeApp::new(
                workspace,
                chrome,
                folders,
                backends,
                Parts {
                    natives: Natives::default(),
                    views,
                },
                window,
                cx,
            )
        });
        app = Some(view.clone());
        let view = cx.new(|_| crate::app::MainWindow::new(view));
        Root::new(view, window, cx)
    });
    let mut window = Window {
        cx,
        app: app.unwrap(),
        _folder: dir,
    };
    window.settle();
    window
}

impl Window<'_> {
    /// Lets parses land, messages arrive and frames draw.
    fn settle(&mut self) {
        for _ in 0..4 {
            self.cx.run_until_parked();
            self.cx.update(|window, cx| {
                window.simulate_next_frame(cx);
                window.refresh();
            });
            self.cx.run_until_parked();
        }
    }

    /// What the shell's view says, as a string to search.
    fn view(&mut self) -> String {
        let app = self.app.clone();
        self.cx.read_entity(&app, |app, _| {
            format!(
                "{:?}",
                app.workspace.slot("shell").and_then(|slot| slot.view())
            )
        })
    }

    fn log(&mut self) -> String {
        let view = self.view();
        let start = view.find("log:").expect("the shell's log") + 4;
        let rest = &view[start..];
        rest[..rest.find('"').unwrap_or(rest.len())].to_owned()
    }

    fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("nothing drawn as `{selector}`"))
    }

    fn shows(&mut self, selector: &'static str) -> bool {
        self.cx.debug_bounds(selector).is_some()
    }

    fn click_at(&mut self, position: Point<Pixels>) {
        self.cx
            .simulate_mouse_move(position, None, Modifiers::none());
        self.cx.simulate_click(position, Modifiers::none());
        self.settle();
    }

    fn click(&mut self, selector: &'static str) {
        let center = self.bounds(selector).center();
        self.click_at(center);
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.settle();
    }

    fn typed(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.settle();
    }

    fn wheel(&mut self, selector: &'static str, down: f32) {
        let position = self.bounds(selector).center();
        self.cx.simulate_event(ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(-down))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: Default::default(),
        });
        self.settle();
    }
}

/// A document long enough to scroll, with a heading every few lines.
const LONG: &str = r##"
local parts = {}
for i = 1, 40 do
	table.insert(parts, `## Section {i}\n\nA paragraph about the ledger number {i}.\n`)
end
local LONG = table.concat(parts, "\n")
"##;

/// A shell around a scrolling document whose messages are logged.
fn reader(top: &str, end: &str) -> String {
    format!(
        r##"{LONG}
local DOC = [==[{top}]==] .. "\n\n" .. LONG .. "\n\n" .. [==[{end}]==]
return mod {{
	init = function(previous) return previous or {{ log = "" }} end,
	update = function(state, msg)
		if type(msg) == "table" and msg.msg == "reading" then
			logged(state, "section=" .. tostring(msg.heading))
		elseif type(msg) == "table" and msg.msg == "link" then
			logged(state, "link=" .. msg.url)
		end
	end,
	view = function(state)
		return shell(state, ui.markdown {{
			text = DOC, grow = true, scroll = true,
			section = "reading", link = "link",
		}})
	end,
}}
"##
    )
}

/// The last section the log reached.
fn section(log: &str) -> usize {
    log.rsplit("section=")
        .next()
        .and_then(|rest| rest.split('|').next())
        .and_then(|number| number.parse().ok())
        .unwrap_or(0)
}

#[gpui_kit::test]
fn wheel_scrolling_reports_the_section_being_read(cx: &mut TestAppContext) {
    let mut window = open(cx, &reader("", ""));
    assert!(section(&window.log()) <= 1, "{}", window.log());
    window.wheel("markdown", 1500.0);
    let reached = section(&window.log());
    assert!(reached > 3, "{}", window.log());
    window.wheel("markdown", 1500.0);
    assert!(section(&window.log()) > reached, "{}", window.log());
}

#[gpui_kit::test]
fn a_footnote_mark_scrolls_to_its_note(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        &reader("See this[^far].", "[^far]: The note at the end."),
    );
    window.click("markdown-footnote");
    // The note ends the document, so the view stops short of putting it on top.
    assert!(section(&window.log()) > 20, "{}", window.log());
}

#[gpui_kit::test]
fn an_anchor_link_scrolls_to_its_heading(cx: &mut TestAppContext) {
    let mut window = open(cx, &reader("[jump](#section-20)", ""));
    let frame = window.bounds("markdown");
    window.click_at(frame.origin + point(px(12.), px(10.)));
    let log = window.log();
    assert!(!log.contains("link="), "an anchor is not the mod's: {log}");
    assert_eq!(section(&log), 20, "{log}");
}

#[gpui_kit::test]
fn an_outline_jumps_to_its_heading_and_can_again(cx: &mut TestAppContext) {
    let main = format!(
        r#"{LONG}
local outline = use "outline"
return mod {{
	init = function(previous)
		return previous or {{ log = "", headings = {{}} :: any, seq = 0, reveal = nil :: any }}
	end,
	update = function(state, msg)
		if msg.msg == "headings" then
			state.headings = table.move(msg.headings, 1, 15, 1, {{}})
		elseif msg.msg == "reading" then
			logged(state, "section=" .. tostring(msg.heading))
		elseif msg.heading then
			state.seq += 1
			state.reveal = {{ at = state.headings[msg.heading].at, seq = state.seq }}
		end
	end,
	view = function(state)
		return shell(state, ui.row {{ grow = true,
			ui.column {{ width = 200, outline {{ headings = state.headings }} }},
			ui.markdown {{ text = LONG, grow = true, scroll = true, outline = "headings",
				section = "reading", reveal = state.reveal }},
		}})
	end,
}}"#
    );
    let mut window = open(cx, &main);
    // The last row drawn takes the selector: heading 15.
    window.click("outline-row");
    assert_eq!(section(&window.log()), 15, "{}", window.log());
    window.wheel("markdown", 1500.0);
    assert!(section(&window.log()) > 15, "{}", window.log());
    window.click("outline-row");
    assert_eq!(section(&window.log()), 15, "{}", window.log());
}

#[gpui_kit::test]
fn a_link_goes_to_the_mod_and_opens_nothing(cx: &mut TestAppContext) {
    let mut window = open(cx, &reader("[out](https://example.com/page)", ""));
    let frame = window.bounds("markdown");
    window.click_at(frame.origin + point(px(10.), px(10.)));
    assert!(
        window.log().contains("link=https://example.com/page"),
        "{}",
        window.log()
    );
    assert_eq!(window.cx.opened_url(), None);
}

#[gpui_kit::test]
fn the_copy_button_copies_a_code_block(cx: &mut TestAppContext) {
    let main = r#"
return mod {
	init = function() return { log = "" } end,
	view = function(state)
		return shell(state, ui.markdown { "```rust\nlet x = 1;\n```\n", grow = true })
	end,
}"#;
    let mut window = open(cx, main);
    window.click("markdown-copy");
    let copied = window.cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref().map(str::trim_end), Some("let x = 1;"));
}

fn pictures(document: &str) -> TempDir {
    let main = format!(
        r#"
return mod {{
	init = function() return {{ log = "" }} end,
	update = function(state, msg)
		logged(state, `picture={{msg.src}},{{msg.alt}}`)
	end,
	view = function(state)
		return shell(state, ui.markdown {{ [==[{document}]==], images = true, image = "open", grow = true }})
	end,
}}"#
    );
    let dir = folder(&main);
    let picture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/components/demos/scan.jpg");
    std::fs::copy(picture, dir.path().join("pic.jpg")).unwrap();
    dir
}

#[gpui_kit::test]
fn a_click_on_a_picture_tells_the_mod(cx: &mut TestAppContext) {
    let mut window = open_in(cx, pictures("![A scan](pic.jpg)"));
    window.click("markdown-picture");
    assert!(
        window.log().contains("picture=pic.jpg,A scan"),
        "{}",
        window.log()
    );
}

#[gpui_kit::test]
fn a_picture_from_the_web_is_never_shown(cx: &mut TestAppContext) {
    let mut window = open_in(cx, pictures("![Remote](https://example.com/a.png)"));
    assert!(!window.shows("markdown-picture"));
}

const RECORDER: &str = r#"
local shortcut_field = use "shortcut-field"
return mod {
	init = function(previous) return previous or { log = "", recording = false } end,
	update = function(state, msg)
		if msg.shortcut == "record" then
			state.recording = true
		elseif msg.shortcut == "cancel" then
			state.recording = false
			logged(state, "stopped")
		elseif msg.shortcut == "set" then
			state.recording = false
			logged(state, "keys=" .. msg.keys)
		end
	end,
	view = function(state)
		return shell(state, shortcut_field { recording = state.recording })
	end,
}"#;

#[gpui_kit::test]
fn a_shortcut_field_records_the_next_chord(cx: &mut TestAppContext) {
    let mut window = open(cx, RECORDER);
    window.click("shortcut-field");
    window.keys("cmd-shift-p");
    assert!(
        window.log().contains("keys=shift-cmd-p"),
        "{}",
        window.log()
    );
}

#[gpui_kit::test]
fn a_shortcut_field_ignores_plain_typing_and_gives_up_on_escape(cx: &mut TestAppContext) {
    let mut window = open(cx, RECORDER);
    window.click("shortcut-field");
    window.keys("a");
    assert!(!window.log().contains("keys="), "{}", window.log());
    window.keys("escape");
    assert!(window.log().contains("stopped"), "{}", window.log());
    assert!(!window.log().contains("keys="), "{}", window.log());
}

#[gpui_kit::test]
fn the_selection_toolbar_formats_what_is_selected(cx: &mut TestAppContext) {
    let main = r#"
local selection_toolbar = use "selection-toolbar"
return mod {
	init = function(previous) return previous or { log = "", selected = 0 } end,
	update = function(state, msg)
		if msg.msg == "moved" then
			state.selected = msg.selected
		elseif msg.msg == "type" then
			logged(state, "text=" .. msg.value)
		end
	end,
	view = function(state)
		return shell(state, ui.column { style = { padding = { 60, 0, 0, 0 } },
			selection_toolbar { field = "notes", selected = state.selected, tools = { "bold" },
				ui.input { id = "notes", value = "words", focus = true, select = "moved", change = "type" },
			},
		})
	end,
}"#;
    let mut window = open(cx, main);
    assert!(!window.shows("selection-toolbar-tool"));
    window.keys(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    window.click("selection-toolbar-tool");
    assert!(window.log().contains("text=**words**"), "{}", window.log());
}

#[gpui_kit::test]
fn a_lightbox_steps_through_pictures_and_closes(cx: &mut TestAppContext) {
    let main = r#"
local lightbox = use "lightbox"
local PICTURES = { { src = "demos/scan.jpg", title = "One" }, { src = "demos/scan.jpg", title = "Two" } }
return mod {
	init = function(previous) return previous or { log = "", open = false, index = 1 } end,
	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
			logged(state, "closed")
		elseif type(msg) == "table" and msg.picture then
			state.index = msg.picture
			logged(state, "picture=" .. msg.picture)
		end
	end,
	view = function(state)
		return shell(state, ui.column {
			ui.row { class = "opener", msg = "open", ui.text "Open" },
			lightbox { open = state.open, index = state.index, pictures = PICTURES, dismiss = "close" },
		})
	end,
}"#;
    let mut window = open(cx, main);
    assert!(!window.shows("lightbox"));
    window.click("opener");
    assert!(window.shows("lightbox"));
    window.keys("right");
    assert!(window.log().contains("picture=2"), "{}", window.log());
    window.keys("right");
    assert!(
        window.log().ends_with("picture=1"),
        "wraps round: {}",
        window.log()
    );
    window.keys("escape");
    assert!(window.log().contains("closed"), "{}", window.log());
    assert!(!window.shows("lightbox"));
}

#[gpui_kit::test]
fn the_find_bar_counts_and_steps_with_the_keyboard(cx: &mut TestAppContext) {
    let main = r#"
local find_bar = use "find-bar"
return mod {
	init = function(previous) return previous or { log = "", query = "", match = 1, count = 0 } end,
	update = function(state, msg)
		if msg.find == "next" or msg.find == "previous" then
			local by = if msg.find == "next" then 0 else -2
			state.match = (state.match - 1 + by + 1) % math.max(state.count, 1) + 1
			logged(state, "match=" .. state.match)
		elseif msg.find == "close" then
			logged(state, "closed")
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "found" then
			state.count = msg.count
			logged(state, "count=" .. msg.count)
		end
	end,
	view = function(state)
		return shell(state, ui.column { grow = true,
			find_bar { value = state.query, count = state.count, index = state.match, focus = true,
				change = "query" },
			ui.markdown { "a ledger, a ledger and a Ledger", grow = true,
				find = state.query, find_index = state.match, found = "found" },
		})
	end,
}"#;
    let mut window = open(cx, main);
    window.typed("ledger");
    assert!(window.log().contains("count=3"), "{}", window.log());
    window.keys("enter");
    assert!(window.log().ends_with("match=2"), "{}", window.log());
    window.keys("shift-enter");
    assert!(window.log().ends_with("match=1"), "{}", window.log());
    window.keys("escape");
    assert!(window.log().ends_with("closed"), "{}", window.log());
}

#[gpui_kit::test]
fn a_slash_menu_hands_its_pick_to_the_mod(cx: &mut TestAppContext) {
    let main = r#"
local mention_field = use "mention-field"
return mod {
	init = function(previous) return previous or { log = "", text = "", caret = 0 } end,
	update = function(state, msg)
		if msg.msg == "type" then
			state.text, state.caret = msg.value, msg.caret
		elseif msg.mention == "choose" then
			logged(state, "item=" .. tostring(msg.item))
			return state, { { to = "app:edit", msg = msg.edit } }
		end
	end,
	view = function(state)
		return shell(state, mention_field {
			id = "note", value = state.text, caret = state.caret, trigger = "/", heading = "",
			people = { { id = "file", name = "File", icon = "file", insert = "" } },
			change = "type", focus = true,
		})
	end,
}"#;
    let mut window = open(cx, main);
    window.typed("/fi");
    window.keys("enter");
    assert!(window.log().contains("item=file"), "{}", window.log());
}
