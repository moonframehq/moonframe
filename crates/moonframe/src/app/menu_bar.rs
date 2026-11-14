//! What mods put in the menu bar and claim of the keyboard, app-wide:
//! `app:status-item` (an item with a menu, `crate::status_item`) and
//! `app:hotkey` (a system-wide shortcut, `crate::hotkeys`). What either
//! sends goes to the front main window's mod, as a menu item's does; the
//! app does not come forward for it, so a hotkey can act on the app the
//! user is in. A mod that goes away (reloaded without it, its package
//! removed) takes its item and hotkeys with it.

use {
    super::*,
    crate::{
        hotkeys,
        status_item::{self, Icon, Line, Look},
    },
    moonframe_core::{Hotkey, StatusEntry, StatusIcon, StatusItem, StatusLook},
};

impl MoonframeApp {
    /// Does what the mod `from` asked with `app:status-item`.
    pub(super) fn status_item_request(
        &mut self,
        from: &str,
        request: StatusItem,
        cx: &mut Context<Self>,
    ) {
        if !self.built_with(from, cfg!(feature = "status-item"), "status-item") {
            return;
        }
        let done = match request {
            StatusItem::Remove => {
                status_item::remove(from);
                Ok(())
            },
            StatusItem::Show(look) => self
                .status_look(from, look)
                .and_then(|look| status_item::show(from, look)),
            StatusItem::Popup(_)
                if cfg!(target_os = "macos")
                    && !status_item::owners().iter().any(|owner| owner == from) =>
            {
                Err("`popup` needs the mod's item shown first".to_owned())
            },
            StatusItem::Popup(entries) => {
                let (owner, lines) = (from.to_owned(), lines(from, entries));
                // AppKit tracks the menu until it closes: never inside an
                // update of the app.
                cx.spawn(async move |_, _| {
                    if let Err(problem) = status_item::popup(&owner, lines) {
                        tracing::warn!("{owner}: `app:status-item`: {problem}");
                    }
                })
                .detach();
                Ok(())
            },
        };
        if let Err(problem) = done {
            self.workspace
                .refuse(from, Error::new(format!("`app:status-item`: {problem}")));
        }
    }

    /// The item `look` asks for, its image read from the mod's folders.
    fn status_look(&self, from: &str, look: StatusLook) -> Result<Look, String> {
        let icon = match look.icon {
            None => None,
            Some(StatusIcon::Symbol(name)) => Some(Icon::Symbol(name)),
            Some(StatusIcon::Image(src)) => {
                let file = self
                    .mod_file(from, &src, icon::MAX_ICON_BYTES)
                    .ok_or_else(|| {
                        format!("no image `{src}` of at most 4 MiB in the app's folders")
                    })?;
                Some(Icon::Image(icon::read_icon(&file)?))
            },
        };
        Ok(Look {
            icon,
            template: look.template,
            title: look.title,
            tooltip: look.tooltip,
            menu: lines(from, look.items),
        })
    }

    /// Does what the mod `from` asked with `app:hotkey`, and says how the
    /// app's hotkeys stand in `app.hotkeys`.
    pub(super) fn hotkey_request(&mut self, from: &str, request: Hotkey, cx: &mut Context<Self>) {
        if !self.built_with(from, cfg!(feature = "hotkey"), "hotkey") {
            return;
        }
        let done = match request {
            Hotkey::Claim {
                keys,
                chord,
                msg,
                to,
            } => hotkeys::claim(
                from,
                &keys,
                chord,
                to.unwrap_or_else(|| from.to_owned()),
                msg,
            ),
            Hotkey::Release(keys) => hotkeys::release(from, &keys),
        };
        match done {
            Ok(()) => share_hotkeys(cx),
            Err(problem) => self
                .workspace
                .refuse(from, Error::new(format!("`app:hotkey`: {problem}"))),
        }
    }

    /// Takes away the items and hotkeys of mods no longer here.
    pub(super) fn forget_gone_owners(&mut self, cx: &mut Context<Self>) {
        let gone = |owner: &str| self.workspace.slot(owner).is_none();
        for owner in status_item::owners() {
            if gone(&owner) {
                status_item::remove(&owner);
            }
        }
        if hotkeys::owners().iter().any(|owner| gone(owner)) {
            hotkeys::forget(gone);
            share_hotkeys(cx);
        }
    }
}

/// Shares `app.hotkeys` with every main window, once this update is over.
fn share_hotkeys(cx: &mut Context<MoonframeApp>) {
    let data = hotkeys::data();
    cx.defer(move |cx| hub::share_everywhere("hotkeys", data, cx));
}

/// The lines of a menu the mod `owner` asked for: a line without `to`
/// sends to it.
fn lines(owner: &str, entries: Vec<StatusEntry>) -> Vec<Line> {
    entries
        .into_iter()
        .map(|entry| match entry {
            StatusEntry::Separator => Line::Separator,
            StatusEntry::Item {
                label,
                msg,
                to,
                enabled,
                checked,
                shortcut,
            } => Line::Item {
                label,
                action: msg.map(|msg| (to.unwrap_or_else(|| owner.to_owned()), msg)),
                enabled,
                checked,
                shortcut,
            },
        })
        .collect()
}

/// Hands each menu line clicked and each hotkey pressed to the front main
/// window's mod, from now on.
pub(super) fn listen(cx: &mut App) {
    if let Some(mut clicks) = status_item::listen() {
        cx.spawn(async move |cx| {
            while let Some(click) = clicks.next().await {
                cx.update(|cx| deliver("app:status-item", &click.to, click.msg, cx));
            }
        })
        .detach();
    }
    if let Some(mut presses) = hotkeys::listen() {
        cx.spawn(async move |cx| {
            while let Some(press) = presses.next().await {
                cx.update(|cx| deliver("app:hotkey", &press.to, press.msg, cx));
            }
        })
        .detach();
    }
}

fn deliver(from: &str, to: &str, msg: Data, cx: &mut App) {
    if let Some(app) = hub::front(cx) {
        app.update(cx, |app, cx| {
            app.workspace.send(from, to, msg);
            app.flush("app", cx);
        });
    }
}
