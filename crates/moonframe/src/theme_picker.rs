//! View > Themes: every theme in a window of its own, to preview and pick.
//! Pointing at a theme, or moving to it with the arrow keys, shows it on the
//! whole app; a click or Enter picks it and saves it. Leaving the list or
//! closing the window goes back to the picked theme.

use {
    crate::{
        actions::Dismiss,
        app::MoonframeApp,
        themes::{self, Scheme},
    },
    gpui_kit::{
        component::{
            ActiveTheme as _, Root,
            input::{Input, InputEvent, InputState},
        },
        prelude::FluentBuilder as _,
        *,
    },
};

const WIDTH: f32 = 460.;
const HEIGHT: f32 = 640.;
const ROW_HEIGHT: f32 = 40.;

pub(crate) struct ThemePicker {
    app: Entity<MoonframeApp>,
    search: Entity<InputState>,
    /// The names matching the search, in catalog order.
    shown: Vec<&'static str>,
    selected: usize,
    scroll: ScrollHandle,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ThemePicker {
    /// Opens the window; the caller keeps the handle to raise it.
    pub(crate) fn open(app: Entity<MoonframeApp>, cx: &mut App) -> Option<WindowHandle<Root>> {
        let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
        let options = WindowOptions {
            focus: !crate::background(),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(320.), px(300.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Themes".into()),
                ..Default::default()
            }),
            app_id: Some(crate::app_id()),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| Self::new(app, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .inspect_err(|error| tracing::warn!("cannot open the themes window: {error}"))
        .ok()
    }

    fn new(app: Entity<MoonframeApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes…"));
        search.update(cx, |search, cx| search.focus(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, _, cx| match event {
                InputEvent::Change => this.filter(cx),
                InputEvent::PressEnter { .. } => this.pick_selected(cx),
                _ => {},
            }),
            cx.observe(&app, |_, _, cx| cx.notify()),
            // However the window closes, the app goes back to the picked theme.
            cx.on_release(|this, cx| {
                this.app.update(cx, |app, cx| app.preview_theme(None, cx));
            }),
        ];
        let current = app.read(cx).theme_name();
        let shown: Vec<&'static str> = themes::names().collect();
        let selected = shown.iter().position(|name| *name == current).unwrap_or(0);
        let scroll = ScrollHandle::new();
        scroll.scroll_to_item(selected);
        Self {
            app,
            search,
            shown,
            selected,
            scroll,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.shown = themes::names()
            .filter(|name| name.to_lowercase().contains(&query))
            .collect();
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    /// Moves the selection by `step` rows, wrapping, and shows that theme.
    fn step(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.shown.len();
        if count == 0 {
            return;
        }
        self.selected = (self.selected as isize + step).rem_euclid(count as isize) as usize;
        self.scroll.scroll_to_item(self.selected);
        self.preview(Some(self.shown[self.selected]), cx);
    }

    fn preview(&mut self, name: Option<&'static str>, cx: &mut Context<Self>) {
        self.app.update(cx, |app, cx| app.preview_theme(name, cx));
        cx.notify();
    }

    fn pick(&mut self, name: &str, cx: &mut Context<Self>) {
        self.app.update(cx, |app, cx| app.pick_theme(name, cx));
        cx.notify();
    }

    fn pick_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(name) = self.shown.get(self.selected).copied() {
            self.pick(name, cx);
        }
    }

    fn row(
        &self,
        index: usize,
        name: &'static str,
        current: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        div()
            .id(index)
            .h(px(ROW_HEIGHT))
            .px_3()
            .mx_2()
            .flex()
            .items_center()
            .gap_3()
            .rounded(theme.radius)
            .cursor_pointer()
            .when(index == self.selected, |row| row.bg(theme.list_active))
            .hover(|row| row.bg(theme.list_hover))
            .child(swatch(themes::find(name)))
            .child(div().flex_1().min_w_0().truncate().child(name))
            .when(current, |row| {
                row.child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Current"),
                )
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.selected = index;
                    this.preview(Some(name), cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.pick(name, cx)))
    }
}

/// A theme in miniature: its text on its background, and its colors.
fn swatch(scheme: Option<&Scheme>) -> Div {
    let chip = || {
        div()
            .w(px(34.))
            .h(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
    };
    let Some(scheme) = scheme else {
        // The stock look: light and dark.
        return div().flex().gap_1().children([
            chip()
                .bg(rgb(0xffffff))
                .border_color(rgb(0xd4d4d8))
                .text_color(rgb(0x18181b))
                .child("Aa"),
            chip()
                .bg(rgb(0x0a0a0a))
                .border_color(rgb(0x27272a))
                .text_color(rgb(0xfafafa))
                .child("Aa"),
        ]);
    };
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            chip()
                .bg(rgb(scheme.background))
                .border_color(rgb(themes::mix(scheme.background, scheme.foreground, 20)))
                .text_color(rgb(scheme.foreground))
                .child("Aa"),
        )
        .child(
            div().flex().gap(px(3.)).children(
                scheme.ansi[1..7]
                    .iter()
                    .map(|&color| div().size(px(8.)).rounded_full().bg(rgb(color))),
            ),
        )
}

impl Render for ThemePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.app.read(cx).theme_name();
        let total = themes::names().count();
        let rows: Vec<_> = self
            .shown
            .iter()
            .enumerate()
            .map(|(index, &name)| self.row(index, name, name == current, cx))
            .collect();
        let theme = cx.theme();
        div()
            .key_context("ThemePicker")
            .track_focus(&self.focus)
            .on_action(|_: &Dismiss, window, _| window.remove_window())
            // Before the search field takes the keys.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => this.step(-1, cx),
                    "down" => this.step(1, cx),
                    "escape" => window.remove_window(),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .child(
                div()
                    .flex_none()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(Input::new(&self.search))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} of {total} themes", self.shown.len())),
                    ),
            )
            .child(
                div()
                    .id("themes")
                    .flex_1()
                    .min_h_0()
                    .py_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    // Leaving the list shows the picked theme again.
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered {
                            this.preview(None, cx);
                        }
                    }))
                    .children(rows)
                    .when(self.shown.is_empty(), |list| {
                        list.child(
                            div()
                                .p_4()
                                .text_color(theme.muted_foreground)
                                .child("No matching themes."),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Point or ↑↓ to preview · Click or Enter to apply · Esc to close"),
            )
    }
}
