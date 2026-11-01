## Host elements

The `ui.*` functions the host draws itself. Components are built from these.
Text-like elements take their main value positionally: `ui.text "hi"`,
`ui.button { "Save", msg = "save" }`.

### Layout

| Element | Use it for | Keys (besides children) |
| --- | --- | --- |
| `ui.row { ... }`, `ui.column { ... }` | any box of children; with `msg`/`command` the whole box is clickable (rows, tabs, tiles); `drag = true` makes it a title bar that moves the window | frame keys, `align`, `justify`, `wrap`, `selected`, `drag`, `key` (names it among its siblings: with a `transition` rule it grows in as it appears and shrinks away as it goes), `msg`/`to`, `command` or `window` (a mod opened in a window of its own), extras |
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
| `ui.image "avatar.png"` | a picture from a file inside the app's folders or `~/.cache/moonframe/images` (relative to the mod, `~/`, or absolute; no URLs: a provider downloads into that cache); `radius` rounds it into an avatar | `src`, `width`, `height`, `fit` (`contain` `cover` `fill` `scale-down` `none`), extras |
| `ui.badge "..."` | a small pill: counts, tags, states | `text`, `tone`, extras |
| `ui.terminal { id = "main" }` | a live shell that fills its space | `id`, `cwd`, `engine` (`alacritty`, `ghostty`) |
| `ui.browser { id = "docs", url = "https://..." }` | a web page in a native view that fills its space (macOS, Windows; Linux offers the system browser); drive it with `command = "browser-back"` (`-forward`, `-reload`, `-stop`) and `browser = "<id>"` on a control | `id`, `url` (http, https, file), `zoom`, `navigate` (sent with `url`, `title`, `loading`, `back`, `forward` added) and `loading` (with `loading`, `url`); a `ui.input { browser = "<id>" }` is its address bar |
| `ui.view { name = "chart", id = "sales", props = {...} }` | a view the app writes in Rust and registers by `name` (apps on the Moonframe library; the starter registers none): what Luau can't draw, such as an editor or a canvas. Kept by mod and `id`, it gets `props` again when they change, and sends this mod's `update` messages of its own | `name`, `id`, `props`, `class`, `style` |

### Controls

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.button { "Label", ... }` | an action with a label and/or an icon (`icon` alone is an icon-only button) | `label`, `icon`, `variant`, `small`, `disabled`, `msg`/`to` or `command`, extras |
| `ui.item { "Label", ... }` | a plain selectable list row | `label`, `detail`, `badge`, `selected`, `msg`/`to` or `command`, extras |

### Overlays

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.popover { ..., content = el }` | content floating beside its anchor (the children): dropdowns, context menus, sheets from a status bar; `side` is `top`, `bottom`, `left` or `right`, and `align` (`start`, `end`) the anchor's edge it lines up with | `open`, `dismiss`, `to`, `side`, `align`, `gap`, `width`, `content`, `keys` |
| `ui.overlay { ..., content = el }` | content over the whole window: dialogs and palettes (`center`, `top`), sheets (`left`, `right`, `bottom`), toasts (`top-end`, `bottom-end`, ...) | `open`, `dismiss`, `to`, `place`, `backdrop`, `width`, `height`, `keys`, `content` |

**Keys** (`overlay`, `popover`): `keys = { escape = msg, enter = msg, up = msg,
down = msg, ["secondary-k"] = msg }` while it is open and topmost; nothing
else sees those keys. Escape sends `dismiss` unless `keys` names `escape`.
`to = "mod"` sends `dismiss` and `keys` messages to another mod. A click
goes to the innermost clickable element only.
Names are modifiers (`ctrl`, `alt`, `shift`, `cmd`, `secondary` = cmd on
macOS, ctrl elsewhere) then a key, joined by `-`.

### Shared keys

- **Frame keys** (`row`, `column`, `panel`): `grow`, `width`, `height`,
  `gap`, `padding`, `surface`, `border`, `scroll` (`true` along its axis, or `"x"`, `"y"`, `"both"`: code that scrolls sideways), `shrink = false` (keep
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
  on an icon's corner). `grow` with a `width` or `height` grows from that size.
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
  `browser = "<id>"`.
- **Window:** the shell's `mod { window = { titlebar = "hidden",
  traffic_lights = { x, y } } }` draws the layout under the title bar (at
  startup only); `app.window` tells views the `platform`, `titlebar`,
  `maximized`, `fullscreen` and `active`, so a title bar knows whether to
  draw window buttons or leave room for traffic lights.
- **Extras** (`row`, `column`, `button`, `item`, `text`, `icon`, `image`,
  `badge`):
  - `tooltip = "text"` shows under the pointer, styled by the `tooltip` rule.
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

`background`, `color`, `border`, `border_color`, `radius`, `padding`, `gap`,
`margin`, `shadow`, `size`, `weight` (`light` to `bold`, `semibold` between `medium` and `bold`), `font` (`sans`, `mono`), `case`
(`upper`, `lower`, on text), `opacity`, `width` and `height` (under the
element's own; on containers),
`transition`. Selectors: a part (`row`, `item`, `list`, `popover`, `overlay`, `overlay-backdrop`, `tooltip`, `image`, ...),
a class (`.tab`), and a state: `:hover`, `:selected`, `:focus`,
`:disabled`, or one the view names with `state = "..."` (`.drawer:open`).
