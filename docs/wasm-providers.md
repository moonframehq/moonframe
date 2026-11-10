# Building WebAssembly (WASM) providers

Build a small Rust component that publishes JSON to Moonframe, then read it
from a Luau view and send it messages. This guide is for people writing a
provider by hand and coding agents building one for an app.

A provider supplies data; a mod draws it. A provider in `providers/counter/`
publishes as `app.counter`, and mods address it as `provider:counter`.
Moonframe runs each WASM provider in its own sandbox with Wasmtime. You do
not rebuild or restart the Moonframe executable to add one.

## Choose a provider kind

| Need | Kind |
| --- | --- |
| Transform data, keep an in-memory model, or use a host capability | `wasm` |
| Publish a sample JSON file without running code | `static` |
| Run a CLI, read files, make HTTP requests, or reuse a Python/Node program | `process` |
| Use the host app's Rust libraries or asynchronous services directly | `native` |

WASM providers cannot read your files, use the network, inherit your
environment, or spawn processes. The built-in machine-data capability
is `system` (CPU, memory and network counters). Embedding apps can register
namespaced capabilities such as `mshub.entities`. There are no built-in `http`, `fs`
or `exec` capabilities. Process and native providers run with the user's
access instead. See the [library guide](https://github.com/moonframehq/moonframe/blob/main/docs/library.md)
for native providers.

Rust is the path Moonframe builds automatically. Other languages work if
they produce a **WebAssembly Component Model** component implementing
the same WIT interface; a plain core WASM module is not enough.

## Set up the folder

You need Moonframe with its default `wasm` feature enabled, Rust/Cargo,
and the WASI Preview 2 target:

```sh
rustup target add wasm32-wasip2
```

Use a current stable Rust toolchain (the example uses edition 2024). In
the Moonframe repository, use its pinned `rust-toolchain.toml`.

Choose the folder that already holds your app's `main.luau`:

| Working on | App folder |
| --- | --- |
| The cloned starter or an app on the Rust library | `app/` in that project |
| A profile made with `moonframe app new` | The profile project root |
| Your installed Moonframe | `~/.config/moonframe` (or `$XDG_CONFIG_HOME/moonframe`) |

For an installed app, write to the user's folder, not inside the signed
application bundle. In the commands below, **run from this app folder**.
Pick an unused provider name: a user provider replaces a built-in provider
with the same folder name. Names use lowercase `a-z`, digits, `-` and `_`.

Create this layout:

```text
providers/counter/
  provider.toml
  Cargo.toml
  src/lib.rs
  wit/provider.wit
mods/wasm-counter/
  main.luau
```

```sh
mkdir -p providers/counter/src providers/counter/wit mods/wasm-counter
```

Copy [wit/provider.wit](https://github.com/moonframehq/moonframe/blob/main/wit/provider.wit)
from the Moonframe version you are using into
`providers/counter/wit/provider.wit`. In a cloned starter, from `app/`:

```sh
cp ../wit/provider.wit providers/counter/wit/provider.wit
```

For a separate project, obtain that file from the matching Moonframe
release tag or commit. Keep a local copy in the provider so it builds
wherever its folder is copied. WIT is the interface definition shared by
the component and the host; do not invent or change its exports.

## Declare the provider

Write `providers/counter/provider.toml`:

```toml
kind = "wasm"
interval = 1.0
capabilities = []

[settings]
initial = 0
```

The **folder name** determines `app.counter`, not the Cargo package name.
Manifest fields for a WASM provider are:

| Field | Meaning |
| --- | --- |
| `kind` | Must be `"wasm"`; no `command` or `file` field. |
| `interval` | Seconds to wait for a message before calling `tick`; defaults to `1.0`, allowed range `0.1`–`3600.0`. |
| `start` | `"on-demand"` (default), or `"always"` to start with the app. |
| `capabilities` | An empty list by default; `"system"` and names registered by the embedding host. Unknown names are manifest errors. |
| `[settings]` | Your configuration, converted to JSON and passed to `start`. |

Unknown manifest fields are errors. Keep provider-specific configuration
under `[settings]`.

Write `providers/counter/Cargo.toml`:

```toml
[package]
name = "counter"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
serde_json = "1"
wit-bindgen = "0.63"

[profile.release]
lto = true
opt-level = "s"
strip = true

[workspace]
```

The empty `[workspace]` makes this an independent crate, even inside the
host's Rust workspace. Keep the generated `Cargo.lock` in source control.
Leave the library filename at its default: Moonframe locates the output
using the package name, replacing hyphens with underscores.

## Implement the component

Write `providers/counter/src/lib.rs`:

```rust
use std::cell::RefCell;
use serde_json::{Value, json};

wit_bindgen::generate!({ path: "wit/provider.wit", world: "provider" });

struct Counter;

#[derive(Default)]
struct State {
    initial: u32,
    count: u32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn snapshot(state: &State) -> String {
    json!({ "count": state.count }).to_string()
}

impl Guest for Counter {
    fn start(settings: String) -> Result<(), String> {
        let settings: Value = serde_json::from_str(&settings)
            .map_err(|error| error.to_string())?;
        let initial = match settings.get("initial") {
            None => 0,
            Some(value) => value.as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("initial must be an integer from 0 to 4294967295")?,
        };
        STATE.with_borrow_mut(|state| *state = State { initial, count: initial });
        Ok(())
    }

    fn tick() -> Option<String> {
        Some(STATE.with_borrow(snapshot))
    }

    fn receive(message: String) -> Option<String> {
        let message: Value = serde_json::from_str(&message).ok()?;
        STATE.with_borrow_mut(|state| {
            if message.get("add").and_then(Value::as_bool) == Some(true) {
                state.count = state.count.saturating_add(1);
            } else if message.get("reset").and_then(Value::as_bool) == Some(true) {
                state.count = state.initial;
            } else {
                return None;
            }
            Some(snapshot(state))
        })
    }
}

export!(Counter);
```

`wit-bindgen` generates the `Guest` trait and `export!` macro from the WIT
file. Implement all three exports:

| Export | Called when | Result |
| --- | --- | --- |
| `start(settings)` | Once per component instance, before the first tick | `Ok(())`, or `Err(message)` to fail startup. |
| `tick()` | Immediately after startup, then after each interval without a message | `Some(json)` publishes a snapshot; `None` keeps the previous value. |
| `receive(message)` | A mod sends a message to the provider | `Some(json)` publishes immediately; `None` keeps the previous value. |

Frequent messages can postpone periodic ticks: `interval` is an idle
timeout, not a fixed-rate timer. Keep work bounded and return promptly.

Each publication **replaces** `app.counter`; it is not a partial merge.
Return the whole object the views need. JSON objects become Luau tables,
arrays become lists, and `null` becomes `nil`. Publications are limited to
1 MiB of JSON, 10,000 values and nesting depth 16 (the root is depth 0).
Invalid JSON or a component trap stops the provider and reports a problem.

Messages contain the **payload alone**. The button below sends
`{"add":true}` to `receive`, not a `{"from":...,"message":...}` envelope.
That envelope belongs to process providers. WASM's current interface does
not expose the sender or provide native-provider reply routing; publish
results for views or `provider/counter` subscribers to consume.

The `STATE` value survives calls, but resets when a new build replaces the
provider, unless it [keeps its state](#keep-state-across-a-new-build). It is
saved across launches for namespaced entries using `stateful-provider` (below).
Ordinary app-folder providers keep their existing reload-only behavior. Use a
mod's persisted state for UI choices that should survive quitting.

## Keep state across a new build

Editing a provider rebuilds and restarts it. To carry its state over, as a
mod's state is across a reload, generate the bindings from the
`stateful-provider` world and add two exports:

```rust
wit_bindgen::generate!({ path: "wit/provider.wit", world: "stateful-provider" });
```

```rust
// In `impl Guest for Counter`:
fn save() -> Option<String> {
    Some(STATE.with_borrow(snapshot))
}

fn restore(state: String) -> Result<(), String> {
    let saved: Value = serde_json::from_str(&state)
        .map_err(|error| error.to_string())?;
    let count = saved.get("count")
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or("no count in the saved state")?;
    STATE.with_borrow_mut(|state| state.count = count);
    Ok(())
}
```

| Export | Called when | Result |
| --- | --- | --- |
| `save()` | This build stops, because a new one replaces it | `Some(text)` hands it to the next build; `None` keeps nothing. |
| `restore(state)` | On a new build, after `start` and before the first tick | `Ok(())`, or `Err(message)` to start fresh. |

The state is any text, usually JSON, up to 1 MiB. Ordinary app-folder providers
keep it in memory for this launch. External entries also save it atomically on
clean shutdown under the app's state directory, keyed by namespace and id;
moving the entry's folder does not change its identity. `--no-user` keeps no
durable state. Headless hosts set `ProviderOptions::state_dir` explicitly.
`start` still runs first, so settings stay current: restore only what the
user built up. Plain `provider` components keep working and start over.
`examples/research/providers/research` is a working example, including how to
let a changed setting win over the saved state.

- **Your state changed shape.** Read what you can in `restore`, with defaults
  for what is new. Return `Err` if nothing fits: the provider starts fresh
  and the reason goes to the terminal log. The saved state is then gone.
- **A build fails.** A build or instantiation error leaves the previous worker
  answering messages and publishing data, with `stale` status. Once the new
  component instantiates, the old one saves and stops. A panic in `restore`
  or the first tick retains the saved state for the next attempt.
- **A running provider panics.** It has nothing left to save, so the next
  build starts fresh. Only state from a clean stop is handed on.

## Host-defined capabilities and external miniapps

An embedding app can expose a narrow JSON API to Wasm:

```rust
use moonframe::{Caller, HostCapability, Moonframe, RootPolicy};

struct Entities;
impl HostCapability for Entities {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String> {
        // Authorize using this loader-supplied identity, never an id in args.
        if caller.provider != "mshub:notes" || method != "echo" {
            return Err("not allowed".into());
        }
        Ok(args.to_owned())
    }
}

let app = Moonframe::new("mshub")
    .app_dir("app")
    .capability("mshub.entities", Entities)
    .root("mshub", "/path/to/miniapps", RootPolicy::default())
    .entry("mshub", "notes", "/path/to/miniapps/notes")
    .on_entry_status(|status| eprintln!("{}: {:?}", status.name, status.state));
let entries = app.entry_handle(); // Clone into host threads before app.run().
// After the host's own platform/enabled filtering:
entries.add_entry("mshub", "tasks", "/path/to/tasks")?;
entries.remove_entry("mshub", "tasks");
```

Roots register a namespace, path and policy; **they do not scan**. Use the
`Entries` handle's `add_root` / `remove_root` and `add_entry` / `remove_entry`
to control exactly what loads at runtime. Each entry may have `main.luau`,
`provider/provider.toml`, or both. Entry paths are chosen by the trusted host
and may lie outside the root directory. Guests cannot change this registry.

The name is exactly `<namespace>:<id>`:

- `ui.mount "mshub:notes"`, or `ui.mount { "mshub:notes", id = "second" }`;
- `app["mshub:notes"]`, `to = "provider:mshub:notes"`;
- `subscribe = { "provider/mshub:notes" }`;
- persisted mod/instance keys include `mshub:notes` (for example
  `mod:mshub:notes#second`). Removing an entry does not erase its saved state.

Namespace and id each use 1–48 lowercase ASCII letters, digits, `-`, `_`.
Namespaces `app`, `provider`, `topic`, `demo`, `component`, `shell` and `parent`
are reserved because those prefixes route messages elsewhere. `:` is supported
in topics and persistence keys. Entry edits are watched like app-folder edits,
including in release bundles; invalid mod parse/init/first-view edits keep the
previous working code. Native code is compiled into the host: register
`.native("mshub:notes", factory)` and put `kind = "native"` in that entry's
provider manifest. There is no runtime native-library loading.

`RootPolicy::default()` uses `BuildPolicy::PrebuiltOnly` and disables process
and socket providers. Prebuilt-only entries require `provider.wasm` and reject
`Cargo.toml`, even when both are present. For reviewed development sources use
`RootPolicy { build: BuildPolicy::Cargo, ..Default::default() }`; Cargo still
refuses local `build.rs` and `.cargo/` hooks. `allow_process` and `allow_socket`
are explicit opt-ins. Ordinary app layers retain their existing behavior.

An entry may also bring `components/<name>.luau` and `style.luau`:

```text
notes/
  main.luau            the mod, mounted as ui.mount "mshub:notes"
  style.luau           optional: return style { rules = { ... } }
  components/
    card.luau          use "card" finds this before the app's `card`
  provider/
    provider.toml
```

The entry's `use` looks in its own `components/` first, then in the app's;
other mods never see an entry's components, and they stay out of the
Components window. Its `style.luau` takes only `rules`, which apply to what
the entry draws (under its mod's own `styles`, over the app's stylesheet) and
may name the app's colors; colors, fonts and the other stylesheet keys belong
to the app and are refused. Both reload live with the entry; a broken
`style.luau` keeps its last good rules and reports through `on_entry_status`.

**Built-in miniapps.** A miniapp shipped inside the host binary needs no files
on disk: embed its folder with `rust-embed` and select it with
`entry_embedded`. It is written to
`~/.cache/<app>/entries/<namespace>/<id>/<hash>` as the app starts (once per
content), so `main.luau`, `components/` and a `kind = "native"` provider work
in a release bundle. In debug, point `entry` at the source folder instead, for
live reload, as with `embedded_app` and `app_dir`:

```rust
#[derive(moonframe::rust_embed::RustEmbed)]
#[crate_path = "moonframe::rust_embed"]
#[folder = "miniapps/notes"]
struct Notes;

let app = Moonframe::new("mshub").root("builtin", "miniapps", RootPolicy::default());
let app = if cfg!(debug_assertions) {
    app.entry("builtin", "notes", concat!(env!("CARGO_MANIFEST_DIR"), "/miniapps/notes"))
} else {
    app.entry_embedded::<Notes>("builtin", "notes")
};
```

On the runtime handle, `use moonframe::EmbeddedEntries` and call
`entries.add_entry_embedded::<Notes>("builtin", "notes")`; one added before
`run` waits until the app knows its cache folder.

**Checking an entry.** `moonframe::check_entry(path, &policy, app_roots,
&["mshub.entities"])` returns a `Vec<Diagnostic>` (`severity`, `file`,
`message`) for a host CLI's `<app> miniapp check`: it compiles and loads
`main.luau`, draws its first view headlessly, loads each component and
`style.luau`, reads `provider/provider.toml` with its capabilities checked
against the names given, and applies `policy` (prebuilt-only, process,
socket). With `app_roots` as `None`, a `use` of a component the entry does not
bring is only a warning. The Luau-only part is
`moonframe_core::entries::check(path, &policy)` (or `check_in` with the app's
`Roots`). Static type-checking stays with `luau-lsp analyze`, as `just types`
runs it.

`EntryStatus` contains `name`, `kind` (`Mod`/`Provider`), `state`
(`Ok`/`Building`/`Stale`/`Failed`), optional `error`, and `revision` (an opaque
fingerprint of the attempted revision, not a monotonic counter). The callback
runs on the loading thread; keep it short. Wasm build/instantiate progress and
errors use this callback. It does not publish a topic. On-demand providers
report once wanted. Headless integrations use `Capabilities::capability`,
`Manifest::load_with` / `check_with`, and `Providers::start_configured` with
`ProviderOptions` for capabilities, status and durable state.

In the guest's `provider.toml`:

```toml
kind = "wasm"
capabilities = ["mshub.entities"]
```

Both WIT worlds import `host-call(capability, method, args) -> result<string,
string>`. `system-load` is unchanged, including for old prebuilt components.
Undeclared calls return `Err` without invoking the host. Capability names must
be namespaced with dots; unprefixed names and `moonframe.*` are reserved.
`Caller` includes the registered provider name, provider folder, root path,
optional namespace, and all declared capability names.

The standalone Rust guest SDK is `crates/moonframe-provider` (add it as a path
or git dependency). Implement `moonframe_provider::bindings::Guest` and export
with `moonframe_provider::bindings::export!(MyProvider with_types_in
moonframe_provider::bindings)`. Enable its `stateful` feature for save/restore.
Its typed helper serializes arguments and deserializes the JSON answer:

```rust
let answer: serde_json::Value = moonframe_provider::host_call(
    "mshub.entities", "echo", &serde_json::json!({ "id": 42 }),
)?;
```

`examples/capability-provider` is a complete guest using `example.echo`;
register that name in its host. For asynchronous services implement
`AsyncHostCapability` and use `.capability_async(...)`; it uses the existing
`.tokio(handle)` runtime. Configure `.host_call_limits(HostCallLimits { ... })`
as needed: defaults are 1 MiB each for arguments/results, 5 seconds per call,
and 32 outstanding host calls. Host work consumes no Wasm fuel. Timeout, panic,
invalid JSON and size errors return strings to the guest. Async timeouts drop
the future; synchronous Rust cannot be forcibly cancelled, so a timed-out call
may finish later. Until it finishes that instance cannot start another host
call, and it still counts toward the outstanding-call limit. Host APIs should
account for this when implementing writes.

## Draw it and send messages

Write `mods/wasm-counter/main.luau`:

```lua
return mod {
    region = "right",
    view = function(state, app)
        local counter = app.counter
        if not counter then
            return ui.text "Waiting for the WASM counter…"
        end
        return ui.panel {
            title = "WASM counter",
            ui.column {
                ui.text(`Count: {counter.count}`),
                ui.button { "Add one", to = "provider:counter", msg = { add = true } },
                ui.button { "Reset", to = "provider:counter", msg = { reset = true } },
            },
        }
    end,
}
```

The starter shell places mods from the `right` region automatically. A
custom shell must include that region or mount `wasm-counter` explicitly.
Handle `app.counter == nil` while the component builds or before it
publishes its first value.

Reading `app.counter` from a drawn view starts the provider on demand.
Subscribing to `provider/counter` or sending a message also starts it.
Once started, it keeps running even if the view is hidden. Use
`start = "always"` only when it should work before any mod needs it.

## Build, run and check

From the app folder, compile the exact component:

```sh
cargo build --release --target wasm32-wasip2 \
  --manifest-path providers/counter/Cargo.toml \
  --target-dir target/providers
```

The output is `target/providers/wasm32-wasip2/release/counter.wasm`.
This target produces a component; `wasm32-unknown-unknown` does not produce
the required artifact. You do not need `cargo component` for this example.

Check and run using the command for your app:

| App | Validate UI and manifests | Open the app |
| --- | --- | --- |
| Cloned starter (from repository root) | `just check` and `just types` | `just run` |
| Linked profile | `moonframe --profile NAME --check` | `moonframe --profile NAME` |
| App on the library (from Rust project root) | `cargo run -- --check` | `cargo run` |
| Installed starter with user mods | `moonframe --check` | `moonframe` |

**`--check` does not compile or execute WASM providers.** A passing check
validates the manifest and Luau but is not a runtime test. Build the
component too, then verify in the running app:

1. The panel shows `Count: 0` (or your configured `initial`).
2. **Add one** updates the count immediately; **Reset** restores `initial`.
3. Changing `initial` in `provider.toml` restarts this provider with the new value.
4. Editing `src/lib.rs` rebuilds it; a compile error shows as a provider problem.
5. With `save` and `restore`, the count survives that edit.

Agents should report which checks they ran and which runtime interactions
still need the user's verification.

When `Cargo.toml` and Cargo are available, Moonframe builds the source
automatically on startup and when the provider's folder changes. Builds
go to the app's cache (the starter uses `~/.cache/moonframe/providers`,
or its `$XDG_CACHE_HOME` equivalent). Only the changed provider restarts;
the window and other providers keep running. Changes to dependencies
outside the provider folder may require a manual reload.

A failed source build is an error, even if an older `provider.wasm` exists.
Without source or without Cargo, Moonframe loads `provider.wasm` from the
provider folder instead. If neither path is available, startup fails.

## Host calls and limits

To log a diagnostic, use the generated host interface:

```rust
moonframe::provider::host::log("counter started");
```

This `moonframe` module comes from `wit-bindgen`, not a dependency on the
Moonframe Rust library. Logs go through the host's logging system; run
the app from a terminal to inspect them. Publishing happens through the
return value of `tick` or `receive`, not `println!`.

For machine load, put `capabilities = ["system"]` at the top level of
`provider.toml`, before `[settings]`, then call:

```rust
let sample = moonframe::provider::host::system_load()?;
let load: serde_json::Value = serde_json::from_str(&sample)
    .map_err(|error| error.to_string())?;
```

This fragment belongs in a function returning `Result<_, String>`.
Without the capability the call returns an error. `cpu` and `memory` are
fractions from 0 to 1; `memory_used` and `memory_total` are bytes, and
`received` and `transmitted` are bytes per second. See the complete
[system provider](https://github.com/moonframehq/moonframe/tree/main/app/providers/system)
for collecting a bounded history and handling a failed host call.

Each component has a 64 MiB aggregate linear-memory cap and a fresh fuel
budget per call (currently 2 billion Wasmtime fuel units, not a wall-clock
deadline). Bound histories, collections and computation accordingly.
There are no preopened directories or inherited environment variables;
adding a crate does not grant access to the machine.

The **running component** is sandboxed; the **build is not**. Cargo runs as
the user. Moonframe refuses provider/local-dependency build scripts,
`build-dependencies`, `.cargo` folders and toolchains named by path, and
builds outside the provider's directory. Proc macros and published
dependencies' build scripts can still execute during compilation. Review
source and dependencies before building a provider from elsewhere.

## Package and share

For source development, keep `provider.toml`, `Cargo.toml`, `Cargo.lock`,
`src/` and the local `wit/` together. Other developers need Rust and the
`wasm32-wasip2` target.

For a prebuilt distribution, the runtime needs only:

```text
providers/counter/
  provider.toml
  provider.wasm
```

Copy the built `counter.wasm` to the literal filename `provider.wasm`
in a **separate staging folder** beside its manifest. Leave `Cargo.toml`
out of that staged folder so machines with Cargo do not rebuild it.
Keep your development sources. The component is portable across the
platforms supported by the matching Moonframe host interface.

In the cloned starter, `just stage` stages the built-in `app/` with its
WASM providers prebuilt; `just bundle` uses that staging step. For an app
on the library, `cargo run --release -- bundle` also packages its WASM
providers. See [building and bundling apps](https://github.com/moonframehq/moonframe/blob/main/.claude/skills/moonframe-apps/SKILL.md)
for profile bundles.

Git packages installed with `moonframe app install` or `moonframe mod install`
are **source-only**: they reject binary files, including `provider.wasm`.
Ship source through those commands; prebuilt components belong in bundled
releases or manually staged app folders. Do not include `target/` or
compiled components in a source package.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Nothing builds; `app.counter` stays nil | Is the mod drawn, and does its view read `app.counter`? Does the shell place its region? Check the red problem pill at the top of the window and the terminal log. |
| Cannot find `std` for the target | Install `wasm32-wasip2` for the toolchain actually building the provider. |
| Cannot find the WIT file | The macro path is relative to the provider crate root. Keep `wit/provider.wit` inside it. |
| Missing exports or component instantiation error | Use `wasm32-wasip2`, the matching WIT, all three `Guest` methods and `export!(Counter)`. |
| Source build fails despite a `provider.wasm` | Source takes precedence when Cargo is available; fix the build or use a separate prebuilt-only staging folder. |
| `start: initial must be…` | Fix `[settings] initial`; a `start` error prevents the first tick. |
| Invalid JSON, memory error or fuel exhaustion | Return valid, bounded JSON and limit work per call; inspect the provider's error. |
| `system_load` returns an error | Put `capabilities = ["system"]` before `[settings]`. |
| No data in a custom library app | Ensure its `wasm` Cargo feature is enabled; without it providers publish nothing and report the disabled feature on a message. |

## Coding-agent recipe

When asked to build a WASM provider:

1. Identify the app folder and choose an unused provider name. Confirm
   that WASM's available capabilities cover the task.
2. Copy the matching WIT locally; create an independent `cdylib` crate
   with `kind = "wasm"`. Keep manifest keys separate from `[settings]`.
3. Define the published JSON shape and accepted message payloads before
   writing the view. Validate settings and handle unknown messages.
4. Implement `start`, `tick`, `receive` and `export!`. Publish whole
   snapshots, bound work and memory, and do not assume runtime state persists.
5. Read `app.<name>` with a nil fallback; send payloads to
   `provider:<name>`. Follow the Moonframe and component skills for UI.
6. Build for `wasm32-wasip2`, run the app's headless checks and Luau type
   checks where configured, then verify startup, a message and reload.
   State explicitly when an interactive check was not performed.
7. Keep source packages and prebuilt release staging distinct. Link to
   this guide instead of duplicating a different provider protocol.

Further examples: [system](https://github.com/moonframehq/moonframe/tree/main/app/providers/system)
uses the host capability; [Gantry telemetry](https://github.com/moonframehq/moonframe/tree/main/examples/gantry/providers/telemetry)
keeps a larger in-memory model and handles messages (its data is simulated).
