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
  <a href="#whole-apps-as-profiles">Apps</a> ·
  <a href="#example-apps">Example apps</a> ·
  <a href="#your-own-app-on-the-library">Library</a> ·
  <a href="#commands">Commands</a> ·
  <a href="#license">License</a>
</p>

A starter for native desktop apps whose whole window is written in
[Luau](https://luau.org). Rust and [GPUI](https://www.gpui.rs) draw it, check
it, and keep it safe; every panel, split, toolbar and status bar is a Luau
file that reloads as you save, with its state kept. Clone it, rename it, and
build anything, by hand or by asking an agent while the app runs, even from
inside the app: its **Send feedback** button hands what you write to a coding
agent, which changes the app in front of you.

Whole apps run on it too, each as a profile of its own: `examples/` holds
25 of them, from a mail client to an infra console, all in one gallery with
`just demo`. A Rust app can also use it as a library.

## Gallery

<p align="center">
  <img src="docs/screenshots/window.png" alt="The Moonframe window: a title bar with panel toggles and a terminal tab; an Explorer panel listing the repository's files on the left; a terminal in the middle; a Changes panel with the changed files as a tree and one file's diff on the right; and a status bar with usage meters, the git branch, a CPU sparkline, a memory meter and network rates" width="900">
  <br>
  <sub><b>The window that ships.</b> Every part of it is a mod in <code>app/mods/</code>: the explorer, the terminal tabs, the changes and their diffs, and the status bar widgets, whose CPU, memory and git data come from providers. <b>+</b> opens more tabs: terminals, browsers, files; a markdown file opens in an editor with a live preview.</sub>
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

<p align="center">
  <img src="docs/screenshots/apps/demo.png" alt="The demo gallery: cards for the example apps, each with a screenshot, its title, what it is and the profile it runs as; Emberline, Gantry, Hearth, Tandem, Lantern and Ledger show, with Open all at the top right" width="900">
  <br>
  <sub><b>The example apps</b>, in the gallery <code>just demo</code> opens: a click opens one as a window of its own, beside the others. See <a href="#example-apps">Example apps</a>.</sub>
</p>

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
- **Over 140 components** that mods share with `use "name"`: tabs, tables,
  charts, a command palette, a markdown editor, dialogs, sheets, toasts,
  calendars, chat messages and more, each with a live preview in the
  Components window.
- **Terminals.** `ui.terminal` runs your login shell on
  [`alacritty_terminal`](https://crates.io/crates/alacritty_terminal) or on
  Ghostty's [libghostty-vt](https://crates.io/crates/libghostty-vt). With
  herdr-core or tmux installed, terminals keep running after the app quits
  and come back on the next launch.
- **Providers** for data Luau can't reach, publishing what views read as
  `app.<name>`: sandboxed WebAssembly components, plain processes, or Rust
  built into the app. The built-in ones cover git (status, diffs, commit and
  push), GitHub and GitLab, the project's files and markdown documents, the
  system's load, and coding agents.
- **Coding agents in the app.** The `acp` provider runs Claude, Codex,
  Gemini, OpenCode or Copilot over the
  [Agent Client Protocol](https://agentclientprotocol.com) for any mod, and
  `agent-run` shows what each is doing: its plan, tool calls, permission
  questions and answer.
- **State that lasts.** Each mod's state is kept in SQLite across launches,
  and survives a reload.
- **An inspector.** <b>View &gt; Inspect Element</b> picks anything in the
  window, like a browser's developer tools, and tells an agent which file
  and line drew it.
- **Your folder over the app.** `~/.config/moonframe` holds your own mods,
  providers and stylesheet, laid over the built-in app; updates never touch
  it, and mods install from git into it.
- **A skill for agents** in `.claude/skills/moonframe` that teaches them to
  build with all of the above. The app offers to copy its skills to
  `~/.config/moonframe/.claude/skills` (only if you agree, and again when an
  update brings newer ones), so an agent working on your own mods knows
  them too.
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

Moonframe is developed on macOS. CI builds, lints and tests it on macOS,
Linux (x86_64 and ARM64) and Windows (x86_64), and each release ships a macOS disk
image, Linux tarballs and Windows zips, with checksums, Sigstore signatures
and GitHub attestations; see [docs/releasing.md](docs/releasing.md).

## Make it yours

Moonframe is meant to be copied and renamed:

1. **The app.** Replace what's in `app/`: `main.luau` lays out the window,
   `mods/` fills it, `style.luau` styles it. Keep what helps and delete the
   rest; the explorer, the tabs and the changes are a place to start.
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
crates/moonframe/       the library: the window, drawn with GPUI, inspector, menus
crates/moonframe-app/   the `moonframe` binary: the starter, on the library
crates/moonframe-providers/  runs Wasm, process and native providers
crates/moonframe-terminal/   shells on alacritty_terminal and libghostty-vt
crates/moonframe-mux/   keeps terminals in herdr-core or tmux, else a local pty
crates/moonframe-store/ the state kept across launches, in SQLite
crates/moonframe-git/   the native git provider
crates/moonframe-acp/   coding agents over the Agent Client Protocol
crates/moonframe-docs/  reads and saves the project's documents for the editor
examples/               apps built on it, and a package they share
types/moonframe.d.luau  types for the Luau API, used by `just types` and editors
wit/provider.wit        the interface a Wasm provider implements
```

## Working with an agent

The window reloads while an agent edits it, so you can watch it work.

- **Send feedback.** The `?` in the title bar (or in a corner of an example
  app) opens a box: write what should change, attach an element picked with
  Inspect if it helps, and send. A coding agent in the background changes
  that app's own files with every permission, checks them, then screenshots
  your window and looks at the result before it answers; the window reloads
  with the change. Each request becomes a conversation under **Feedback &
  fixes**, so when something is not quite right you reply and the agent
  carries on from there.
- **The skill.** Claude Code picks up `.claude/skills/moonframe`, which
  tells an agent where a feature belongs, how to build it, and how to check
  it with `just check` and `just types`.
- **"Add it here."** <b>View &gt; Inspect Element</b> (⌥⌘I), click an
  element, and say "add a list of branches here". The selection, with its
  file and line, is saved where the agent reads it.
- **Pick a component.** In the Components window, <b>Copy for agent</b>
  copies a request to add that component at your selection.
- **See it before it is made.** Ask for ideas ("show me three takes on the
  status bar") and the `moonframe-mockups` skill draws them live, side by
  side, in a Mockups tab of your window. <b>Pick</b> copies your choice for
  the agent; nothing real changes until then.

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

## Whole apps as profiles

The starter stays as it is; a whole other app installs beside it as a
profile, with its own shell, mods, packages and theme, and runs as an app of
its own:

```sh
moonframe app install github.com/owner/some-app   # clones it and what it requires
moonframe --profile some-app                      # opens it
moonframe app new ~/code/my-app                   # start one: moonframe.toml, a shell, a mod
moonframe app link ~/code/my-app                  # run it in place, editing live
moonframe app list | update NAME | remove NAME
```

An app is a repository laid out like `app/`, with a `moonframe.toml`:

```toml
[app]
name = "my-app"                  # the profile it installs as
title = "My App"                 # its name in the menu bar and bundle
icon = "icon.png"                # its icon (default icon.png)
keep = ["terminal"]              # built-in mods it shows; it brings the rest
requires = ["owner/repo@v1"]     # packages it installs with it, or "../shared"
```

The built-in providers, components and styles are there for it to use; the
built-in mods are not, unless it keeps them. Profiles live in
`~/.config/moonframe/profiles/`, and your own `style.luau` follows you
into each.

`just bundle-app NAME` builds a profile's app as a standalone release:
`target/bundle/<Title>.app` on macOS, or a prefix with a `.desktop` file
on Linux, with the app's title, id and icon. A release leaves out the
Components menu, Inspect Element and Reload; `--release` runs any window
that way, and `--dev` brings the tools back in a bundle. Outside a release,
**Build App Bundle** in the app menu builds the window's own app the same
way and shows it in Finder.

An app's `main.luau` can also add `menus` to the menu bar, with
shortcuts, and say what its About panel shows (`about = { version,
description, website, copyright }`); the menu bar carries the app's title.

### Example apps

`examples/` holds apps built this way, each with its own name, icon, menus,
About panel and Send feedback button. The dashboards and chats run on sample
data or a scripted agent, labelled as such.

`just demo` opens them all in one gallery (the screenshot at the top): a
picture of each, and a click opens it as a window of its own, or brings it
forward; **Open all** opens every one to compare them. Their profiles go in
`~/.local/share/moonframe/demo`, so your own folder stays as it is. Each app
runs from its folder in place, so a change, yours or an agent's from Send
feedback, shows at once.

<table>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/sessions.png" alt="Sessions: a list of projects with their branch and changes on the left, a shell in the open project in the middle, and its uncommitted files on the right">
      <br>
      <sub><b>Sessions</b> (<code>sessions</code>): coding sessions across your repositories, each with a shell, its branch, and its changes by commit, uncommitted or branch.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/workbench.png" alt="Workbench in a dark theme: a terminal with tabs on the left, an editor in the middle, and the project's files on the right">
      <br>
      <sub><b>Workbench</b> (<code>workbench</code>): a terminal, an editor and the project's files, in tabs and splits.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/daybook.png" alt="Daybook: a sidebar of views, favorites and folders; today's tasks and events under a week strip; and an inbox of tasks by section">
      <br>
      <sub><b>Daybook</b> (<code>daybook</code>): a planner with today's tasks and events, an inbox, a month, notes, and three looks.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/ledger.png" alt="Ledger: a fund overview with a 13-week cash forecast against a floor, items waiting with date tiles, a raise as a stacked bar, and a table of funds with curves">
      <br>
      <sub><b>Ledger</b> (<code>ledger</code>): a fund overview: cash against a floor, what is waiting on you, a raise, and funds with their curves.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/audience.png" alt="Audience: a newsletter dashboard with KPI cells, a dot chart of daily signups, acquisition sources with bars, and locations, devices and engagement tiers">
      <br>
      <sub><b>Inkwell</b> (<code>audience</code>): a newsletter's audience: KPIs, signups per day, where readers come from, and how engaged they are; and Learn, with courses, lessons, guides and live sessions.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/pipeline.png" alt="Pipeline: a sales kanban with KPIs above five columns of deal cards, and an agent chat panel on the right">
      <br>
      <sub><b>Tally</b> (<code>pipeline</code>): a sales kanban with KPIs, deal cards, a list view, a week of meetings, and a scripted agent panel labelled demo.</sub>
    </td>
  </tr>
</table>

<p align="center">
  <img src="docs/screenshots/apps/stage.png" alt="Stage: a bento board of components: a forecast chart, KPI cells, a month grid, a dot chart, share bars, a day strip with tasks, a progress stack and date tiles" width="900">
  <br>
  <sub><b>Stage</b> (<code>stage</code>): the components on a bento board, with sample data, for stills. View hides the captions and changes the backdrop.</sub>
</p>

<p>More, each a study of a kind of app:</p>

<table>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/parlance.png" alt="Parlance">
      <br>
      <sub><b>Parlance</b> (<code>parlance</code>): a desktop chat app with a scripted demo agent: chats, pinned chats and folders, projects and recents in a sidebar, rich markdown answers that stream in.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/cinder.png" alt="Cinder">
      <br>
      <sub><b>Cinder</b> (<code>cinder</code>): a desktop agent app in code mode: your coding-agent CLI sessions grouped by repository (read-only), their transcripts, and a scripted demo agent for new sessions.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/msgvault.png" alt="msgvault">
      <br>
      <sub><b>msgvault</b> (<code>msgvault</code>): a reader for a msgvault archive (mail, chats, calendar), after msgvault-gpui.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/tidings.png" alt="Tidings">
      <br>
      <sub><b>Tidings</b> (<code>tidings</code>): a mail client in two layouts: the list beside the open email, or one line per email across the window.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/mailbox.png" alt="Mailbox: a sidebar of views and labels beside an inbox of seven conversations">
      <br>
      <sub><b>Mailbox</b> (<code>mailbox</code>): a mail client's frame: a sidebar of folders and labels that folds to an icon rail (labels fade out one after another, the icons stay put) and the folder's messages beside it. Sample data.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/trellis.png" alt="Trellis">
      <br>
      <sub><b>Trellis</b> (<code>trellis</code>): a terminal multiplexer around projects and their work trees: each work tree keeps its own tabs of split terminals, with its source control beside them.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/lantern.png" alt="Lantern">
      <br>
      <sub><b>Lantern</b> (<code>lantern</code>): a Halloween terminal workspace: pages of projects on the left, a grid of framed panes (terminals, files, processes, changes) in tabs, a pumpkin.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/tessel.png" alt="Tessel">
      <br>
      <sub><b>Tessel</b> (<code>tessel</code>): an agents hub: your agents as pixel-icon cards or a table, the hub's runs with their steps, and a workflow to fix a failed one. Sample data.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/helpdesk.png" alt="Tandem">
      <br>
      <sub><b>Tandem</b> (<code>helpdesk</code>): an IT help desk inbox: tickets by priority, and the open ticket with the assistant's health check and the chat thread it came from.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/research.png" alt="Sorrel">
      <br>
      <sub><b>Sorrel</b> (<code>research</code>): an agent's research workspace: a thread with the agent's plan on the left, the sheet it fills cell by cell on the right (a scripted, labelled demo).</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/cadence.png" alt="Cadence">
      <br>
      <sub><b>Cadence</b> (<code>cadence</code>): a workflow automation dashboard: runs, success, tokens, the active workflows and templates, and a notification drawer behind the bell. Sample data.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/vigil.png" alt="Vigil">
      <br>
      <sub><b>Vigil</b> (<code>vigil</code>): workflow operations at a glance: time within SLA, latency and lag against their limits, what fails most, and a live feed of runs as they start and finish.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/duly.png" alt="Duly">
      <br>
      <sub><b>Duly</b> (<code>duly</code>): compliance notices for a public buyer: what is due this week, exceptions against the forecast, the contract register, a payments notice getting ready to sign, and reports for the audit committee. Sample data.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/emberline.png" alt="Emberline">
      <br>
      <sub><b>Emberline</b> (<code>emberline</code>): a CRM's leads report in the dark: pipeline figures, revenue by month, leads by status, visits, deals and sources.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/gantry.png" alt="Gantry">
      <br>
      <sub><b>Gantry</b> (<code>gantry</code>): an infra console: services, and one service's resources, usage history and alerts, in a dark monospace look. Sample telemetry that moves.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/hearth.png" alt="Hearth">
      <br>
      <sub><b>Hearth</b> (<code>hearth</code>): a code review bot's analytics in an observability tool: pull requests reviewed, issues caught, CI and the repositories behind them, on a warm black, monospace page.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/apps/storefront.png" alt="Storefront">
      <br>
      <sub><b>Storefront</b> (<code>storefront</code>): a store's admin with its audit log: browser-style tabs in the title bar, grouped settings in the sidebar, and a filterable, paged table of every activity. Sample data.</sub>
    </td>
  </tr>
</table>

Sessions, Workbench, Stage, Trellis and Lantern require the `projects`
package beside them (`examples/projects`). To run one by hand, link it in
place and open its profile:

```sh
cargo build --release -p moonframe-app
target/release/moonframe app link examples/ledger --yes
target/release/moonframe --profile ledger &
```

## Your own app, on the library

Profiles share the `moonframe` binary. An app with Rust of its own (or an
existing GPUI app moving over) depends on the `moonframe` crate instead,
and is a binary of its own:

```rust
fn main() -> std::process::ExitCode {
    moonframe::Moonframe::new("notes")            // ~/.config/notes, its state, its id
        .title("Notes")
        .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/app"))
        .native("index", Index::default)          // a native provider: `app.index`
        .view("editor", Editor::new)              // a GPUI view: `ui.view { name = "editor" }`
        .run()                                    // the window, --check, mod, app, bundle
}
```

Its `app/` folder is laid out like this repository's, and stands on
Moonframe's components, providers and stylesheet (compiled into the
library, so a git dependency needs no checkout beside it). `moonframe init
DIR` starts one, or adds the app folder, types and agent skills to an
existing crate; `cargo run --release -- bundle` builds its bundle.
[examples/host](examples/host) is a small one, and
[docs/library.md](docs/library.md) has the rest.

## Commands

| Command | Does |
| --- | --- |
| `just run` | opens the window on `app/`, with `~/.config/moonframe` over it |
| `just check` | loads everything without a window and reports problems |
| `just types` | type-checks the Luau in strict mode |
| `just providers` | builds and lints the Wasm providers |
| `just bundle` | builds `target/bundle/Moonframe.app` |
| `just bundle-app NAME` | builds the profile NAME's app as a standalone release |
| `just check-examples` | loads and type-checks the apps in `examples/` |
| `just demo` | opens the gallery of every example app, to open and compare them |
| `just install` | builds the app and installs it in `/Applications` |
| `just ci` | formatting, lints, tests, types, providers and check |
| `just audit` | zizmor and actionlint on the workflows, cargo-deny on the dependencies |
| `just release` | publishes `main` as the version in `Cargo.toml` |
| `just run-host` | opens the example app on the library, `examples/host` |

`moonframe --help` lists the app's own options, such as `--user DIR`,
`--no-user`, `--profile NAME`, `--check`, `--release`, and `--background`
(open without taking focus).

To see where time goes, run with `RUST_LOG=moonframe::perf=debug`: each
message, provider publish and frame is logged with how long it took and
which views it ran (`trace` adds each view's Luau and tree time). A message
runs only the views of the mods it reached, and a publish only those that
read that provider's data.

## License

[MIT](LICENSE).
