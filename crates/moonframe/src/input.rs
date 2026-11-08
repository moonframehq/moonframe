//! The text that `ui.input` elements edit. Each field is kept by its mod and
//! `id` (or its place in the tree), so what the user typed outlives
//! re-renders and reloads. The editing itself (caret, selection, IME, undo)
//! is gpui-base's unstyled input engine; the look is the stylesheet's.
//!
//! Edits, Enter and the field's `keys` become messages for the drawing
//! mod's `update`, shaped by [`moonframe_core::Input::message`]. A field
//! with `browser` is that page's address bar: Enter loads what it says,
//! focus selects it, and Escape puts back the page's address. A field
//! with `prose`, `dim` or `typewriter` is a writing field (the `prose` feature).

#[cfg(feature = "prose")]
mod prose;

use {
    crate::app::MoonframeApp,
    gpui_kit::{
        base::input::{
            Input as LineElement, InputEditorStyle, InputEvent, InputState, Textarea, TextareaState,
        },
        *,
    },
    moonframe_core::{Action, Data, Edit, Input, KeyChord, Markdown},
    std::{cell::RefCell, collections::HashMap, rc::Rc},
};

#[cfg(feature = "prose")]
use gpui_kit::base::input::{Editor as ProseElement, EditorState};

/// Fields the app keeps at once. Past this the ones drawn longest ago go,
/// so a mod drawing inputs from data can't grow the set without end.
const MAX_FIELDS: usize = 256;

/// The engine behind one field: one line, several, or a writing field.
#[derive(Clone)]
pub(crate) enum Editor {
    Line(Entity<InputState>),
    Lines(Entity<TextareaState>),
    #[cfg(feature = "prose")]
    Prose(Entity<EditorState>, Rc<prose::Writing>),
}

/// Runs `$body` on whichever state the editor holds.
macro_rules! on_state {
    ($editor:expr, |$state:ident| $body:expr) => {
        match $editor {
            Editor::Line($state) => $body,
            Editor::Lines($state) => $body,
            #[cfg(feature = "prose")]
            Editor::Prose($state, _) => $body,
        }
    };
}

impl Editor {
    fn new(input: &Input, window: &mut Window, cx: &mut App) -> Self {
        #[cfg(feature = "prose")]
        if input.writing() {
            let writing = Rc::new(prose::Writing::default());
            return Self::Prose(prose::editor(&writing, window, cx), writing);
        }
        if input.multiline {
            Self::Lines(cx.new(|cx| TextareaState::new(window, cx)))
        } else {
            Self::Line(cx.new(|cx| InputState::new(window, cx)))
        }
    }

    /// Whether `input` asks for an engine of another kind than this one.
    fn other_kind(&self, input: &Input) -> bool {
        let (multiline, writing) = match self {
            Self::Line(_) => (false, false),
            Self::Lines(_) => (true, false),
            #[cfg(feature = "prose")]
            Self::Prose(..) => (true, true),
        };
        multiline != input.multiline || writing != input.writing()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        on_state!(self, |state| state.read(cx).focus_handle(cx))
    }

    pub(crate) fn value(&self, cx: &App) -> SharedString {
        on_state!(self, |state| state.read(cx).value())
    }

    fn set_value(&self, value: &str, window: &mut Window, cx: &mut App) {
        let value = value.to_owned();
        on_state!(self, |state| state
            .update(cx, |state, cx| state.set_value(value, window, cx)))
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        on_state!(self, |state| state
            .update(cx, |state, cx| state.focus(window, cx)))
    }

    fn set_placeholder(&self, placeholder: &str, window: &mut Window, cx: &mut App) {
        let placeholder = placeholder.to_owned();
        on_state!(self, |state| state.update(cx, |state, cx| state
            .set_placeholder(placeholder, window, cx)))
    }

    fn select_all(&self, window: &mut Window, cx: &mut App) {
        on_state!(self, |state| state
            .update(cx, |state, cx| state.select_all(window, cx)))
    }

