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
is `system` (CPU, memory and network counters); `moonframe.page` renders
a public web page for it ([below](#rendering-a-web-page)), and `moonframe.view`
reads a mod's view and acts on it ([below](#reading-and-driving-a-view)). Embedding apps can register
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
| `capabilities` | An empty list by default; `"system"`, Moonframe's own (`"moonframe.page"`, `"moonframe.view"`), and names registered by the embedding host. Unknown names are manifest errors. |
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

## Compile it into the app

A provider written against the guest SDK builds either way from the same
source: as a component, sandboxed and rebuilt as you edit, or linked into the
app as an ordinary crate, with no wasmtime, Cargo or sandbox at run time.

```rust
// providers/counter/src/lib.rs
use {moonframe_provider::Provider, serde_json::Value};

pub struct Counter(u64);

impl Provider for Counter {
    fn start(_settings: Value) -> Result<Self, String> {
        Ok(Self(0))
    }

    fn tick(&mut self) -> Option<Value> {
        self.0 += 1;
        Some(self.0.into())
    }

    fn receive(&mut self, message: Value) -> Option<Value> {
        (message == "reset").then(|| { self.0 = 0; self.0.into() })
    }
}

moonframe_provider::export!(Counter); // the component's exports; nothing natively
```

Give the crate `crate-type = ["cdylib", "rlib"]` so the app can depend on it,
then register it under the folder's name and switch the folder's kind:

```rust
Moonframe::new(...).native("counter", moonframe::Portable::<counter::Counter>::new)
```

```toml
# providers/counter/provider.toml
kind = "native"     # was "wasm"; the rest stays as it was
```

Compiled in, it is called in the same order (`start`, `restore`, a first
`tick` published at once, then ticks and messages) and `log`, `system_load`
and `host_call` reach the app with the same `capabilities` from provider.toml
(`system` needs the app's `system` or `wasm` feature). What differs: it has
the user's access and no fuel or memory limits, so link only code you would
run yourself; `save` hands state to the next start only within one launch;
and a change needs the app rebuilt.

`examples/portable` is a complete example to copy: a focus timer crate in
`app/providers/focus` (settings, ticks, messages, `save`/`restore`, and unit
tests that run natively), run as a component from that folder and compiled in
for `app/providers/focus-native`, side by side in one window. Its `host/`
binary sits beside the app folder, not around it: Cargo counts any crate
inside a workspace member's folder as a member, so a provider crate there
could not keep its own `[workspace]`. `examples/capability-provider` builds
both ways too.

### How the two builds compare

`examples/portable/host/examples/bench` runs providers both ways through the
provider runtime the app uses, so the numbers are what a view waits for. All
figures are medians, from three runs on an Apple M4 Max (macOS, release build).

**Calls and startup.** The example's `focus` timer does almost no work per call,
so this is the runtime's own cost:

| | Wasm | Native |
| --- | --- | --- |
| Start, to the first data | 15.6 ms prebuilt (`provider.wasm`); 236 ms from source, Cargo finding nothing to build; 7.2 s for the first build | 0.2 ms |
| Message to published answer, median | 8.6 µs | 7.0 µs |
| Same, p99 | 17.4 µs | 13.5 µs |
| Messages per second, one at a time | about 109,000 | about 145,000 |

Most of a round trip is the hop to the provider's thread and back, which both
builds share; Wasm adds about 1.5 µs a call. Startup differs more: wasmtime
compiles the component each time it starts (160 KiB here), and a provider built
from source asks Cargo first.

**A real workload.** `examples/portable/bench/providers/digest` does what Wasm
providers are for, shaping data: each cycle it fetches a batch of request-log
lines through a host capability, parses every line, folds numeric path
segments into `:id`, works out per-route counts, error rates and p50/p95/p99
latencies, and publishes the 50 busiest routes. One cycle, message to
published summary (the `opt-level = 3` column, in brackets in the last, is
from one run):

| Lines | Batch | Wasm, `opt-level = "s"` | Wasm, `opt-level = 3` | Native | Wasm ÷ native |
| --- | --- | --- | --- | --- | --- |
| 1,000 | 59 KiB | 0.7 ms | 0.5 ms | 0.3 ms | 2.4× (1.7×) |
| 10,000 | 596 KiB | 4.5 ms | 3.3 ms | 1.8 ms | 2.5× (1.8×) |
| 50,000 | 2.9 MiB | 21.6 ms | 15.2 ms | 8.9 ms | 2.4× (1.7×) |

Moving the lines alone (fetching without digesting, measured in one run each)
is 1.5× slower in Wasm (1.3× at `opt-level = 3`), so the parsing and sorting itself runs about 3×
slower, or 2× when optimised for speed. The repository's providers build with
`opt-level = "s"` for small components; for one that computes much, use
`opt-level = 3` in its `[profile.release]`.

What this means:

- At an interval of seconds, either build is fast enough: even 50,000 lines a
  cycle is 22 ms of work as a component.
- Compiling in pays off for an app that starts often or runs many providers,
  and for a provider that digests large batches often: 50,000 lines every
  half second is about 4% of a core as a component, 2% compiled in.
- A Wasm provider's results cross as JSON text, so a float can come back one
  bit away from the native build's: the host parses JSON with serde_json's
  fast float parser.
- A host call carries at most 1 MiB by default; the 50,000-line batch is
  2.9 MiB, so the bench raises `HostCallLimits::max_bytes` (for both builds).

Run it yourself (it needs `rustup target add wasm32-wasip2`):

```sh
cargo run --release -p moonframe-portable-example --example bench
```

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

**Checking an entry.** `moonframe::check_entry(path, &policy, &options)`
returns a `Vec<Diagnostic>` (`severity`, `file`, `line`, `message`) for a host
CLI's `<app> miniapp check`. It loads `main.luau`, components and
`style.luau`, draws the view with no data and then with each case
`CheckOptions` gives (fake `app` tables, starting states, messages), reads
`provider/provider.toml` with its capabilities checked against
`options.capabilities`, and applies `policy`. An entry that ships a
`sample.json` (beside `main.luau` or in `provider/`) is drawn with it as its
provider's data, so a branch that only draws once data arrives is checked:
ship one. See [checking what miniapps draw](library.md#checking-what-miniapps-draw)
for every option, live diagnostics, snapshots and headless renders.

`EntryStatus` contains `name`, `kind` (`Mod`/`Provider`), `state`
(`Ok`/`Building`/`Stale`/`Failed`), optional `error`, `revision` (an opaque
fingerprint of the attempted revision, not a monotonic counter), and
`view_errors`: what the entry's views drew wrong, as `ViewDiagnostic`s, sent
when they change and empty once the view draws cleanly. The callback
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
or git dependency). Implement `moonframe_provider::Provider` and export it with
`moonframe_provider::export!(MyProvider)`; the same crate can then be compiled
into the app too (see [Compile it into the app](#compile-it-into-the-app)).
Enable its `stateful` feature for save/restore. The raw WIT bindings stay
available on wasm32 as `moonframe_provider::bindings`. Its typed helper
serializes arguments and deserializes the JSON answer:

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

## Declare its types

Without types, `app.counter` is `any` to the Luau checker: a view that
reads a field the provider never publishes, or sends it a message it
ignores, passes `just types` and fails quietly at runtime. Declare both in
`providers/counter/types.luau`:

```lua
-- What app.counter holds.
export type Published = {
    count: number,
}

-- What send "provider:counter" takes.
export type Message = { add: boolean } | { reset: boolean }
```

Then send with `send`, which builds the same `{ to = ..., msg = ... }` a
literal would, so its message is checked:

```lua
update = function(state, msg)
    return state, { send "provider:counter" { add = true } }
end,
```

`just types DIR` checks the app's mods against them: reading
`counter.total`, or sending `{ ad = true }`, is a type error with its line.
For a provider in the starter's `app/providers/`, `just provider-types`
also copies them into `types/moonframe.d.luau`, so every app sees them;
`just provider-types-check` (in `just ci`) fails when that copy is stale.

Keep an example of what it publishes beside them as `published.json`, and
hold both sides to it: `just provider-types-check` fails when the file's
keys or kinds differ from `Published`, and a test in the crate compares a
real `tick` against it (see `examples/portable/app/providers/focus`). Then
neither the Rust nor the Luau side can change without the other.

Limits: only `Published`'s own fields are held to the example, a level
deep; button `msg = ..., to = ...` and plain `{ to = ..., msg = ... }`
tables are not checked, only `send`; and the type names (`Published`) are
not visible in mods, whose own annotations are checked against them by
shape. Nothing checks the published JSON at runtime.

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

### Rendering a web page

`moonframe.page` loads a web page the way a reader sees it, with its
scripts, in a hidden web view, and answers with its text and, when asked,
a picture. List it, `capabilities = ["moonframe.page"]`, then:

```rust
let page: serde_json::Value = moonframe_provider::host_call(
    "moonframe.page",
    "render",
    &serde_json::json!({ "url": "https://example.com", "screenshot": true }),
)?;
```

`render` takes `url` (http or https, no user name or password),
`screenshot` (default false), `width` and `height` (the window it lays out
in, 1280 by 900 points by default), `timeout` (seconds to load, 20 by
default, at most 60) and `drop` (up to 64 CSS selectors left out of the
text, such as `["nav", "footer"]`). It answers `url` (where it ended up,
after redirects), `title`, `text` (the rendered text, `innerText`, up to
2 MiB), `truncated`, `timed_out` (it had not finished loading in its time
and was read as it showed), and with `screenshot`, `screenshot` (`mime`
`image/png`, `base64`, `width`, `height`: what showed, `width` pixels
wide whatever the screen's scale, up to 4 MiB) or `screenshot_error`.
A page that does not load is an error.

The page gets a session of its own, kept nowhere, that reaches the public
internet alone: every connection goes through the proxy a PDF of a web
page uses, which refuses this machine and its network after each redirect
and lookup, and WebRTC is off. Its calls have their own bounds (a minute
and a quarter, about 8 MiB), not `host_call_limits`. Four render at once.
It needs the `browser` feature and, so far, macOS; elsewhere the call
answers why not.

### Reading and driving a view

`moonframe.view` lets a provider look at a mod's view in the front main
window as a person would describe it, and do one thing a person could do
to it: test a page it wrote, or drive one for its user. List it,
`capabilities = ["moonframe.view"]`, then:

```rust
let page: serde_json::Value =
    moonframe_provider::host_call("moonframe.view", "tree", &serde_json::json!({ "mod": "notes" }))?;
let after: serde_json::Value = moonframe_provider::host_call(
    "moonframe.view",
    "act",
    &serde_json::json!({ "mod": "notes", "ref": "e0.3", "action": "fill", "value": "Groceries" }),
)?;
```

`tree` takes `mod` and answers `tree`, one line per element indented by
depth (`button "Save" [e0.3]`), the same as `nodes` (`ref`, `role`, `name`,
`value`, `selected`, `disabled`, `depth`), `truncated` past 2,000 elements
or 128 KiB of names and values (each cut at 4,000 characters), and
`problems`, what the mods it read report wrong. Components and mounted
mods are read through, a mod that mounts itself once; text inside a
clickable row names the row, and a field's `value` is the text it holds. Roles are `button` (a button or a clickable row),
`item`, `textbox`, `text`, `badge`, `panel`, `document` (markdown),
`image`, `list`, `popover`, `dialog`, `terminal`, `browser`, `map`,
`video`, `view` and `problem`.

`act` takes `mod`, `ref` (from the last tree), `action` and its `value`,
and answers the tree again, from the window it acted in. It sends what the element itself sends, as a
person's act would: `click` (the element, or the nearest one around it
that takes a click), `fill` (a field's text, in place of what it holds,
typed into the field while it is drawn, which sends its `change`; a field
with no `id` hears its `change` with the text, so give one an `id` to fill
it as typing would),
`press` (a key such as `enter`, `escape` or `cmd-k`: an open popover's or
dialog's `keys` or `dismiss` around it first, then the field's `keys` or
`submit` unless it is disabled, Enter in an address bar taking its browser
there, else the mod's own `keys`) and `select` (the option of that name inside the
element: an item, button or clickable row, by its whole name, else the
only one whose name has it). One that does nothing says why. A step the
window does not take within 5 s fails, and is never carried out later.

The app's own providers reach any mod; a miniapp's provider reaches only
the mods of its own namespace (`<namespace>:...`), whatever its view
mounts: another mod drawn inside it is left out of the tree.

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
   writing the view, in `types.luau` (`Published`, `Message`) with an
   example `published.json` and a test holding `tick` to it. Validate
   settings and handle unknown messages.
4. Implement `start`, `tick`, `receive` and `export!`. Publish whole
   snapshots, bound work and memory, and do not assume runtime state persists.
5. Read `app.<name>` with a nil fallback; send payloads with
   `send "provider:<name>" { ... }`. Follow the Moonframe and component skills for UI.
6. Build for `wasm32-wasip2`, run the app's headless checks and Luau type
   checks where configured, then verify startup, a message and reload.
   State explicitly when an interactive check was not performed.
7. Keep source packages and prebuilt release staging distinct. Link to
   this guide instead of duplicating a different provider protocol.

Further examples: [system](https://github.com/moonframehq/moonframe/tree/main/app/providers/system)
uses the host capability; [Gantry telemetry](https://github.com/moonframehq/moonframe/tree/main/examples/gantry/providers/telemetry)
keeps a larger in-memory model and handles messages (its data is simulated).
