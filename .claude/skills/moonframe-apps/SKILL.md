---
name: moonframe-apps
description: Create, run, install, share and bundle whole apps on Moonframe, each running as a profile of its own beside the starter app. Covers the app package (moonframe.toml with name, title, icon, keep, requires), `moonframe app new | link | install | list | update | remove`, `--profile`, shared packages an app requires, the shell's menus and About panel, release mode, `just bundle-app` for a standalone macOS .app or Linux prefix, icons, checking and type-checking an app, and measuring its speed with the perf log. Use when the user wants a new app (not a mod in the starter), to run or ship one, to turn a design or screenshot into an app, or when something about profiles, bundles, menus or app performance comes up. For the UI inside an app (mods, components, styles, providers) use the moonframe skill.
---

# Moonframe apps

An app that needs Rust of its own (native views, its own binary, an
existing GPUI app moving over) is not a profile: it depends on the
`moonframe` library crate and runs as its own binary. `moonframe init DIR`
starts one; `docs/library.md` and `examples/host` show how. Everything
below is about profiles.

Moonframe stays a starter. A **whole app** (its own shell, mods, style,
menus, icon) is an *app package* that runs as a **profile**: a folder in
`~/.config/moonframe/profiles/<name>`, opened with `moonframe --profile
<name>`, as an app of its own beside any other. Everything inside an app is
built as the **moonframe** skill describes (mods, components, providers,
`style.luau`); this skill is about the app around them.

## An app package

A git repository (or a folder) laid out like `app/`, with a `moonframe.toml`:

```text
my-app/
  moonframe.toml      what makes it an app
  main.luau           its shell: layout, window, menus, about
  style.luau          its look, merged over the built-in stylesheet
  mods/<name>/        its mods
  components/         its own components, if any
  providers/          its own providers, if any
  icon.png            its icon, 1024 px square (Dock, bundles)
```

```toml
[app]
name = "my-app"                # the profile it installs as: a-z 0-9 - _
title = "My App"               # the menu bar's and bundle's name
icon = "icon.png"              # default icon.png
keep = ["terminal"]            # built-in mods it shows (default none)
requires = ["owner/repo@v1"]   # packages installed with it; "../shared" is one beside it, linked
version = "1.0.0"              # its version: the bundle's, and what updates compare with
updates = "owner/my-app"       # GitHub repo it is released from: Check for Updates (needs version)
```

What a profile loads, lowest first: the built-in app (its providers and
components and stylesheet; **its mods only if `keep` names them**, and never
its shell), the profile's packages, the app package, the user's global
`style.luau` (and nothing else of theirs), then the profile's own folder,
where the user's edits to this app go. Built-in providers (`git`, `files`,
`agents`, `system`, `github`, `gitlab`) and every component in the catalogue
are there to use.

## Commands

```sh
moonframe app new ~/code/my-app           # starts one: moonframe.toml, a shell, a mod, a README, git init
moonframe app link ~/code/my-app          # a profile running the folder in place (edits reload live)
moonframe app install owner/repo[@ref]    # clone an app and what it requires into a new profile
moonframe app list | update NAME | remove NAME [--force]
moonframe --profile my-app                # open it
moonframe --profile my-app --check        # load everything headlessly: run after each edit
moonframe --profile my-app mod install …  # add a package to this app only
```

