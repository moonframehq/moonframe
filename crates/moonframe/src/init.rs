//! `moonframe init [DIR]`: an app of its own on the Moonframe library. In a
//! new folder, a whole crate (examples/host, renamed); in an existing crate,
//! the app folder and what agents need beside it, and what to add to its
//! Cargo.toml and `main` (printed, never written into its sources).

use {
    crate::packages,
    std::{
        fs,
        path::{Path, PathBuf},
        process::ExitCode,
    },
};

const USAGE: &str = "\
moonframe init: an app of your own on the Moonframe library

Usage: moonframe init [DIR] [OPTIONS]

  DIR             the crate's folder (default: the current one). Without a
                  Cargo.toml there it becomes a new crate: Cargo.toml,
                  src/main.rs, and the app folder. With one, only the app
                  folder and the files below are added; what to put in your
                  Cargo.toml and `main` is printed

Options:
  --name NAME     the app's name: its folders (~/.config/NAME) and its
                  binary (default: DIR's name)
  --title TITLE   its name for people (default: from NAME)
  --path CHECKOUT depend on a checkout of Moonframe instead of its git
                  repository

It writes, keeping any file already there:
  app/main.luau, app/style.luau      the shell and the stylesheet
  app/providers/clock/provider.toml  the example's native provider
And refreshes what follows Moonframe's version:
  types/moonframe.d.luau             types for luau-lsp
  .claude/skills/moonframe*/         the guides agents read to build in it";

/// The example host app, renamed: the template.
const MAIN_RS: &str = include_str!("../../../examples/host/src/main.rs");
const MAIN_LUAU: &str = include_str!("../../../examples/host/app/main.luau");
const STYLE_LUAU: &str = include_str!("../../../examples/host/app/style.luau");
const CLOCK_TOML: &str = include_str!("../../../examples/host/app/providers/clock/provider.toml");
const TOOLCHAIN: &str = include_str!("../../../rust-toolchain.toml");
const TYPES: &str = include_str!("../../../types/moonframe.d.luau");
const SKILL: &str = include_str!("../../../.claude/skills/moonframe/SKILL.md");
const COMPONENTS_SKILL: &str =
    include_str!("../../../.claude/skills/moonframe-components/SKILL.md");

const REPOSITORY: &str = "https://github.com/penso/moonframe";

struct Options {
    dir: PathBuf,
    name: Option<String>,
    title: Option<String>,
    path: Option<PathBuf>,
}

fn parse(words: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options {
        dir: PathBuf::from("."),
        name: None,
        title: None,
        path: None,
    };
    let mut dir = None;
    let mut words = words.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "help" | "-h" | "--help" => return Ok(None),
            "--name" => options.name = Some(words.next().ok_or("--name needs a name")?.clone()),
            "--title" => options.title = Some(words.next().ok_or("--title needs a title")?.clone()),
            "--path" => {
                options.path = Some(PathBuf::from(
                    words.next().ok_or("--path needs Moonframe's checkout")?,
                ));
            },
            flag if flag.starts_with('-') => return Err(format!("unknown option: {flag}")),
            path if dir.is_none() => dir = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument: {extra}")),
        }
    }
    if let Some(dir) = dir {
        options.dir = dir;
    }
    Ok(Some(options))
}

pub(crate) fn run(words: &[String]) -> ExitCode {
    let options = match parse(words) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        },
        Err(problem) => {
            eprintln!("moonframe init: {problem}\n\n{USAGE}");
            return ExitCode::from(2);
        },
    };
    match init(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("moonframe init: {problem}");
            ExitCode::FAILURE
        },
    }
}

