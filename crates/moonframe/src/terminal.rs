//! Optional terminal sessions; shared keyboard routing lives in `crate::keys`.

#[cfg(feature = "terminal")]
mod sessions;
#[cfg(feature = "terminal")]
pub(crate) use sessions::{Selection, Spot, TerminalView, Terminals};

#[cfg(not(feature = "terminal"))]
impl crate::app::MoonframeApp {
    pub(crate) fn terminal_keystroke(
        &mut self,
        _: &gpui_kit::Keystroke,
        _: &gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> bool {
        false
    }
}
