# Moonframe as a library

The `moonframe` binary is the starter app. The same window, with its Luau
shell, mods, components, providers, packages and profiles, is a library
crate any app can depend on, so an app with Rust of its own never has to
fork this repository.

## Start one

```sh
moonframe init ~/code/notes                 # a new crate: Cargo.toml, src/main.rs, app/
moonframe init ~/code/herdr --name herdr    # an existing crate: app/ and docs only
moonframe init ~/code/notes --path ~/code/moonframe   # depend on a checkout
```

`init` writes the app folder (`app/main.luau`, `app/style.luau`, an example
native provider's `provider.toml`), `types/moonframe.d.luau` for luau-lsp,
and the `moonframe` and `moonframe-components` skills in `.claude/skills`
so an agent can build in it. It never overwrites a file you have; the types
and skills, which follow Moonframe's version, are refreshed. In an existing
crate it leaves `Cargo.toml` and `src/` alone and prints what to add.

The new crate is [examples/host](../examples/host), renamed.

## The dependency

```toml
[dependencies]
moonframe = { git = "https://github.com/moonframehq/moonframe", branch = "main" }

# In the workspace root: GPUI and Luau are unusably slow unoptimised.
[profile.dev.package."*"]
opt-level = 2
```

`Cargo.lock` pins the commit (`cargo update -p moonframe` moves it). The
library needs the toolchain in [rust-toolchain.toml](../rust-toolchain.toml)
or newer.

Features: `acp`, `alacritty`, `browser`, `chat`, `docs`, `forge`, `git`,
`herdr-core`, `markdown`, `prose`, `shell`, `studio`, `tokio`, `updates`, `usage` and
`wasm` by default. `ghostty` (a second terminal emulator, built from Ghostty's
source with Zig; see `mise.toml`) is opt-in: `features = ["ghostty"]`, and
so are `map` (`ui.map` street maps), `speech` (dictation), `video` (`ui.video`) and what long runs
and live machines ask for (`awake`, `crash`, `displays`, `dock`, `guard`,
`restart`, `socket`), which few apps need. The starter has them all.

An app that needs none of git, the markdown editor or kept terminals can
leave them out, and with them the herdr-gpui crates they build on:

```toml
moonframe = { default-features = false, features = ["acp", "alacritty", "tokio"], git = "..." }
```

A built-in provider a feature leaves out (`git`, `docs`, `acp`) publishes
nothing: the starter's mods reading `app.git` see nil and carry on, and only
a message to it reports that the feature is off.

`wasm` is the runtime for sandboxed WebAssembly (WASM) providers (wasmtime, the
largest dependency). An app whose providers are all native or processes
leaves it out to build faster and ship a smaller binary; the line above
does, with no `wasm`. Its WASM providers then behave the same way, the
toolbox's `system` among them: nothing published, and a message to one
says the feature is off. `bundle` then builds none of them.

To add a sandboxed component alongside native providers, follow
[Building WebAssembly (WASM) providers](wasm-providers.md).

### Choosing capabilities

Flags follow native capabilities and their dependencies. Basic UI and the
Luau component library remain available in ordinary builds; sealed bundles
ship only their selected components.

| Feature | What it includes |
| --- | --- |
| `awake` | `app:awake`: keeping the machine from sleeping during a long job, on macOS, Windows and Linux. Off by default. See [Live video](video.md#long-runs). |
| `crash` | A panic leaves a report in the state folder, and the next launch offers it as `app.crash`. Off by default. |
| `displays` | `app.displays`, and a mod's window placed with `display` and `fullscreen`. Off by default; without it such a window opens where it would have, and the mod hears why. |
| `dock` | `app:dock`: a badge and a progress bar on the Dock icon (macOS). Off by default. |
| `guard` | `app:guard`: a mod holds back closing and quitting. Off by default. |
| `restart` | `app:restart`, and `restart = "on-failure"` for process providers. Off by default; without it a process that goes stays gone. |
| `socket` | Socket providers (`kind = "socket"`), on Unix. Off by default; without it one fails as it starts. |
| `browser` | Native web views and Wry on macOS/Windows. Linux keeps its system-browser fallback. |
| `terminal` | Terminal rendering, local PTYs and tmux; choose an engine below to run a shell. |
| `alacritty`, `ghostty` | Each enables `terminal` and its respective emulator. |
| `herdr-core` | herdr-core state storage and, with `terminal`, persistent terminals. |
| `video` | `ui.video`: live frames from another process in a 16-bit, colour-managed Metal view (macOS). Off by default. See [Live video](video.md). |
| `map` | `ui.map` street maps: tiles from OpenStreetMap and other listed providers, pins, and routes along streets, fetched over HTTP. Off by default. |
| `map-benchmark` | A `ui.map` run with `MOONFRAME_MAP_BENCHMARK` drives itself through input events, reports what that cost, and quits. For measuring; off by default. |
| `markdown` | Rich `ui.markdown` rendering (headings, lists, tables, links and code). |
| `prose` | The writing editor for `ui.input` with `prose`, `dim` or `typewriter`. Ordinary single-line and multiline inputs remain available without it. |
| `sealed` | Staged app resources instead of the full embedded toolbox. |
| `studio` | Development commands and the component gallery. |
| `updates` | Release checks, downloads and in-place app updates. Omitting it hides the updater menu and disables automatic checks. |
| `acp`, `chat`, `docs`, `forge`, `git`, `shell`, `usage` | The respective built-in native providers. |
| `speech` | The `speech` native provider: dictation from the microphone (`use "dictation"`), through Apple's on-device recognizer, a local program or a remote service. Opt-in. |
| `tokio` | Async native-provider support. |
| `wasm` | Wasmtime and building/running Wasm providers. |

An app with just ordinary UI can use `default-features = false` with no
features. SQLite state saving still works. Without `terminal`, Moonframe's
terminal crate and PTY dependency are omitted. `herdr-core` is independent:
it can keep state without Moonframe terminals, although its upstream client
currently brings its own terminal dependencies.

Views can check `app.capabilities.browser`, `app.capabilities.alacritty`,
`app.capabilities.wasm`, etc. Every feature above has a boolean entry;
hyphenated names use brackets (`app.capabilities["herdr-core"]`). These are
compiled capabilities, not checks that a backend is installed, a provider
is configured, or the platform implements native web views. They are present
in headless checks as well as the window. Requesting an omitted browser or
terminal displays a diagnostic explaining which feature to enable.
The same applies to rich Markdown and writing fields. The `docs` file
provider, `markdown` renderer and `prose` editor are independent; select the
ones the app uses. For example, a Markdown preview needs `markdown`, while
a prose-writing app needs `prose`. `chat-markdown` draws through the native
renderer, so it needs `markdown` too. `markdown` brings `syntax`, the
tree-sitter grammars its code blocks are highlighted with, and `math` adds
LaTeX through latex-rust, with its font.

`updates` controls the host's updater, not all networking: providers such
as `forge` and `usage` can bring their own HTTP dependencies. With every
optional capability off, Ureq, Rustls and Ring are absent. Some shared GPUI
dependencies (including Markdown parsing and image codecs) are unconditional
upstream, so renderer flags remove linked code but do not eliminate those
crates from the build graph.

The starter forwards these same flags explicitly:

```sh
# Full starter (including both terminal engines), as before:
cargo build -p moonframe-app --release
# The library's defaults, including Alacritty, without Ghostty/Zig:
cargo build -p moonframe-app --release --no-default-features --features moonframe/default
# Basic UI, with no optional capabilities:
cargo build -p moonframe-app --release --no-default-features
# Local/tmux terminals and web views, with no Wasm or built-in providers:
cargo build -p moonframe-app --release --no-default-features --features alacritty,browser
```

Select one app/package when building lean. Cargo features are additive:
building the entire workspace or another app that enables defaults can
enable those dependencies again. Changing Luau at runtime cannot change
what the executable includes; changing capabilities requires a rebuild.

For comparisons, build each selection in a fresh `--target-dir` with
`--release --timings`, then repeat the same build to measure the warm-cache
case. Compare executable and staged bundle sizes separately. The embedded
Luau toolbox and shared GPUI dependencies remain in lean builds, so dependency
removal does not imply the same proportional reduction in bundle size.

The first macOS release measurement, after gating browser and terminal
support but before adding `markdown`, `prose` and `updates`, used fresh Cargo
target directories and cached downloads:

| | Full starter | Minimal |
| --- | ---: | ---: |
| Clean build, wall time | 4m28s | 3m52s |
| Executable | 73.2 MiB | 38.0 MiB |
| Staged `.app` bundle | 74.8 MiB | 39.8 MiB |

These are single-run timings, with some Cargo cache-lock contention. Warm
no-change builds took 8.29s and 16.44s respectively, so this run demonstrates
size savings rather than a reliable incremental-build speedup. Both bundles
used the same staging script and retained the toolbox resources.

With `markdown`, `prose` and `updates` also optional, the same macOS release
profile produces a **35.36 MiB** minimal executable and **37.18 MiB** staged
bundle, versus **73.22 MiB** for the full executable. That saves another
2.60 MiB (6.8%) over the first minimal build, or 51.7% versus full. The
minimal build graph drops from 450 to 437 distinct packages (full: 614).

Enabling each new capability alone on that minimal build gives:

| Selection | Executable | Added over minimal |
| --- | ---: | ---: |
| Minimal | 35.36 MiB | — |
| `markdown` | 35.45 MiB | 88.5 KiB |
| `prose` | 35.58 MiB | 219.8 KiB |
| `updates` | 37.75 MiB | 2.39 MiB |

These marginal sizes are not additive: capabilities can share linked code.
All five measured selections (these four and full) passed the headless
starter check. Build timings were not repeated for this second comparison.

`python3 scripts/check-features.py --lint` checks dependency removal and
lints minimal and individual-capability builds; CI runs it on every platform.

The `bundle` command remains available without `studio`, so a library host
can package its executable without including developer tools.

For profile apps, [sealed bundles](sealed-bundles.md) select these features
from the app's Luau and provider manifests and prune the shipped toolbox.

Use the GPUI the library re-exports, `moonframe::gpui_kit` (GPUI with
gpui-component): one app links one GPUI. If your crates name `gpui`
directly, pin the same version (`gpui-pre` 0.3.10 today).

## The app

```rust
use moonframe::{Moonframe, gpui_kit::*};

fn main() -> std::process::ExitCode {
    Moonframe::new("notes")
        .title("Notes")
        .app_id("com.example.notes")
        .version(env!("CARGO_PKG_VERSION"))
        .updates("example/notes")
        .icon(include_bytes!("../assets/icon.png"))
        .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/app"))
        .keep(["terminal"])
        .native("index", Index::default)
        .view("editor", Editor::new)
        .run()
}
```

| Call | Does |
| --- | --- |
| `Moonframe::new(name)` | the app's name: `~/.config/<name>`, `~/.cache/<name>`, `~/.local/state/<name>`, its document in herdr-core, its default id `dev.moonframe.<name>` |
| `.title`, `.app_id`, `.version`, `.icon` | how the menu bar, About panel, Dock and bundle name it; an `icon.png` (or a `moonframe.toml` `[app]` title and icon) in its app folder wins |
| `.updates("owner/repo")` | the GitHub repository it is released from: **Check for Updates…** in the app menu, a daily check in a release build, and on macOS an install over the running `.app`; see [updates.md](updates.md) |
| `.app_dir(path)` | its app folder while it runs from source; edits reload live |
| `.embedded_app::<E>()` | its app folder compiled in (`#[derive(moonframe::rust_embed::RustEmbed)]`, `#[crate_path = "moonframe::rust_embed"]`), for a lone executable |
| `.project_dir(path)` | the source checkout for an embedded app's debug branch bar; pass `env!("CARGO_MANIFEST_DIR")` from the host crate, including when Cargo builds into an external target folder |
| `.keep(mods)` | the starter's mods it shows (from `app/mods` here), as a profile's `keep`; `feedback` brings the Developer tools with it, `devtools` brings them alone |
| `.native(name, make)` | a native provider (below) |
| `.native_async(name, make)` | a native provider written with `async fn`, on tokio (below) |
| `.tokio(handle)` | the tokio runtime async providers and `on_runtime` use (below) |
| `.view(name, make)` | a view (below) |
| `.assets(source)` | the app's own `AssetSource`, for its views: its paths win over Moonframe's, and the rest fall through |
| `.log(filter)` | the log filter when `RUST_LOG` is unset |
| `.run()` | the command line: the window, `--check`, `--release`, `--profile`, `mod`, `app`, `init`, `bundle`; see `--help` |

The debug branch bar reads the selected app folder (`DIR`, then the bundle's
app folder, then `.app_dir`). An embedded app instead uses `.project_dir`;
without it, no checkout is assumed and no bar is shown. Release builds and
checkouts on `main` or `master` show no bar.

### Its layers

Lowest first:

1. **The toolbox**: Moonframe's `app/`, giving its components, providers and
   stylesheet, but not its shell or mods unless the app keeps them. From
   the bundle (`Resources/toolbox`), else this checkout's `app/` when the
   binary runs where it was built, else the copy compiled into the library,
   written once per version to `~/.cache/<name>/toolbox/<hash>`.
2. **The app's own folder**: its shell, mods, components, providers and
   style: `DIR` on the command line if given, else the bundle's
   `Resources/app`, else `.app_dir` or `.embedded_app`.
3. **Packages** the user installed (`<app> mod install`). They can replace
   the app's mods by name, but never its shell.
4. **The user's folder**, `~/.config/<name>`, on top.

Terminals are the app's own: a tmux server named after it (`tmux -L
<name>`, `<name>-<profile>` in a profile) and a label in herdr-core, so two
apps never attach to each other's shells.

Profiles work as in the starter: `<app> --profile NAME` runs another app
(installed with `<app> app install`) on the same toolbox, and a profile
without an app of its own runs the host's.

### Native providers

Rust that publishes data as `app.<name>` and answers messages from mods.
Implement `moonframe::Native`, register it with `.native("index",
Index::default)` (the closure runs each time the provider starts, so it
may capture what it needs), and put a folder in the app that runs it:

```toml
# app/providers/index/provider.toml
interval = 1.0      # seconds between `tick`s
kind     = "native"
```

Like every provider, it starts only once something wants it (a view reads
`app.index`, a mod hears `provider/index` or sends it a message), unless
its manifest says `start = "always"`. Moonframe's own providers (git,
github, system, ...) cost an app nothing until it uses them.

Each call runs on the provider's own thread; `Handle` (`publish`,
`publish_json`, `send`, `fail`) can be cloned into others. A mod sends it
messages with `to = "provider:index"`.

### Async and tokio

With the `tokio` feature (on by default), a provider can be written with
`async fn` and use any tokio library (an HTTP client, a database driver):

```rust
use moonframe::{AsyncNative, Data, Handle};

#[derive(Default)]
struct Weather { client: reqwest::Client }

impl AsyncNative for Weather {
    async fn tick(&mut self, host: &Handle) {
        match self.client.get(URL).send().await {
            Ok(reply) => host.publish_json(&reply.json().await.unwrap_or_default()),
            Err(error) => host.fail(error.to_string()),
        }
    }

    async fn receive(&mut self, host: &Handle, from: &str, message: Data) { /* ... */ }
}

// .native_async("weather", Weather::default)
```

`start`, `tick` and `receive` keep a native provider's order: each is
awaited in turn on the provider's own thread, while the runtime's workers
drive its sockets and timers; each has a default that does nothing. Work
that should go on past a call goes on the runtime with `tokio::spawn` and
a clone of the `Handle`.

A view awaits what needs tokio with `moonframe::on_runtime`, from GPUI's
own executor:

```rust
cx.spawn(async move |this, cx| {
    if let Ok(page) = moonframe::on_runtime(fetch(url)).await {
        let _ = this.update(cx, |view, cx| view.show(page, cx));
    }
})
.detach();
```

Construct timers and other futures that need a runtime inside an `async`
block, so their constructors run on Tokio too:

```rust
moonframe::on_runtime(async {
    moonframe::tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}).await?;
```

Passing `on_runtime(tokio::time::sleep(...))` directly from GPUI panics:
`sleep` needs a reactor when it is called, before the future is awaited.

The runtime is the app's own when it gives one, so providers and views
share its clients and pools. GPUI holds the main thread, so `run` blocks
there; keep the runtime on other threads:

```rust
fn main() -> std::process::ExitCode {
    let runtime = moonframe::tokio::runtime::Runtime::new().expect("a tokio runtime");
    Moonframe::new("notes")
        .tokio(runtime.handle().clone())
        .native_async("weather", Weather::default)
        .run()
}
```

`#[tokio::main]` works too: `run` takes the multi-thread runtime it is
called in. A current-thread runtime is passed over (the window would starve
it), and without one, Moonframe makes a multi-thread runtime the first time
something needs it. `moonframe::tokio` is the tokio it uses, and
`moonframe::runtime()` the runtime's handle. [examples/host](../examples/host)
has an async provider (`lookup`) and a view that awaits a tokio timer.

### Views

A GPUI view that Luau places where it likes:

```lua
ui.view { name = "editor", id = "main", props = { path = state.open } }
```

```rust
struct Editor { messenger: Messenger, /* ... */ }

impl Editor {
    fn new(init: &ViewInit, window: &mut Window, cx: &mut Context<Self>) -> Self { /* ... */ }
}

impl HostView for Editor {
    fn set_props(&mut self, props: &Data, cx: &mut Context<Self>) { /* new props */ }
}

impl Render for Editor { /* ... */ }
```

- `make` runs the first time a mod draws that `name` and `id`, after the
  frame (the first frame shows the empty box). The view is then kept by
  mod and id, like a terminal: across reloads, and while its pane is
  hidden.
- `ViewInit` gives the mod (`owner`), the `id`, the first `props`, and a
  `Messenger`. `messenger.send(data)` reaches that mod's `update`, from any
  thread.
- When the mod's `props` change, `set_props` runs and the view draws again.
- The `view` stylesheet rule styles the box it fills (`radius`, `padding`,
  `border`, ...). Previews in the Components window show a placeholder.
- Trackpad gestures: GPUI's `on_pinch` and `on_mouse_pressure` work as
  usual, and `moonframe::gestures::gestures()` adds what it lacks, as a
  child of the element it listens for (macOS; elsewhere it hears nothing):
  `div().relative().child(gestures().on_rotate(|event, window, cx| ...))`,
  with `on_smart_zoom` and `on_swipe` beside it.

`moonframe::Data` is what crosses between Rust and Luau: `Data::map`,
`get`, `as_str`, `as_i64`, `as_f64`, `as_bool`, and `From` for strings,
numbers, booleans and lists.

## Check, run, ship

```sh
cargo run                       # the window, reloading app/ as it changes
cargo run -- --check            # load every module headlessly: run after each edit
cargo run --release -- bundle   # target/bundle/<Title>.app, or a Linux prefix
cargo run --release -- bundle --dist dist   # and dist/<Title>-<version>-<target>.zip + SHA256SUMS
```

`bundle` ships the running binary with the app's folder and the toolbox,
each with its WASM providers prebuilt and no sources, plus an icon and an
Info.plist, signed ad hoc. `--dist DIR` also writes what a GitHub release
carries: the bundle archived under a name with its version and platform,
and `SHA256SUMS`. With [`.updates("owner/repo")`](updates.md), the app
finds and installs those releases itself.

**Developer Mode**, in the app menu beside About, switches the tools for
changing the app as it runs (View's Components…, Inspect Element ⌥⌘I,
Developer Tools ⌥⌘J, Reload Modules ⌘R) on and off, at once and without a rebuild, and is kept
for the app (and each profile) in `$XDG_STATE_HOME/<name>/developer`. It is
on until the user turns it off, in every build, a bundle included; an app
that must never show the tools builds without the `studio` feature. `--dev` and `--release` set it for one run. Its keys are bound in the
`developer` key context, so with it off ⌘R is the app's own again.

## Moving an existing GPUI app over

The way in is to make Moonframe the host from day one, with one big view:

1. `main` becomes `Moonframe::new("app")...run()`, and the app's current
   root view is registered as a view (`.view("app.workspace", ...)`).
2. `app/main.luau` is a shell that draws `ui.view { name =
   "app.workspace" }` filling the window. Nothing looks different, but the
   Luau shell, live reload, menus, themes and packages are already there.
3. Move pieces out one at a time: a sidebar, a status bar, a dialog becomes
   Luau (with the components), or a smaller `ui.view`, with the app's state
   as native providers.

The app's crates must use the library's GPUI version.
