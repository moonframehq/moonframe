//! What holds no other nodes: text, controls, icons and fields, and the
//! marker for elements this backend does not draw yet.

use {
    super::{Painter, Place},
    crate::{
        Ui, icons,
        keys::{chord_name, is_chord, pressed},
        style::{looked, pixels, weight},
    },
    moonframe_core::{
        Action, Badge, Button, Case, DEMO, Data, Direction, Extras, Fit, Icon, Image, Input, Item,
        Look, Markdown, Part, States, Text, Token, Variant,
    },
    rune::{El, ScrollSpec, SurfaceFit, Tx, col, edit_access, el, row, text, text_input},
    rune_kit::{Markdown as RuneMarkdown, MarkdownStyle},
};

impl<'w> Painter<'w> {
    /// What a click on `element` does.
    pub(super) fn clicked(&self, element: El, action: Option<&Action>, place: &Place) -> El {
        let Some(action) = action else {
            return element;
        };
        let (owner, action) = (place.owner.to_owned(), action.clone());
        element.on_press_tx(move |ui: &mut Ui, tx: &mut Tx| ui.act(&owner, &action, tx))
    }

    /// An element's tooltip and right-click.
    pub(super) fn extras(&self, element: El, extras: &Extras, place: &Place) -> El {
        let element = match &extras.tooltip {
            Some(tooltip) => element.help(tooltip.clone()),
            None => element,
        };
        match &extras.context {
            Some(action) => {
                let (owner, action) = (place.owner.to_owned(), action.clone());
                element.on_secondary_press(move |ui: &mut Ui, _, tx: &mut Tx| {
                    ui.act(&owner, &action, tx);
                })
            },
            None => element,
        }
    }

    pub(super) fn text(&self, node: &'w Text, place: &Place<'w>) -> El {
        let look = self.look(Part::Text, &node.look, States::default(), place);
        let mut ink = place.ink.under(&look.base, self.palette);
        if let Some(tone) = node.tone {
            ink.color = self.palette.tone(tone);
        }
        if let Some(size) = node.size {
            ink.size = pixels(size, ink.base);
        }
        if let Some(chosen) = node.weight {
            ink.weight = weight(chosen);
        }
        let content = match look.base.case {
            Some(Case::Upper) => node.text.to_uppercase(),
            Some(Case::Lower) => node.text.to_lowercase(),
            _ => node.text.clone(),
        };
        let length = content.len();
        let element = text(content).font(ink.text_style()).color(ink.color);
        let element = if node.truncate {
            element.truncate()
        } else if let Some(lines) = node.lines {
            element.wrap().max_lines(usize::from(lines))
        } else if node.wrap {
            element.wrap()
        } else {
            element
        };
        let element = if node.strike {
            element.strike(0, length, None)
        } else {
            element
        };
        let element = looked(element, &look, None, self.palette);
        self.extras(element, &node.extras, place)
    }

    pub(super) fn button(&self, button: &'w Button, place: &Place<'w>) -> El {
        let states = States {
            disabled: button.disabled,
            ..States::default()
        };
        let look = self.look(Part::Button, &button.look, states, place);
        let (fill, ink) = match button.variant {
            Variant::Primary => (Some(Token::Accent), Token::AccentForeground),
            Variant::Danger => (Some(Token::Danger), Token::AccentForeground),
            Variant::Normal => (Some(Token::Secondary), Token::Foreground),
            Variant::Ghost | Variant::Outline => (None, Token::Foreground),
        };
        let mut pen = place.ink;
        pen.color = self.palette.token(ink);
        let pen = pen.under(&look.base, self.palette);
        let height = if button.small {
            24.
        } else {
            32.
        };
        let mut element = row()
            .key(place.path.clone())
            .items_center()
            .justify_center()
            .gap(6.)
            .h(height)
            .px(if button.small {
                8.
            } else {
                12.
            })
            .radius(6.);
        if let Some(fill) = fill {
            element = element.fill(self.palette.token(fill));
        }
        if button.variant == Variant::Outline {
            element = element.border(1., self.palette.token(Token::Border));
        }
        if let Some(name) = &button.icon
            && let Some(found) = icons::icon(name)
        {
            element = element.child(rune::icon(found, pen.size).color(pen.color));
        }
        if !button.label.is_empty() {
            element = element.child(
                text(button.label.clone())
                    .font(pen.text_style())
                    .color(pen.color),
            );
        }
        let element = looked(element, &look, None, self.palette);
        let element = if button.disabled {
            element.opacity(0.5)
        } else {
            self.clicked(element, button.action.as_ref(), place)
        };
        self.extras(element, &button.extras, place)
    }

