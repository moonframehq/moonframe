//! A `ui.browser`: a box styled by the `browser` rule, which the page's
//! native view fills (see `crate::browser`). Previews and platforms without
//! a web view show its address instead.

use {
    super::{Cx, Draw, Painter, Place, problem, styling::*},
    crate::browser::{Page, SUPPORTED, snap},
    gpui_kit::{
        component::{ActiveTheme as _, Sizable as _, button::Button},
        *,
    },
    moonframe_core::{Browser, Part},
    std::rc::Rc,
};

/// The least room a page asks for.
const MIN_HEIGHT: f32 = 120.;

impl<'w> Draw<'w> for Browser {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        if !cfg!(feature = "browser") {
            return problem("this build has no browser support".to_owned(), cx);
        }
        let look = painter.look(Part::Browser, &self.look, false, place);
        let theme = cx.theme();
        let frame = div()
            .id(place.id())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h(px(MIN_HEIGHT))
            .overflow_hidden()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        let address = self.url.clone().unwrap_or_else(|| "about:blank".to_owned());
        let browsers = match painter.browsers.as_deref_mut() {
            Some(browsers) if SUPPORTED && !painter.preview => browsers,
            // A preview shows what it is; it starts no page.
            _ if painter.preview => return placeholder(frame, address, false, cx),
            _ => return placeholder(frame, address, self.url.is_some(), cx),
        };
        match browsers.open(place.owner, self) {
            Ok(page) => match page.failed() {
                Some(error) => problem(format!("browser `{}`: {error}", self.id), cx),
                None => frame.child(PageElement { page }).into_any_element(),
            },
            Err(error) => problem(format!("browser `{}`: {error}", self.id), cx),
        }
    }
}

/// The address where the page would be, and on a platform without a web
/// view a button that opens it in the system browser.
fn placeholder(frame: Stateful<Div>, address: String, open: bool, cx: Cx) -> AnyElement {
    // Only a web address goes to the system browser.
    let open = open && moonframe_core::is_remote_url(&address);
    let theme = cx.theme();
    let note = if SUPPORTED {
        "a web page goes here"
    } else {
        "this platform has no web view"
    };
    let frame = frame
        .items_center()
        .justify_center()
        .gap_2()
        .p_4()
        .text_color(theme.muted_foreground)
        .child(div().text_sm().child(note))
        .child(div().text_xs().font_family("Menlo").child(address.clone()));
    if !open {
        return frame.into_any_element();
    }
    frame
        .child(
            Button::new("browser-open")
                .small()
                .outline()
                .label("Open in browser")
                .on_click(move |_, _, cx| cx.open_url(&address)),
        )
        .into_any_element()
}

/// Fills its space; the page's view follows wherever that is.
struct PageElement {
    page: Rc<Page>,
}

impl IntoElement for PageElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PageElement {
    type PrepaintState = ();
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            size: Size::full(),
            flex_grow: 1.,
            flex_shrink: 1.,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scale = window.scale_factor();
        let bounds = snap(bounds, scale);
        // Only what shows of it: a pane sliding in, or a scrolled box,
        // clips it.
        let visible = snap(bounds.intersect(&window.content_mask().bounds), scale);
        self.page.laid_out(bounds, visible, window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}
