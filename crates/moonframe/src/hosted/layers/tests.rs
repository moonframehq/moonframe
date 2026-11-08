use {
    super::{Layers, Slot, refinement},
    gpui_kit::{AnyView, AppContext as _, FontWeight, Refineable as _, TextStyle, WhiteSpace, px},
    std::rc::Rc,
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
    });
    assert!(layers.any());
    let frames = layers.frames();
    layers.begin();
    assert!(!layers.any());
    assert_eq!(layers.frames(), frames + 1);
}

/// A view to note, never drawn.
fn placeholder_view() -> AnyView {
    let cx = gpui_kit::TestAppContext::single();
    cx.update(|cx| AnyView::from(cx.new(|_| gpui_kit::EmptyView)))
}
