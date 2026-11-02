//! `<app> bundle [OUT]`: an app on the library, built as a standalone
//! bundle in OUT (default `target/bundle`), as `scripts/bundle-app.sh` does
//! for the starter. Run from a release build (`cargo run --release --
//! bundle`): the running binary is what ships.
//!
//! On macOS it is `<Title>.app`: the binary, the app's folder in
//! `Contents/Resources/app`, the toolbox in `Contents/Resources/toolbox`,
//! each with its wasm providers prebuilt and no Rust sources, an icon and
//! an Info.plist, signed ad hoc so it runs on this Mac. On Linux a folder
//! laid out like a prefix (`bin/`, `share/<binary>/`, a `.desktop` file and
//! an icon) to copy under `/usr/local` or `~/.local`.
//!
//! A bundle runs as a release: no Components menu or Inspect Element.
//!
//! With `--dist DIR` it is also packaged for a GitHub release: an archive
//! named for its version and platform, and its line in `SHA256SUMS`, what
//! Check for Updates (`Moonframe::updates`) looks for.

use {
    crate::paths::{self, Folders},
    std::{
        env, fs, io,
        path::{Path, PathBuf},
        process::{Command, ExitCode},
    },
};

const USAGE: &str = "\
Usage: <app> bundle [--dist DIR] [OUT]

Builds this app as a standalone bundle in OUT (default target/bundle), from
the running binary: run it from a release build, `cargo run --release --
bundle`. Its title, id, version and icon are the ones the app was launched
with (an icon.png in its app folder wins).

  --dist DIR   also write what a GitHub release of it carries to DIR: the
               bundle as <Title>-<version>-<target>.zip (macOS) or .tar.gz
               (Linux), and its line in DIR/SHA256SUMS, which Check for
               Updates verifies downloads with";

/// Moonframe's icon, for an app that has none.
const ICON: &[u8] = include_bytes!("../../../assets/icon.png");

pub fn run(folders: &Folders, words: &[String]) -> ExitCode {
    if matches!(words, [word] if matches!(word.as_str(), "help" | "-h" | "--help")) {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let mut out = None;
    let mut dist = None;
    let mut words = words.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--dist" => match words.next() {
                Some(dir) if dist.is_none() => dist = Some(PathBuf::from(dir)),
                _ => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                },
            },
            _ if out.is_none() && !word.starts_with('-') => out = Some(PathBuf::from(word)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            },
        }
    }
    let out = out.unwrap_or_else(|| PathBuf::from("target/bundle"));
    let built = bundle(folders, &out).and_then(|bundle| match &dist {
        Some(dist) => {
            let archive = package(&bundle, &crate::identity().version, dist)?;
            println!("{}", archive.display());
            Ok(bundle)
        },
        None => Ok(bundle),
    });
    match built {
        Ok(bundle) => {
            println!("{}", bundle.display());
            ExitCode::SUCCESS
        },
        Err(problem) => {
            eprintln!("{} bundle: {problem}", paths::name());
            ExitCode::FAILURE
        },
    }
}

fn bundle(folders: &Folders, out: &Path) -> Result<PathBuf, String> {
    let identity = crate::identity();
    let Some(host) = &folders.host else {
        return Err(
            "the starter bundles with `just bundle` (or `just bundle-app NAME` for a profile)"
                .to_owned(),
        );
    };
    if cfg!(debug_assertions) {
        return Err(
            "this is a debug build: bundle from a release one, `cargo run --release -- bundle`"
                .to_owned(),
        );
    }
    let title = folders.title();
    if !crate::packages::valid_title(&title) {
        return Err(format!("the app's title `{title}` cannot name a bundle"));
    }
    let exe = env::current_exe().map_err(|error| format!("cannot find this binary: {error}"))?;
    let exe_name = exe
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("this binary has no name")?
        .to_owned();
    let icon = match folders.icon() {
        Some(path) => fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?,
        None => identity.icon.unwrap_or(ICON).to_vec(),
    };
    let builds = paths::cache().join("bundle-providers");
    // The app's own id: a bundle is made before any profile names one.
    let id = identity.id.clone();

    if cfg!(target_os = "macos") {
        let bundle = out.join(format!("{title}.app"));
        remove(&bundle)?;
        let contents = bundle.join("Contents");
        let resources = contents.join("Resources");
        fs::create_dir_all(contents.join("MacOS")).map_err(text)?;
        copy_file(&exe, &contents.join("MacOS").join(&exe_name))?;
        stage(&host.path, &resources.join("app"), &builds)?;
        stage(&folders.builtin, &resources.join("toolbox"), &builds)?;
        icns(&icon, &resources.join("AppIcon.icns"))?;
        fs::write(
            contents.join("Info.plist"),
            info_plist(&title, &id, &exe_name, &identity.version),
        )
        .map_err(text)?;
        let signed = Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(&bundle)
            .output();
        if !signed.is_ok_and(|output| output.status.success()) {
            tracing::warn!("cannot sign {}; it may not open", bundle.display());
        }
        return Ok(bundle);
    }

    // Linux: a prefix, whose binary finds its files in share/<its name>.
    let bundle = out.join(&title);
    remove(&bundle)?;
    let share = bundle.join("share");
    fs::create_dir_all(bundle.join("bin")).map_err(text)?;
    copy_file(&exe, &bundle.join("bin").join(&exe_name))?;
    stage(&host.path, &share.join(&exe_name).join("app"), &builds)?;
    stage(
        &folders.builtin,
        &share.join(&exe_name).join("toolbox"),
        &builds,
    )?;
    let icons = share.join("icons/hicolor/512x512/apps");
    fs::create_dir_all(&icons).map_err(text)?;
    fs::write(icons.join(format!("{id}.png")), &icon).map_err(text)?;
    let applications = share.join("applications");
    fs::create_dir_all(&applications).map_err(text)?;
    fs::write(
        applications.join(format!("{id}.desktop")),
        format!(
            "[Desktop Entry]\nType=Application\nName={title}\nExec={exe_name}\nIcon={id}\n\
             StartupWMClass={id}\nCategories=Development;\n"
        ),
    )
    .map_err(text)?;
    Ok(bundle)
}

