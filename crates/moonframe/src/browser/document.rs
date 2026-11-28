//! A `ui.document`'s page: a [`Page`] whose view is sealed (see
//! [`sealed`]), kept apart from the browsers by its key, and given its HTML,
//! look, editing switch and scrolling as the view can take them.

use {
    super::{Browsers, Event, Outbox, Page, State, sealed},
    moonframe_core::{Data, Document},
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::{Arc, atomic::AtomicBool},
    },
};

impl Browsers {
    /// The page the document `element` of the mod `owner` shows, with `html`
    /// (its own, or its file's) and the app's look, made now if it is new.
    /// Its view is built as it first lays out. Err says why there is none.
    pub(crate) fn open_document(
        &mut self,
        owner: &str,
        element: &Document,
        html: &str,
        dark: bool,
    ) -> Result<Rc<Page>, String> {
        // `#` is in no id: a browser of the same id is another page.
        let key = format!("{owner}/#document/{}", element.id);
        if let Some(page) = self.pages.get(&key) {
            page.update_document(element, html, dark);
            return Ok(page.clone());
        }
        self.room(owner)?;
        let mut document = sealed::State::default();
        document.update(element, html, dark);
        let page = Rc::new(Page {
            owner: owner.to_owned(),
            outbox: Outbox {
                key: key.clone(),
                sender: self.sender.clone(),
                wake: self.wake.clone(),
            },
            native: RefCell::new(None),
            local: Arc::new(AtomicBool::new(false)),
            // A document is shown at its own size.
            state: RefCell::new(State {
                zoom: 1.0,
                ..State::default()
            }),
            drawn: Cell::new(None),
            document: Some(RefCell::new(document)),
            dark: Cell::new(None),
        });
        self.pages.insert(key, page.clone());
        Ok(page)
    }
}

impl Page {
    fn update_document(&self, element: &Document, html: &str, dark: bool) {
        if let Some(document) = &self.document {
            document.borrow_mut().update(element, html, dark);
        }
        self.apply();
    }

    /// The view is built, and on macOS keeps its content rules: it may load
    /// the document.
    // Only the native web views seal; Linux has none yet.
    #[cfg_attr(
        not(all(feature = "browser", any(target_os = "macos", windows))),
        allow(dead_code)
    )]
    pub(crate) fn sealed(&self) {
        if let Some(document) = &self.document {
            document.borrow_mut().seal();
        }
        self.apply();
    }

    /// Gives the view what the document asks of it now.
    fn apply(&self) {
        let Some(document) = &self.document else {
            return;
        };
        let native = self.native.borrow();
        let Some(view) = &*native else {
            return;
        };
        let work = document.borrow_mut().work(self.dark.get());
        if let Some(dark) = work.dark {
            view.set_dark(dark);
            self.dark.set(Some(dark));
        }
        if let Some(html) = work.load {
            view.load_html(&html);
        }
        for (function, argument) in work.calls {
            view.call(function, &argument);
        }
    }

    /// The message `event` makes for a document's mod.
    pub(super) fn document_report(&self, event: Event) -> Option<Data> {
        let document = self.document.as_ref()?;
        match event {
            Event::Finished(_) => {
                document.borrow_mut().loaded();
                self.apply();
                None
            },
            Event::Document(reported) => document.borrow_mut().report(reported),
            Event::Started(_) | Event::Title(_) | Event::Open(_) => None,
        }
    }
}