    pub(super) fn item(&self, item: &'w Item, place: &Place<'w>) -> El {
        let states = States {
            selected: item.selected,
            ..States::default()
        };
        let look = self.look(Part::Item, &item.look, states, place);
        let pen = place.ink.under(&look.base, self.palette);
        let mut element = row().key(place.path.clone()).items_center().gap(8.).child(
            text(item.label.clone())
                .font(pen.text_style())
                .color(pen.color)
                .truncate(),
        );
        if let Some(detail) = &item.detail {
            element = element.child(
                text(detail.clone())
                    .font(pen.text_style())
                    .color(self.palette.token(Token::MutedForeground))
                    .truncate(),
            );
        }
        if let Some(badge) = &item.badge {
            element = element.child(rune::spacer()).child(
                text(badge.clone())
                    .font(pen.text_style())
                    .color(self.palette.token(Token::MutedForeground)),
            );
        }
        let element = looked(element, &look, None, self.palette);
        let element = self.clicked(element, item.action.as_ref(), place);
        self.extras(element, &item.extras, place)
    }

    pub(super) fn badge(&self, badge: &'w Badge, place: &Place<'w>) -> El {
        let look = self.look(Part::Badge, &badge.look, States::default(), place);
        let mut pen = place.ink.under(&look.base, self.palette);
        if let Some(tone) = badge.tone {
            pen.color = self.palette.tone(tone);
        }
        let element = row().key(place.path.clone()).items_center().child(
            text(badge.text.clone())
                .font(pen.text_style())
                .color(pen.color),
        );
        let element = looked(element, &look, None, self.palette);
        self.extras(element, &badge.extras, place)
    }

    pub(super) fn icon(&self, node: &'w Icon, place: &Place<'w>) -> El {
        let look = self.look(Part::Icon, &node.look, States::default(), place);
        let mut pen = place.ink.under(&look.base, self.palette);
        if let Some(tone) = node.tone {
            pen.color = self.palette.tone(tone);
        }
        if let Some(size) = node.size {
            pen.size = pixels(size, pen.base);
        }
        let element = match icons::icon(&node.name) {
            Some(found) => rune::icon(found, pen.size).color(pen.color),
            None => el().size(pen.size, pen.size),
        };
        self.extras(element, &node.extras, place)
    }

