//! Type-checking a miniapp's Luau without running it: `luau-lsp analyze`
//! in strict mode against the Moonframe declarations, as `just types` runs
//! it on the app. A wrong token or a value of the wrong type is an error
//! with its line, before the window ever reads the view.

use {
    super::{Diagnostic, Severity},
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU32, Ordering},
    },
};

/// The declarations of the whole Luau API, as this build knows them.
const DEFINITIONS: &str = include_str!("../../../../types/moonframe.d.luau");

/// What a miniapp without a `.luaurc` of its own is checked as: strict,
/// as the app is, and a missing `return` at a function's end is fine.
const BASE_LUAURC: &str = r#"{
  "languageMode": "strict",
  "lint": {
    "ImplicitReturn": false
  }
}
"#;

/// Type-checks every `.luau` file under `path` (but `*.d.luau`) with the
/// `luau-lsp` binary at `luau_lsp`. Type and syntax errors are errors,
/// lints warnings, each with its line. `Err` when the binary cannot run.
pub fn typecheck(path: &Path, luau_lsp: &Path) -> Result<Vec<Diagnostic>, String> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let mut files = Vec::new();
    let root = if path.is_file() {
        files.push(path.clone());
        path.parent().unwrap_or(&path).to_owned()
    } else if path.is_dir() {
        collect(&path, &mut files);
        path.clone()
    } else {
        return Ok(vec![Diagnostic::error(&path, "no such folder")]);
    };
    if files.is_empty() {
        return Ok(Vec::new());
    }
    files.sort();
    // luau-lsp names files relative to where it runs: run it in `root`
    // and give it the files from there.
    let relative: Vec<PathBuf> = files
        .iter()
        .map(|file| file.strip_prefix(&root).unwrap_or(file).to_owned())
        .collect();
    let scratch = Scratch::new()?;
    let definitions = scratch.write("moonframe.d.luau", DEFINITIONS)?;
    let luaurc = scratch.write("base.luaurc", BASE_LUAURC)?;
    let output = Command::new(luau_lsp)
        .arg("analyze")
        .arg("--platform=standard")
        .arg(format!("--base-luaurc={}", luaurc.display()))
        .arg(format!(
            "--definitions=@moonframe={}",
            definitions.display()
        ))
        .args(&relative)
        .current_dir(&root)
        .output()
        .map_err(|error| format!("could not run {}: {error}", luau_lsp.display()))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let found = parse(&text, &relative)
        .into_iter()
        .map(|diagnostic| Diagnostic {
            file: root.join(&diagnostic.file),
            ..diagnostic
        })
        .collect::<Vec<_>>();
    if output.status.code().is_none() || (!output.status.success() && found.is_empty()) {
        return Err(format!(
            "{} analyze failed ({}): {}",
            luau_lsp.display(),
            output.status,
            text.trim()
        ));
    }
    Ok(found)
}

/// The `.luau` files under `dir`, but declarations and hidden folders.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(&path, files);
        } else if name.ends_with(".luau") && !name.ends_with(".d.luau") {
            files.push(path);
        }
    }
}

/// Reads `luau-lsp analyze` output: `file(line,col): Kind: message`, the
/// message running on over the lines after it. Log lines (`[INFO] ...`)
/// are skipped.
fn parse(text: &str, files: &[PathBuf]) -> Vec<Diagnostic> {
    let mut found: Vec<Diagnostic> = Vec::new();
    for line in text.lines() {
        if let Some(diagnostic) = header(line, files) {
            found.push(diagnostic);
        } else if line.starts_with("[INFO]") || line.starts_with("[WARN]") {
            continue;
        } else if let Some(last) = found.last_mut() {
            last.message.push('\n');
            last.message.push_str(line.trim_end());
        }
    }
    found
}

/// One diagnostic's first line, if `line` starts with one of `files`.
fn header(line: &str, files: &[PathBuf]) -> Option<Diagnostic> {
    files.iter().find_map(|file| {
        let rest = line.strip_prefix(file.to_str()?)?.strip_prefix('(')?;
        let (at, rest) = rest.split_once("): ")?;
        let (row, column) = at.split_once(',')?;
        let row: u32 = row.parse().ok()?;
        column.parse::<u32>().ok()?;
        let (kind, message) = rest.split_once(": ")?;
        if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        let severity = match kind {
            "TypeError" | "SyntaxError" => Severity::Error,
            _ => Severity::Warning,
        };
        let message = format!("{kind}: {message}");
        let diagnostic = match severity {
            Severity::Error => Diagnostic::error(file, message),
            Severity::Warning => Diagnostic::warning(file, message),
        };
        Some(diagnostic.at_line(row))
    })
}

