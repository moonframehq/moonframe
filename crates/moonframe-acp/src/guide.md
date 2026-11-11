## How a Moonframe app is built

The window is Luau. Rust only checks the trees the scripts return and draws
them. Change Luau, never Rust, for anything a stylesheet or a mod can do.

Folders (each layer has this shape; a higher layer's file wins at the same
path, and the window reloads the moment a file is saved):

```text
main.luau              the shell: lays out the whole window
style.luau             colors, fonts, corners, and style rules
mods/<name>/main.luau  a mod; the shell places it by region or by name
components/<name>.luau a piece of view mods `use "name"`
providers/<name>/      data Luau can't make (provider.toml)
```

A mod file returns:

```lua
local stat = use "stat"           -- a component, by file name
return mod {
	region = "left",              -- optional
	subscribe = { "topic" },      -- optional
	styles = { [".mine"] = { padding = "small" } }, -- rules for what it draws
	init = function(previous) return previous or { open = false } end,
	update = function(state, msg)  -- a click's `msg`; return state[, sends]
		if msg == "toggle" then state.open = not state.open end
	end,
	view = function(state, app)     -- one element; app.<provider> is data
		return ui.column { grow = true, class = "mine",
			ui.text { "Title", size = "title", weight = "bold" },
			ui.button { "Toggle", msg = "toggle" },
		}
	end,
}
```

Rules that trip agents up:
- Unknown elements, keys and token values are errors. The mod table takes
  only `init update view region order title styles subscribe tab window`.
- No `require`, `os`, `io`, `print`, clock or randomness in views: state and
  messages are plain data. Share code through components and `use`.
- Children are the table's array part; `nil`/`false` children are skipped.
  For a computed list pass `table.unpack(items)` last.

Elements: `ui.row` `ui.column` (flex; `gap`, `padding`, `align`, `justify`,
`grow`, `width`, `height`, `surface`, `border`, `scroll`, `msg`), `ui.panel
{ title, actions }`, `ui.hsplit`/`ui.vsplit` of `ui.pane { size, min, max }`,
`ui.text`, `ui.button { "Label", msg, variant, icon }`, `ui.item`,
`ui.badge`, `ui.icon "name"` (Lucide names), `ui.input { id, value, change }`,
`ui.list { count, row_height, row }`, `ui.markdown`, `ui.divider()`,
`ui.spacer()`, `ui.popover`, `ui.overlay`. Most take `class = "a b"` and
`style = { ... }`; rows, columns, text, buttons, items and icons also take
`tooltip`.

Tokens:
- space (`gap`, `padding`, `margin`): `none` `small`(4) `normal`(8) `large`(16), or pixels
- tone: `normal` `muted` `accent` `success` `warning` `danger`
- text size: `small` `normal` `large` `title` `headline` `display` `hero`
- weight: `light` `normal` `medium` `semibold` `bold`
- surface: `background` `sidebar` `muted` `secondary`
- border: `true` `all` `top` `bottom` `left` `right` `none`
- colors: theme names (`background` `foreground` `muted` `muted-foreground`
  `sidebar` `card` `secondary` `border` `accent` `success` `warning`
  `danger`), `"#rrggbb"`, `"accent/15"` (15% alpha), or a name from
  `style.luau`'s `colors`, which take `{ light = ..., dark = ... }`.

Style properties: `background` `color` `border` `border_color` `radius`
`padding` `margin` `gap` `shadow` `size` `weight` `font` (`sans`/`mono`)
`opacity` `width` `height` `gradient` `transition`. Selectors: an element
type (`panel`, `text`, `button`, `row`...), a class `.name`, either with a
state (`:hover`, `:selected`, `:focus`, `:disabled`, or a named one set with
`state = "open"`). Parts with a rule of their own: `panel-header` (a
panel's title bar, which is where page and panel titles are drawn),
`tooltip`, `popover`, `overlay`, `overlay-backdrop`, `input-placeholder`,
`input-selection`, `input-caret`, `divider`. The window's own background is
the `background` color; panels draw on `card`. Rules apply as written here:
there is no need to read the host's Rust to confirm one.

Precedence, lowest first: built-in default; type rules (the mod's `styles`,
then `style.luau`); class rules in the order listed (mod's then
`style.luau`'s); the element's `style = {}`; its own keys (`padding =`,
`tone =`...). So to restyle, prefer one rule in `style.luau` or the mod's
`styles` over inline styles, and drop an element's own key when a rule
should decide. A `style.luau` in a higher layer is merged over the lower
one: write only the colors and rules that change.

```lua
-- style.luau
return style {
	colors = { accent = { light = "#4f46e5", dark = "#818cf8" } },
	rules = {
		panel = { background = "card", radius = 12 },
		[".page-title"] = { weight = "bold" },
	},
}
```

Changing a component or mod that lives in a lower layer (the toolbox):
copy its file to the same path in the folder you write in and change the
copy; it wins at once. Keep copies to what must change, as a copy no longer
follows updates of the original. For looks alone, a rule is enough.

Finding the spot: an Inspect pick in the context names the element, its mod
and `file:line` (the `ui` call that drew it); start there. Otherwise use the
file index below, and search for the visible text.

Doing it fast:
- Read only the files you change. Prefer one exact edit to rewriting a file.
- Edit with your file tools, not shell commands such as `sed`: edits in the
  folder you write in, and the check command as given, run without asking
  the user; other commands may wait for them.
- After editing, run the check command once; fix what it names; run it again.
- The user's window already shows your change: there is nothing to build,
  launch or reload.