fn init(options: &Options) -> Result<(), String> {
    fs::create_dir_all(&options.dir)
        .map_err(|error| format!("cannot create {}: {error}", options.dir.display()))?;
    let dir = options
        .dir
        .canonicalize()
        .map_err(|error| format!("{}: {error}", options.dir.display()))?;
    let name = match &options.name {
        Some(name) => name.clone(),
        None => dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(moonframe_core::mod_name)
            .unwrap_or_default(),
    };
    if !packages::valid_name(&name) {
        return Err(format!(
            "`{name}` cannot name an app: pass --name with a-z, 0-9, - and _"
        ));
    }
    let title = options.title.clone().unwrap_or_else(|| titled(&name));
    let dependency = dependency(options.path.as_deref())?;
    let template = Template {
        name: &name,
        title: &title,
    };
    let existing = dir.join("Cargo.toml").is_file();
    let mut wrote = Vec::new();

    let mut keep = |relative: &str, text: &str| -> Result<(), String> {
        let path = dir.join(relative);
        if path.exists() {
            println!("kept      {relative}");
            return Ok(());
        }
        write(&path, text)?;
        wrote.push(relative.to_owned());
        println!("wrote     {relative}");
        Ok(())
    };
    if !existing {
        keep("Cargo.toml", &template.cargo_toml(&dependency))?;
        keep("src/main.rs", &template.main_rs())?;
        keep("rust-toolchain.toml", TOOLCHAIN)?;
        keep(".gitignore", "/target\n")?;
    }
    keep("app/main.luau", &template.text(MAIN_LUAU))?;
    keep("app/style.luau", STYLE_LUAU)?;
    keep("app/providers/clock/provider.toml", CLOCK_TOML)?;
    for (relative, text) in [
        ("types/moonframe.d.luau", TYPES),
        (".claude/skills/moonframe/SKILL.md", SKILL),
        (
            ".claude/skills/moonframe-components/SKILL.md",
            COMPONENTS_SKILL,
        ),
    ] {
        let path = dir.join(relative);
        if fs::read_to_string(&path).is_ok_and(|old| old == text) {
            println!("current   {relative}");
            continue;
        }
        let verb = if path.exists() {
            "updated"
        } else {
            "wrote"
        };
        write(&path, text)?;
        println!("{verb:<9} {relative}");
    }

    println!();
    if existing {
        println!(
            "Add to {}:\n\n  [dependencies]\n  {dependency}\n\n  # In the workspace's root \
             Cargo.toml: GPUI and Luau are unusably slow unoptimised.\n  \
             [profile.dev.package.\"*\"]\n  opt-level = 2\n",
            dir.join("Cargo.toml").display()
        );
        println!(
            "Then run the app from `main` (src/main.rs in a new crate shows the rest: a \
             native provider and a view):\n\n  fn main() -> std::process::ExitCode {{\n      \
             moonframe::Moonframe::new({name:?})\n          .title({title:?})\n          \
             .version(env!(\"CARGO_PKG_VERSION\"))\n          \
             // .updates(\"owner/repo\") // Check for Updates from its GitHub releases\n          \
             .app_dir(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/app\"))\n          \
             .run()\n  }}\n"
        );
        if wrote.iter().any(|path| path.ends_with("provider.toml")) {
            println!(
                "app/providers/clock/provider.toml runs a native provider named `clock`: \
                 register one (`.native(\"clock\", ...)`) or delete the folder.\n"
            );
        }
    } else {
        println!("Run it:  cd {} && cargo run", dir.display());
    }
    println!("Check it after each edit:  cargo run -- --check");
    println!("Bundle it:  cargo run --release -- bundle");
    println!(
        "Release it:  cargo run --release -- bundle --dist dist, then attach dist/* to a \
         GitHub release; with `.updates(\"owner/repo\")` the app finds it (docs/updates.md)"
    );
    Ok(())
}

