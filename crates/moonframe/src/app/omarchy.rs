//! Following the Omarchy desktop's theme as it changes.

use {super::*, crate::themes::omarchy};

/// Choices to resolve when the first palette arrives after a startup swap.
pub(super) fn pending(saved: impl Fn(&str) -> Option<String>) -> [bool; 2] {
    let default = saved(settings::THEME).is_none();
    [settings::LIGHT_THEME, settings::DARK_THEME]
        .map(|key| saved(key).map_or(default, |name| name == omarchy::NAME))
}

/// Watches for Omarchy changing its theme, on Omarchy only. Its own watcher,
/// because `watch` stays off in a release bundle and this must not.
pub(super) fn watching(cx: &mut Context<MoonframeApp>) -> Option<RecommendedWatcher> {
    let (sender, mut changes) = futures::channel::mpsc::unbounded();
    let watcher = omarchy::watch(move || {
        let _ = sender.unbounded_send(());
    })
    .inspect_err(|error| tracing::warn!("not following the omarchy theme: {error}"))
    .ok()
    .flatten()?;
    cx.spawn(async move |this, cx| {
        while changes.next().await.is_some() {
            cx.background_executor().timer(SETTLE).await;
            while changes.try_recv().is_ok() {}
            if this
                .update(cx, |this, cx| this.omarchy_changed(cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    Some(watcher)
}

impl MoonframeApp {
    /// Shows Omarchy's new theme wherever its theme is the one picked. Other
    /// picks stay, and nothing is saved: the name is what is kept.
    fn omarchy_changed(&mut self, cx: &mut Context<Self>) {
        if !omarchy::load() {
            return;
        }
        for (slot, pending) in [&mut self.light_scheme, &mut self.dark_scheme]
            .into_iter()
            .zip(&mut self.pending_omarchy)
        {
            refresh(slot, pending, omarchy::current());
        }
        self.preview = None;
        self.apply_theme(cx);
        cx.notify();
    }
}

fn refresh(
    slot: &mut Option<&'static Scheme>,
    pending: &mut bool,
    current: Option<&'static Scheme>,
) {
    if current.is_some() && (*pending || slot.is_some_and(|scheme| scheme.name == omarchy::NAME)) {
        *slot = current;
        *pending = false;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::{pending, refresh},
        crate::{settings, themes},
    };

    #[test]
    fn a_startup_swap_defers_only_unset_or_omarchy_choices() {
        assert_eq!(pending(|_| None), [true, true]);
        let saved = |key: &str| match key {
            settings::LIGHT_THEME => Some(themes::DEFAULT.to_owned()),
            settings::DARK_THEME => Some(themes::omarchy::NAME.to_owned()),
            _ => None,
        };
        assert_eq!(pending(saved), [false, true]);
        assert_eq!(
            pending(|key| { (key == settings::THEME).then(|| "Nord".to_owned()) }),
            [false, false]
        );
    }

    #[test]
    fn the_first_palette_fills_pending_slots_but_keeps_explicit_choices() {
        let nord = themes::find("Nord").unwrap();
        let mut scheme = nord.clone();
        scheme.name = themes::omarchy::NAME;
        let scheme = &*Box::leak(Box::new(scheme));
        let current = Some(scheme);
        let mut slot = None;
        let mut waiting = true;

        refresh(&mut slot, &mut waiting, None);
        assert!(waiting);
        assert_eq!(slot, None);
        refresh(&mut slot, &mut waiting, current);
        assert!(!waiting);
        assert_eq!(slot, current);

        // A saved Default, or a choice made while waiting, stays picked.
        for picked in [None, Some(nord)] {
            let mut slot = picked;
            refresh(&mut slot, &mut false, current);
            assert_eq!(slot, picked);
        }

        // Once resolved, the Omarchy slot still follows later palettes.
        let mut next = scheme.clone();
        next.background = 0xffffff;
        let next = Some(&*Box::leak(Box::new(next)));
        refresh(&mut slot, &mut waiting, next);
        assert_eq!(slot, next);
    }
}
