//! The starter app: Moonframe on its own `app/` folder, which the user's
//! `~/.config/moonframe` goes over. An app of your own runs the same way on
//! the library (see `moonframe init`).

fn main() -> std::process::ExitCode {
    moonframe::Moonframe::starter().run()
}
