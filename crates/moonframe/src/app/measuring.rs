//! How Inspect measures: chosen on its banner or with 1 to 5 while
//! picking, and kept in `settings.toml` between runs.

use {super::*, crate::overlay::Measure};

impl MoonframeApp {
    /// The style saved last time, if any.
    pub(super) fn load_measure(&mut self) {
        let saved = self
            .folders
            .settings()
            .iter()
            .find_map(|path| settings::get(path, settings::INSPECT_MEASURE));
        if let Some(name) = saved {
            match Measure::named(&name) {
                Some(style) => self.inspector.measuring.style = style,
                None => tracing::warn!("no Inspect measure called {name:?}"),
            }
        }
    }

    /// Measures with `style` from now on, and saves it.
    pub(crate) fn set_measure(&mut self, style: Measure, cx: &mut Context<Self>) {
        self.inspector.measuring.style = style;
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[(settings::INSPECT_MEASURE, style.name())])
        {
            tracing::warn!("cannot save how Inspect measures: {problem}");
        }
        cx.notify();
    }

    /// 1 to 5 while picking, with no modifier: picks a style.
    pub(crate) fn measure_keystroke(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.inspector.picking || keystroke.modifiers.modified() {
            return false;
        }
        let Some(style) = Measure::numbered(&keystroke.key) else {
            return false;
        };
        self.set_measure(style, cx);
        true
    }
}