/// Writes `bundle` to `dist` as a GitHub release of `version` carries it:
/// `<Title>-<version>-<target>.zip` on macOS (`universal-apple-darwin` when
/// its binary has several architectures), `.tar.gz` on Linux, named so
/// Check for Updates picks it for its platform, and its line in
/// `dist/SHA256SUMS`, which it verifies the download with.
fn package(bundle: &Path, version: &str, dist: &Path) -> Result<PathBuf, String> {
    let file = bundle
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("the bundle has no name")?;
    let title = file.strip_suffix(".app").unwrap_or(file).replace(' ', "-");
    fs::create_dir_all(dist).map_err(text)?;
    let archive = if cfg!(target_os = "macos") {
        let exe = fs::read_dir(bundle.join("Contents/MacOS"))
            .map_err(text)?
            .flatten()
            .map(|entry| entry.path())
            .next()
            .ok_or("the bundle has no executable")?;
        let archs = Command::new("lipo")
            .arg("-archs")
            .arg(&exe)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        let archs: Vec<&str> = archs.split_whitespace().collect();
        let target = match archs.as_slice() {
            [one] => format!("{}-apple-darwin", rust_arch(one)),
            [] => format!("{}-apple-darwin", env::consts::ARCH),
            _ => "universal-apple-darwin".to_owned(),
        };
        let archive = dist.join(format!("{title}-{version}-{target}.zip"));
        remove(&archive)?;
        // ditto keeps the bundle's signature, links and permissions.
        command(
            Command::new("ditto")
                .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
                .arg(bundle)
                .arg(&archive),
        )?;
        archive
    } else {
        let target = format!("{}-unknown-linux-gnu", env::consts::ARCH);
        let archive = dist.join(format!("{title}-{version}-{target}.tar.gz"));
        remove(&archive)?;
        let parent = bundle.parent().ok_or("the bundle has no folder")?;
        command(
            Command::new("tar")
                .arg("-C")
                .arg(parent)
                .arg("-czf")
                .arg(&archive)
                .arg(file),
        )?;
        archive
    };
    let name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("the archive has no name")?
        .to_owned();
    let sum = sha256(&archive)?;
    let sums = dist.join("SHA256SUMS");
    let mut lines: Vec<String> = fs::read_to_string(&sums)
        .unwrap_or_default()
        .lines()
        .filter(|line| {
            line.split_whitespace()
                .nth(1)
                .is_none_or(|listed| listed.trim_start_matches('*') != name)
        })
        .map(str::to_owned)
        .collect();
    lines.push(format!("{sum}  {name}"));
    lines.sort_by(|a, b| {
        a.split_whitespace()
            .nth(1)
            .cmp(&b.split_whitespace().nth(1))
    });
    fs::write(&sums, lines.join("\n") + "\n").map_err(text)?;
    Ok(archive)
}

/// Rust's name for a Mach-O architecture.
fn rust_arch(arch: &str) -> &str {
    match arch {
        "arm64" => "aarch64",
        other => other,
    }
}

