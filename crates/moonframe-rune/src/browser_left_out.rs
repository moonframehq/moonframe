//! `ui.browser` where wry has no child view for Rune: the element's marker.

use {
    crate::Ui,
    moonframe_core::{BrowserCommand, Data},
    rune::{Tx, WindowHandle},
    std::{cell::RefCell, collections::HashMap, rc::Rc},
};

#[expect(
    dead_code,
    reason = "what a view hands over, for a platform with web views"
)]
pub(crate) struct Wanted {
    pub(crate) owner: String,
    pub(crate) url: Option<String>,
    pub(crate) navigate: Option<Data>,
    pub(crate) loading: Option<Data>,
    pub(crate) zoom: f32,
}

#[derive(Default)]
pub(crate) struct Browsers {
    pub(crate) parent: Rc<RefCell<Option<WindowHandle>>>,
    pub(crate) wanted: RefCell<HashMap<String, Wanted>>,
}

impl Browsers {
    pub(crate) fn can_show(&self) -> bool {
        false
    }

    pub(crate) fn built(&self, _: &str) -> bool {
        true
    }

    pub(crate) fn place(&self, _: &str, _: [f32; 4]) {}
}

impl Ui {
    pub(crate) fn visit(&self, _: &str, _: &str, _: &str) -> bool {
        false
    }

    pub(crate) fn settle_browsers(&mut self) -> bool {
        false
    }

    pub(crate) fn browser_command(&self, _: &str, _: &str, _: BrowserCommand, _: &mut Tx) {}
}