    /// The selection, in bytes from the start; empty at the caret.
    pub(crate) fn selection(&self, cx: &App) -> std::ops::Range<usize> {
        on_state!(self, |state| state.read(cx).selected_range())
    }

    fn caret(&self, cx: &App) -> usize {
        on_state!(self, |state| state.read(cx).cursor())
    }

    /// Selects `range`, in bytes from the start; empty puts the caret there.
    pub(crate) fn select(&self, range: std::ops::Range<usize>, cx: &mut App) {
        on_state!(self, |state| state
            .update(cx, |state, cx| state.set_selected_range(range, cx)))
    }

    fn set_disabled(&self, disabled: bool, cx: &mut App) {
        on_state!(self, |state| state
            .update(cx, |state, cx| state.set_disabled(disabled, cx)))
    }

    /// Colors the engine paints itself: the placeholder, the selection and
    /// the caret. The text takes the color of the frame around it.
    pub(crate) fn set_style(&self, style: InputEditorStyle, cx: &mut App) {
        on_state!(self, |state| state
            .update(cx, |state, _| state.set_editor_style(style)))
    }

    /// The text element, to place in the field's frame.
    pub(crate) fn element(&self) -> AnyElement {
        match self {
            Self::Line(state) => LineElement::new(state).into_any_element(),
            Self::Lines(state) => Textarea::new(state).into_any_element(),
            #[cfg(feature = "prose")]
            Self::Prose(state, _) => ProseElement::new(state).into_any_element(),
        }
    }
}

/// What a field's messages need: who gets them, and which ones it sends.
#[derive(Clone)]
pub(crate) struct Wiring {
    pub(crate) owner: String,
    pub(crate) to: Option<String>,
    pub(crate) change: Option<Data>,
    pub(crate) select: Option<Data>,
    pub(crate) submit: Option<Data>,
    pub(crate) blur: Option<Data>,
    pub(crate) keys: Vec<(KeyChord, Data)>,
    pub(crate) capture: Option<Data>,
    pub(crate) multiline: bool,
    pub(crate) disabled: bool,
    /// The browser it is the address bar of.
    pub(crate) browser: Option<String>,
}

impl Wiring {
    pub(crate) fn of(owner: &str, input: &Input) -> Self {
        Self {
            owner: owner.to_owned(),
            to: input.to.clone(),
            change: input.change.clone(),
            select: input.select.clone(),
            submit: input.submit.clone(),
            blur: input.blur.clone(),
            keys: input.keys.clone(),
            capture: input.capture.clone(),
            multiline: input.multiline,
            disabled: input.disabled,
            browser: input.browser.clone(),
        }
    }

    /// The mod whose browser the field drives, and the browser's id.
    fn browser(&self) -> Option<(&str, &str)> {
        let id = self.browser.as_deref()?;
        Some((self.to.as_deref().unwrap_or(&self.owner), id))
    }

    /// `msg` with the field's text, sent as the field's mod would send it.
    fn send(&self, msg: &Data, value: &str) -> Action {
        Action::Send {
            to: self.to.clone(),
            msg: Input::message(msg, value),
        }
    }
}

/// One field the host keeps.
struct Field {
    editor: Editor,
    /// The editor's value survives unmounting; its key routing does not.
    mounted: WeakEntity<()>,
    wiring: Wiring,
    /// The last `value` the mod passed, to tell a new one from the same.
    applied: Option<String>,
    placeholder: Option<String>,
    rows: Option<u32>,
    /// The mod's `focus` when last drawn, so it takes focus as it turns true.
    focus: bool,
    /// The frame it was last drawn in.
    seen: u64,
    window: WindowId,
    selection: (std::ops::Range<usize>, usize),
    _events: Subscription,
    _selection: Subscription,
    /// A writing field follows its caret, for what it keeps in focus.
    _caret: Option<Subscription>,
}