Options: `--as NAME` names the profile on `install`/`link`; `--yes` allows
providers that run as the user without asking (agents must not pass it
without the user's say-so); `--user DIR` uses another user folder (use a
temporary one to try things without touching the user's).

## Building one

1. `moonframe app new` (or copy `examples/workbench`), then `moonframe app
   link` it and open it with `--profile`. The window reloads as you save.
2. Lay out the shell in `main.luau`. For cards on a soft background: a
   `ui.hsplit { divider = false, ... }` whose panes have padding and whose
   mods draw a `card` class (`background = "card"`, `radius`, `border`,
   `shadow`). A hidden title bar: `window = { titlebar = "hidden",
   traffic_lights = { x, y } }`, where `y` centres the lights on the
   header row (row centre minus about 6) and the row has `drag = true` and
   room (about 70 px) for them.
3. Fill it with mods; data comes from providers. If the app needs data the
   built-ins lack, write a provider (a Rust Wasm provider for sample data
   or anything the sandbox allows; a process provider in Python or bash for
   files, commands and databases), or put shared ones in a package the app `requires` (see
   `examples/projects`).
4. Before writing UI, read the **moonframe-components** catalogue; when a
   piece the design needs is missing and reusable, add it as a component
   (in the app, or in `app/components` when it belongs to everyone), with
   `when`, `props`, `example`, and run `just components`.
5. `moonframe --profile NAME --check`, and `just types DIR` to type-check
   the app's Luau (copy `app/.luaurc` beside it for strict mode).
6. Compare with the design: screenshot your own window (start it with
   `--background`, note its PID, `just screenshot OUT PID`) and look at the
   result beside the reference: spacing, type sizes, corners, alignment.

## From a design: what to use

Turning a screenshot or a mockup into an app, read it in this order, and
reach for these before writing anything new. The **moonframe-components**
skill has each one's props; the apps in `examples/` use all of them.

**The frame**
| The design has | Use |
| --- | --- |
| sidebar + one big white sheet | `ui.hsplit { divider = false }`, the sheet a `ui.column` with a `sheet` class (`background`, `radius`, `border`, `shadow`) inside a padded pane (ledger, audience, pipeline) |
| several floating cards | each card a column with a `card` class in its own pane, panes padded, `divider = false` (workbench, daybook) |
| a bar across the whole top | `ui.row { height = 60, shrink = false, ui.region { name = "top", direction = "row" } }` in the shell: a region stretches otherwise (daybook) |
| traffic lights in the sidebar or a header | `window = { titlebar = "hidden", traffic_lights = { x, y } }`, `y` ≈ the row's centre minus 6; leave ~76 px for them |
| a big layout | `window = { size = { width, height } }` |
| looks/themes the user picks | the look in a provider (kept), classes per look (`card-sepia`), colours `{ light, dark }` in style.luau (daybook) |

**Navigation and headers**
| The design has | Use |
| --- | --- |
| sidebar rows with icons, counts, sections | own rows (`ui.row` + `selected` + a `:selected` rule), or `sidebar` / `sidebar-section`; counts in a small bordered text |
| workspace/org switcher at the top | a row: logo tile, name over a muted line, `chevrons-up-down` |
| Workspace › Page | `breadcrumbs` |
| Board / List, Light / Dark | `segmented` (or `tabs` for documents) |
| back / forward | `nav-buttons` |
| a search box with ⌘K | `ui.input` in a row, `kbd { keys = "cmd-k", plain = true }`; style the field through a class (`.bare-input`), since the app's `input` rule outranks a mod's |
| filter pills with a chevron | `dropdown { icon, options, toggle, open }` |
| a dark or blue primary button | `ui.button { class = "…" }` with `background`/`color`, or a clickable `ui.row` |

**Figures and charts**
| The design has | Use |
| --- | --- |
| big number + label + note | `metric` (`size = "hero"` for the main one) |
| KPI cell with icon tile, ⋯, delta pill, caption | `kpi-cell`; a tile with a trend: `stat-tile`; in a status bar: `stat` |
| delta / tag / count in a tint | `pill` (`tone` or any `color`) |
| bars over weeks against a floor, hatched below it | `forecast-chart` |
| stacked bars by series | `bar-chart`; a few labelled bars, future hatched: `mini-bars` |
| columns of rounded squares | `dot-chart` |
| a line, a curve in a table cell, a dashed rule | `ui.line` (`baseline`, `area`, `dots`, `dashed`) |
| a tiny trend | `sparkline` (bars) or `ui.line` |
| one bar split into shares | `share-bar`; a row with label, count, %, bar under: `ratio-row` |
| progress to a target in stages, with a marker | `progress-stack`; one fraction: `meter`; a quota with a reset: `usage-bar` |
| hatched, held, pending, missing | `ui.hatch` |

**Lists, records, people, dates**
| The design has | Use |
| --- | --- |
| a table with mixed cells (icons, curves) | `table` (a cell may be an element) |
| tasks with round checks, struck when done | `task-row`, `task-check`; `ui.text { strike = true }` |
| kanban columns of cards | `kanban-column` + `board-card` (moves through a `context` menu: no drag and drop) |
| a calendar leaf ("NOV / 9") | `date-tile`; a strip of days: `day-strip`; a month: `month-grid` |
| avatars overlapping + a count | `avatar-stack`; one person: `avatar` |
| a country code chip | `country-code` |
| a dashboard section (uppercase head, ⋯, white body) | `dashboard-card`; a titled box: `card`; a heading: `section` |
| an agent/chat panel | `chat-message`, `thinking`, `record-card`, `numbered-list`, `composer` |
| files and changes | `file-tree`, `change-tree`, `code` (examples/projects) |

**When nothing fits**, write a component in `app/components` (or the app's
own `components/` when only it needs it): `title`, `category`, `icon`,
`description`, `when` (what to use instead), `props`, theme-colour `styles`,
an `example`, then `just components`. Only lines, curves, patterns, pictures
and live things (terminals, pages, text fields) need the host.

### Traps

- Inline `gap` and `padding` take only `none` `small` `normal` `large`; put
  pixel values in `styles`.
- No negative margins: overlap with `float = { top, left }` in a sized row.
- Views cannot measure: give charts and bars pixel widths (`width`), or let
  flex share the space (`grow`) where the content allows.
- A list whose entries differ in shape fails the type check: give every
  entry the same keys, or type it (`local items: { any } = …`); menus too.
- `ui.text` does not take `case`; use `string.upper`.
- A region or a scroll column squeezes its children: `shrink = false` on
  the column inside a `scroll` column, a fixed-height row around a top
  region.
- Hidden windows (behind others, or a locked screen) build no frames, so a
  `--background` screenshot may show a state from before the data came.
  Start the window when it can be seen, or check headlessly.
- Provider output that changes every look (a clock) redraws every view
  that reads it: round it.

## Menus and About

The shell (`main.luau` only) adds menus and fills the About panel; the host
puts its app menu first (About <title>, Quit <title>), then these, View (a
shell menu titled "View" merges into it), Components outside a release, and
Window and Help last.

```lua
-- A typed local: items with `msg` and with `command` mix in one list.
local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Shell", msg = "new", to = "agent", shortcut = "secondary-t" },
		"-",
		{ label = "Close File", msg = "close-current", to = "editor", shortcut = "secondary-w" },
	} },
	{ title = "Window", items = { { label = "Minimize", command = "minimize", shortcut = "secondary-m" } } },
}

return mod {
	about = { version = "1.0", description = "…", website = "https://…", copyright = "…" },
	menus = MENUS,
	-- window, init, update, view …
}
```

An item sends `msg` to `to` (the shell without it) or runs a `command`;
`shortcut` is a key as `keys` name them (`secondary` is ⌘ on macOS, Ctrl
elsewhere). The receiving mod handles the message in `update` like a click.
About is macOS's own panel (icon, name, version, credits); a toast elsewhere.

**Check for Updates…** sits under About when the app names the GitHub
repository it is released from (`updates` and `version` in `moonframe.toml`;
`.updates("owner/repo")` for a library app). It offers the newest published
release (tag `v1.2.3`); on macOS a bundle installs its `.dmg` in place and
starts again, elsewhere the download opens. A release build checks once a
day by itself. A shell can place it too: `command = "check-for-updates"`.
Releases need assets named for their platform
(`App-1.2.3-universal-apple-darwin.dmg`) and optionally `SHA256SUMS`; see
`docs/updates.md`.

## Release mode and bundles

- **Developer Mode** (app menu, beside About) switches the Components menu,
  Inspect Element and Reload Modules, and their keys, on and off live; the
  choice is kept per app and profile. Until the user picks, it is off in a
  bundle and in a library app's release build. `--release` and `--dev` set
  it for one run.
- `just bundle-app NAME` builds the profile's app standalone:
  `target/bundle/<Title>.app` on macOS (id `dev.moonframe.app.NAME`, its
  icon, its packages staged inside, Wasm providers prebuilt), or on Linux a
  prefix with `bin/moonframe-NAME`, `share/moonframe-NAME/` and a `.desktop`
  file and icon. `MOONFRAME_USER=DIR` finds the profile in another user
  folder. What its users change goes in `~/.config/moonframe/profiles/NAME`.
- **Build App Bundle** in the app menu (outside a release, when the app
  runs from its checkout) does the same for the window's own app, the
  profile's or the starter's, in the background, then shows the bundle in
  Finder. The build's output goes to the log.
- The app's name in the menu bar and Dock is its `title`, also when run
  from a terminal.

## Speed

Views run when their inputs change: a message runs only the mods it reaches,
and a provider's publish only the views that read `app.<that provider>`.
So keep each mod's reads narrow (read `app.projects` where it is needed, not
in every mod), keep provider output stable (no clock that ticks every
second; round it), and draw long lists a page at a time.

