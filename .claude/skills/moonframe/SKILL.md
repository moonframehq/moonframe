---
name: moonframe
description: Build anything in Moonframe, a starter app whose window is laid out entirely by Luau. Covers panels, side panels, splits, toolbars, status bars, popovers, mods and extensions, reusable components (`use`), live terminals (`ui.terminal`), data providers (Wasm or process), icons, and styling through style.luau (colors, corners, shadows, spacing, animations). Also covers acting on the user's Inspect Element selection ("add X here"). Use whenever the task is to add, move, restyle or experiment with any UI in moonframe, to build a widget, component, provider or extension, or to customise the installed app in ~/.config/moonframe. The window reloads live as files change, so no Rust build is needed for Luau work.
---

# Moonframe

Luau describes every part of the window. Rust only checks the trees the
scripts return and draws them with GPUI. To change the UI, edit Luau under
`app/`, never Rust, unless the task needs a new element type (see "Extending
the host" below).

```text
app/
  main.luau              the shell: lays out the whole window
  style.luau             the stylesheet: colors, corners, fonts, rules (optional)
  mods/<name>/main.luau  a mod; the shell places it by region or by name
  components/<name>.luau a ready-made piece of view mods `use` (see below)
  providers/<name>/      a provider: data Luau can't produce (see below)
```

**Two folders, laid over each other.** The window loads the built-in app
(`app/` in this repository; `Contents/Resources/app` in an installed
bundle), then the user's folder, `~/.config/moonframe` (or
`$XDG_CONFIG_HOME/moonframe`), on top. Both have the layout above.
- A mod or provider in the user's folder replaces the built-in one of the
  same name, whole. A new name adds one.
