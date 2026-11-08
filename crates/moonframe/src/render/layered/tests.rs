use {
    super::floats,
    moonframe_core::{Align, Direction, Frame, Justify, Look, Node, Offsets, Scroll, Stack},
};

fn column() -> Stack {
    Stack {
        look: Look::default(),
        direction: Direction::Column,
        frame: Frame {
            gap: None,
            padding: None,
            grow: false,
            width: None,
            height: None,
            surface: None,
            border: None,
            scroll: Scroll::No,
            follow: false,
            shrink: true,
            self_align: None,
            hug: false,
            max_width: None,
            max_height: None,
            float: None,
            area: None,
        },
        align: Align::Stretch,
        justify: Justify::Start,
        wrap: false,
        selected: false,
        scroll_to: None,
        action: None,
        extras: Default::default(),
        drag: false,
        key: None,
        pointer: None,
        children: Vec::new(),
    }
}

#[test]
fn only_what_is_placed_out_of_the_flow_floats() {
    assert!(!floats(&Node::Stack(column())));
    let mut floating = column();
    floating.frame.float = Some(Offsets::default());
    assert!(floats(&Node::Stack(floating)));
    assert!(!floats(&Node::Divider));
}