/// `file`'s SHA-256, in hex, from the system's tool.
fn sha256(file: &Path) -> Result<String, String> {
    let (tool, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("shasum", &["-a", "256"])
    } else {
        ("sha256sum", &[])
    };
    let output = Command::new(tool)
        .args(args)
        .arg(file)
        .output()
        .map_err(|error| format!("cannot run {tool}: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let sum = text.split_whitespace().next().unwrap_or_default();
    if !output.status.success() || sum.len() != 64 {
        return Err(format!("{tool} cannot read {}", file.display()));
    }
    Ok(sum.to_owned())
}

fn command(command: &mut Command) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run {command:?}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{command:?} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Copies the app folder `from` to `to` as it ships: without what git and
/// builds keep, and with each wasm provider built to `provider.wasm` in
/// place of its Rust sources, so users need no toolchain.
fn stage(from: &Path, to: &Path, builds: &Path) -> Result<(), String> {
    copy_tree(from, to).map_err(|error| format!("cannot copy {}: {error}", from.display()))?;
    // A build without the runtime never runs them: none is worth building.
    if !cfg!(feature = "wasm") {
        return Ok(());
    }
    let Ok(providers) = fs::read_dir(to.join("providers")) else {
        return Ok(());
    };
    for provider in providers.flatten().map(|entry| entry.path()) {
        // Built where it is written, whose paths (to `wit/`, say) are
        // relative to it there.
        let Some(name) = provider.file_name() else {
            continue;
        };
        let manifest = from.join("providers").join(name).join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let package = package_name(&manifest)?;
        let built = Command::new("cargo")
            .args(["build", "--quiet", "--release", "--target", "wasm32-wasip2"])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(builds)
            .status()
            .map_err(|error| format!("cannot run cargo: {error}"))?;
        if !built.success() {
            return Err(format!("cannot build the provider {}", provider.display()));
        }
        let wasm = builds
            .join("wasm32-wasip2/release")
            .join(format!("{}.wasm", package.replace('-', "_")));
        copy_file(&wasm, &provider.join("provider.wasm"))?;
        for source in ["Cargo.toml", "Cargo.lock", "src", "target"] {
            remove(&provider.join(source))?;
        }
    }
    Ok(())
}

fn package_name(manifest: &Path) -> Result<String, String> {
    let text = fs::read_to_string(manifest).map_err(text)?;
    let value: toml::Table =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", manifest.display()))?;
    value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} names no package", manifest.display()))
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(".claude" | ".git" | "target" | ".DS_Store" | ".moonframe-embedded")
        ) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            copy_tree(&path, &to.join(&name))?;
        } else {
            fs::copy(&path, to.join(&name))?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("cannot copy {}: {error}", from.display()))
}

fn remove(path: &Path) -> Result<(), String> {
    let removed = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match removed {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(format!("cannot remove {}: {error}", path.display()))
        },
        _ => Ok(()),
    }
}

fn text(error: io::Error) -> String {
    error.to_string()
}

/// The icon at every size Finder and the Dock ask for, with `sips` and
/// `iconutil`.
fn icns(png: &[u8], out: &Path) -> Result<(), String> {
    let work = paths::cache().join("bundle-icon");
    remove(&work)?;
    fs::create_dir_all(&work).map_err(text)?;
    let source = work.join("icon.png");
    fs::write(&source, png).map_err(text)?;
    let set = work.join("AppIcon.iconset");
    fs::create_dir_all(&set).map_err(text)?;
    for size in [16, 32, 128, 256, 512] {
        let mut sizes = vec![(size, format!("icon_{size}x{size}.png"))];
        if size * 2 <= 512 {
            sizes.push((size * 2, format!("icon_{size}x{size}@2x.png")));
        }
        for (pixels, name) in sizes {
            let resized = Command::new("sips")
                .args(["-z", &pixels.to_string(), &pixels.to_string()])
                .arg(&source)
                .arg("--out")
                .arg(set.join(name))
                .output()
                .map_err(|error| format!("cannot run sips: {error}"))?;
            if !resized.status.success() {
                return Err("sips cannot read the icon: is it a PNG?".to_owned());
            }
        }
    }
    let made = Command::new("iconutil")
        .args(["-c", "icns"])
        .arg(&set)
        .arg("-o")
        .arg(out)
        .output()
        .map_err(|error| format!("cannot run iconutil: {error}"))?;
    let _ = fs::remove_dir_all(&work);
    if made.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&made.stderr).trim().to_owned())
    }
}

fn info_plist(title: &str, id: &str, exe: &str, version: &str) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let (title, id, exe, version) = (escape(title), escape(id), escape(exe), escape(version));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>{title}</string>
    <key>CFBundleDisplayName</key><string>{title}</string>
    <key>CFBundleIdentifier</key><string>{id}</string>
    <key>CFBundleExecutable</key><string>{exe}</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{version}</string>
    <key>CFBundleVersion</key><string>{version}</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#
    )
}
