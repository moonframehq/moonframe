//! `ui.video` where rings of shared memory aren't built: nothing to show.

use {rune::Pixels, std::cell::RefCell};

#[derive(Default)]
pub(crate) struct Videos {
    none: RefCell<Option<Pixels>>,
}

impl Videos {
    pub(crate) fn pixels(&self, _: &str) -> Pixels {
        self.none
            .borrow_mut()
            .get_or_insert_with(Pixels::new)
            .clone()
    }
}
