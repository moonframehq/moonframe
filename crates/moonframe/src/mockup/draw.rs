//! `render` and `page`: the variants drawn headless to PNGs, and the page
//! that lays them out for Sheprd (or any browser) to pick from. Only a build
//! with the `capture` feature has a headless renderer.

mod page;
mod render;

use {
    super::{USAGE, check, read},
    moonframe_core::Roots,
    std::path::{Path, PathBuf},
};

/// One variant as drawn.
struct Shot {
    letter: String,
    name: String,
    note: Option<String>,
    /// Its width in pixels, as the page shows it.
    width: f32,
    /// `(appearance, png)` for each appearance drawn.
    files: Vec<(String, PathBuf)>,
}

/// `A` for the first variant, then `B`, ...
fn letter(index: usize) -> String {
    let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    match letters.get(index) {
        Some(letter) => char::from(*letter).to_string(),
        None => format!("V{}", index + 1),
    }
}

fn png_name(letter: &str, appearance: &str) -> String {
    let letter = letter.to_lowercase();
    match appearance {
        "dark" => format!("{letter}-dark.png"),
        _ => format!("{letter}.png"),
    }
}

/// `--out DIR`, `--light`, `--dark`, `--open` from `rest`.
struct Flags {
    out: Option<PathBuf>,
    appearances: Vec<&'static str>,
    open: bool,
}

fn flags(rest: &[String], page: bool) -> Result<Flags, String> {
    let mut flags = Flags {
        out: None,
        appearances: vec!["light", "dark"],
        open: false,
    };
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--out" => flags.out = Some(PathBuf::from(words.next().ok_or("--out needs a folder")?)),
            "--light" => flags.appearances = vec!["light"],
            "--dark" => flags.appearances = vec!["dark"],
            "--open" if page => flags.open = true,
            other => return Err(format!("unexpected argument: {other}\n\n{USAGE}")),
        }
    }
    Ok(flags)
}

/// Where the PNGs go: `--out`, else `<file's folder>/<stem>.mockup`.
fn out_dir(file: &Path, out: Option<PathBuf>) -> PathBuf {
    out.unwrap_or_else(|| {
        let stem = file
            .file_stem()
            .map_or("mockup".into(), |stem| stem.to_string_lossy());
        file.with_file_name(format!("{stem}.mockup"))
    })
}

/// `moonframe mockup render FILE [--out DIR] [--light | --dark]`.
pub(super) fn render_command(roots: &Roots, file: &Path, rest: &[String]) -> Result<(), String> {
    let flags = flags(rest, false)?;
    render_into(roots, file, &out_dir(file, flags.out), &flags.appearances).map(|_| ())
}

/// Checks `file`, then draws it into `out`, saying where each PNG went.
fn render_into(
    roots: &Roots,
    file: &Path,
    out: &Path,
    appearances: &[&str],
) -> Result<(Option<String>, Vec<Shot>), String> {
    check(roots, file)?;
    let (ask, shots) = render::render(&read(file)?, roots, out, appearances)?;
    for shot in &shots {
        for (_, path) in &shot.files {
            println!("{} {}: {}", shot.letter, shot.name, path.display());
        }
    }
    Ok((ask, shots))
}

/// `moonframe mockup page FILE [--out DIR] [--open]`.
pub(super) fn page_command(roots: &Roots, file: &Path, rest: &[String]) -> Result<(), String> {
    let flags = flags(rest, true)?;
    let file = std::path::absolute(file).map_err(|error| error.to_string())?;
    let out = out_dir(&file, flags.out);
    let (ask, shots) = render_into(roots, &file, &out, &flags.appearances)?;
    let page = page::write(&out, &file, ask.as_deref(), &shots)?;
    println!("page: {}", page.display());
    if flags.open {
        show_in_sheprd(&page)?;
    }
    Ok(())
}

/// Opens `page` in a Sheprd browser tab beside the agent's terminal.
fn show_in_sheprd(page: &Path) -> Result<(), String> {
    let home = std::env::var_os("HOME").ok_or("no HOME to find Sheprd in")?;
    let tool = PathBuf::from(home).join(".local/state/herdr/gpui/bin/herdr-gpui");
    if !tool.is_file() {
        return Err(format!(
            "Sheprd is not installed here: open {} in a browser",
            page.display()
        ));
    }
    let status = std::process::Command::new(tool)
        .args(["browser", "open"])
        .arg(page)
        .status()
        .map_err(|error| error.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("Sheprd could not open {}", page.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_names() {
        assert_eq!(letter(0), "A");
        assert_eq!(letter(25), "Z");
        assert_eq!(letter(26), "V27");
        assert_eq!(png_name("B", "dark"), "b-dark.png");
        assert_eq!(png_name("B", "light"), "b.png");
    }

    #[test]
    fn the_out_folder_sits_beside_the_file() {
        assert_eq!(
            out_dir(Path::new("/x/bar.luau"), None),
            PathBuf::from("/x/bar.mockup")
        );
        assert_eq!(
            out_dir(Path::new("/x/bar.luau"), Some("/y".into())),
            PathBuf::from("/y")
        );
    }

    #[test]
    fn flags_read_out_and_appearance() -> Result<(), String> {
        let words = ["--out", "/o", "--dark"].map(str::to_owned);
        let read = flags(&words, false)?;
        assert_eq!(read.out, Some(PathBuf::from("/o")));
        assert_eq!(read.appearances, vec!["dark"]);
        assert!(flags(&["--open".to_owned()], false).is_err());
        assert!(flags(&["--open".to_owned()], true)?.open);
        Ok(())
    }
}
