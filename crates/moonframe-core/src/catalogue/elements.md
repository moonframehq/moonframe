## Host elements

The `ui.*` functions the host draws itself. Components are built from these.
Text-like elements take their main value positionally: `ui.text "hi"`,
`ui.button { "Save", msg = "save" }`.

### Layout

| Element | Use it for | Keys (besides children) |
| --- | --- | --- |
| `ui.row { ... }`, `ui.column { ... }` | any box of children; with `msg`/`command` the whole box is clickable (rows, tabs, tiles); `drag = true` makes it a title bar that moves the window | frame keys, `align`, `justify`, `wrap`, `selected`, `drag`, `key` (names it among its siblings: with a `transition` rule it grows in as it appears and shrinks away as it goes), `msg`/`to`, `command` or `window` (a mod opened in a window of its own), pointer keys, extras |
| `ui.panel { ... }` | a titled section with header actions over a body | frame keys, `title`, `actions = { elements }` |
| `ui.hsplit { ... }`, `ui.vsplit { ... }` | areas the user resizes by dragging; children must be `ui.pane` | `id` |
| `ui.pane { ... }` | one side of a split; slides in and out when `visible` changes; `lazy = true` starts what it mounts only once it first shows, and keeps it asleep (not run, not drawn) while hidden | `size`, `min`, `max`, `fixed`, `visible`, `lazy`, `gap`, `padding`, `surface`, `scroll` |
| `ui.list { count = n, row_height = 24, row = function(i) ... end }` | long lists of same-height rows (files, logs, results): only the rows that show are built and drawn, so any length costs a screenful; it scrolls itself and takes the space it is given | `id` (two lists in one mod need two), `count`, `row_height`, `row` (builds row `i`, from 1; called again as it scrolls, so keep it cheap), frame keys |
| `ui.region "name"` | the slot mods fill by naming it in `region` (shell only) | `name`, `direction`, `gap`, `justify` |
| `ui.mount "name"` | one mod's view, by name | `mod` |
| `ui.divider()` | a line across the parent's axis | |
| `ui.spacer()` | take the free space, to push what follows to the far end | |