- A user `main.luau` replaces the shell.
- A user `style.luau` is merged over the built-in one: each color and speed
  replaces the one of its name, each rule's properties replace those of the
  same rule (`panel = { radius = 4 }` keeps the app's panel background), and
  any other key replaces the app's.
- Where to write: when working on this repository, edit `app/`. When the
  user wants to customise the app they use (or it is installed), write to
  `~/.config/moonframe` and never into the bundle, which is signed and
  replaced by each update. To change one built-in mod, copy its folder there
  and edit the copy.
- The window watches both folders, so either kind of edit reloads live.
- **Packages** go between the two: mods others publish as git repositories,
  installed with `moonframe mod install` (see "Sharing mods"). A package
  never replaces the shell, and the user's folder goes over packages.

Layout and look are separate. Modules decide *what* goes *where*;
`style.luau` decides how it looks. Prefer a stylesheet rule or class over
repeating `style = { ... }` on many elements.

## Building something: where it goes

Pick the lowest layer that can do the job:

| The task needs | Build | Where | Rust build? |
| --- | --- | --- | --- |
| something on screen, from data Luau already has | a **mod** (or edit one) | `mods/<name>/main.luau` | no |
| the same piece of UI in several places | a **component** | `components/<name>.luau` | no |
| a shell or command line in the window | `ui.terminal` or `use "terminal"` | in a mod | no |
| data from the machine, files, network or a CLI | a **provider** feeding `app.<name>` | `providers/<name>/` | no (the host builds Wasm providers itself) |
| a look change | a rule in `style.luau`, or the mod's `styles` | | no |
| a new kind of element (input field, tree, chart) | a host element | `crates/` | yes, and the app restarts |

Prefer what already exists. Check `app/components/` before writing a widget,
and the element table before asking for a host change. Live-reloaded Luau
keeps the user's window running; Rust changes don't.

### Recipes

**"Add X here."** Read `~/.cache/moonframe/selection.md`
(see "Here: the user's selection"). Open the file at the line it gives,
use a component if one fits, `just check`, and tell the user what you added
and where.

**A new panel in a side area.** Create `mods/<name>/main.luau` with
`region = "left"` (or `right`, `main`, `bottom`, `status`, `status-right`)
and a `view` returning `ui.panel { title = ..., grow = true, ... }`. It
appears without touching the shell.

**A status bar widget.** A mod with `region = "status-right"` (or
`status`) and `order` to place it. The view is a small `ui.row { align =
"center", gap = "small", ... }`, for example from `use "stat"`. Give
changing numbers a fixed width.

**A widget with live data.** Read `app.<provider>`, and draw a placeholder
while it is nil. If no provider has the data, write one (see "Providers").
Never invent the data in Luau unless it is labelled demo data.

**Something clickable that opens a sheet.** `ui.popover` around the
widget, with `open` in the mod's state (see "Sheets and menus").

**A reusable piece.** A component with a good `example`, so it previews
in the Components window, then `use` it from the mod.

**A terminal.** `local terminal = use "terminal"` and
`terminal { id = "main", cwd = "~/code" }` inside a container with
`grow = true` (see "Terminals").

**Mods that talk.** Send `msg = ..., to = "other"` from a control, or
return sends from `update`. Messages to `provider:<name>` go to a provider.

**Customising the installed app.** Write to `~/.config/moonframe`
(same layout as `app/`). Copy a built-in mod's folder there to change it.
Personal mods belong there, never in this repository. `just install` puts
the current build in `/Applications` so you can try them against it.

Then always: `just check`, `just types`, and if the user's window is open,
it has already reloaded.

## "Here": the user's selection

The user picks elements in the window with **View > Inspect Element**
(⌥⌘I), like a browser's developer tools. The pick is saved to
`~/.cache/moonframe/selection.md` (`$XDG_CACHE_HOME` if set), and the
user can copy the same text with **Copy for agent**:

```text
Selected in Moonframe (View > Inspect Element):
- element: text.status-text "⎇ main +29"
- mod: status, drawn by /…/app/mods/status/main.luau:33
- inside: mod shell > column > … > region "status-right" > mod status > row > text.status-text "⎇ main +29"
```

When the user says "here", "this", "the selected element" or "add X there"
without saying where, read that file first. The `file:line` is the `ui` call
that built the element. For an element a component built, it is the line in
the mod that used the component. Change or add next to that line. "Inside"
lists every element around it, outermost first, so "add a list here" on a
row means adding a child to that row; on a text, add a sibling. If the file
is missing, nothing is selected: ask, or pick a sensible place and say which.

"Copy for agent" copies that text and closes the card, but the file stays,
so the selection is still there for you. "Clear" (or Esc) deletes it. The
Components window's "Copy for agent" copies a request to add that component
at the current selection, with its usage. Treat either as the user's
instruction.

## Components: ready-made pieces

`components/<name>.luau` (in `app/` or `~/.config/moonframe`, which
wins by name) holds a component: a function from props to elements, plus
what the Components menu shows. The user browses them in their own window
with **Components > Browse Components…** (⇧⌘K), or picks one by category
from that menu. Each gets its icon, a live preview of `example`, and its
usage; **Copy for agent** asks you to add it at the selection. Adding or
editing a component file updates the menu and the window live.

Use one in a mod with `use`, and call it wherever an element goes:

```lua
local stat = use "stat"
local list = use "list"

return mod {
	view = function(state, app)
		return ui.column {
			stat { icon = "cpu", label = "CPU", value = "12%", width = 32 },
			list { rows = state.rows, selected = state.selected },
		}
	end,
}
```

Before writing a widget, check `app/components/` for one that fits.
`just check` lists them all:

| Component | For | Main props |
| --- | --- | --- |
| `card` | a titled, bordered box | `title`, `subtitle`, `actions`, children |
| `section` | a small heading over content | `title`, children |
| `toolbar` | a row of small buttons | `buttons = { { label, msg or command, variant } }` |
| `list` | selectable rows; a click sends `{ select = id }` | `rows = { { id, label, detail, badge } }`, `selected`, `to` |
| `key-values` | aligned label/value pairs | `rows = { { "Label", "value" } }`, `width` |
| `meter` | a bar filled to a fraction | `value` (0 to 1), `width`, `warn`, `label` |
| `sparkline` | recent samples as tiny bars | `values` (0 to 1), `samples`, `height`, `hot` |
| `stat` | icon, label, fixed-width value | `icon`, `label`, `value`, `width`, `tone` |
| `tags` | wrapping badges | `tags`, `tone` |
| `empty-state` | nothing to show yet | `icon`, `title`, `message`, `action`, `msg` |
| `terminal` | a live shell (alacritty) | `id`, `cwd`, `style` |
| `ghostty-terminal` | a live shell (libghostty) | `id`, `cwd`, `style` |

A new component looks like this:

```lua
return component {
	title = "Stat",                 -- the menu and gallery name
	category = "Status bar",        -- the Components submenu
	icon = "cpu",                   -- beside its title in the browser
	description = "One line on what it is for.",
	example = { value = "12%" },    -- props for the preview
	-- usage = [[...]],             -- Luau to show, when `example` holds elements
	build = function(props)
		return ui.row { ui.text { props.value } }
	end,
}
```

- `build` runs in the sandbox of the mod that uses it and returns elements
  like a `view`. Clicks it builds go to that mod's `update`.
- Without `usage`, the gallery shows `local x = use "name"` and a call
  with `example`. Give `usage` when the example has elements or children.
- Write components to work anywhere: theme colors in `style`, not the
  app's classes, and nothing a mod must set up.
- The preview is drawn from `example`, and a broken component shows its
  error in the gallery and in `just check`. Previews never start anything:
  a terminal shows a placeholder.
- A component can `use` another; one that uses itself is an error.
- Name it after what it is (`branch-list`, not `widget2`); the file name
  is the name `use` takes.
- Types: the definition file's type names (`IconName`, `Tone`, `Style`)
  are not visible to scripts, so annotate props with `string` or `any`,
  and cast where a `ui` field wants a union (`name = props.icon :: any`).

## Terminals

`ui.terminal` (or the `terminal` and `ghostty-terminal` components) shows a
live shell. The host runs it, so it can't be written in Luau:

- **What runs:** always the user's login shell. A mod chooses `cwd`, never
  a command, so the sandbox still can't run programs on its own.
- **Lifetime:** the host keeps one shell per mod and `id`. It survives
  reloads and hidden panes, and ends with the app or when the shell exits
  (Enter starts another). Two `id`s are two shells.
- **Engines:** `engine = "alacritty"` (default, pure Rust) or `"ghostty"`
  (libghostty-vt). Both are cargo features of `moonframe`, on by default.
  A build without one shows why in the element's place.
- **Sizing:** it grows to fill its container (give the container `grow`),
  and the shell is resized to match. Style it with a `terminal` rule or
  `style`: `background` and `color` set its default colors.
- **Input:** click to focus. While it has focus every key goes to the
  shell before the app's shortcuts: Tab (completion), Esc, Ctrl keys,
  arrows. ⌘V pastes (control characters are dropped from pastes, so they
  can't escape bracketed paste); other ⌘ shortcuts stay the app's. The
  scroll wheel moves through the scrollback.
- **Limits:** `id` is 1 to 64 of `a-z A-Z 0-9 - _`. A mod can have 8
  terminals and the app 32; past that the element shows why. Don't make
  ids from data (one per list row): each is a process.
- **In the component browser** a terminal shows a placeholder and starts
  no shell.

Building the `ghostty` engine needs Zig 0.15 (in `mise.toml`). On macOS 27
and later, Zig 0.15 can't link natively, so `tools/zig/zig` (first on
`PATH` through mise and the justfile) works around it. That needs a macOS
SDK of version 15 or older installed. Build without it with
`--no-default-features --features alacritty`.

## Loop

1. Edit or create `.luau` files under `app/`.
2. Run `just check`. It loads every module headlessly, the user's folder
   included (`just check-builtin` skips it), and prints
   `ok: ...` or one `module: problem` line per module. Fix the problems until
   it prints `ok`. The messages give the exact place, e.g.
   `explorer: view: panel[2].item: unknown key `lable`; it takes ...`.
3. If the window is running, it has already reloaded. Module state is kept
   across reloads. To see the result, run `just screenshot`, then Read
   `target/screenshot.png`.
4. The user often has their own window open (`just run` or `cargo run`).
   Never stop it: no `pkill`/`killall` by name, which also kills theirs.
   If you need a window of your own, start the built binary in the
   background, note its PID (`target/release/moonframe app & echo $!`),
   capture it with `just screenshot target/screenshot.png <PID>`, and stop
   only that PID with `kill <PID>`. Their window reloads from the same files
   anyway, so often you don't need your own.
5. Never type into, click or bring forward any window to test (no
   `osascript … keystroke`, no focus changes): input goes to whatever app
   the user has in front, and they are usually working. Test behaviour
   with `just check`, unit tests and screenshots by PID; ask the user to try
   interactions themselves.

`just types` type-checks every `.luau` file in `app/` against
`types/moonframe.d.luau` in strict mode (set in `app/.luaurc`). It runs in
`just ci`, and downloads the pinned luau-lsp into `target/tools` the first
time. Run it after `just check`.
- It catches unknown `ui` functions, wrong token values (`tone = "loud"`),
  and wrong value types before anything runs.
- It does not catch misspelled keys (`lable`), because Luau allows extra
  fields in a table literal. `just check` catches those.
- Annotations are optional. Add them where they help, e.g.
  `type State = { open: boolean }` and `init = function(previous: State?): State`.
- When you add an element or token to the host, add it to
  `types/moonframe.d.luau` too. A Rust test fails if they drift apart.

Mistakes never move the layout:

- **A bad element** (unknown key, value or icon) is drawn as a small red
  warning mark in its place, with the message on hover. The rest of the view
  draws as before. Its error is still a problem in `just check` and the log,
  so fix it.
- **A broken module** (syntax error, failing `view`) keeps running its last
  good version, with its error floating over the corner of that view instead
  of pushing it down. Stylesheet and provider errors float over the top of
  the window the same way.
- **A broken copy in `~/.config/moonframe`** of a built-in mod or shell
  that has never loaded falls back to the app's own version, with the error
  saying so, rather than leaving a hole.
- Only the view's root, or going past the size and depth limits, fails a
  view as a whole.

An unknown icon's error names the closest bundled icons. Check
`IconName` in `types/moonframe.d.luau` before picking one.

## A module

Each file returns `mod { ... }`:

```lua
return mod {
	region = "left",   -- optional: the shell region to draw this mod in
	styles = {         -- optional: style rules for what this mod draws (see Stylesheet)
		[".row"] = { padding = { 4, 8 } },
	},
	order = 0,         -- optional: position in the region, lower first, then by name
	title = "Files",   -- optional: informational

	-- Optional. `previous` is the state from before a reload (nil on first load).
	init = function(previous)
		return previous or { selected = nil }
	end,

	-- Optional. Return the new state, or nothing to keep the table you changed.
	-- A second result sends messages on: { { to = "mod", msg = ... }, ... }
	update = function(state, msg)
		if type(msg) == "table" and msg.open then
			state.selected = msg.open
			return state, { { to = "inspector", msg = { select = msg.open } } }
		end
	end,

	-- Required. Returns one element. `app` is
	-- { dark = bool, mods = { names }, regions = { name = { mod names } },
	--   <provider> = what each provider last published, e.g.
	--   system = { cpu, cpu_history, memory, memory_used, memory_total,
	--              received, transmitted }, git = { repository, branch, changed, ahead, behind } }
	-- Provider data is nil until its first publish.
	view = function(state, app)
		return ui.panel { title = "Files", grow = true, ui.text "hello" }
	end,
}
```

Rules that trip agents up:

- Unknown element types, keys and token values are errors, not ignored.
  `just check` names them.
- The keys allowed on the `mod { }` table are only `init`, `update`, `view`,
  `region`, `order`, `title` and `styles`.
- State and messages must be plain data: nil, booleans, numbers, strings,
  lists, and string-keyed tables. Functions in state stop it surviving a
  reload. A table is a list without gaps or has string keys, never both.
- There is no `require`, `print`, `os`, `io` or `debug`, and each mod is one
  file; share code through components and `use`. Use `log(...)` to print to
  the terminal that ran `just run`.
- Children are the array part of the table. `nil` and `false` children are
  skipped, so `cond and ui.text "x"` works. For a computed list, build a
  table and pass `table.unpack(items)` as the last entry.
- Each call has an instruction budget, so an endless loop is stopped with an
  error instead of freezing the window.
- The mod name is the folder name: `a-z`, `0-9`, `-` and `_`. `shell` is
  reserved for `main.luau`.

## Elements

Text-like elements take their main value positionally:
`ui.text "hi"`, `ui.text { "hi", tone = "muted" }`, `ui.button { "Save", msg = "save" }`.

| Element | Keys (besides children) | Notes |
| --- | --- | --- |
| `ui.row { ... }`, `ui.column { ... }` | frame keys, `align`, `justify`, `wrap`, and `msg`/`to` or `command` | flex containers; with `msg` or `command` the whole stack is clickable (style it with `.class:hover`). Defaults: `gap = "normal"`, `padding = "none"` |
| `ui.panel { ... }` | frame keys, `title`, `actions = { elements }` | header (title plus right-aligned actions) over a column body. Defaults: `gap` and `padding` `"normal"` |
| `ui.hsplit { ... }`, `ui.vsplit { ... }` | `id` | user-resizable; children must be `ui.pane`. Always grows to fill |
| `ui.pane { ... }` | `size`, `min`, `max`, `fixed`, `visible`, `gap`, `padding`, `surface`, `scroll` | only inside a split; content is a column. `fixed = true` keeps its size on window resize and needs `size` |
| `ui.text "..."` | `text`, `tone`, `size`, `weight`, `truncate` | a bare string child is plain text |
| `ui.button { "Label", ... }` | `label`, `variant`, `small`, `disabled`, and `msg`/`to` or `command` | |
| `ui.item { "Label", ... }` | `label`, `detail`, `badge`, `selected`, and `msg`/`to` or `command` | clickable list row for sidebars |
| `ui.badge "..."` | `text`, `tone` | pill |
| `ui.terminal { id = "main", cwd = "~/code", engine = "alacritty" }` | `id`, `cwd`, `engine` (`alacritty` or `ghostty`), `class`, `style` | a live shell (the user's login shell) that fills the space it gets; see "Terminals" |
| `ui.icon "cpu"` | `name`, `tone`, `size` | a bundled Lucide icon at the text's size and color; an unknown name is an error that lists them all (also in `types/moonframe.d.luau` as `IconName`) |
| `ui.divider()` | | a line across the parent's axis |
| `ui.spacer()` | | takes the free space, e.g. to push items right |
| `ui.region "name"` or `ui.region { name = "x", direction = "row", gap = "large" }` | `name`, `direction`, `gap`, `justify` | draws every mod with `region = name`, in order; `justify = "end"` pushes them to the far edge |
| `ui.mount "name"` | `mod` | draws one mod's view by name |
| `ui.popover { ..., content = el }` | `open`, `dismiss`, `side` (`top`/`bottom`), `align` (`start`/`end`), `gap`, `width`, `content` | floats `content` over everything beside its anchor (its children). While `open`, a click anywhere outside sends `dismiss` to this mod. Styled and animated by the `popover` rule |

**Frame keys** (`row`, `column`, `panel`):

- `grow = true` takes the free space along the parent's axis. Use it on the
  root of every view that should fill its pane.
- `width` and `height` are pixels.
- `gap` and `padding` take a space token.
- `surface` sets the background and `border` the edges.
- `scroll = true` scrolls along the container's axis. Give the container
  `grow = true` or a fixed size so it has something to overflow.

**Tokens:**

- space: `none` `small` (4px) `normal` (8px) `large` (16px)
- tone: `normal` `muted` `accent` `success` `warning` `danger`
- size (text): `small` `normal` `large` `title`
- weight: `normal` `medium` `bold`
- align: `stretch` `start` `center` `end`
- justify: `start` `center` `end` `between`
- surface: `background` `sidebar` `muted` `secondary`
- border: `true`, `all`, `top`, `bottom`, `left`, `right`, `none`
- direction: `row` `column`
- variant (button): `normal` `primary` `ghost` `outline` `danger`
- command: `quit` `reload` `toggle-theme` `show-themes`

**Actions:** `msg = <data>` calls `update(state, msg)` on the mod that drew the
control. Add `to = "name"` to send it to another mod, or to `"shell"`.
`command = "..."` runs an app command instead. Use one or the other, not both.

## Stylesheet

`app/style.luau` returns `style { ... }`. A broken stylesheet keeps the
previous one and shows its error at the top of the window, and in
`just check` under `style`.

```lua
return style {
	font_size = 14,      -- base text size; text sizes scale from it
	font_family = "Inter",
	radius = 8,          -- the theme's corner radius, which buttons use
	speeds = { drawer = "220ms" }, -- named durations (fast/normal/slow exist already)
	animation_speed = 1, -- every animation: 2 twice as fast, 0.5 half, 0 off

	colors = {
		-- A theme color's name replaces it app-wide (buttons and other controls too)
		-- in the Default theme; a theme picked in View > Themes brings its own:
		accent = { light = "#4f46e5", dark = "#818cf8" },
		card = { light = "#ffffff", dark = "#1e1e22" },
		-- Any other name is a new color for use in rules and styles:
		brand = "#ff3366",   -- one value for both modes
	},

	rules = {
		panel = { background = "card", radius = 12, margin = "small", border = true },
		["panel-header"] = { border = "none", color = "accent" },
		["item:selected"] = { background = "accent/15", color = "accent" },
		["item:hover"] = { background = "accent/8" },
		[".hero"] = { size = "title", weight = "bold", color = "accent" },
	},
}
```

**Selectors.**
- An element type: `row` `column` `panel` `panel-header` `pane` `text`
  `button` `item` `badge` `icon` `terminal` `divider` `popover`.
  `panel-header` is a panel's title bar.
- A class: `.name`.
- Either one can take a state: `:hover` or `:selected` (`selected` applies
  to `ui.item`). Hover rules only change `background`, `color` and
  `border_color`.

**Classes and inline styles.** These elements take `class = "a b"` (or
`{ "a", "b" }`) and `style = { ...same properties... }`: `row`, `column`,
`panel`, `pane`, `text`, `button`, `item`, `badge`, `icon`, `terminal`,
`popover`.

**Numbers that change.** Text is proportional, so a reading such as a rate
or a percentage changes width as it ticks and pushes its neighbours. Put it
in a fixed-width box: `ui.row { width = 64, justify = "end", ui.text { rate } }`
(see `app/mods/system`).

**Mod styles.** A mod can bring its own rules under `styles = { selector = { ... } }`.
They use the same selectors, properties and colors (including palette names
from `style.luau`), but **apply only to elements that mod draws**, so
an extension can't restyle the rest of the app. An extension should style
itself this way, ideally with classes named after it, rather than asking
for edits to `style.luau`.

**Precedence**, lowest first:
1. the element's built-in default
2. type rules: the mod's, then `style.luau`'s
3. class rules, in the order the element lists them, each the mod's then
   `style.luau`'s
4. its `style = { }`
5. its own keys (`padding = ...`, `surface = ...`, `tone = ...`, `border = ...`, `size = ...`)

So `style.luau` always outranks a mod at the same step (the app can restyle
any extension), and a mod's class rule outranks `style.luau`'s type rule.
Leave an element's own keys off if the stylesheet should decide.

**Properties.**

| Property | Value |
| --- | --- |
| `background`, `color` (text, inherited), `border_color` | a color |
| `border` | `true`, `all`, `top`, `bottom`, `left`, `right`, `none` |
| `radius` | pixels |
| `padding`, `margin` | a space token or pixels for every edge, `{ y, x }`, `{ top, right, bottom, left }`, or named edges `{ x = 12, top = 4 }` (edge names win over `x`/`y`) |
| `gap` | a space token or pixels |
| `shadow` | a preset (`small` `medium` `large`, or `none` to remove one set by a lower rule), a CSS box-shadow string, a layer table, or a list of layers (see below) |
| `size`, `weight` | text tokens (inherited) |
| `opacity` | 0 to 1 |
| `transition` | how a `pane` animates when its `visible` changes (see below) |

**Shadows.** Write them like CSS `box-shadow`. Lengths are pixels, with or
without `px`, and layers are separated by commas. The color can be any
color below; the default is black at 25%. Hex and names only, not `rgba()`.

```lua
panel = { shadow = "0 4px 12px -2px #00000033" },              -- x y blur spread color
[".lifted"] = { shadow = "0 1 2 #0002, 0 8px 24px accent/20" }, -- two layers
[".pressed"] = { shadow = "inset 0 2 4 #0003" },                -- inside the edge
pane = { shadow = { y = 2, blur = 6, color = "accent/30" } },   -- one layer as a table
row = { shadow = { "0 1 2 #0002", { y = 8, blur = 24 } } },     -- a list of layers
[".flat"] = { shadow = "none" },                                -- remove it
```

A table layer takes `x`, `y`, `blur`, `spread`, `color` and `inset`; missing
lengths are 0. A string layer takes 2 to 4 lengths (`x y [blur [spread]]`),
then the color, with an optional `inset` first.

**Transitions.** When a pane's `visible` flips (for example from a "Hide
left" button), it animates by its `transition` instead of jumping:

```lua
pane = { transition = "slide 220ms ease-out" },                   -- every pane
[".drawer"] = { transition = "slide-fade 0.3s cubic-bezier(0.2, 0, 0, 1)" },
[".popover"] = { transition = { kind = "fade", duration = 150, curve = "linear" } },
[".instant"] = { transition = "none" },
```

- **Kinds:** `slide` (the size grows from or shrinks to nothing), `fade`
  (opacity, then it takes or gives back its space), `slide-fade` (both),
  `none`.
- **Curves:** `linear` `ease` `ease-in` `ease-out` `ease-in-out`
  `cubic-bezier(x1, y1, x2, y2)`, as in CSS.
- **Duration:** `220ms`, `0.22s`, or a **speed name** in the string;
  milliseconds or any of those strings in the table; up to 10s.
- **Speeds:** `fast` (120ms), `normal` (220ms) and `slow` (400ms) always
  exist. `speeds = { name = "90ms" | 90 }` at the top of `style.luau`
  redefines them or adds names; a name can't be a kind or a curve. Prefer
  speed names over raw durations so one edit retimes the whole app.
- **`animation_speed`:** at the top of `style.luau`, it divides every
  duration (mod `styles` and inline `style` included): `2` is twice as fast,
  `0` turns animations off.
- **Defaults:** slide, the `normal` speed, `ease`.

Words can come in any order. Toggling again mid-animation turns back from
where the pane is. A slide moves across the pane's `size` (or `max`), not a
width the user dragged it to. Put the transition on the pane (type rule,
class, or `style`), not on what is inside it.

**Colors.**
- `#rgb`, `#rrggbb` or `#rrggbbaa`.
- A name from `colors`.
- A theme color: `background` `foreground` `muted` `muted-foreground`
  `sidebar` `card` `secondary` `border` `accent` `accent-foreground` `success`
  `warning` `danger` `list-hover` `list-active`. These follow light and dark
  mode, and the theme picked in View > Themes.
- Any of the above with `/NN` for percent opacity, e.g. `accent/15`.

Padding and margin layer **per edge**: a class with `padding = { top = 20 }`
changes only the top and keeps the other edges from the type rule. An
element's own `padding = "small"` key sets every edge.

**Where each element takes the properties.**
- On a `panel`, `padding` and `gap` apply to the body under the header.
- `button` takes `background`, `color` and `radius`. Setting a background or
  color overrides its `variant`.
- `divider` takes only `background`.

## Patterns

**Shell with toggleable side panels.** Keep the visibility in the shell's
state, flip it from buttons, and pass it to `visible`:

```lua
ui.hsplit {
	ui.pane { size = 240, min = 160, max = 480, fixed = true, visible = state.left, surface = "sidebar", ui.region "left" },
	ui.pane { ui.region "main" },
	ui.pane { size = 280, fixed = true, visible = state.right, surface = "sidebar", ui.region "right" },
}
```

**Bottom panel.** Put a `ui.vsplit` inside the center pane, with a second
fixed pane (`size = 180`, `visible = state.bottom`) holding `ui.region "bottom"`.

**A mod that fills its pane.** The root should be `ui.panel { grow = true, ... }`.
Use `scroll = true` for long lists. Several mods in one region stack. Give
`grow = true` only to those that should share the leftover height.

**Selectable list.** Use `ui.item { name, selected = name == state.selected, msg = { open = name } }`,
and in `update` set `state.selected` and send the choice on to whatever
shows it.

**Tabs.** Draw a row of small buttons in a panel's `actions`, with
`variant = state.tab == id and "primary" or "ghost"` and `msg = { tab = id }`,
then switch the body on `state.tab`.

**Toolbar or status bar.** Use `ui.row { padding = "small", align = "center", surface = "sidebar", border = "bottom", ... }`
with `ui.spacer()` between the left and right groups. For a status bar that
mods can extend, use `ui.region { name = "status", direction = "row" }`.

**Placing a mod explicitly.** `ui.mount "editor"` draws exactly that mod
wherever the shell puts it, whether or not it has a `region`.

**Restyling.** Change `style.luau`, not the modules: for example, make
panels flat with `panel = { radius = 0, margin = 0, border = "none", shadow = "none" }`.
For dark-only tweaks, give palette colors `{ light, dark }`. Give a mod its
own look with `styles` in the mod itself (see `app/mods/inspector`). To
restyle an extension from the app side, add a rule for the same selector in
`style.luau`, which wins.

**Live data.** Mods can't read the machine (no `os`, `io` or network).
Providers share data under `app`: `app.system` (CPU, memory, network, from
the Wasm provider each second) and `app.git` (branch, changed files, ahead and behind, from a
process provider). Views re-run with each publish. If a widget needs data
that isn't there, write a provider (below); don't fake it in Luau unless it
is labelled demo data, as in `mods/usage`.

**Providers: data Luau can't produce.** A provider is a folder
`providers/<name>/` with a `provider.toml`, in either folder. What it
publishes appears to every view as `app.<name>`, and a mod messages it with
`msg = ..., to = "provider:<name>"`. Adding, editing or removing a provider
folder starts, restarts or stops only that provider, while the window keeps
running; the others keep their state. A failed provider shows a banner, and its error appears in the
terminal log. There are two kinds:

- **`kind = "wasm"`:** a WebAssembly component implementing
  `wit/provider.wit`, shipped as `provider.wasm` in the folder, or as the
  Rust crate it is built from (`Cargo.toml` with `crate-type = ["cdylib"]`
  and its own empty `[workspace]`, plus `src/lib.rs` using
  `wit_bindgen::generate!`).
  - It exports `start(settings)`, `tick()` every `interval` seconds, and
    `receive(message)`. All values are JSON text, and `tick` and `receive`
    return JSON to publish, or nothing.
  - With a `Cargo.toml` and Cargo installed (looked up on `PATH`, then in
    `~/.cargo/bin`), the host builds the crate when it starts and when its
    source changes, into `~/.cache/moonframe/providers`; a compile error
    becomes the provider's problem, never a quiet fallback to an older
    `provider.wasm`. Building needs the `wasm32-wasip2` target
    (`rustup target add wasm32-wasip2`). Otherwise it loads `provider.wasm`.
  - Installed apps ship `provider.wasm` only: `just stage` copies `app/`
    as the bundle holds it, each wasm provider prebuilt, without sources.
  - It runs sandboxed: no files, network or environment, a 64 MB memory cap,
    and an instruction budget per call.
  - It reaches the machine only through `capabilities`. Today that is
    `"system"`, which gives `host::system_load()`. Use this kind by default.
  - Only the running component is sandboxed. Building it runs Cargo and the
    crate's proc macros with the user's access. So never add a provider's
    source from an untrusted source without reading it, and keep its
    dependencies minimal. Something from elsewhere should come as
    `provider.wasm` alone.
    The host refuses to build a folder with a `build.rs` or a `.cargo/`.
  - `app/providers/system` is the example.
- **`kind = "process"`, `command = [...]`:** any program run in the folder.
  - Each line it prints on stdout is one JSON value to publish. Each message
    arrives on stdin as a `{"message": ...}` line. Its stderr goes to the log.
  - It has the user's full access, so use it only when a sandbox can't reach
    the data, and say so in `provider.toml`.
  - It should exit when stdin closes.
  - `app/providers/git` is the example.

`[settings]` in `provider.toml` reaches `start`. `just check` validates the
manifests, and `just providers` builds and lints the Wasm ones. When a view
reads provider data, handle it being absent (`app.git` is nil until the
first publish).

To write one, copy the closest example folder, rename it, and change what it
publishes. Keep the published JSON small and shaped for the view (numbers
and short lists, not raw dumps), and pick the longest `interval` the widget
can live with. Wasm needs a host capability for anything outside its
sandbox; if none fits (today only `system`), use a process provider or add a
capability in `crates/moonframe-providers` (a Rust change).

**Progress bars and sparklines** are plain stacks: a fixed-`width` track
row holding a fill row of `width * fraction`, with `.track` and `.fill`
classes in the stylesheet. Sparkline bars are thin rows of varying `height`
in a row with `align = "end"`. See `mods/system` and `mods/usage`.

**Sheets and menus from a widget.** Wrap the widget in `ui.popover` and keep
`open` in the mod's state:

```lua
ui.popover {
	open = state.open, dismiss = "close", side = "top", width = 340,
	content = ui.column { ... },
	ui.row { class = "widget", msg = "toggle", ... },   -- the anchor
}
-- style.luau: popover = { background = "card", radius = 12, padding = 16,
--   shadow = "0 12px 32px -8px #0004", transition = "slide-fade fast ease-out" }
```

**A new extension.** Create `app/mods/<name>/main.luau` with a `region` the
shell already draws. It appears without touching the shell. To give it a new
place, add a `ui.region "<new>"` to the shell. A region with no mods draws
nothing.

The shell in `app/main.luau` and the mods in `app/mods/` are working
examples of each pattern. Read them before larger changes.

## Extending the host

When a request needs something no element can express (an input field, a
tree, a chart), add an element in Rust. Say so to the user first: it needs a
build and an app restart, unlike everything above. `ui.icon` and
`ui.terminal` are recent examples to copy.

1. `crates/moonframe-core/src/node.rs`: add it to the `Element` tokens, and a
   struct and `Node` variant. Give it `look: Look` so classes, `style` and
   source lines reach it, and a `Part` in `crates/moonframe-core/src/style.rs`
   so the stylesheet can target it. Tokens of its own go in a `tokens!`.
2. `crates/moonframe-core/src/node/read.rs`: `impl Build` with its `KEYS`,
   and a `Read` type for any value that must be checked (as `IconName` and
   `TerminalId` are).
3. `crates/moonframe-core/src/prelude.luau`: its `ui.<name>` constructor,
   with the positional key if it has one.
4. `crates/moonframe/src/render/`: `impl Draw` for it (`elements.rs`, or
   a file of its own as `terminal.rs` is), and its arm in `draw_node` in
   `render.rs`.
5. `crates/moonframe/src/inspect.rs`: how the inspector names it
   (`describe`) and its line (`line`).
6. `types/moonframe.d.luau`: its props type and `ui.<name>`.
7. A test in `crates/moonframe-core/src/tests.rs`, then `just ci`.
8. This skill: the element table, the selector list, and a recipe if it is
   something agents will reach for. A component wrapping it makes it show up
   in the Components menu.

Anything that runs code or touches the machine belongs in a provider or
behind a host element that the mod can only parameterise (as terminals take
a `cwd`, never a command). The Luau sandbox is the boundary; keep it.

## Sharing mods

A package is a git repository, in one of two shapes:
- **one mod**: `main.luau` at the root. The mod is named after the
  repository, lowercased, with other characters as `-` (`My.Clock` is
  `my-clock`).
- **an app folder**: any of `mods/`, `providers/`, `components/`,
  `style.luau`, laid out as above. No `main.luau` at the root (that would
  make it one mod), and so no shell.

```sh
moonframe mod install github.com/owner/repo   # or owner/repo, a git URL, a folder; @ref pins it
moonframe mod list | update [NAME] | remove NAME | enable NAME | disable NAME
moonframe mod new ~/code/my-mod                # a git repo with one mod and a README
moonframe mod link ~/code/my-mod               # use a folder in place, to develop it live
```

Installs clone into `~/.config/moonframe/packages/<host>/<owner>/<repo>`
and are listed in `~/.config/moonframe/packages.toml`; an open window
picks up every change. Mods, components and styles are sandboxed Luau;
providers that run as the user (process providers, and Wasm, which is
built from source) are shown and must be approved, or passed `--yes`.
**Packages ship source only**: install, update and link refuse a package
holding a `provider.wasm` or any other file that is not text (images
aside), so the user, or an agent, can review everything it runs before
installing. A Wasm provider in a package ships its crate (`Cargo.toml`,
`src/`) and never a build; gitignore `target/`. When asked whether a
package is safe, read its providers' source and commands, not just the
mods. To publish, push the
repository anywhere git can clone from and share the install line. When
the user asks to share something they built in `~/.config/moonframe`,
copy it into a new package (`mod new`), `link` it, and remove the original
so the two do not shadow each other. Don't edit installed packages in
`packages/`: an update replaces them; copy the mod into the user's `mods/`
instead.

## Commands

| Command | Does |
| --- | --- |
| `just check` | loads everything headlessly (your folder and packages included); run after each edit |
| `just check-builtin` | the same for `app/` alone |
| `just types` | strict Luau type check against `types/moonframe.d.luau` |
| `just screenshot OUT PID` | captures the window of the process you started |
| `just run` | opens the window on `app/` (the user usually has one open) |
| `just providers` | builds and lints the Wasm providers |
| `just stage` | stages `app/` as the bundle ships it, Wasm prebuilt |
| `just bundle` | builds `target/bundle/Moonframe.app` (binary, staged app, icon) |
| `just install` | builds the app and installs it in `/Applications`; use it to try mods in `~/.config/moonframe` against the app as users get it |
| `just ci` | format check, lint, tests, types, providers, check |