    pub(super) fn input(&self, input: &'w Input, place: &Place<'w>) -> El {
        let look = self.look(Part::Input, &input.look, States::default(), place);
        let mut pen = place.ink.under(&look.base, self.palette);
        pen.mono |= input.mono;
        let id = input.id.as_deref().unwrap_or(&place.path);
        let key = format!("{}:{id}", place.owner);
        let mut fields = self.ui.fields.borrow_mut();
        let field = fields.entry(key.clone()).or_default();
        if input.value.is_some() && input.value != field.passed {
            field.passed.clone_from(&input.value);
            field
                .edit
                .replace_text(input.value.clone().unwrap_or_default());
        }
        if input.focus && !field.asked_focus {
            field.edit.focused = true;
        }
        field.asked_focus = input.focus;
        field.blur = input.blur.clone().map(|msg| {
            let to = input.to.clone().unwrap_or_else(|| place.owner.to_owned());
            (place.owner.to_owned(), to, msg)
        });
        let shown = text_input(&field.edit, pen.text_style());
        let shown_text = field.edit.text.clone();
        drop(fields);
        let element = if input.multiline {
            shown
        } else {
            shown.single_line()
        };
        let muted = self.palette.token(Token::MutedForeground);
        let element = element
            .key(place.path.clone())
            .placeholder(input.placeholder.clone().unwrap_or_default(), muted)
            .color(pen.color)
            .px(8.)
            .py(6.)
            .radius(6.)
            .border(1., self.palette.token(Token::Border));
        let edit_key = key.clone();
        let element = element.on_edit(edit_access(move |ui: &mut Ui| {
            &mut ui
                .fields
                .get_mut()
                .entry(edit_key.clone())
                .or_default()
                .edit
        }));
        let owner = place.owner.to_owned();
        let to = input.to.clone().unwrap_or_else(|| owner.clone());
        let (owner_of_keys, to_of_keys) = (owner.clone(), to.clone());
        let element = match input.change.clone() {
            Some(msg) => {
                let (key, owner, to) = (key.clone(), owner.clone(), to.clone());
                element.on_input_change(move |ui: &mut Ui, _: &mut Tx| {
                    let value = ui.field_text(&key);
                    let caret = ui.field_caret(&key);
                    let message = Input::message_at(&msg, &value, caret);
                    ui.send(&owner, &to, message);
                })
            },
            None => element,
        };
        let element = match input.submit.clone() {
            Some(msg) => element.on_submit(move |ui: &mut Ui, _: &mut Tx| {
                let value = ui.field_text(&key);
                ui.send(&owner, &to, Input::message(&msg, &value));
            }),
            None => element,
        };
        let element = if input.keys.is_empty() && input.capture.is_none() {
            element
        } else {
            let (owner, to) = (owner_of_keys.clone(), to_of_keys.clone());
            let (chords, capture) = (input.keys.clone(), input.capture.clone());
            element.on_input_key(move |ui: &mut Ui, event: &rune::KeyEvent, _: &mut Tx| {
                let found = pressed(event, |pressed| {
                    let named = chords
                        .iter()
                        .find(|(chord, _)| is_chord(chord, pressed))
                        .map(|(_, msg)| msg.clone());
                    named.or_else(|| {
                        let name = chord_name(pressed)?;
                        let msg = capture.as_ref()?;
                        Some(Markdown::note(msg, [("keys", Data::String(name))]))
                    })
                });
                match found.flatten() {
                    Some(msg) => {
                        ui.send(&owner, &to, msg);
                        true
                    },
                    None => false,
                }
            })
        };
        let element = if input.multiline {
            let lines = input.visible_rows(&shown_text);
            element.h(lines as f32 * pen.size * 1.4 + 12.)
        } else {
            element
        };
        let element = if input.disabled {
            element.read_only(true).opacity(0.5)
        } else {
            element
        };
        let element = if input.grow {
            element.grow(1.).min_w(0.)
        } else {
            element
        };
        let element = match input.width {
            Some(width) => element.w(width),
            None => element,
        };
        looked(element, &look, None, self.palette)
    }

    /// A `ui.markdown`, through rune-kit's renderer.
    pub(super) fn markdown(&self, markdown: &'w Markdown, place: &Place<'w>) -> El {
        let look = self.look(Part::Markdown, &markdown.look, States::default(), place);
        let document = RuneMarkdown::new(markdown.text.clone()).selectable(true);
        let document = match markdown.link.clone() {
            Some(msg) => {
                let owner = place.owner.to_owned();
                let to = markdown.to.clone().unwrap_or_else(|| owner.clone());
                document.on_link(move |ui: &mut Ui, url: &str| {
                    ui.send(&owner, &to, Markdown::message(&msg, url));
                })
            },
            None => document,
        };
        let element = col()
            .key(place.path.clone())
            .child(document.render(&MarkdownStyle::neutral(self.palette.dark)));
        let element = match (markdown.width, markdown.height) {
            (Some(width), Some(height)) => element.size(width, height),
            (Some(width), None) => element.w(width),
            (None, Some(height)) => element.h(height),
            (None, None) => element,
        };
        let element = match markdown.max_width {
            Some(width) => element.max_w(width),
            None => element,
        };
        let element = if markdown.grow {
            element.grow(1.).min_h(0.).min_w(0.)
        } else {
            element
        };
        let element = if markdown.scroll {
            element.min_h(0.).scroll(ScrollSpec {
                stick_bottom: false,
            })
        } else {
            element
        };
        looked(element, &look, None, self.palette)
    }

