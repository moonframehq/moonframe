# Smaller, sealed app bundles

A sealed bundle builds a dedicated executable with only the app's selected
native capabilities. It ships the Luau components and providers reachable
from the app, without the full embedded fallback toolbox. Restoring a removed
capability requires a new build.

```sh
# Inspect dependencies first; this neither builds nor runs the app.
just bundle-plan examples/ledger

# Build directly from an app folder, with local package requirements.
just bundle-sealed examples/ledger

# Or use an installed/linked profile, including its resolved remote packages.
just bundle-app-sealed ledger

# Release archives and checksums work as with ordinary bundles.
DIST=target/dist just bundle-app-sealed ledger
```

Python 3.11 or newer is required. Builds target the local macOS or Linux
machine. `MOONFRAME_USER` selects the profile installation directory.
`MOONFRAME_BINARY` is deliberately unsupported: an existing full executable
cannot be made smaller by deleting resource files.

## What is selected

The planner starts from the shell, every app/package mod, the built-in mods
named by `app.keep`, and the stylesheets. Mods can subscribe or send messages
without being mounted, so an app's own mods are retained even if initially
hidden. Effective definitions follow the bundle's layer order; overridden
fallback definitions are omitted.

- Literal `use "component"` and `use("component")` imports are followed
  recursively, including imports inside branches that have not run yet.
- Provider reads (`app.git`, `app["git"]`), literal `provider:name`
  destinations and `provider/name` subscriptions select providers.
- `start = "always"` providers are kept even without a reference.
- Native provider manifests select their Cargo features. Any selected Wasm
  provider enables Wasmtime; only selected Wasm providers are built.
- `ui.terminal`, `ui.browser` and `ui.markdown` select their host capabilities.
  Literal terminal property tables select their engine, defaulting to
  Alacritty; literal `ghostty` and `herdr-core` selections enable those
  optional capabilities too. Otherwise
  automatic terminal backend selection uses tmux or a local pty.
- Component demos, unused built-in mods/providers/components, development
  folders and Wasm build sources are omitted. Other app/package files are
  preserved, because process providers and image paths can use them dynamically.

The executable is built with Cargo defaults disabled, using the `bundle`
profile: full LTO, one codegen unit, stripped symbols and normal release
optimisation. Standard builds still enable the full feature set.

## Explicit and dynamic dependencies

Additional dependencies go in the app's `moonframe.toml`:

```toml
[bundle]
components = ["empty-state"]
providers = ["files"]
features = ["herdr-core"]

[bundle.dynamic]
# Complete sets of possible runtime targets, not just the initial screen.
components = ["terminal", "browser"]
providers = ["git", "docs"]
features = ["terminal", "browser"]
```

Known computed forms such as `use(name)`, `app[name]`, `ui[element]`, and
interpolated provider destinations fail planning until their category has
an explicit `bundle.dynamic` array. An empty array is an explicit assertion
that no additional dependency of that category is needed. Dynamic component
declarations are themselves traversed for further imports and providers.
For example, `tab-view` dynamically imports the selected tab's component:
declare every tab kind the app offers, including ones restored from state.

This is conservative lexical analysis, **not a proof of arbitrary Luau
behaviour**. Dependencies hidden behind renamed parameters, helper functions,
provider code, externally supplied names, or shared CSS-like classes need
explicit declarations. Comments and quoted documentation are not scanned as
code. A runtime trace alone would miss unopened screens and is not used.

Supported extra features are `acp`, `alacritty`, `browser`, `chat`, `docs`,
`forge`, `ghostty`, `git`, `herdr-core`, `markdown`, `shell`, `terminal`,
`tokio`, `usage`, and `wasm`. A custom Rust native provider needs a custom
library host; the profile bundler cannot compile an unknown native provider.

## Behaviour and verification

Sealed executables run only their bundled profile and source layers. User
mods, replacement shells, global stylesheets and installed packages cannot
restore pruned code. Settings and saved state still persist normally.
Development/package-management commands, Developer Mode, the component
gallery and app-building commands are unavailable. The inspector used by
the feedback component remains available to app code; it is not currently
a separately removable capability.

The planner does not remove the feedback mod implicitly: if `app.keep`
retains it, its ACP and other dependencies remain. For a fixed-function app,
remove that entry and its shell mount in the app's source before bundling.

The bundle runs `--no-user --check` before signing/archiving. Exercise the
app's later interactions too, especially dynamic dependencies; the headless
check cannot simulate every state or start every provider. A
`sealed-build.json` in the bundle's resources records the selected features,
components, providers, removed definitions, declarations and byte counts.
Source paths in the report are relative to their app/package/toolbox layer.

The remaining baseline includes GPUI, Luau, text input, persistence, common
UI primitives, themes and icon assets. Sealing removes optional subsystems;
it does not attempt function-level tree shaking inside arbitrary Luau, prune
every image or stylesheet rule, or rebuild GPUI with a different codec set.

Run the planner regression tests with:

```sh
just test-sealed-bundles
```