### Content

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.text "..."` | any text; a bare string child is plain text | `text`, `tone`, `size`, `weight` (`light` `normal` `medium` `semibold` `bold`), `truncate` (one line), `lines` (at most N lines), `wrap = false` (keep lines whole, for code that scrolls sideways), extras |
| `ui.icon "name"` | a bundled icon at the text's size and color; `spin = true` turns it (loaders). Lucide glyphs (git: `git-branch` `git-commit-horizontal` `git-merge` `git-pull-request` `git-pull-request-draft` `git-pull-request-closed` `git-compare` `circle-dot`), agent logos `agent-claude` `agent-codex` `agent-gemini` `agent-cursor` `agent-copilot` `agent-opencode` `agent-amp` `agent-droid` `agent-kiro` `agent-qwen` `agent-grok`, brands `brand-github` `brand-gitlab` `brand-anthropic` `brand-openai` `brand-mistral` `brand-ollama`; the full list is `IconName` in types/moonframe.d.luau | `name`, `tone`, `size`, `spin`, extras |
| `ui.image "avatar.png"` | a picture from a file inside the app's folders or `~/.cache/moonframe/images` (relative to the mod, `~/`, or absolute; no URLs: a provider downloads into that cache); `radius` rounds it into an avatar | `src`, `width`, `height`, `fit` (`contain` `cover` `fill` `scale-down` `none`), `zoom` (screen pixels per picture pixel, 1 is 100 %: a viewer that fills its parent unless sized) panned to `center = { x, y }` (fractions of the picture), `overlay = { elements }` drawn over the picture wherever it lands (their `area` is in fractions of the picture: a crop frame, spots), `measure` (sent with the picture's `width`, `height`, `box_width`, `box_height` and the `fit` zoom when they change), extras |
| `ui.badge "..."` | a small pill: counts, tags, states | `text`, `tone`, extras |
| `ui.terminal { id = "main" }` | a live shell that fills its space | `id`, `cwd`, `engine` (`alacritty`, `ghostty`), `exit` (sent once, with `exited = true` added, as the shell exits; without it Enter starts another) |
| `ui.browser { id = "docs", url = "https://..." }` | a web page in a native view that fills its space (macOS, Windows; Linux offers the system browser); drive it with `command = "browser-back"` (`-forward`, `-reload`, `-stop`) and `browser = "<id>"` on a control | `id`, `url` (http, https, file), `zoom`, `navigate` (sent with `url`, `title`, `loading`, `back`, `forward` added) and `loading` (with `loading`, `url`); a `ui.input { browser = "<id>" }` is its address bar |
| `ui.markdown "# Notes"` | a markdown document drawn by the host (headings, emphasis, lists, task lists, tables, quotes, highlighted code), up to 1 MiB; links send `link` instead of opening, pictures are not loaded; styled by `markdown`, `markdown-code` and `markdown-quote` rules | `text`, `link` (sent with `url` added), `to`, `grow`, `scroll`, `width`, `height`, `max_width` |
| `ui.paths { paths = { { points = { 0, 8, 40, 8, 40, 30 }, arrow = "end", radius = 6 } } }` | lines the view places in pixels from its top-left corner: arrows between dependent bars, connectors in a diagram; without `width` and `height` it fills its parent, so it floats over what it links (`float = "fill"` on a wrapper) | `paths` (each `points` flat x, y pairs, `color`, `stroke`, `dashed`, `radius` rounds its corners, `arrow` `end` `start` `both`), `width`, `height` |
| `ui.view { name = "chart", id = "sales", props = {...} }` | a view the app writes in Rust and registers by `name` (apps on the Moonframe library; the starter registers none): what Luau can't draw, such as an editor or a canvas. Kept by mod and `id`, it gets `props` again when they change, and sends this mod's `update` messages of its own | `name`, `id`, `props`, `class`, `style` |

### Controls

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.button { "Label", ... }` | an action with a label and/or an icon (`icon` alone is an icon-only button) | `label`, `icon`, `variant`, `small`, `disabled`, `msg`/`to` or `command`, extras |
| `ui.item { "Label", ... }` | a plain selectable list row | `label`, `detail`, `badge`, `selected`, `msg`/`to` or `command`, extras |
| `ui.input { id = "title", value = state.title, change = "type" }` | a text field the host edits (`multiline` for a text area); what the user typed stays across renders, `value` replaces it when the mod passes a new one. A writing field with `prose` (markdown styled as typed), `dim = "sentence"` or `"paragraph"` (focus mode: the rest fades) and `typewriter` (the caret's line stays in the middle), as `prose-editor` draws it | `id`, `value`, `placeholder`, `change` (each edit, with `value` and `caret`, the bytes before the caret), `select` (selection movement, also with `value` and `caret`), `submit` (Enter, or ⌘↩ when multiline), `blur` (with `value`, as it loses focus: save a title edited in place), `keys`, `to`, `multiline`, `rows`, `max_rows` (grows with its lines up to it), `focus`, `disabled`, `mono`, `width`, `grow`, `browser`, `prose`, `dim`, `typewriter` |

### Overlays

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.popover { ..., content = el }` | content floating beside its anchor (the children): dropdowns, context menus, sheets from a status bar; `side` is `top`, `bottom`, `left` or `right` (it opens on the other side when that one has no room in the window), and `align` (`start`, `end`) the anchor's edge it lines up with; `anchor_clicks = true` keeps the anchor clickable while open (a field with suggestions under it) instead of a click on it sending `dismiss` | `open`, `dismiss`, `to`, `side`, `align`, `gap`, `width`, `content`, `keys`, `anchor_clicks` |
| `ui.overlay { ..., content = el }` | content over the whole window: dialogs and palettes (`center`, `top`), sheets (`left`, `right`, `bottom`), toasts (`top-end`, `bottom-end`, ...) | `open`, `dismiss`, `to`, `place`, `backdrop`, `width`, `height`, `keys`, `content` |

**Keys** (`overlay`, `popover`): `keys = { escape = msg, enter = msg, up = msg,
down = msg, ["secondary-k"] = msg }` while it is open and topmost; nothing
else sees those keys. Escape sends `dismiss` unless `keys` names `escape`.
`to = "mod"` sends `dismiss` and `keys` messages to another mod. A click
goes to the innermost clickable element only.
Names are modifiers (`ctrl`, `alt`, `shift`, `cmd`, `secondary` = cmd on
macOS, ctrl elsewhere) then a key, joined by `-`.

### Shared keys

- **Scroll to a child** (`row`, `column`): with `scroll` enabled,
  `scroll_to = 12` reveals direct child 12 (one-based) when the index
  changes. It scrolls only as far as needed and leaves wheel scrolling
  free between changes. Set it to nil to reset the request.

- **Frame keys** (`row`, `column`, `panel`): `grow`, `width`, `height`,
  `gap`, `padding`, `surface`, `border`, `scroll` (`true` along its axis, or `"x"`, `"y"`, `"both"`: code that scrolls sideways), `follow = true` (with `scroll`:
  while at its end, stay there as the content grows, as a chat or log
  does), `shrink = false` (keep
  the content's size instead of being squeezed), `self_align` (`start`,
  `center`, `end`, `stretch`: e.g. keep a capsule at its own width inside a
  column, which otherwise stretches it), `hug = true` (keep its own width in
  a column, and follow a row's `align`: the usual choice for pills, badges
  and dots), `max_width`, `max_height`,
  `float` (out of the flow, over the parent: pinned to an edge or corner,
  `float = "top-right"` (`top-left`, `bottom-left`, `bottom-right`, `top`,
  `bottom`, `left`, `right`, `fill`), set in from it,
  `float = { at = "top-right", inset = 4 }`, or at pixel offsets,
  `float = { top = -4, right = -4 }`, negative overhanging: a count bubble
  on an icon's corner), `area` (out of the flow at fractions of the parent,
  `area = { left = 0.1, top = 0.2, width = 0.5, height = 0.3 }`: a crop
  frame, a marker on a track). `grow` with a `width` or `height` grows from that size.
- **Pointer keys** (`row`, `column`): `pointer = <msg>` is sent as a button
  goes down on it, as the pointer moves while held (anywhere in the
  window), and as it comes up, with `phase` (`down` `move` `up`), `x`, `y`
  (fractions of the box as it was at the press), `dx`, `dy` (pixels since
  the press), `width`, `height`, `button`, `clicks` and the keys held
  (`shift` `alt` `ctrl` `cmd` `secondary`) added. `hover = true` also
  sends `hover` moves and `leave`. `range = { min, max, step, value, speed,
  axis }` adds the `value` a drag sets: where the pointer is between `min`
  and `max` (a slider), or `value` plus pixels times `speed` (a number
  scrubbed by its label). `wheel = <msg>` gets scrolls and pinches (`dx`,
  `dy`, `zoom`, `x`, `y`); `drop = <msg>` gets files dropped from other
  apps (`paths`), which that mod's `ui.image` may then show; `cursor` sets the pointer's
  shape (`crosshair`, `grab`, `grabbing`, `ew-resize`, `ns-resize`,
  `nwse-resize`, `nesw-resize`, `move`, `pointer`, `text`, `not-allowed`).
- **Drag and drop** (`row`, `column`): `carry = <data>` lets the user drag
  it (a card, a row, a calendar event); a box of its size, the `carry`
  rule, follows the pointer while it dims, and a click still clicks.
  `accept = <msg>` on another row or column is sent as it moves over it
  (`phase = "over"`), off it (`"leave"`) and is dropped on it (`"drop"`),
  with `carry` (the data), `from` (the mod that drew it), and `x`, `y`
  (fractions of the target's box: before or after a row's middle) added.
  The innermost target takes the drop; nothing follows a drop, so clear
  what `over` showed then. Other entered targets hear `leave` when the
  drag ends, including cancellation. Not with `pointer` or `drag`.
- **Children:** `false` and `nil` children are skipped, wherever they are.
- **Look keys** (every element but splits): `class = "a b"`,
  `style = { ... }`, and `state = "open"`: a state of the view's own naming,
  which `:open` rules match (`[".drawer:open"] = { width = 280 }`). With a
  `transition` in its rules, a change of state eases colors, opacity,
  width and height from one state's look to the next.
- **Actions:** `msg = <data>` calls the drawing mod's `update` (`to = "name"`
  for another mod or `"provider:<name>"`); `command = "..."` runs an app
  command instead: `quit`, `reload`, `toggle-theme`, `minimize`, `maximize`,
  `close-window`. A browser command (`browser-back`, `browser-forward`,
  `browser-reload`, `browser-stop`, `browser-open`) also takes
  `browser = "<id>"`. To run a command from `update`, without a click, send
  `{ to = "app:command", msg = "minimize" }`.
- **Files:** `{ to = "app:pick", msg = { kind = "open", types = { "tif" },
  multiple = true, reply = "opened" } }` opens the system's panel (`kind`
  `open`, `folder` or `save` with `name` and `folder`); the answer comes
  back to the mod as `reply` with `paths` (`path` for a save) or
  `cancelled = true`. Picked files may then show in that mod's `ui.image`.
- **Text edits:** `{ to = "app:edit", msg = { field = "notes", wrap = {
  "**", "**" } } }` edits the mod's `ui.input` with that `id` around its
  selection, keeping undo and focus: `wrap` puts the two around it (or
  takes them off), `lines = "- "` starts each selected line with it (or
  takes it off), `insert = "@Sam ", back = 3` replaces the selection and
  the 3 bytes before it. A completion can add `at = <caret offset>` and
  `expect = "@sa"`: it applies only with an empty selection there and that
  exact preceding text (`back` must equal its byte length). The field then
  sends its `change`. A toolbar button sends directly (`to = "app:edit"`);
  editing restores focus in the field's own window.
- **Keys anywhere:** `mod { keys = { ["secondary-1"] = "histogram" } }`
  sends its message when the key is pressed and nothing nearer (an open
  overlay, a terminal) took it, to every mod and every mounted copy of
  one (a tab's) that takes it. While a text field has focus it keeps all
  its keys, so mods hear none.
- **Window:** the shell's `mod { window = { titlebar = "hidden",
  traffic_lights = { x, y } } }` draws the layout under the title bar (at
  startup only); `app.window` tells views the `platform`, `titlebar`,
  `maximized`, `fullscreen` and `active`, so a title bar knows whether to
  draw window buttons or leave room for traffic lights.
- **A mod's own window** takes `titlebar = "hidden"` and `traffic_lights`
  in its `window` too, beside `size` and `min_size`, with `drag = true` on
  what moves it. While open, the mod and its instances read that window's
  state as `app.window`; closing it restores the main window's state.
  The mod's view is shared wherever it is mounted.
  `window = "<mod>"` on a row or column opens that
  mod in a window of its own; from `update`, send `{ to = "app:window",
  msg = { open = "<mod>", send = <msg> } }` to open it on a page (`send`
  reaches its `update` first) or `{ close = "<mod>" }` to close it. The
  mod sizes it with `mod { window = { size = { width, height }, min_size
  = { width, height } } }`.
- **Extras** (`row`, `column`, `button`, `item`, `text`, `icon`, `image`,
  `badge`):
  - `tooltip = "text"` shows under the pointer, styled by the `tooltip` rule.
    The rule's `transition` eases it in (`fade`, `slide` rises it a little,
    `slide-fade`), and its delay (`"fade fast 400ms"`, or `delay`) is how
    long the pointer rests before it shows (500ms without one).
  - `context = <data>` is sent on a right-click, to open a context menu
    (to `to = "mod"` if given).
  - `copy = "text"` puts the text on the clipboard when clicked.
- **`ui.memo(key, deps, build)`** returns what `build()` returned the last
  time this view asked for `key` with the same `deps` (a list, compared with
  `==`), or calls it now: for a costly part of a view that depends on a few
  values. A table counts as the same while it is the same table, even if
  changed inside.
- **Views run only when needed:** a view runs again when its state or props
  change (any message to its `update`), when a key of `app` it read
  changes, or on a reload; otherwise its last tree is kept. Read `app.x`
  where you use it rather than copying all of `app`; going through all of
  `app` (`for k, v in app`) runs it on any change.
- **`selected = true`** on a row or column matches `:selected` rules, like an
  item: tabs, segments, chosen tiles.

### Style keys

`background`, `gradient` (`"180deg, #ffa53a, #ff6200"`), `color`, `border`, `border_color`, `radius`, `padding`, `gap`,
`margin`, `shadow`, `size`, `weight` (`light` to `bold`, `semibold` between `medium` and `bold`), `font` (`sans`, `mono`), `case`
(`upper`, `lower`, on text), `opacity`, `width` and `height` (under the
element's own; on containers),
`transition` (a second duration is its delay: `"fade 150ms 40ms"`), `delay`
(replaces the transition's delay, set inline per item to stagger a list),
`clip = true` (hide what overflows the box). Selectors: a part (`row`, `item`, `list`, `popover`, `overlay`, `overlay-backdrop`, `tooltip`, `image`, `markdown`, `markdown-code`, `markdown-quote`, ...),
a class (`.tab`), and a state: `:hover`, `:selected`, `:focus`,
`:disabled`, or one the view names with `state = "..."` (`.drawer:open`).
