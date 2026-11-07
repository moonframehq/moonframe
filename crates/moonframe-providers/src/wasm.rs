//! Wasm providers: prebuilt or built with Cargo, then run with wasmtime in
//! a sandbox with no files, network, or environment, one memory cap for the
//! whole component, and a fresh instruction budget for every call.

use {
    crate::{Capability, Context, PREBUILT, json, system::SystemLoad},
    state::Handoff,
    std::{
        collections::HashSet,
        env, fs,
        path::{Component as Part, Path, PathBuf},
        process::{Command, Stdio},
        sync::mpsc::RecvTimeoutError,
    },
    wasmtime::{
        Config, Engine, ResourceLimiter, Store,
        component::{Component, HasSelf, Linker, ResourceTable},
    },
    wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView},
};

mod state;

wasmtime::component::bindgen!({ path: "../../wit/provider.wit", world: "provider" });

const TARGET: &str = "wasm32-wasip2";
/// Linear memory for the whole component, all its memories together.
const MEMORY: usize = 64 * 1024 * 1024;
/// A component is several core instances (its WASI adapter among them).
const INSTANCES: usize = 32;
/// Tables, and elements in each, past what any real component needs.
const TABLES: usize = 64;
const TABLE_ELEMENTS: usize = 100_000;
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
    let instance = linker.instantiate(&mut store, &component).map_err(text)?;
    let provider = Provider::new(&mut store, &instance).map_err(text)?;
    let handoff = Handoff::find(&instance, &mut store)?;
    if !state::wait_for_old_build(context) {
        return Ok(());
    }
    refuel(&mut store)?;
    provider
        .call_start(&mut store, &context.manifest.settings.to_string())
        .map_err(text)?
        .map_err(|problem| format!("start: {problem}"))?;
    if context.stopped() {
        return Ok(());
    }
    tracing::info!(provider = %context.name, "running");
    if let Some(handoff) = &handoff {
        handoff.restore(context, &mut store)?;
    }
    refuel(&mut store)?;
    let mut published = provider.call_tick(&mut store).map_err(text)?;
    if handoff.is_some() {
        // Only now: a build that fails this early leaves it for the next.
        context.kept.forget(&context.dir);
    }
    // After a trap the instance cannot be called, so only these ends save.
    let hand_on = |store: &mut Store<State>| {
        if let Some(handoff) = &handoff {
            handoff.save(context, store);
        }
    };
    loop {
        if let Some(text) = published.take() {
            match json::data(&text) {
                Ok(data) => context.publish(data),
                Err(problem) => {
                    hand_on(&mut store);
                    return Err(problem.to_string());
                },
            }
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
            Err(RecvTimeoutError::Disconnected) => {
                hand_on(&mut store);
                return Ok(());
            },
        };
        if context.stopped() {
            if call.is_ok() {
                hand_on(&mut store);
            }
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

/// Cargo: the one running this build, else on `PATH` or in the usual
/// places, rustup's among them (an app opened from Finder has a short
/// `PATH`).
fn cargo() -> Option<PathBuf> {
    let mut path = crate::program::default_search_path();
    if let Some(home) = env::var_os("CARGO_HOME") {
        let mut dirs: Vec<PathBuf> = env::split_paths(&path).collect();
        dirs.push(Path::new(&home).join("bin"));
        path = env::join_paths(dirs).unwrap_or(path);
    }
    env::var_os("CARGO")
        .map(PathBuf::from)
        .filter(|cargo| cargo.is_file())
        .or_else(|| crate::program::find("cargo", &path))
}

/// Builds the crate in the provider's folder and returns its component.
/// This runs Cargo with the user's access; only the component it produces is
/// sandboxed. Code of the provider's own that would run while building is
/// refused: a build script, the local crates it uses having one, and a
/// toolchain file naming a toolchain by path. Cargo runs from the build
/// folder, so no Cargo config in the provider's folders applies either.
/// What is left is what any crate can do: proc macros, and the build
/// scripts of published dependencies.
fn build(context: &Context, cargo: &Path) -> Result<PathBuf, String> {
    tracing::info!(provider = %context.name, "building");
    check_hooks(&context.dir)?;
    let toolchain = toolchain(&context.dir)?;
    let manifest = context.dir.join("Cargo.toml");
    let package = read_manifest(&context.dir)?
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_owned)
        .ok_or("Cargo.toml has no [package] name")?;
    fs::create_dir_all(&context.build_dir)
        .map_err(|error| format!("cannot make {}: {error}", context.build_dir.display()))?;
    let mut command = Command::new(cargo);
    if let Some(toolchain) = toolchain {
        // Found as rustup would from the provider's folder, which Cargo no
        // longer runs in.
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }
    let output = command
        .current_dir(&context.build_dir)
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

/// How deep local path dependencies are followed.
const MAX_LOCAL_CRATES: usize = 8;
/// How many local crates are checked in all: links can make one crate
/// look like many.
const MAX_CHECKED_CRATES: usize = 256;

fn read_manifest(dir: &Path) -> Result<toml::Table, String> {
    let manifest = dir.join("Cargo.toml");
    fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?
        .parse::<toml::Table>()
        .map_err(|error| format!("{}: {error}", manifest.display()))
}

/// Refuses build scripts in the crate at `dir` and the local crates it
/// depends on or patches in: they would run with the user's access.
pub(crate) fn check_hooks(dir: &Path) -> Result<(), String> {
    refuse_hooks(dir, 0, &mut Checked::default())
}

/// What was already checked: crates of one workspace depend on each other
/// and share its root, and each is checked once.
#[derive(Default)]
struct Checked {
    crates: HashSet<PathBuf>,
    workspaces: HashSet<PathBuf>,
}

impl Checked {
    /// Whether the crate (or workspace root) at `key` is new; past
    /// [`MAX_CHECKED_CRATES`] in all, the build is refused. Keys are paths
    /// as Cargo works them out, not through links: one folder reached two
    /// ways can sit in two workspaces, so each way is checked, and the cap
    /// keeps links from making that endless.
    fn first(&mut self, key: &Path, workspace: bool) -> Result<bool, String> {
        let set = if workspace {
            &mut self.workspaces
        } else {
            &mut self.crates
        };
        if !set.insert(key.to_owned()) {
            return Ok(false);
        }
        if self.crates.len() + self.workspaces.len() > MAX_CHECKED_CRATES {
            return Err(format!(
                "a wasm provider can use at most {MAX_CHECKED_CRATES} local crates"
            ));
        }
        Ok(true)
    }
}

fn refuse_hooks(dir: &Path, depth: usize, checked: &mut Checked) -> Result<(), String> {
    // Cargo resolves `..` in a path dependency before it looks for the
    // workspace above it; walking the path as written could look elsewhere.
    let dir = &normalize(dir)?;
    if !checked.first(dir, false)? {
        return Ok(());
    }
    let hook = |what: &str| {
        Err(format!(
            "a wasm provider cannot have {what}: it would run code with your access while building"
        ))
    };
    if dir.join("build.rs").exists() {
        return hook("a `build.rs`");
    }
    if dir.join(".cargo").exists() {
        return hook("a `.cargo` folder");
    }
    let manifest = read_manifest(dir)?;
    if manifest
        .get("package")
        .and_then(|package| package.get("build"))
        .is_some_and(|build| build.as_bool() != Some(false))
    {
        return hook("a build script (`package.build`)");
    }
    // The manifest's top level, and each `[target.<cfg>]` table: both can
    // list dependencies of either kind.
    let sections: Vec<&toml::Table> = std::iter::once(&manifest)
        .chain(
            manifest
                .get("target")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|targets| targets.values())
                .filter_map(toml::Value::as_table),
        )
        .collect();
    if sections
        .iter()
        .any(|section| section.contains_key("build-dependencies"))
    {
        return hook("`build-dependencies`");
    }
    // Every table of dependencies the build can use.
    let tables: Vec<&toml::Table> = sections
        .iter()
        .filter_map(|section| section.get("dependencies").and_then(toml::Value::as_table))
        .collect();
    refuse_local_crates(dir, tables, depth, checked)?;
    refuse_workspace_crates(dir, &manifest, depth, checked)?;
    // A crate in a workspace of its own sees nothing above it; one that
    // belongs to a parent's inherits that workspace's dependencies
    // (`helper = { workspace = true }`) and its `[patch]`es.
    if manifest.contains_key("workspace") {
        return Ok(());
    }
    let Some(root) = workspace_root(dir, &manifest) else {
        return Ok(());
    };
    let root = normalize(&root)?;
    if !checked.first(&root, true)? {
        return Ok(());
    }
    let parent = read_manifest(&root)?;
    refuse_workspace_crates(&root, &parent, depth, checked)
}

/// Refuses hooks in the local crates a workspace root at `dir` lists for
/// its members or swaps in with `[patch]` or `[replace]`.
fn refuse_workspace_crates(
    dir: &Path,
    manifest: &toml::Table,
    depth: usize,
    checked: &mut Checked,
) -> Result<(), String> {
    let mut tables: Vec<&toml::Table> = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .into_iter()
        .collect();
    tables.extend(
        manifest
            .get("patch")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flat_map(|sources| sources.values())
            .filter_map(toml::Value::as_table),
    );
    tables.extend(manifest.get("replace").and_then(toml::Value::as_table));
    refuse_local_crates(dir, tables, depth, checked)
}

/// Follows each `path` dependency in `tables`, relative to `dir`.
fn refuse_local_crates(
    dir: &Path,
    tables: Vec<&toml::Table>,
    depth: usize,
    checked: &mut Checked,
) -> Result<(), String> {
    for path in tables
        .into_iter()
        .flat_map(|table| table.values())
        .filter_map(|dependency| dependency.get("path").and_then(toml::Value::as_str))
    {
        if depth >= MAX_LOCAL_CRATES {
            return Err(format!(
                "local crates nest deeper than {MAX_LOCAL_CRATES} at `{path}`"
            ));
        }
        refuse_hooks(&dir.join(path), depth + 1, checked)?;
    }
    Ok(())
}

/// `path` made absolute with its `.` and `..` worked out on the text, as
/// Cargo does (`normalize_path`): following links instead, as
/// `canonicalize` would, could find a workspace other than the one Cargo
/// uses.
fn normalize(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut normal = PathBuf::new();
    for part in absolute.components() {
        match part {
            Part::CurDir => {},
            Part::ParentDir => {
                normal.pop();
            },
            part => normal.push(part),
        }
    }
    Ok(normal)
}

/// The folder of the workspace Cargo puts the crate at `dir` in, when it
/// is not its own: the one `package.workspace` names, or else the nearest
/// folder above whose `Cargo.toml` has a `[workspace]`.
fn workspace_root(dir: &Path, manifest: &toml::Table) -> Option<PathBuf> {
    if let Some(named) = manifest
        .get("package")
        .and_then(|package| package.get("workspace"))
        .and_then(toml::Value::as_str)
    {
        return Some(dir.join(named));
    }
    dir.ancestors()
        .skip(1)
        .find(|folder| {
            fs::read_to_string(folder.join("Cargo.toml"))
                .ok()
                .and_then(|text| text.parse::<toml::Table>().ok())
                .is_some_and(|table| table.contains_key("workspace"))
        })
        .map(Path::to_owned)
}

/// The toolchain rustup would pick for `dir` from a toolchain file, which
/// Cargo running elsewhere would not see. One named by path is refused:
/// it could be any program.
fn toolchain(dir: &Path) -> Result<Option<String>, String> {
    let Some(file) = dir.ancestors().find_map(|folder| {
        ["rust-toolchain.toml", "rust-toolchain"]
            .iter()
            .map(|name| folder.join(name))
            .find(|file| file.is_file())
    }) else {
        return Ok(None);
    };
    let text = fs::read_to_string(&file)
        .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let channel = match text.parse::<toml::Table>() {
        Ok(table) => {
            let toolchain = table.get("toolchain");
            if toolchain
                .and_then(|toolchain| toolchain.get("path"))
                .is_some()
            {
                return Err(format!(
                    "{} names a toolchain by path, which could run anything while building",
                    file.display()
                ));
            }
            toolchain
                .and_then(|toolchain| toolchain.get("channel"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        },
        // The older form: the toolchain's name alone.
        Err(_) => text.lines().next().map(|line| line.trim().to_owned()),
    };
    match channel {
        Some(channel)
            if !channel.is_empty()
                && channel
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) =>
        {
            Ok(Some(channel))
        },
        Some(channel) => Err(format!(
            "{} names the toolchain `{channel}`, which is not a channel",
            file.display()
        )),
        None => Ok(None),
    }
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
    limits: Limits,
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
            limits: Limits::default(),
            name: context.name.clone(),
            capabilities: context.manifest.capabilities.clone(),
            system: None,
        }
    }
}