#[derive(Default)]
struct Registry {
    fields: HashMap<String, Field>,
    /// Counts the app's renders, to tell a field drawn again from one that
    /// came back after being gone.
    frame: u64,
}

/// The fields of the app, shared with the elements that draw them (which
/// lay out after the app's render, outside it).
#[derive(Clone, Default)]
pub(crate) struct Inputs(Rc<RefCell<Registry>>);

impl Inputs {
    /// Called once per render of the app, before anything is drawn.
    pub(crate) fn begin_frame(&self) {
        self.0.borrow_mut().frame += 1;
    }

    /// The editor at `key`, made now if it is new, with the mod's latest
    /// `input` applied: its value when it is a new one, its placeholder, and
    /// focus when `focus` turns true.
    pub(crate) fn sync(
        &self,
        key: &str,
        owner: &str,
        input: &Input,
        app: &WeakEntity<MoonframeApp>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Editor> {
        if input.writing() && !cfg!(feature = "prose") {
            return None;
        }
        let mounted = window.use_keyed_state(
            SharedString::from(format!("input/{key}/mounted")),
            cx,
            |_, _| (),
        );
        let mut registry = self.0.borrow_mut();
        let frame = registry.frame;
        let wiring = Wiring::of(owner, input);
        // A field whose kind changed keeps its text and last applied value,
        // so an unchanged initial value cannot overwrite the carried edits.
        let carried = match registry.fields.get(key) {
            Some(field) if field.editor.other_kind(input) => {
                let text = field.editor.value(cx).to_string();
                let applied = field.applied.clone();
                registry.fields.remove(key);
                Some((text, applied))
            },
            _ => None,
        };
        if !registry.fields.contains_key(key) {
            let editor = Editor::new(input, window, cx);
            if let Some((text, _)) = &carried {
                editor.set_value(text, window, cx);
            }
            let events = subscribe(app, &editor, key, window, cx)?;
            let selection = observe_selection(app, &editor, key, window, cx)?;
            let caret = match &editor {
                #[cfg(feature = "prose")]
                Editor::Prose(state, writing) => Some(prose::follow(writing, state, cx)),
                _ => None,
            };
            prune(&mut registry.fields, frame, window, cx);
            registry.fields.insert(
                key.to_owned(),
                Field {
                    selection: (editor.selection(cx), editor.caret(cx)),
                    editor,
                    mounted: mounted.downgrade(),
                    wiring: wiring.clone(),
                    applied: carried.and_then(|(_, applied)| applied),
                    placeholder: None,
                    rows: None,
                    focus: false,
                    seen: frame,
                    window: window.window_handle().window_id(),
                    _events: events,
                    _selection: selection,
                    _caret: caret,
                },
            );
        }
        let field = registry.fields.get_mut(key)?;
        // Gone for a frame or more: it appears again, as if new.
        if field.seen + 1 < frame {
            field.focus = false;
        }
        field.seen = frame;
        field.mounted = mounted.downgrade();
        field.window = window.window_handle().window_id();
        field.wiring = wiring;
        let editor = field.editor.clone();
        if input.value != field.applied {
            if let Some(value) = &input.value
                && editor.value(cx).as_ref() != value
            {
                editor.set_value(value, window, cx);
            }
            field.applied = input.value.clone();
        }
        if input.placeholder != field.placeholder {
            editor.set_placeholder(input.placeholder.as_deref().unwrap_or(""), window, cx);
            field.placeholder = input.placeholder.clone();
        }
        let rows = input.visible_rows(&editor.value(cx));
        if let Editor::Lines(state) = &editor
            && field.rows != Some(rows)
        {
            state.update(cx, |state, cx| state.set_rows(rows as usize, cx));
            field.rows = Some(rows);
        }
        #[cfg(feature = "prose")]
        if let Editor::Prose(state, writing) = &editor {
            writing.apply(input, state, window, cx);
        }
        editor.set_disabled(input.disabled, cx);
        let take_focus = input.focus && !field.focus && !input.disabled;
        field.focus = input.focus;
        drop(registry);
        if take_focus {
            editor.focus(window, cx);
        }
        Some(editor)
    }

    /// The field at `key`: how it sends, and its text.
    fn wiring(&self, key: &str, cx: &App) -> Option<(Wiring, String)> {
        let registry = self.0.borrow();
        let field = registry.fields.get(key)?;
        Some((field.wiring.clone(), field.editor.value(cx).to_string()))
    }

    /// The editor of the field at `key`, or of the focused one.
    fn editor(&self, key: Option<&str>, window: &Window, cx: &App) -> Option<Editor> {
        let registry = self.0.borrow();
        let field = match key {
            Some(key) => registry.fields.get(key),
            None => registry.fields.values().find(|field| {
                field.mounted.upgrade().is_some()
                    && field.editor.focus_handle(cx).is_focused(window)
            }),
        };
        field.map(|field| field.editor.clone())
    }

    /// Carries out `app:edit` from the mod `owner` on its field `edit.field`:
    /// replaces the spliced text as one undoable edit, selects what the
    /// edit leaves selected, and keeps the keyboard there. The field sends
    /// its `change` as for any edit.
    pub(crate) fn edit(
        &self,
        owner: &str,
        edit: &Edit,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), String> {
        let key = format!("{owner} {}", edit.field);
        let editor = self
            .editor(Some(&key), window, cx)
            .ok_or_else(|| format!("no field with `id = \"{}\"` drawn by `{owner}`", edit.field))?;
        let text = editor.value(cx).to_string();
        if !edit.applies(&text, editor.selection(cx)) {
            return Ok(());
        }
        let splice = edit.splice(&text, editor.selection(cx));
        editor.select(splice.range, cx);
        on_state!(&editor, |state| state.update(cx, |state, cx| {
            state.replace(splice.text, window, cx);
        }));
        editor.select(splice.select, cx);
        if !editor.focus_handle(cx).is_focused(window) {
            editor.focus(window, cx);
        }
        Ok(())
    }

    /// App edits must run in the window that actually draws the field.
    pub(crate) fn window(&self, owner: &str, field: &str) -> Option<WindowId> {
        self.0
            .borrow()
            .fields
            .get(&format!("{owner} {field}"))
            .map(|field| field.window)
    }

    /// Whether a field has keyboard focus in `window`.
    pub(crate) fn any_focused(&self, window: &Window, cx: &App) -> bool {
        self.focused(window, cx).is_some()
    }

    /// The field with keyboard focus in `window`, if any.
    pub(crate) fn focused(&self, window: &Window, cx: &App) -> Option<(Wiring, String)> {
        let registry = self.0.borrow();
        registry
            .fields
            .values()
            .find(|field| {
                field.mounted.upgrade().is_some()
                    && field.editor.focus_handle(cx).is_focused(window)
            })
            .map(|field| (field.wiring.clone(), field.editor.value(cx).to_string()))
    }
}

/// Edits, Enter, focus and blur on `editor` reach the app as events of the
/// field at `key`.
fn subscribe(
    app: &WeakEntity<MoonframeApp>,
    editor: &Editor,
    key: &str,
    window: &Window,
    cx: &mut App,
) -> Option<Subscription> {
    let key = key.to_owned();
    app.update(cx, |_, cx| {
        on_state!(editor, |state| cx.subscribe_in(
            state,
            window,
            move |app: &mut MoonframeApp, _, event: &InputEvent, window, cx| {
                app.input_event(&key, event, window, cx)
            }
        ))
    })
    .ok()
}

/// gpui-base notifies on caret movement without emitting a text Change.
/// Compare selections so cursor blinking and styling don't send messages.
fn observe_selection(
    app: &WeakEntity<MoonframeApp>,
    editor: &Editor,
    key: &str,
    window: &mut Window,
    cx: &mut App,
) -> Option<Subscription> {
    let key = key.to_owned();
    app.update(cx, |_, cx| {
        on_state!(editor, |state| cx.observe_in(
            state,
            window,
            move |app, _, window, cx| {
                app.input_selection(&key, window, cx);
            }
        ))
    })
    .ok()
}

/// Makes room for one more field: drops those drawn longest ago, never one
/// with focus.
fn prune(fields: &mut HashMap<String, Field>, frame: u64, window: &Window, cx: &App) {
    while fields.len() >= MAX_FIELDS {
        // Never one drawn in this frame: its text would be lost as it is
        // typed. The frame's fields are bounded by what a view may draw.
        let oldest = fields
            .iter()
            .filter(|(_, field)| field.seen < frame)
            .filter(|(_, field)| !field.editor.focus_handle(cx).is_focused(window))
            .min_by_key(|(_, field)| field.seen)
            .map(|(key, _)| key.clone());
        match oldest {
            Some(key) => fields.remove(&key),
            None => {
                tracing::warn!(
                    "more than {MAX_FIELDS} text fields are drawn at once; keeping them all"
                );
                return;
            },
        };
    }
}

impl MoonframeApp {
    fn input_selection(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let send = {
            let mut registry = self.inputs.0.borrow_mut();
            let Some(field) = registry.fields.get_mut(key) else {
                return;
            };
            let selection = (field.editor.selection(cx), field.editor.caret(cx));
            if field.selection == selection {
                return;
            }
            field.selection = selection.clone();
            field.wiring.select.as_ref().map(|msg| {
                (
                    field.wiring.owner.clone(),
                    Action::Send {
                        to: field.wiring.to.clone(),
                        msg: Input::selection_message(
                            msg,
                            &field.editor.value(cx),
                            selection.1,
                            selection.0.len(),
                        ),
                    },
                )
            })
        };
        if let Some((owner, action)) = send {
            self.dispatch(&owner, &action, window, cx);
        }
    }

