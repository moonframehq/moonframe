use {
    super::{Layers, Placeholder, Slot, refinement},
    gpui_kit::{
        AnyView, AppContext as _, Bounds, ContentMask, Context, Element as _, EmptyView,
        FontWeight, IntoElement, Refineable as _, Render, Styled as _, TestAppContext, TextStyle,
        WhiteSpace, Window, canvas, point, px, size,
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

#[test]
fn a_refinement_of_a_text_style_is_that_style_wherever_it_is_set() {
    let style = TextStyle {
        color: gpui_kit::red(),
        font_family: "Menlo".into(),
        font_size: px(13.).into(),
        font_weight: FontWeight::BOLD,
        background_color: Some(gpui_kit::blue()),
        white_space: WhiteSpace::Nowrap,
        line_clamp: Some(2),
        ..TextStyle::default()
    };
    // Over another style, as the window's view sets one around the layers.
    let around = TextStyle {
        font_family: "Helvetica".into(),
        underline: None,
        ..TextStyle::default()
    };
    assert_eq!(around.refined(refinement(&style)), style);
}

#[test]
fn a_frame_notes_its_host_views_afresh() {
    let layers = Layers::default();
    assert!(!layers.any());
    layers.slots.borrow_mut().push(Slot {
        view: placeholder_view(),
        spot: Rc::default(),
        focus: None,
    });
    assert!(layers.any());
    let frames = layers.frames();
    layers.begin();
    assert!(!layers.any());
    assert_eq!(layers.frames(), frames + 1);
}

#[test]
fn an_app_with_a_view_inside_it_is_not_replayed() {
    let layers = Layers::default();
    layers.slots.borrow_mut().push(Slot {
        view: placeholder_view(),
        spot: Rc::default(),
        focus: None,
    });
    assert!(layers.replayable());
    layers.drawn_inline();
    assert!(!layers.replayable());
    // The next frame notes it afresh.
    layers.begin();
    layers.slots.borrow_mut().push(Slot {
        view: placeholder_view(),
        spot: Rc::default(),
        focus: None,
    });
    assert!(layers.replayable());
}

#[gpui_kit::test]
fn placements_reuse_metadata_until_a_placeholder_prepaints(cx: &mut TestAppContext) {
    struct Probe(Rc<Cell<bool>>);

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let complete = self.0.clone();
            canvas(
                move |_, window, cx| {
                    let view = AnyView::from(cx.new(|_| EmptyView));
                    let layers = Layers::default();
                    let mut placeholders = Vec::new();
                    for _ in 0..2 {
                        let spot = Rc::new(RefCell::new(None));
                        layers.slots.borrow_mut().push(Slot {
                            view: view.clone(),
                            spot: spot.clone(),
                            focus: None,
                        });
                        placeholders.push(Placeholder {
                            spot,
                            placements: layers.placements.clone(),
                        });
                    }
                    let empty = layers.placements();
                    assert!(empty.is_empty());
                    let original = Bounds::new(point(px(10.), px(20.)), size(px(300.), px(200.)));
                    placeholders[0].prepaint(None, None, original, &mut (), window, cx);
                    let single = layers.placements();
                    assert_eq!(single.len(), 1);
                    assert!(!Rc::ptr_eq(&single, &empty));
                    assert!(Rc::ptr_eq(&single, &layers.placements()));

                    placeholders[1].prepaint(None, None, original, &mut (), window, cx);
                    let both = layers.placements();
                    assert_eq!(both.len(), placeholders.len());
                    assert_ne!(both[0].id, both[1].id);
                    assert_eq!(both[0].id, single[0].id);
                    assert!(!Rc::ptr_eq(&both, &single));

                    let moved = Bounds::new(point(px(40.), px(50.)), size(px(200.), px(100.)));
                    let style = TextStyle {
                        font_family: "Menlo".into(),
                        color: gpui_kit::red(),
                        ..TextStyle::default()
                    };
                    window.with_rem_size(Some(px(24.)), |window| {
                        window.with_text_style(Some(refinement(&style)), |window| {
                            window.with_content_mask(
                                Some(ContentMask { bounds: moved }),
                                |window| {
                                    placeholders[0].prepaint(
                                        None,
                                        None,
                                        moved,
                                        &mut (),
                                        window,
                                        cx,
                                    );
                                },
                            );
                        });
                    });
                    let updated = layers.placements();
                    assert!(!Rc::ptr_eq(&updated, &both));
                    assert_eq!(updated[0].spot.bounds, moved);
                    assert_eq!(updated[0].spot.text, style);
                    assert_eq!(updated[0].spot.rem, px(24.));
                    assert_eq!(updated[0].spot.mask.bounds, moved);
                    assert_eq!(both[0].spot.bounds, original);
                    assert!(Rc::ptr_eq(&updated, &layers.placements()));
                    layers.begin();
                    let cleared = layers.placements();
                    assert!(cleared.is_empty());
                    let _placeholder = layers.place(view, None);
                    assert!(!Rc::ptr_eq(&cleared, &layers.placements()));
                    complete.set(true);
                },
                |_, _, _, _| (),
            )
            .size_full()
        }
    }

    let complete = Rc::new(Cell::new(false));
    let (_, cx) = cx.add_window_view(|_, _| Probe(complete.clone()));
    cx.run_until_parked();
    assert!(complete.get());
}

/// A view to note, never drawn.
fn placeholder_view() -> AnyView {
    let cx = TestAppContext::single();
    cx.update(|cx| AnyView::from(cx.new(|_| EmptyView)))
}
