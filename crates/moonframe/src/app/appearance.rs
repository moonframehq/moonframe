//! Picking and keeping the themes for each appearance, the same in every
//! main window: GPUI has one theme for the whole app.

use super::*;

/// The themes and appearance picked.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Look {
    /// The theme light windows show and the one dark windows show
    /// (`None`: the stock look), picked in View > Themes or by a mod.
    pub(super) light_scheme: Option<&'static Scheme>,
    pub(super) dark_scheme: Option<&'static Scheme>,
    /// Light/dark choices waiting for Omarchy's first readable palette.
    pub(super) pending_omarchy: [bool; 2],
    /// The one the picker shows while the pointer is over another.
    pub(super) preview: Option<Option<&'static Scheme>>,
    /// Light, dark, or as the system is.
    pub(super) appearance: Appearance,
}

impl Look {
    /// As `folders`' settings keep it; `omarchy` says whether Omarchy's
    /// theme is watched.
    pub(super) fn saved(folders: &Folders, omarchy: bool) -> Self {
        let saved = |key: &str| {
            folders
                .settings()
                .iter()
                .find_map(|path| settings::get(path, key))
        };
        let pending_omarchy = if omarchy && themes::omarchy::current().is_none() {
            omarchy::pending(saved)
        } else {
            [false; 2]
        };
        // Omarchy's theme is the default, for those who never picked one.
        let default = saved(settings::THEME).map_or_else(themes::omarchy::current, |_| None);
        // The one theme kept before light and dark had one each.
        let scheme = saved(settings::THEME).and_then(|name| {
            let scheme = themes::find(&name);
            if scheme.is_none() && name != themes::DEFAULT {
                tracing::warn!("no theme called {name:?}; using {}", themes::DEFAULT);
            }
            scheme
        });
        let slot = |key, dark: bool| match saved(key) {
            Some(name) if name == themes::omarchy::NAME && omarchy => themes::omarchy::current(),
            Some(name) => themes::fits(&name, dark)
                .inspect_err(|problem| {
                    tracing::warn!("{key}: {problem}; using {}", themes::DEFAULT);
                })
                .unwrap_or(None),
            None => scheme.filter(|scheme| scheme.dark() == dark).or(default),
        };
        // Before the appearance was kept, a theme picked set it.
        let appearance = match saved(settings::APPEARANCE) {
            Some(name) => Appearance::named(&name).unwrap_or_else(|| {
                tracing::warn!("no appearance called {name:?}; following the system");
                Appearance::System
            }),
            None => match scheme {
                Some(scheme) if scheme.dark() => Appearance::Dark,
                Some(_) => Appearance::Light,
                None => Appearance::System,
            },
        };
        Self {
            light_scheme: slot(settings::LIGHT_THEME, false),
            dark_scheme: slot(settings::DARK_THEME, true),
            pending_omarchy,
            preview: None,
            appearance,
        }
    }
}

impl MoonframeApp {
    /// Has the other main windows look as this one does now.
    pub(super) fn share_look(&self, cx: &mut Context<Self>) {
        let (me, look) = (self.id, self.look);
        cx.defer(move |cx| {
            for app in hub::every(cx) {
                app.update(cx, |app, cx| {
                    if app.id != me && app.look != look {
                        app.look = look;
                        app.apply_theme(cx);
                        cx.notify();
                    }
                });
            }
        });
    }

    /// Picks the theme `name` for the appearance the window is in (View >
    /// Themes lists only those that fit it), and saves it for the next run.
    pub(crate) fn pick_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        self.look.preview = None;
        let (light, dark) = if self.dark() {
            (None, Some(name))
        } else {
            (Some(name), None)
        };
        if let Err(problem) = self.set_themes(light, dark, cx) {
            tracing::warn!("{problem}");
        }
    }

    /// Picks the theme light windows show, the one dark windows show, or
    /// both, and saves them for the next run. Neither changes when a name
    /// is not a theme of its kind.
    pub(crate) fn set_themes(
        &mut self,
        light: Option<&str>,
        dark: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let light = light.map(|name| themes::fits(name, false)).transpose()?;
        let dark = dark.map(|name| themes::fits(name, true)).transpose()?;
        if let Some(scheme) = light {
            self.look.light_scheme = scheme;
            self.look.pending_omarchy[0] = false;
        }
        if let Some(scheme) = dark {
            self.look.dark_scheme = scheme;
            self.look.pending_omarchy[1] = false;
        }
        self.apply_theme(cx);
        self.share_look(cx);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[
                (
                    settings::LIGHT_THEME,
                    kept_name(self.look.light_scheme, self.look.pending_omarchy[0]),
                ),
                (
                    settings::DARK_THEME,
                    kept_name(self.look.dark_scheme, self.look.pending_omarchy[1]),
                ),
            ])
        {
            tracing::warn!("cannot save the themes: {problem}");
        }
        cx.notify();
        Ok(())
    }
}

/// Saving the other slot must not turn a pending Omarchy choice into Default.
fn kept_name(scheme: Option<&Scheme>, pending: bool) -> &'static str {
    if pending {
        themes::omarchy::NAME
    } else {
        name_of(scheme)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::kept_name,
        crate::{settings, themes},
    };

    #[test]
    fn saving_another_choice_keeps_a_pending_omarchy_slot() -> Result<(), String> {
        let home = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = home.path().join("settings.toml");
        settings::save(&path, &[
            (settings::LIGHT_THEME, kept_name(None, true)),
            (settings::DARK_THEME, kept_name(None, false)),
        ])?;
        assert_eq!(
            settings::get(&path, settings::LIGHT_THEME).as_deref(),
            Some(themes::omarchy::NAME)
        );
        assert_eq!(
            settings::get(&path, settings::DARK_THEME).as_deref(),
            Some(themes::DEFAULT)
        );
        Ok(())
    }
}
