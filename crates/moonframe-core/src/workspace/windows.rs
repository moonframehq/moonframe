//! Window state for mods opened in windows of their own.

use super::*;

impl Workspace {
    /// Shares a mod's own window state with its view and the instances it
    /// draws. `None` closes that context, returning to the main window's
    /// state. Like its state, a mod's view is shared wherever it is mounted.
    pub fn set_mod_window(&mut self, name: &str, window: Option<WindowState>) {
        if name == SHELL || self.mod_windows.get(name).copied() == window {
            return;
        }
        match window {
            Some(window) => {
                self.mod_windows.insert(name.to_owned(), window);
            },
            None => {
                self.mod_windows.remove(name);
            },
        }
        self.settle();
    }
}