    /// Something happened in the field at `key`: an edit sends its `change`,
    /// Enter in a single-line field its `submit`; focus and blur redraw it
    /// in its `:focus` look or out of it.
    pub(crate) fn input_event(
        &mut self,
        key: &str,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((wiring, value)) = self.inputs.wiring(key, cx) else {
            return;
        };
        // An edit reports where the caret is, in bytes from the start.
        let mut caret = None;
        let msg = match event {
            InputEvent::Change => {
                caret = self
                    .inputs
                    .editor(Some(key), window, cx)
                    .map(|editor| editor.caret(cx));
                wiring.change.as_ref()
            },
            InputEvent::PressEnter { .. } if !wiring.multiline => {
                // An address bar loads what it says, and hands the keyboard
                // back so the next keys don't edit it.
                if let Some((owner, id)) = wiring.browser()
                    && self.browsers.visit(owner, id, &value)
                {
                    window.blur(cx);
                    cx.notify();
                }
                wiring.submit.as_ref()
            },
            InputEvent::PressEnter { .. } => None,
            InputEvent::Focus => {
                // As in a browser, the whole address, to type over.
                if wiring.browser.is_some()
                    && let Some(editor) = self.inputs.editor(Some(key), window, cx)
                {
                    editor.select_all(window, cx);
                }
                cx.notify();
                None
            },
            InputEvent::Blur => {
                cx.notify();
                wiring.blur.as_ref()
            },
        };
        if let Some(msg) = msg {
            let action = match caret {
                Some(caret) => Action::Send {
                    to: wiring.to.clone(),
                    msg: Input::message_at(msg, &value, caret),
                },
                None => wiring.send(msg, &value),
            };
            self.dispatch(&wiring.owner, &action, window, cx);
        }
    }

