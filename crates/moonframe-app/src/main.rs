//! The starter app: Moonframe on its own `app/` folder, which the user's
//! `~/.config/moonframe` goes over. An app of your own runs the same way on
//! the library (see `moonframe init`).
//!
//! The `gpui` and `rune` features choose what draws it; `app/` is the same
//! for both. The `moonframe` dependency being on is what means GPUI: `gpui`
//! turns it on, and so does any `moonframe/<capability>` alone.

#[cfg(not(any(feature = "moonframe", feature = "rune")))]
compile_error!("choose what draws the window: the `gpui` or `rune` feature");

#[cfg(feature = "rune")]
fn main() -> std::process::ExitCode {
    moonframe_rune::Moonframe::starter().run()
}

#[cfg(all(feature = "moonframe", not(feature = "rune")))]
fn main() -> std::process::ExitCode {
    moonframe::Moonframe::starter().run()
}
