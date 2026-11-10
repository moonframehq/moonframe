//! A `ui.input`: the host's editor for the field, in a frame the stylesheet
//! styles (`input`, `input:hover`, `input:focus`, `input:disabled`). The
//! engine paints only the text, the placeholder (`input-placeholder`), the
//! selection (`input-selection`) and the caret (`input-caret`).

use {
    super::{
        Cx, Draw, Painter, Place,
        styling::{LookExt as _, StyleExt as _, text_pixels},
    },
    crate::{app::MoonframeApp, color::ThemeColor as _, input::Inputs},
    gpui_kit::{
        base::input::InputEditorStyle,
        component::{ActiveTheme as _, Theme},
        *,
    },
    moonframe_core::{Border, Input, Part, Resolved, States, Style},
};

/// Line height as a multiple of the text size.
const LINE: f32 = 1.4;
/// Padding when the stylesheet gives none: `{ 4, 8 }`.
const PADDING: (f32, f32) = (4., 8.);
/// The least width of a field with neither `width` nor `grow`, so one in a
/// row has room to type.
const MIN_WIDTH: f32 = 160.;
/// The selection, when `input-selection` sets no background: the accent,
/// light enough to read the text through.
const SELECTION_ALPHA: f32 = 0.3;

impl<'w> Draw<'w> for Input {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        if self.writing() && !cfg!(feature = "prose") {
            return super::problem(
                "Writing editor support was left out of this build; enable `prose`".into(),
                cx,
            );
        }
        // A disabled code editor is read only, not faded: `:disabled` rules
        // are for fields that can't be used.
        let at = |focus| States {
            selected: false,
            focus,
            disabled: self.disabled && self.code.is_none(),
        };
        let colors = |part| painter.look(part, &Default::default(), false, place).base;
        let theme = cx.theme();
        let placeholder = colors(Part::InputPlaceholder)
            .color
            .map_or(theme.muted_foreground, |color| color.hsla(theme));
        let selection = colors(Part::InputSelection)
            .background
            .map_or(theme.primary.opacity(SELECTION_ALPHA), |color| {
                color.hsla(theme)
            });
        let caret = colors(Part::InputCaret)
            .background
            .map_or(theme.primary, |color| color.hsla(theme));
        // Focus outranks hover: a focused field keeps its focus look under
        // the pointer.
        let mut focused = painter.look_in(Part::Input, &self.look, at(true), place);
        focused.hover = Style::default();
        let field = Field {
            id: place.id(),
            key: match &self.id {
                // A space never appears in a path or an id, so the two can't meet.
                Some(id) => format!("{} {id}", place.owner),
                None => place.path.clone(),
            },
            owner: place.owner.to_owned(),
            input: self.clone(),
            rest: painter.look_in(Part::Input, &self.look, at(false), place),
            focused,
            placeholder,
            selection,
            caret,
            inputs: painter.inputs.clone(),
            app: cx.entity().downgrade(),
            // A demo runs live and keeps to itself, so its fields edit;
            // one drawn from an example only shows.
            preview: painter.preview && !place.owner.starts_with(moonframe_core::DEMO),
        };
        field.into_any_element()
    }
}

/// A field, drawn when the window lays it out: only then are the window
/// and the editor's focus at hand.
#[derive(IntoElement)]
struct Field {
    id: ElementId,
    key: String,
    owner: String,
    input: Input,
    /// Its look out of focus, and in it.
    rest: Resolved,
    focused: Resolved,
    placeholder: Hsla,
    selection: Hsla,
    caret: Hsla,
    inputs: Inputs,
    app: WeakEntity<MoonframeApp>,
    /// In the component browser outside a demo: drawn, never edited.
    preview: bool,
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let editor = (!self.preview)
            .then(|| {
                self.inputs
                    .sync(&self.key, &self.owner, &self.input, &self.app, window, cx)
            })
            .flatten();
        let focused = editor
            .as_ref()
            .is_some_and(|editor| editor.focus_handle(cx).is_focused(window));
        let look = if focused {
            &self.focused
        } else {
            &self.rest
        };
        let theme = cx.theme();
        let size = text_pixels(look.base.size, theme);
        let line = (size * LINE).round();
        let rows = match (&editor, self.input.multiline) {
            (_, false) => 1,
            (Some(editor), true) => self.input.visible_rows(&editor.value(cx)),
            (None, true) => self
                .input
                .visible_rows(self.input.value.as_deref().unwrap_or_default()),
        };
        let mut frame = div()
            .id(self.id)
            .flex()
            .flex_col()
            .min_w_0()
            .text_size(size)
            .line_height(line)
            .py(px(PADDING.0))
            .px(px(PADDING.1))
            .look(look, theme)
            .edges(look.base.border.or(Some(Border::All)), &look.base, theme)
            .padded(&look.base.padding);
        if self.input.mono {
            frame = frame.font_family(theme.mono_font_family.clone());
        }
        frame = match (self.input.width, self.input.grow) {
            (Some(width), _) => frame.w(px(width)).flex_shrink_0(),
            (None, true) => frame.flex_1(),
            (None, false) => frame.min_w(px(MIN_WIDTH)),
        };
        let body = div().relative().w_full().overflow_hidden();
        // A growing text area fills the height it gets (an editor's pane),
        // never shorter than its rows; the editor scrolls inside it.
        let body = if self.input.multiline && self.input.grow {
            frame = frame.min_h_0();
            body.flex_1().min_h(line * rows as f32)
        } else {
            body.h(line * rows as f32)
        };
        let Some(editor) = editor else {
            // A preview shows its value, or its placeholder.
            let (text, color) = match self.input.value.as_deref().filter(|v| !v.is_empty()) {
                Some(value) => (value.to_owned(), None),
                None => (
                    self.input.placeholder.clone().unwrap_or_default(),
                    Some(self.placeholder),
                ),
            };
            // A text area keeps its lines and wraps them, as it would edit.
            let text = if self.input.multiline {
                div().child(text)
            } else {
                div().truncate().child(text)
            };
            let text = match color {
                Some(color) => text.text_color(color),
                None => text,
            };
            return frame.child(body.child(text)).into_any_element();
        };
        editor.set_style(
            engine_style(
                &look.base,
                self.placeholder,
                self.selection,
                self.caret,
                theme,
            ),
            cx,
        );
        if !self.input.disabled {
            let clicked = editor.clone();
            frame = frame.cursor(CursorStyle::IBeam).on_mouse_down(
                MouseButton::Left,
                move |_, window, cx| {
                    if !clicked.focus_handle(cx).is_focused(window) {
                        clicked.focus(window, cx);
                    }
                },
            );
        }
        frame.child(body.child(editor.element())).into_any_element()
    }
}

/// The colors the engine paints with, from the field's look.
fn engine_style(
    base: &Style,
    placeholder: Hsla,
    selection: Hsla,
    caret: Hsla,
    theme: &Theme,
) -> InputEditorStyle {
    InputEditorStyle {
        foreground: base
            .color
            .map_or(theme.foreground, |color| color.hsla(theme)),
        muted_foreground: placeholder,
        background: base
            .background
            .map_or(theme.background, |color| color.hsla(theme)),
        border: base
            .border_color
            .map_or(theme.border, |color| color.hsla(theme)),
        selection,
        caret,
        ..InputEditorStyle::default()
    }
}
