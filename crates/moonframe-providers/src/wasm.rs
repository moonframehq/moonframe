//! Wasm providers: prebuilt or built with Cargo, then run with wasmtime in
//! a sandbox with no files, network, or environment, a memory cap, and a
//! fresh instruction budget for every call.

use {
    crate::{Capability, Context, PREBUILT, json, system::SystemLoad},
    std::{
        env, fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::mpsc::RecvTimeoutError,
    },
    wasmtime::{
        Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
        component::{Component, HasSelf, Linker, ResourceTable},
    },
    wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView},
};

wasmtime::component::bindgen!({ path: "../../wit/provider.wit", world: "provider" });

const TARGET: &str = "wasm32-wasip2";
const MEMORY: usize = 64 * 1024 * 1024;
/// A component is several core instances (its WASI adapter among them).
const INSTANCES: usize = 32;
/// Instructions, roughly, for one call: plenty for a tick, and an end to a
/// loop that never returns.
const FUEL: u64 = 2_000_000_000;
/// Build output kept for a failed build's problem.
const BUILD_LINES: usize = 20;

pub(crate) fn run(context: &Context) {
    if let Err(problem) = serve(context) {
        context.fail(problem);
    }
}

fn serve(context: &Context) -> Result<(), String> {
    let wasm = component(context)?;
    if context.stopped() {
        return Ok(());
    }
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config).map_err(text)?;
    let component = Component::from_file(&engine, &wasm).map_err(text)?;
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(text)?;
    Provider::add_to_linker::<State, HasSelf<State>>(&mut linker, |state| state).map_err(text)?;
    let mut store = Store::new(&engine, State::new(context));
    store.limiter(|state| &mut state.limits);

    // Instantiating runs the component's own setup, which burns fuel too.
    refuel(&mut store)?;
    let provider = Provider::instantiate(&mut store, &component, &linker).map_err(text)?;
    refuel(&mut store)?;
    provider
        .call_start(&mut store, &context.manifest.settings.to_string())
        .map_err(text)?
        .map_err(|problem| format!("start: {problem}"))?;
    tracing::info!(provider = %context.name, "running");
    refuel(&mut store)?;
    let mut published = provider.call_tick(&mut store).map_err(text)?;
    loop {
        if let Some(text) = published.take() {
            context.publish(json::data(&text).map_err(|problem| problem.to_string())?);
        }
        let call = match context.messages.recv_timeout(context.manifest.interval) {
            Ok(message) => {
                refuel(&mut store)?;
                provider.call_receive(&mut store, &json::json(&message.data).to_string())
            },
            Err(RecvTimeoutError::Timeout) => {
                refuel(&mut store)?;
                provider.call_tick(&mut store)
            },
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        if context.stopped() {
            return Ok(());
        }
        published = call.map_err(text)?;
    }
}

/// The provider's component: built from its crate while developing (a
/// `Cargo.toml` and Cargo at hand), else the `provider.wasm` it ships with.
/// A failed build is the provider's problem, never a quiet fallback to an
/// older `provider.wasm`, so an edit's compile error always shows.
fn component(context: &Context) -> Result<PathBuf, String> {
    let prebuilt = context.dir.join(PREBUILT);
    let source = context.dir.join("Cargo.toml").is_file();
    match (source, cargo()) {
        (true, Some(cargo)) => build(context, &cargo),
        _ if prebuilt.is_file() => {
            tracing::info!(provider = %context.name, "loading {PREBUILT}");
            Ok(prebuilt)
        },
        _ => Err(format!(
            "cannot build it: Cargo is not installed, and there is no {PREBUILT}"
        )),
    }
}

/// Cargo: the one running this build, else on `PATH`, else where rustup puts
/// it (an app opened from Finder has a short `PATH`).
fn cargo() -> Option<PathBuf> {
    let on_path = env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join("cargo"));
    let rustup = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(".cargo")))
        .map(|home| home.join("bin").join("cargo"));
    env::var_os("CARGO")
        .map(PathBuf::from)
        .into_iter()
        .chain(on_path)
        .chain(rustup)
        .find(|cargo| cargo.is_file())
}

/// Builds the crate in the provider's folder and returns its component.
/// This runs Cargo with the user's access; only the component it produces is
/// sandboxed. A build script or folder-local Cargo config would run code at
/// build time without even a proc macro, so those are refused.
fn build(context: &Context, cargo: &Path) -> Result<PathBuf, String> {
    tracing::info!(provider = %context.name, "building");
    for hook in ["build.rs", ".cargo"] {
        if context.dir.join(hook).exists() {
            return Err(format!(
                "a wasm provider cannot have `{hook}`: it would run code with your access while building"
            ));
        }
    }
    let manifest = context.dir.join("Cargo.toml");
    let package = fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?
        .parse::<toml::Table>()
        .map_err(|error| format!("Cargo.toml: {error}"))?
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_owned)
        .ok_or("Cargo.toml has no [package] name")?;
    let output = Command::new(cargo)
        .current_dir(&context.dir)
        .args([
            "build",
            "--quiet",
            "--release",
            "--target",
            TARGET,
            "--manifest-path",
        ])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&context.build_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if !output.status.success() {
        let log = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = log.lines().collect();
        let tail = lines[lines.len().saturating_sub(BUILD_LINES)..].join("\n");
        return Err(format!("build failed:\n{tail}"));
    }
    Ok(context
        .build_dir
        .join(TARGET)
        .join("release")
        .join(format!("{}.wasm", package.replace('-', "_"))))
}

fn refuel(store: &mut Store<State>) -> Result<(), String> {
    store.set_fuel(FUEL).map_err(text)
}

fn text(error: wasmtime::Error) -> String {
    format!("{error:#}")
}

/// What the provider's store holds: its sandbox and what it was granted.
struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    name: String,
    capabilities: Vec<Capability>,
    system: Option<SystemLoad>,
}

impl State {
    fn new(context: &Context) -> Self {
        Self {
            // No preopened folders, environment, or arguments: WASI's own
            // calls reach nothing of the machine's.
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY)
                .instances(INSTANCES)
                .build(),
            name: context.name.clone(),
            capabilities: context.manifest.capabilities.clone(),
            system: None,
        }
    }
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl moonframe::provider::host::Host for State {
    fn log(&mut self, message: String) {
        tracing::info!(provider = %self.name, "{}", moonframe_core::printable(&message));
    }

    fn system_load(&mut self) -> Result<String, String> {
        if !self.capabilities.contains(&Capability::System) {
            return Err("needs `capabilities = [\"system\"]` in provider.toml".to_owned());
        }
        Ok(self
            .system
            .get_or_insert_with(SystemLoad::new)
            .sample()
            .to_string())
    }
}
