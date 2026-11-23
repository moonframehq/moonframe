use {
    super::Layers,
    gpui_kit::{
        AnyView, AppContext as _, AvailableSpace, Context, Entity, IntoElement, ParentElement as _,
        Render, StyleRefinement, Styled as _, TestAppContext, Window, canvas, div, point, px,
        relative, size,
    },
    std::{cell::Cell, rc::Rc, time::Instant},
};

struct Surface {
    signal: Entity<usize>,
    shown: Rc<Cell<usize>>,
    frames: Rc<Cell<usize>>,
    sidebar: Entity<Sidebar>,
    read_in_paint: bool,
}

impl Render for Surface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames.set(self.frames.get() + 1);
        let value = (!self.read_in_paint).then(|| *self.signal.read(cx));
        let signal = self.signal.clone();
        let shown = self.shown.clone();
        div()
            .size_full()
            .child(AnyView::from(self.sidebar.clone()).cached(full()))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, _, cx| shown.set(value.unwrap_or_else(|| *signal.read(cx))),
                )
                .size_full(),
            )
    }
}

struct Sidebar(Rc<Cell<usize>>);

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.set(self.0.get() + 1);
        div().size_full()
    }
}

struct Region {
    surface: Entity<Surface>,
}

impl Render for Region {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.surface.clone())
    }
}

struct Shell {
    layers: Rc<Layers>,
    host: AnyView,
    copies: usize,
    frames: Rc<Cell<usize>>,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.frames.set(self.frames.get() + 1);
        self.layers.begin();
        div()
            .size_full()
            .children((0..self.copies).map(|_| self.layers.place(self.host.clone())))
    }
}

struct Root {
    shell: Entity<Shell>,
    layers: Rc<Layers>,
}

fn full() -> StyleRefinement {
    let mut style = StyleRefinement::default();
    style.size.width = Some(relative(1.).into());
    style.size.height = Some(relative(1.).into());
    style
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(AnyView::from(self.shell.clone()).cached(full()))
            .child(self.layers.element())
    }
}

#[gpui_kit::test]
fn external_signal_redraws_a_hosted_descendant_without_redrawing_the_shell(
    cx: &mut TestAppContext,
) {
    exercise(cx, true, false, Notification::Model, 120, 1);
}

#[gpui_kit::test]
fn external_signal_redraws_a_hosted_root(cx: &mut TestAppContext) {
    exercise(cx, false, false, Notification::Model, 120, 1);
}

#[gpui_kit::test]
fn external_signal_read_only_during_paint_redraws_a_hosted_descendant(cx: &mut TestAppContext) {
    exercise(cx, true, true, Notification::Model, 120, 1);
}

#[gpui_kit::test]
fn hosted_view_notification_preserves_the_shell_and_host_subview_caches(cx: &mut TestAppContext) {
    exercise(cx, true, false, Notification::Host, 120, 1);
}

#[gpui_kit::test]
fn descendant_notification_preserves_the_shell_and_host_subview_caches(cx: &mut TestAppContext) {
    exercise(cx, true, false, Notification::Descendant, 120, 1);
}

#[gpui_kit::test]
fn repeated_host_views_keep_separate_cached_subviews(cx: &mut TestAppContext) {
    exercise(cx, true, false, Notification::Model, 120, 8);
}

#[gpui_kit::test]
#[ignore]
fn hosted_signal_redraw_benchmark(cx: &mut TestAppContext) {
    exercise(cx, true, false, Notification::Model, 50_000, 1);
}

#[gpui_kit::test]
#[ignore]
fn multiple_hosted_signal_redraw_benchmark(cx: &mut TestAppContext) {
    exercise(cx, true, false, Notification::Model, 50_000, 8);
}

