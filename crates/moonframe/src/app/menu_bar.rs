//! What mods put in the menu bar and claim of the keyboard, app-wide:
//! `app:status-item` (an item with a menu, `crate::status_item`) and
//! `app:hotkey` (a system-wide shortcut, `crate::hotkeys`). What either
//! sends goes to the front main window's mod, as a menu item's does; the
//! app does not come forward for it, so a hotkey can act on the app the
//! user is in. A mod that goes away (reloaded without it, its package
//! removed, an instance no main window mounts) takes its item and hotkeys
//! with it, and one reloaded keeps only those it asks for again as it
//! loads.

use {
    super::*,
    crate::{
        hotkeys,
        status_item::{self, Icon, Line, Look},
    },
    moonframe_core::{Hotkey, StatusEntry, StatusIcon, StatusItem, StatusLook},
    std::collections::BTreeSet,
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
            Hotkey::Release { keys, chord } => hotkeys::release(from, &keys, &chord),
        };
        match done {
            Ok(()) => share_hotkeys(cx),
            Err(problem) => self
                .workspace
                .refuse(from, Error::new(format!("`app:hotkey`: {problem}"))),
        }
    }

    /// Takes away the items and hotkeys of mods no main window has. After
    /// a reload (`reloaded`, what the mods asked as they loaded again),
    /// also those a mod here that loaded cleanly did not ask for again:
    /// one whose keys changed gives the old ones back, and one that no
    /// longer asks for an item loses it. What it asks again stays, so an
    /// item keeps its place in the menu bar.
    pub(super) fn sweep_menu_bar(
        &mut self,
        reloaded: Option<&[(String, Request)]>,
        cx: &mut Context<Self>,
    ) {
        let items = status_item::owners();
        let holders = hotkeys::owners();
        if items.is_empty() && holders.is_empty() {
            return;
        }
        let sweep = Sweep {
            gone: self.gone(items.iter().chain(&holders), cx),
            reloaded: reloaded.map(|requests| Reloaded {
                // Loaded again here without a problem, so its `init` ran.
                renewed: items
                    .iter()
                    .chain(&holders)
                    .filter(|owner| {
                        self.workspace
                            .slot(owner)
                            .is_some_and(|slot| slot.error().is_none())
                    })
                    .map(String::as_str)
                    .collect(),
                requests,
            }),
        };
        for owner in &items {
            if sweep.takes_item(owner) {
                status_item::remove(owner);
            }
        }
        if hotkeys::forget(|owner, keys| sweep.takes_hotkey(owner, keys)) {
            share_hotkeys(cx);
        }
    }

    /// Which of `owners` no main window has: a mod or an instance mounted
    /// in none of them.
    fn gone<'a>(
        &self,
        owners: impl Iterator<Item = &'a String>,
        cx: &Context<Self>,
    ) -> BTreeSet<String> {
        let missing: BTreeSet<&String> = owners
            .filter(|owner| self.workspace.slot(owner).is_none())
            .collect();
        if missing.is_empty() {
            return BTreeSet::new();
        }
        let me = cx.entity_id();
        let others: Vec<_> = hub::every(cx)
            .into_iter()
            .filter(|app| app.entity_id() != me)
            .collect();
        missing
            .into_iter()
            .filter(|owner| {
                !others
                    .iter()
                    .any(|app| app.read(cx).workspace.slot(owner).is_some())
            })
            .cloned()
            .collect()
    }
}

/// What [`MoonframeApp::sweep_menu_bar`] takes away.
struct Sweep<'a> {
    /// Mods no main window has.
    gone: BTreeSet<String>,
    reloaded: Option<Reloaded<'a>>,
}

/// A reload in this window.
struct Reloaded<'a> {
    /// The mods that loaded again cleanly, so their `init` ran.
    renewed: BTreeSet<&'a str>,
    /// What the mods asked as they loaded again.
    requests: &'a [(String, Request)],
}

impl Sweep<'_> {
    fn takes_item(&self, owner: &str) -> bool {
        self.gone.contains(owner)
            || self.not_asked_again(owner, |request| {
                matches!(request, Request::StatusItem(StatusItem::Show(_)))
            })
    }

    fn takes_hotkey(&self, owner: &str, keys: &str) -> bool {
        self.gone.contains(owner)
            || self.not_asked_again(owner, |request| {
                matches!(request, Request::Hotkey(Hotkey::Claim { keys: asked, .. }) if asked == keys)
            })
    }

    /// Whether `owner` loaded again without asking for what `asks` is.
    fn not_asked_again(&self, owner: &str, asks: impl Fn(&Request) -> bool) -> bool {
        self.reloaded.as_ref().is_some_and(|reloaded| {
            reloaded.renewed.contains(owner)
                && !reloaded
                    .requests
                    .iter()
                    .any(|(from, request)| from == owner && asks(request))
        })
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {
        super::{Reloaded, Sweep},
        moonframe_core::{Chord, Data, Hotkey, Request, StatusItem, StatusLook},
        std::collections::BTreeSet,
    };

    fn claim(from: &str, keys: &str) -> (String, Request) {
        (
            from.to_owned(),
            Request::Hotkey(Hotkey::Claim {
                keys: keys.to_owned(),
                chord: Chord::parse(keys).unwrap(),
                msg: Data::from("go"),
                to: None,
            }),
        )
    }

    fn show(from: &str) -> (String, Request) {
        (
            from.to_owned(),
            Request::StatusItem(StatusItem::Show(StatusLook {
                icon: None,
                template: true,
                title: Some("1".to_owned()),
                tooltip: None,
                items: Vec::new(),
            })),
        )
    }

    #[test]
    fn a_reload_keeps_only_what_is_asked_for_again() {
        let requests = [claim("capture", "ctrl-alt-j"), show("lantern")];
        let sweep = Sweep {
            gone: BTreeSet::from(["removed".to_owned()]),
            reloaded: Some(Reloaded {
                renewed: BTreeSet::from(["capture", "lantern", "quiet"]),
                requests: &requests,
            }),
        };
        // Its keys changed from ctrl-alt-k to ctrl-alt-j.
        assert!(sweep.takes_hotkey("capture", "ctrl-alt-k"));
        assert!(!sweep.takes_hotkey("capture", "ctrl-alt-j"));
        assert!(!sweep.takes_item("lantern"));
        // Loaded again asking for nothing.
        assert!(sweep.takes_item("quiet"));
        assert!(sweep.takes_hotkey("quiet", "f5"));
        // Gone; or failed to load, so its `init` did not run again.
        assert!(sweep.takes_item("removed"));
        assert!(!sweep.takes_item("broken"));
        assert!(!sweep.takes_hotkey("broken", "f5"));
    }

    #[test]
    fn without_a_reload_only_what_is_gone_goes() {
        let sweep = Sweep {
            gone: BTreeSet::from(["removed".to_owned()]),
            reloaded: None,
        };
        assert!(sweep.takes_hotkey("removed", "f5"));
        assert!(!sweep.takes_hotkey("capture", "f5"));
        assert!(!sweep.takes_item("lantern"));
    }
}