`RUST_LOG=moonframe::perf=debug` logs each message, publish and frame with
its time and the views it ran; `trace` adds each view's Luau and tree time.
Anything over 8 ms
(a frame, message, publish or provider read) is logged at `info` always,
and over 16 ms at `warn`; fast ones stay at `debug`. Measure in a release build
(`target/release/moonframe`): debug is several times slower.

## Examples

`examples/` has seven apps and a package two of them share; read them first:

- **sessions**: repositories as sessions with a shell each and their changes.
- **workbench**: terminal tabs, an editor with highlighting, files and
  changes.
- **daybook**: a planner: a top bar with one search-or-create field, a
  sidebar, a day card (day strip, a summary of pills, tasks and events) and
  an inbox card; Upcoming, a month, notes; three looks the provider keeps.
  Its own Python provider stores the data in `~/.local/share/moonframe`.
- **ledger**: a fund overview: a forecast against a floor, things waiting
  with date tiles, a raise in stacked progress, funds with curves in a table.
- **audience**: a newsletter dashboard: KPI cells, a dot chart, sources as
  ratio rows, share bars.
- **pipeline**: a sales kanban with KPIs and a (scripted, labelled) agent
  chat panel; deals move through a card's menu.
- **stage**: the components on a bento board with sample data, for stills
  (View > Show or Hide Captions, backdrops).
- **projects**: a Python provider for repositories, their files and changes,
  and the `file-tree`, `change-tree`, `code` and `file-icon` components.

`just check-examples` loads them all and type-checks them; `swift
examples/draw-icons.swift` draws their icons, a pattern for new ones.