/// `my-notes` as `My Notes`.
fn titled(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `moonframe = ...` line for Cargo.toml.
fn dependency(checkout: Option<&Path>) -> Result<String, String> {
    let Some(checkout) = checkout else {
        // Cargo.lock pins the commit; `cargo update -p moonframe` moves it.
        return Ok(format!(
            "moonframe = {{ git = \"{REPOSITORY}\", branch = \"main\" }}"
        ));
    };
    let crate_dir = checkout
        .join("crates/moonframe")
        .canonicalize()
        .map_err(|error| format!("{}: no Moonframe checkout ({error})", checkout.display()))?;
    if !crate_dir.join("Cargo.toml").is_file() {
        return Err(format!("{}: no Moonframe checkout", checkout.display()));
    }
    Ok(format!(
        "moonframe = {{ path = \"{}\" }}",
        crate_dir.display().to_string().replace('\\', "/")
    ))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// The example host app's files, as the new app's.
struct Template<'a> {
    name: &'a str,
    title: &'a str,
}

impl Template<'_> {
    /// `text` with the example's name and title replaced by the app's.
    fn text(&self, text: &str) -> String {
        text.replace(
            "dev.moonframe.host-example",
            &format!("dev.moonframe.{}", self.name),
        )
        .replace("host-example", self.name)
        .replace("Host Example", self.title)
    }

    fn main_rs(&self) -> String {
        self.text(MAIN_RS).replace(
            "`cargo run -p moonframe-host-example` opens it",
            "`cargo run` opens it",
        )
    }

    fn cargo_toml(&self, dependency: &str) -> String {
        format!(
            "[package]\nedition = \"2024\"\nname = \"{name}\"\npublish = false\nversion = \"0.1.0\"\n\n\
             [dependencies]\n{dependency}\n\n\
             # GPUI and Luau are unusably slow unoptimised; keep your own crate\n\
             # debuggable.\n[profile.dev.package.\"*\"]\nopt-level = 2\n\n\
             # A crate of its own, even inside another's folder.\n[workspace]\n",
            name = self.name
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_new_crate_is_the_example_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("my-notes");
        init(&Options {
            dir: target.clone(),
            name: None,
            title: None,
            path: None,
        })
        .unwrap();
        let main = fs::read_to_string(target.join("src/main.rs")).unwrap();
        assert!(main.contains("Moonframe::new(\"my-notes\")"), "{main}");
        assert!(main.contains(".title(\"My Notes\")"), "{main}");
        assert!(main.contains("dev.moonframe.my-notes"), "{main}");
        assert!(!main.contains("host-example"), "{main}");
        let cargo = fs::read_to_string(target.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"my-notes\""), "{cargo}");
        assert!(
            cargo.contains("git = \"https://github.com/penso/moonframe\""),
            "{cargo}"
        );
        let value: toml::Table = toml::from_str(&cargo).unwrap();
        assert!(value.contains_key("workspace"));
        for file in [
            "app/main.luau",
            "app/style.luau",
            "app/providers/clock/provider.toml",
            "types/moonframe.d.luau",
            ".claude/skills/moonframe/SKILL.md",
            ".claude/skills/moonframe-components/SKILL.md",
        ] {
            assert!(target.join(file).is_file(), "{file}");
        }
        // Again: what the user wrote stays.
        fs::write(target.join("app/main.luau"), "-- mine").unwrap();
        init(&Options {
            dir: target.clone(),
            name: None,
            title: None,
            path: None,
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(target.join("app/main.luau")).unwrap(),
            "-- mine"
        );
    }

    #[test]
    fn an_existing_crate_keeps_its_sources() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"herdr\"\n",
        )
        .unwrap();
        init(&Options {
            dir: dir.path().to_owned(),
            name: Some("herdr".to_owned()),
            title: None,
            path: None,
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("Cargo.toml")).unwrap(),
            "[package]\nname = \"herdr\"\n"
        );
        assert!(!dir.path().join("src/main.rs").exists());
        assert!(dir.path().join("app/main.luau").is_file());
    }

    #[test]
    fn names_become_titles() {
        assert_eq!(titled("my-notes"), "My Notes");
        assert_eq!(titled("herdr"), "Herdr");
    }
}
