//! No terminal processes, emulators, renderer or input hooks in this build.

use {
    gpui_kit::{FocusHandle, Window},
    moonframe_core::Data,
    std::{path::PathBuf, sync::Arc},
};

pub(crate) struct Terminals;

impl crate::app::MoonframeApp {
    pub(crate) fn terminal_keystroke(
        &mut self,
        _key: &gpui_kit::Keystroke,
        _window: &Window,
        _cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        false
    }
}

impl Terminals {
    pub(crate) fn new(
        _backends: Arc<crate::backends::Backends>,
        _wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self
    }

    pub(crate) fn begin_frame(&mut self) {}

    pub(crate) fn end_frame(&mut self) -> Option<FocusHandle> {
        None
    }

    pub(crate) fn take_exits(&mut self) -> Vec<(String, Data)> {
        Vec::new()
    }

    pub(crate) fn any_focused(&self, _window: &Window) -> bool {
        false
    }

    pub(crate) fn type_on_start(&mut self, _id: &str, _cwd: Option<PathBuf>, _bytes: Vec<u8>) {
        tracing::error!("this build has no terminal support");
    }
}
