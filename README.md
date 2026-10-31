<p align="center">
  <img src="assets/icon.png" alt="Moonframe icon: an indigo-to-violet rounded square holding a small window split into a sidebar, a main area, a side panel with a bar chart, and a status bar" width="176" height="176">
</p>

<h1 align="center">Moonframe</h1>

<p align="center">
  <a href="#quick-start"><b>Quick start</b></a> ·
  <a href="#make-it-yours">Make it yours</a> ·
  <a href="#building-blocks">Building blocks</a> ·
  <a href="#working-with-an-agent">Working with an agent</a> ·
  <a href="#mods-from-git">Mods from git</a> ·
  <a href="#commands">Commands</a> ·
  <a href="#license">License</a>
</p>

A starter for native desktop apps whose whole window is written in
[Luau](https://luau.org). Rust and [GPUI](https://www.gpui.rs) draw it, check
it, and keep it safe; every panel, split, toolbar and status bar is a Luau
file that reloads as you save, with its state kept. Clone it, rename it, and
build anything, by hand or by asking an agent while the app runs.

## Gallery

<p align="center">
  <img src="docs/screenshots/window.png" alt="The Moonframe window: an icon toolbar; an Explorer panel listing files on the left; a Terminal panel with a shell prompt in the middle; an Inspector panel with the selected file and the loaded mods on the right; and a status bar with usage meters, the git branch, a CPU sparkline, a memory meter and network rates" width="900">
  <br>
  <sub><b>The window that ships.</b> Every part of it is a mod in <code>app/mods/</code>: the explorer, the terminal, the inspector, and the status bar widgets, whose CPU, memory and git data come from providers.</sub>
</p>

<table>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/dark.png" alt="The same window in the Catppuccin Macchiato dark theme">
      <br>
      <sub><b>Themes.</b> Pick one of the bundled color schemes from the palette button or <b>View &gt; Themes…</b>; the window, its controls and the terminals all follow it. <code>style.luau</code> restyles anything on top.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/components.png" alt="The Components window: a list of components by category on the left, and the List component on the right with a live preview of three rows, the Luau that uses it, and Copy code and Copy for agent buttons">
      <br>
      <sub><b>Components.</b> Ready-made pieces mods <code>use</code>, browsed from the <b>Components</b> menu with a live preview, the code to paste, and <b>Copy for agent</b>.</sub>
    </td>
  </tr>
</table>

## What you get

- **A window laid out in Luau.** A shell (`app/main.luau`) places regions,
  and mods fill them. Each mod is one file with its own state, `init`,
  `update` and `view`, like an Elm program; mods talk with messages.
- **Live reload.** Save a file and the window redraws in place, keeping each
  mod's state. A mistake never blanks or shifts the window: a bad element
  shows as a small warning mark, and a broken mod keeps its last good view.
- **A stylesheet.** Colors, corners, shadows, spacing, fonts and animations
  in `app/style.luau`, with rules by element type and class, hover states,
  transitions on panels, and styles a mod brings for itself.
- **Components** that mods share with `use "name"`, each with a preview in
  the Components window.
- **Terminals.** `ui.terminal` runs your login shell on
  [`alacritty_terminal`](https://crates.io/crates/alacritty_terminal) or on
  Ghostty's [libghostty-vt](https://crates.io/crates/libghostty-vt).
- **Providers** for data Luau can't reach: sandboxed WebAssembly components,
  or plain processes, publishing JSON that views read as `app.<name>`.
- **An inspector.** <b>View &gt; Inspect Element</b> picks anything in the
  window, like a browser's developer tools, and tells an agent which file
  and line drew it.
- **Your folder over the app.** `~/.config/moonframe` holds your own mods,
  providers and stylesheet, laid over the built-in app; updates never touch
  it, and mods install from git into it.
- **A skill for agents** in `.claude/skills/moonframe` that teaches them to
  build with all of the above.
- **A sandbox.** Luau runs with no files, network or process access, a
  memory cap and an instruction budget; anything that reaches the machine
  goes through a provider or an element the host owns.

## Quick start

You need Rust (the version in `rust-toolchain.toml` is picked up by rustup)
and [just](https://github.com/casey/just). [mise](https://mise.jdx.dev)
installs the rest from `mise.toml`.

```sh
git clone https://github.com/penso/moonframe my-app
cd my-app
mise install        # Rust, and Zig for the Ghostty terminal
just run            # opens the window on app/
```

Now edit `app/mods/explorer/main.luau` or `app/style.luau` and save: the
window changes as you type. `just check` loads everything without a window
and points at any problem, and `just types` type-checks the Luau.

The Ghostty terminal builds libghostty-vt from source with Zig 0.15. To
build without it, use `cargo run --no-default-features --features alacritty`.
On macOS 27, Zig 0.15 needs the wrapper in `tools/zig`, which mise and the
justfile put first on `PATH`, and a macOS SDK of version 15 or older.

Moonframe is developed on macOS. GPUI also runs on Linux and Windows, but
those aren't tested here yet.

## Make it yours

Moonframe is meant to be copied and renamed:

1. **The app.** Replace what's in `app/`: `main.luau` lays out the window,
   `mods/` fills it, `style.luau` styles it. Keep what helps and delete the
   rest; the explorer and inspector are examples.
2. **The name.** The crate names and `repository` in `Cargo.toml`, the
   app id `APP_ID` in `crates/moonframe/src/main.rs`, the user folder's name
   in `crates/moonframe/src/paths.rs`, the bundle's name in
   `scripts/bundle-app.sh`, and the title in `app/main.luau`.
3. **The icon.** `assets/icon.png`, drawn by `scripts/draw-icon.swift`.
4. **Ship it.** `just bundle` builds a `.app` with your app inside, and
   `just install` puts it in `/Applications`.

## Building blocks

A mod is a folder with one file:

```lua
-- app/mods/counter/main.luau
return mod {
	region = "left",

	init = function(previous)
		return previous or { count = 0 }
	end,

	update = function(state, msg)
		if msg == "add" then
			state.count += 1
		end
	end,

	view = function(state, app)
		return ui.panel {
			title = "Counter",
			ui.text(`Clicked {state.count} times`),
			ui.button { "Add", msg = "add", variant = "primary" },
		}
	end,
}
```

Save it and it appears in the left region. From there:

| Need | Write |
| --- | --- |
| A look | a rule in `style.luau`: `panel = { radius = 12, shadow = "0 4px 12px #0003" }` |
| A reusable piece | `components/<name>.luau`, used with `local stat = use "stat"` |
| A shell | `ui.terminal { id = "main", cwd = "~/code" }` |
| Data from the machine | a provider in `providers/<name>/`, read as `app.<name>` |
| A sheet or menu | `ui.popover { open = state.open, dismiss = "close", content = sheet, widget }` |
| A new kind of element | Rust in `crates/`, the one change that needs a rebuild |

The skill in `.claude/skills/moonframe/SKILL.md` is the full reference:
every element, the stylesheet, components, terminals, providers, and
recipes.

### How it is laid out

```text
app/                    the built-in app: shell, mods, components, providers, style
crates/moonframe-core/  reads Luau into checked element trees, styles, components
crates/moonframe/       the window: draws the trees with GPUI, inspector, menus
crates/moonframe-providers/  runs Wasm and process providers
crates/moonframe-terminal/   shells on alacritty_terminal and libghostty-vt
types/moonframe.d.luau  types for the Luau API, used by `just types` and editors
wit/provider.wit        the interface a Wasm provider implements
```

## Working with an agent

The window reloads while an agent edits it, so you can watch it work.

- **The skill.** Claude Code picks up `.claude/skills/moonframe`, which
  tells an agent where a feature belongs, how to build it, and how to check
  it with `just check` and `just types`.
- **"Add it here."** <b>View &gt; Inspect Element</b> (⌥⌘I), click an
  element, and say "add a list of branches here". The selection, with its
  file and line, is saved where the agent reads it.
- **Pick a component.** In the Components window, <b>Copy for agent</b>
  copies a request to add that component at your selection.

## Mods from git

Mods and providers can live in their own repositories and install into your
folder:

```sh
moonframe mod new ~/code/my-mod               # a git repo with one mod and a README
moonframe mod link ~/code/my-mod              # use it in place, editing live
moonframe mod install github.com/owner/repo   # or owner/repo, a git URL, a folder; @ref pins it
moonframe mod list | update | remove NAME | enable NAME | disable NAME
```

Packages ship source only, so everything they run can be read first.

## Commands

| Command | Does |
| --- | --- |
| `just run` | opens the window on `app/`, with `~/.config/moonframe` over it |
| `just check` | loads everything without a window and reports problems |
| `just types` | type-checks the Luau in strict mode |
| `just providers` | builds and lints the Wasm providers |
| `just bundle` | builds `target/bundle/Moonframe.app` |
| `just install` | builds the app and installs it in `/Applications` |
| `just ci` | formatting, lints, tests, types, providers and check |

`moonframe --help` lists the app's own options, such as `--user DIR`,
`--no-user`, `--check`, and `--background` (open without taking focus).

## License

[MIT](LICENSE).
