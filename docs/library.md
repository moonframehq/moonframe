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
moonframe = { git = "https://github.com/penso/moonframe", branch = "main" }

# In the workspace root: GPUI and Luau are unusably slow unoptimised.
[profile.dev.package."*"]
opt-level = 2
```

`Cargo.lock` pins the commit (`cargo update -p moonframe` moves it). The
library needs the toolchain in [rust-toolchain.toml](../rust-toolchain.toml)
or newer.

Features: `acp`, `alacritty`, `docs`, `git`, `herdr-core` and `tokio` by
default. `ghostty` (a second terminal emulator, built from Ghostty's source
with Zig; see `tools/zig`) is opt-in: `features = ["ghostty"]`.

An app that needs none of git, the markdown editor or kept terminals can
leave them out, and with them the herdr-gpui crates they build on:

```toml
moonframe = { default-features = false, features = ["acp", "alacritty", "tokio"], git = "..." }
```

A built-in provider a feature leaves out (`git`, `docs`, `acp`) publishes
nothing: the starter's mods reading `app.git` see nil and carry on, and only
a message to it reports that the feature is off.

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
| `.app_dir(path)` | its app folder while it runs from source; edits reload live |
| `.embedded_app::<E>()` | its app folder compiled in (`#[derive(moonframe::rust_embed::RustEmbed)]`, `#[crate_path = "moonframe::rust_embed"]`), for a lone executable |
| `.keep(mods)` | the starter's mods it shows (from `app/mods` here), as a profile's `keep` |
| `.native(name, make)` | a native provider (below) |
| `.native_async(name, make)` | a native provider written with `async fn`, on tokio (below) |
| `.tokio(handle)` | the tokio runtime async providers and `on_runtime` use (below) |
| `.view(name, make)` | a view (below) |
| `.log(filter)` | the log filter when `RUST_LOG` is unset |
| `.run()` | the command line: the window, `--check`, `--release`, `--profile`, `mod`, `app`, `init`, `bundle`; see `--help` |

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

`moonframe::Data` is what crosses between Rust and Luau: `Data::map`,
`get`, `as_str`, `as_i64`, `as_f64`, `as_bool`, and `From` for strings,
numbers, booleans and lists.

## Check, run, ship

```sh
cargo run                       # the window, reloading app/ as it changes
cargo run -- --check            # load every module headlessly: run after each edit
cargo run --release -- bundle   # target/bundle/<Title>.app, or a Linux prefix
```

`bundle` ships the running binary with the app's folder and the toolbox,
each with its Wasm providers prebuilt and no sources, plus an icon and an
Info.plist, signed ad hoc.

**Developer Mode**, in the app menu beside About, switches the tools for
changing the app as it runs (the Components menu, Inspect Element ⌥⌘I,
Reload Modules ⌘R) on and off, at once and without a rebuild, and is kept
for the app (and each profile) in `$XDG_STATE_HOME/<name>/developer`. Until
the user picks, it is on in a debug build (`cargo run`) and off in a release
one (`cargo run --release`, an install, a bundle), as the app's users see
it. `--dev` and `--release` set it for one run. Its keys are bound in the
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