    /// A keystroke while a field has focus, before key bindings and any
    /// terminal see it: one of the field's `keys` sends its message instead
    /// of editing, and ⌘/Ctrl-Enter submits a multiline field. Returns
    /// whether the field took it.
    pub(crate) fn input_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((wiring, value)) = self.inputs.focused(window, cx) else {
            return false;
        };
        if wiring.disabled {
            return false;
        }
        let msg = wiring
            .keys
            .iter()
            .find(|(chord, _)| pressed(chord, keystroke))
            .map(|(_, msg)| msg);
        if msg.is_none()
            && let Some((owner, id)) = wiring.browser()
            && KeyChord::parse("escape").is_ok_and(|escape| pressed(&escape, keystroke))
        {
            // Escape in an address bar puts back where the page is.
            let address = self.browsers.address(owner, id).unwrap_or_default();
            if let Some(editor) = self.inputs.editor(None, window, cx) {
                editor.set_value(&address, window, cx);
                editor.select_all(window, cx);
            }
            return true;
        }
        let msg = msg.or_else(|| {
            let submits =
                wiring.multiline && pressed(&KeyChord::parse("secondary-enter").ok()?, keystroke);
            submits.then_some(wiring.submit.as_ref()).flatten()
        });
        if msg.is_none()
            && let Some(capture) = &wiring.capture
        {
            if let Some(keys) = chord_name(keystroke) {
                let action = Action::Send {
                    to: wiring.to.clone(),
                    msg: Markdown::note(capture, [("keys", Data::String(keys))]),
                };
                self.dispatch(&wiring.owner, &action, window, cx);
            }
            return true;
        }
        let Some(msg) = msg else {
            return false;
        };
        let action = wiring.send(msg, &value);
        self.dispatch(&wiring.owner, &action, window, cx);
        true
    }
}

