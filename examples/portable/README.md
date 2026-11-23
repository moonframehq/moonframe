# portable

One provider crate, run both ways side by side: as a sandboxed WebAssembly
component, and compiled into the app as a native provider. Copy it to start a
provider you may want to ship either way.

```text
app/
  main.luau                     two panels, one per build
  providers/
    focus/                      the provider crate (a focus timer)
      provider.toml             kind = "wasm": built and run sandboxed
      Cargo.toml                crate-type = ["cdylib", "rlib"]
      src/lib.rs                impl Provider for Focus; export!(Focus)
      src/tests.rs              plain Rust tests, run natively
    focus-native/
      provider.toml             kind = "native": the same crate, linked in
host/
  src/main.rs                   .native("focus-native", Portable::<focus::Focus>::new)
  examples/bench/               both builds measured (see "How fast each is")
bench/
  providers/digest/             a provider doing real work, for the bench
```

The timer's code is the same in both: `start` reads `[settings]`, `tick`
counts down once a second, `receive` takes `"start"`, `"pause"`, `"reset"`
and `{ minutes = n }`, and `save`/`restore` keep its place across a new
build. Each panel shows `app.focus` or `app["focus-native"]`, with a badge
saying which build published it.

```sh
rustup target add wasm32-wasip2            # for the wasm build
cargo run -p moonframe-portable-example    # opens the window
cargo test --manifest-path examples/portable/app/providers/focus/Cargo.toml
```

Edit `app/providers/focus/src/lib.rs` while it runs: the wasm panel rebuilds
and keeps its time; the native one changes on the next `cargo run`.

## How fast each is

`host/examples/bench.rs` runs this provider both ways through the provider
runtime and prints what it measured:

```sh
cargo run --release -p moonframe-portable-example --example bench
```

It also runs `bench/providers/digest`, a provider doing real work: each cycle
it fetches request-log lines through a host capability, parses them and
publishes per-route latency percentiles.

On an Apple M4 Max (medians of three runs), digesting 50,000 lines takes
8.9 ms native and 21.6 ms as a wasm component built for size (15.2 ms built
for speed, `opt-level = 3`), about 2.4× and 1.7×. The native build starts in
0.2 ms and the prebuilt component in 15.6 ms. See [How the two builds
compare](../../docs/wasm-providers.md#how-the-two-builds-compare).

## Choosing a build

- **wasm** (`kind = "wasm"`): sandboxed (no files, network or environment,
  limited memory and instructions), rebuilt live as you edit, and shareable
  as a folder or a prebuilt `provider.wasm`. The default for most providers.
- **native** (`kind = "native"` plus the `.native(...)` line): no sandbox
  and no wasmtime or Cargo at run time, for an app that ships the provider
  built in. It has the user's access, so link only code you would run
  yourself.

To write code that works in both, keep state in `self`, reach the machine
only through the SDK's `host_call` and `system_load` (gated by
`capabilities` in provider.toml), and put anything that must differ behind
`#[cfg(target_arch = "wasm32")]`. See
[Compile it into the app](../../docs/wasm-providers.md#compile-it-into-the-app).

The host binary lives in `host/`, beside `app/` rather than around it: Cargo
counts any crate inside a workspace member's folder as a member, so the
provider crate could not keep its own `[workspace]` there.