/// A folder of its own in the system's temporary one, removed when done.
struct Scratch(PathBuf);

impl Scratch {
    /// A fresh folder only this process can use: made with `create_dir`,
    /// never reusing one that is there (or a link planted in its place),
    /// with a name no one can guess ahead.
    fn new() -> Result<Self, String> {
        use std::hash::{BuildHasher, Hasher};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let mut last = None;
        for _ in 0..8 {
            let mut random = std::collections::hash_map::RandomState::new().build_hasher();
            random.write_u32(std::process::id());
            random.write_u32(NEXT.fetch_add(1, Ordering::Relaxed));
            random.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos()),
            );
            let dir =
                std::env::temp_dir().join(format!("moonframe-typecheck-{:016x}", random.finish()));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            match builder.create(&dir) {
                Ok(()) => return Ok(Self(dir)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    last = Some(error);
                },
                Err(error) => {
                    return Err(format!("could not create {}: {error}", dir.display()));
                },
            }
        }
        Err(format!(
            "could not create a scratch folder: {}",
            last.map_or_else(String::new, |error| error.to_string())
        ))
    }

    /// Writes a file that must not be there yet, so nothing is followed.
    fn write(&self, name: &str, text: &str) -> Result<PathBuf, String> {
        use std::io::Write as _;
        let path = self.0.join(name);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn reads_luau_lsp_output_with_lines_and_continuations() {
        let file = PathBuf::from("/tmp/app (copy)/main.luau");
        let text = "[INFO] Loading definitions file: @moonframe - x\n\
            /tmp/app (copy)/main.luau(4,17): TypeError: Expected this to be exactly 'StackProps'\n\
            caused by:\n  Property 'padding' is not compatible.\n\
            /tmp/app (copy)/main.luau(2,7): LocalUnused: Variable 'x' is never used\n";
        let found = parse(text, std::slice::from_ref(&file));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].severity, Severity::Error);
        assert_eq!(found[0].line, Some(4));
        assert_eq!(found[0].file, file);
        assert!(found[0].message.starts_with("TypeError: Expected"));
        assert!(
            found[0]
                .message
                .ends_with("Property 'padding' is not compatible.")
        );
        assert_eq!(found[1].severity, Severity::Warning);
        assert_eq!(found[1].line, Some(2));
    }

    /// The luau-lsp `scripts/setup-luau-lsp.sh` downloads, if it has.
    fn luau_lsp() -> Option<PathBuf> {
        let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tools");
        fs::read_dir(tools)
            .ok()?
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("luau-lsp-"))
            .map(|entry| entry.path().join("luau-lsp"))
            .find(|binary| binary.is_file())
    }

    #[test]
    fn a_number_for_a_space_token_is_an_error_on_its_line() {
        let Some(binary) = luau_lsp() else {
            println!("skipped: no luau-lsp in target/tools; run scripts/setup-luau-lsp.sh");
            return;
        };
        let scratch = Scratch::new().unwrap();
        let main = scratch
            .write(
                "main.luau",
                "return mod {\n\tview = function()\n\t\treturn ui.row { padding = 8 }\n\tend,\n}\n",
            )
            .unwrap();
        let found = typecheck(&scratch.0, &binary).unwrap();
        let error = found
            .iter()
            .find(|found| found.severity == Severity::Error)
            .unwrap_or_else(|| panic!("no error in {found:?}"));
        assert_eq!(error.file, main);
        assert_eq!(error.line, Some(3));
        assert!(error.message.contains("padding"), "{}", error.message);
    }

    #[test]
    fn a_binary_that_cannot_run_is_an_err() {
        let scratch = Scratch::new().unwrap();
        scratch.write("main.luau", "return 1\n").unwrap();
        assert!(typecheck(&scratch.0, Path::new("/no/such/luau-lsp")).is_err());
    }
}