    /// A `ui.image` from a file; a missing one leaves an empty box.
    pub(super) fn image(&self, image: &'w Image, place: &Place<'w>) -> El {
        let look = self.look(Part::Image, &image.look, States::default(), place);
        let picture = self.ui.picture(place.owner, image);
        let natural = picture.as_ref().and_then(rune::Image::size);
        let (width, height) = match (image.width, image.height, natural) {
            (Some(width), Some(height), _) => (Some(width), Some(height)),
            (Some(width), None, Some([w, h])) => (Some(width), Some(width * h as f32 / w as f32)),
            (None, Some(height), Some([w, h])) => {
                (Some(height * w as f32 / h as f32), Some(height))
            },
            (None, None, Some([w, h])) => (Some(w as f32), Some(h as f32)),
            (width, height, None) => (width, height),
        };
        let fit = match image.fit {
            Fit::Cover | Fit::Fill => SurfaceFit::Fill,
            Fit::Contain | Fit::ScaleDown | Fit::None => SurfaceFit::Fit,
        };
        let element = el().key(place.path.clone());
        let element = match picture {
            Some(picture) => element.pixels_fit(picture.pixels(), fit),
            None => element,
        };
        let element = match width {
            Some(width) => element.w(width),
            None => element,
        };
        let element = match height {
            Some(height) => element.h(height),
            None => element,
        };
        let inner = Place {
            parent: Direction::Column,
            ..place.clone()
        };
        let element = element.children(self.nodes(&image.overlay, &inner));
        let element = looked(element, &look, None, self.palette);
        self.extras(element, &image.extras, place)
    }

    pub(super) fn divider(&self, place: &Place<'w>) -> El {
        let look = self.look(Part::Divider, &Look::default(), States::default(), place);
        let line = el().fill(self.palette.token(Token::Border));
        let line = match place.parent {
            Direction::Row => line.w(1.).h_full(),
            Direction::Column => line.h(1.).w_full(),
        };
        looked(line, &look, None, self.palette)
    }

    /// A marker for `name`, which is kept among what the view lacked.
    pub(super) fn not_here_named(&self, name: &str, place: &Place<'w>) -> El {
        self.ui.left_out.borrow_mut().insert(name.to_owned());
        row()
            .key(place.path.clone())
            .grow(1.)
            .items_center()
            .justify_center()
            .fill(self.palette.token(Token::Muted))
            .radius(6.)
            .padding(8.)
            .child(
                text(format!("{name} needs the GPUI backend"))
                    .font(place.ink.text_style())
                    .color(self.palette.token(Token::MutedForeground)),
            )
    }
}

impl Ui {
    /// The picture `image` names, loaded on first use, if the drawing mod may
    /// read it: `ui.image`'s own rule, so a mod reaches only the app's files.
    fn picture(&self, owner: &str, image: &Image) -> Option<rune::Image> {
        let cache = self.images.as_ref()?;
        let source = match self.workspace.slot(owner).and_then(|slot| slot.source()) {
            Some(source) => source,
            // A demo made from a component's example has no file of its own:
            // the component's folder is where its pictures are.
            None => {
                let name = owner.strip_prefix(DEMO).unwrap_or(owner);
                let components = self.workspace.components();
                &components
                    .iter()
                    .find(|component| component.name == name)?
                    .path
            },
        };
        let folder = source.parent()?;
        let mut allowed: Vec<_> = self
            .workspace
            .roots()
            .layers()
            .map(ToOwned::to_owned)
            .collect();
        allowed.push(moonframe_packages::paths::images());
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let path = image.resolve(folder, home.as_deref(), &allowed)?;
        let mut pictures = self.pictures.borrow_mut();
        if let Some(picture) = pictures.get(&path) {
            return Some(picture.clone());
        }
        let picture = cache.load(std::fs::read(&path).ok()?);
        pictures.insert(path, picture.clone());
        Some(picture)
    }

    /// Where the caret is, in bytes, in the field kept as `key`.
    pub(crate) fn field_caret(&mut self, key: &str) -> usize {
        self.fields
            .get_mut()
            .get(key)
            .map_or(0, |field| field.edit.focus)
    }

    /// Sends `blur` for each field that has just lost the keyboard.
    pub(crate) fn tell_blurs(&mut self) {
        let mut told = Vec::new();
        for field in self.fields.get_mut().values_mut() {
            if field.had_focus
                && !field.edit.focused
                && let Some((owner, to, msg)) = &field.blur
            {
                let message = Input::message(msg, &field.edit.text);
                told.push((owner.clone(), to.clone(), message));
            }
            field.had_focus = field.edit.focused;
        }
        for (owner, to, msg) in told {
            self.send(&owner, &to, msg);
        }
    }

    /// The text of the field kept as `key`.
    pub(crate) fn field_text(&mut self, key: &str) -> String {
        self.fields
            .get_mut()
            .get(key)
            .map(|field| field.edit.text.clone())
            .unwrap_or_default()
    }
}