#[gpui_kit::test]
fn direct_host_layout_matches_wrapped_layout(cx: &mut TestAppContext) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            canvas(
                |_, window, cx| {
                    let mut elements = Vec::new();
                    for variant in 0..6 {
                        let mut bounds = Vec::new();
                        for wrapped in [true, false] {
                            let measured = Rc::new(std::cell::RefCell::new(None));
                            let record = measured.clone();
                            let child = canvas(
                                move |bounds, _, _| *record.borrow_mut() = Some(bounds),
                                |_, _, _, _| (),
                            )
                            .size_full();
                            let host = match variant {
                                0 => div().size_full(),
                                1 => div().w(px(100.)).h(px(70.)),
                                2 => div().w(relative(0.5)).h(relative(0.5)),
                                3 => div().size_full().m(px(10.)),
                                4 => div().size_full().p(px(10.)),
                                _ => div(),
                            }
                            .child(child);
                            let mut element = if wrapped {
                                div().size_full().child(host).into_any_element()
                            } else {
                                host.into_any_element()
                            };
                            element.layout_as_root(
                                size(
                                    AvailableSpace::Definite(px(400.)),
                                    AvailableSpace::Definite(px(300.)),
                                ),
                                window,
                                cx,
                            );
                            element.prepaint_at(point(px(20.), px(30.)), window, cx);
                            bounds.push(*measured.borrow());
                            elements.push(element);
                        }
                        assert_eq!(bounds[0], bounds[1], "layout variant {variant}");
                    }
                    elements
                },
                |_, mut elements, window, cx| {
                    for element in &mut elements {
                        element.paint(window, cx);
                    }
                },
            )
            .size_full()
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    cx.run_until_parked();
}

#[derive(Clone, Copy)]
enum Notification {
    // keep-sorted start
    Descendant,
    Host,
    Model,
    // keep-sorted end
}

fn exercise(
    cx: &mut TestAppContext,
    nested: bool,
    read_in_paint: bool,
    notification: Notification,
    iterations: usize,
    copies: usize,
) {
    let signal = cx.new(|_| 0usize);
    let shown = Rc::new(Cell::new(0));
    let frames = Rc::new(Cell::new(0));
    let shell_frames = Rc::new(Cell::new(0));
    let sidebar_frames = Rc::new(Cell::new(0));
    let sidebar = cx.new(|_| Sidebar(sidebar_frames.clone()));
    let surface = cx.new(|_| Surface {
        signal: signal.clone(),
        shown: shown.clone(),
        frames: frames.clone(),
        sidebar,
        read_in_paint,
    });
    let host = if nested {
        AnyView::from(cx.new(|_| Region {
            surface: surface.clone(),
        }))
    } else {
        AnyView::from(surface.clone())
    };
    let host_id = host.entity_id();
    let layers = Rc::new(Layers::default());
    let shell = cx.new(|_| Shell {
        layers: layers.clone(),
        host,
        copies,
        frames: shell_frames.clone(),
    });
    let (root, cx) = cx.add_window_view(|_, _| Root { shell, layers });
    root.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let initial_shell_frames = shell_frames.get();
    let initial_frames = frames.get();
    let initial_sidebar_frames = sidebar_frames.get();
    assert!(initial_frames > 0);
    assert!(initial_sidebar_frames > 0);
    let started = Instant::now();
    for step in 1..=iterations {
        signal.update(cx, |value, cx| {
            *value = step;
            if matches!(notification, Notification::Model) {
                cx.notify();
            }
        });
        match notification {
            Notification::Model => {},
            Notification::Host => cx.update(|_, cx| cx.notify(host_id)),
            Notification::Descendant => surface.update(cx, |_, cx| cx.notify()),
        }
        cx.run_until_parked();
        assert_eq!(shown.get(), step, "the changed model was painted");
        assert_eq!(frames.get(), initial_frames + step * copies);
        assert_eq!(shell_frames.get(), initial_shell_frames);
        assert_eq!(sidebar_frames.get(), initial_sidebar_frames);
    }
    eprintln!(
        "{iterations} redraws of {copies} hosted views in {:?}; shell redraws: 0; cached sidebar redraws: 0",
        started.elapsed()
    );
}