/// The store's limits: [`MEMORY`] across every memory a component makes,
/// so many small memories cost no more than one large one.
#[derive(Default)]
struct Limits {
    /// Bytes of linear memory granted so far.
    used: usize,
    /// The last growth granted, given back if it then failed.
    granted: usize,
}

impl ResourceLimiter for Limits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let growth = desired.saturating_sub(current);
        let used = self.used.saturating_add(growth);
        if used > MEMORY || maximum.is_some_and(|maximum| desired > maximum) {
            return Ok(false);
        }
        self.used = used;
        self.granted = growth;
        Ok(true)
    }

    fn memory_grow_failed(&mut self, _: wasmtime::Error) -> wasmtime::Result<()> {
        self.used = self.used.saturating_sub(std::mem::take(&mut self.granted));
        Ok(())
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= TABLE_ELEMENTS && maximum.is_none_or(|maximum| desired <= maximum))
    }

    fn instances(&self) -> usize {
        INSTANCES
    }

    fn tables(&self) -> usize {
        TABLES
    }

    fn memories(&self) -> usize {
        INSTANCES
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
        let system = match &mut self.system {
            Some(system) => system,
            empty => empty.insert(SystemLoad::new().map_err(|error| error.to_string())?),
        };
        Ok(system.sample().to_string())
    }
}