/// A keystroke's name as `kbd` and `keys` write it, such as `cmd-shift-p`.
/// Bare characters aren't shortcuts: they would take over typing. A key
/// with no character (`f5`, `escape`) stands alone.
fn chord_name(keystroke: &Keystroke) -> Option<String> {
    let held = keystroke.modifiers;
    let modified = held.control || held.alt || held.platform;
    if !modified && keystroke.key.chars().count() == 1 {
        return None;
    }
    let mut parts = Vec::new();
    for (on, name) in [
        (held.control, "ctrl"),
        (held.alt, "alt"),
        (held.shift, "shift"),
        (held.platform, "cmd"),
    ] {
        if on {
            parts.push(name);
        }
    }
    parts.push(&keystroke.key);
    Some(parts.join("-"))
}

/// Whether `keystroke` is `chord`, modifiers exactly. `secondary` is ⌘ on
/// macOS and Ctrl elsewhere.
pub(crate) fn pressed(chord: &KeyChord, keystroke: &Keystroke) -> bool {
    let held = keystroke.modifiers;
    let (ctrl, cmd) = match (chord.secondary, cfg!(target_os = "macos")) {
        (true, true) => (chord.ctrl, true),
        (true, false) => (true, chord.cmd),
        (false, _) => (chord.ctrl, chord.cmd),
    };
    keystroke.key == chord.key
        && held.control == ctrl
        && held.alt == chord.alt
        && held.shift == chord.shift
        && held.platform == cmd
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod chord_tests {
    use {super::chord_name, gpui_kit::Keystroke};

    fn named(keys: &'static str) -> Option<String> {
        chord_name(&Keystroke::parse(keys).unwrap())
    }

    #[test]
    fn names_a_chord_as_kbd_writes_it() {
        assert_eq!(named("cmd-shift-p").as_deref(), Some("shift-cmd-p"));
        assert_eq!(named("ctrl-k").as_deref(), Some("ctrl-k"));
        assert_eq!(named("f5").as_deref(), Some("f5"));
    }

    #[test]
    fn a_bare_character_is_no_shortcut() {
        assert_eq!(named("a"), None);
        assert_eq!(named("shift-a"), None);
    }
}
