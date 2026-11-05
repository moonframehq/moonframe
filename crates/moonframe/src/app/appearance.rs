//! Picking and keeping the themes for each appearance.

use super::*;

impl MoonframeApp {
    /// Picks the theme `name` for the appearance the window is in (View >
    /// Themes lists only those that fit it), and saves it for the next run.
    pub(crate) fn pick_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        self.preview = None;
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
            self.light_scheme = scheme;
            self.pending_omarchy[0] = false;
        }
        if let Some(scheme) = dark {
            self.dark_scheme = scheme;
            self.pending_omarchy[1] = false;
        }
        self.apply_theme(cx);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[
                (
                    settings::LIGHT_THEME,
                    kept_name(self.light_scheme, self.pending_omarchy[0]),
                ),
                (
                    settings::DARK_THEME,
                    kept_name(self.dark_scheme, self.pending_omarchy[1]),
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
