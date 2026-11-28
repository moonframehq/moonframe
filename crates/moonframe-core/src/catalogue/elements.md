## Host elements

The `ui.*` functions the host draws itself. Components are built from these.
Text-like elements take their main value positionally: `ui.text "hi"`,
`ui.button { "Save", msg = "save" }`.

### Layout

| Element | Use it for | Keys (besides children) |
| --- | --- | --- |
| `ui.row { ... }`, `ui.column { ... }` | any box of children; with `msg`/`command` the whole box is clickable (rows, tabs, tiles); `drag = true` makes it a title bar that moves the window | frame keys, `align`, `justify`, `wrap`, `selected`, `drag`, `key` (names it among its siblings: with a `transition` rule it grows in as it appears and shrinks away as it goes; with `rise` it fills its box and comes up from below it whole, once the one it replaces has sunk away), `msg`/`to`, `command` or `window` (a mod opened in a window of its own), pointer keys, extras |
| `ui.panel { ... }` | a titled section with header actions over a body | frame keys, `title`, `actions = { elements }` |
| `ui.hsplit { ... }`, `ui.vsplit { ... }` | areas the user resizes by dragging; a clicked divider also moves with the arrow keys (16 px, 64 with Shift), Home and End take the pane before it to its `min` and `max`, and a double-click puts back the `reset` size of a pane beside it (the one before, else after), else a starting `size` the same way; children must be `ui.pane` | `id`, `divider`, `resize` (sent once the user lets go of a divider, or moves it by key or double-click, with `sizes`, each pane's whole pixels in order, 0 for a hidden one: keep them in state and pass them back as the panes' `size`, and they survive a relaunch), `to` |
| `ui.pane { ... }` | one side of a split; slides in and out when `visible` changes; `lazy = true` starts what it mounts only once it first shows, and keeps it asleep (not run, not drawn) while hidden | `size`, `reset` (the size a double-click on its divider puts it back to, apart from `size`: the app's default when `size` passes back the user's kept one; else the starting `size`), `min`, `max`, `fixed`, `visible`, `lazy`, `gap`, `padding`, `surface`, `scroll` |
| `ui.list { count = n, row_height = 24, row = function(i) ... end }` | long lists (files, logs, results, chats): only the rows that show are built and drawn, so any length costs a screenful; it scrolls itself and takes the space it is given | `id` (two lists in one mod need two), `count`, `row_height` (pixels for every row, or `function(i)` giving row `i`'s own, for up to 100,000 rows: a row with a second line can be taller than the rest), `row` (builds row `i`, from 1; called again as it scrolls, so keep it cheap), `range` (sent with `first` and `last`, the rows any part of which shows, and `y`, the pixels scrolled, as they change, at most every 100 ms; `to` names another mod), `scroll_to` (`n` or `{ row = n, seq }` reveals row n as far as needed; `{ y = px, seq }` goes back to a `y` `range` told), frame keys; while scrolled down it keeps the first row showing in place as rows with a `key` arrive above (`range`, `scroll_to` and keeping in place are the GPUI window's, so far) |
| `ui.region "name"` | the slot mods fill by naming it in `region` (shell only) | `name`, `direction`, `gap`, `justify` |
| `ui.mount "name"` | one mod's view, by name | `mod` |
| `ui.divider()` | a line across the parent's axis | |
| `ui.spacer()` | take the free space, to push what follows to the far end | |

### Namespaced mods and providers

Embedding apps can select miniapps outside `app_dir` with `Moonframe::root`
and `entry`, or `entry_handle().add_entry(namespace, id, path)` at runtime.
Roots set policy without scanning. An entry holds `main.luau` and/or
`provider/provider.toml`: mount `ui.mount "acme:notes"` (or give it an `id`
for independent instance state), read `app["acme:notes"]`, send to
`"provider:acme:notes"`, and subscribe to `"provider/acme:notes"`.
Saved mod/instance state includes the namespace. Namespace and id each use
1–48 of `a-z 0-9 - _`; `app provider topic demo component shell parent` are
reserved namespaces. `remove_entry` / `remove_root` unload them without
erasing saved state. An entry's own `components/<name>.luau` come first for
its `use` (and only its), and its `style.luau` (`style { rules = { ... } }`)
styles only what it draws. `entry_embedded::<E>(namespace, id)` ships one in
the binary; `moonframe::check_entry` reports its problems.

An element given a key it does not take, a wrong token or an unknown icon
draws a warning sign in its place and is reported to the host as a
`ViewDiagnostic` (path, key, value, line, suggestion). `padding` and `gap`
take `none`, `small`, `normal` or `large` on an element; pixels go in
`style`. Check a miniapp with data (ship a `sample.json` with what its
provider publishes) so branches that only draw with data are checked too.
`moonframe::schema()` lists every element and its keys as data.

Prefer sandboxed Wasm for end-user providers. The host registers namespaced
JSON capabilities with `capability` / `capability_async`; a guest declares them
in `provider.toml` and calls `moonframe_provider::host_call` from the Rust SDK.
`RootPolicy::default()` accepts only prebuilt Wasm (no Cargo crate) and disables
process/socket kinds. Native providers are compiled into the host and bind by
the full entry name. Entry files reload live, failed replacement mods or Wasm
builds retain the old revision, and `on_entry_status` reports progress/errors.
Stateful entry providers survive clean restarts. See `docs/wasm-providers.md`
for the API, limits, caller identity and build policies.

### Content

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.text "..."` | any text; a bare string child is plain text | `text`, `tone`, `size`, `weight` (`light` `normal` `medium` `semibold` `bold`), `truncate` (one line), `lines` (at most N lines), `wrap = false` (keep lines whole, for code that scrolls sideways), `syntax` (color it as code in this language, a line at a time), extras |
| `ui.icon "name"` | a bundled icon at the text's size and color; `spin = true` turns it (loaders). Lucide glyphs (git: `git-branch` `git-commit-horizontal` `git-merge` `git-pull-request` `git-pull-request-draft` `git-pull-request-closed` `git-compare` `circle-dot`), agent logos `agent-claude` `agent-codex` `agent-gemini` `agent-cursor` `agent-copilot` `agent-opencode` `agent-amp` `agent-droid` `agent-kiro` `agent-qwen` `agent-grok`, brands `brand-github` `brand-gitlab` `brand-anthropic` `brand-openai` `brand-mistral` `brand-ollama` `brand-linkedin` `brand-x` `brand-slack` `brand-wikipedia` `brand-google-scholar`; the full list is `IconName` in types/moonframe.d.luau. An app built on Moonframe adds its own as `app-NAME` (lowercase words joined by hyphens): its `icons/app-NAME.svg`, or `circle-dashed`, a placeholder, where the app lacks it | `name`, `tone`, `size`, `spin`, extras |
| `ui.image "avatar.png"` | a picture from a file inside the app's folders or `~/.cache/moonframe/images` (relative to the mod, `~/`, or absolute; no URLs: a provider downloads into that cache); `radius` rounds it into an avatar | `src`, `width`, `height`, `fit` (`contain` `cover` `fill` `scale-down` `none`), `zoom` (screen pixels per picture pixel, 1 is 100 %: a viewer that fills its parent unless sized) panned to `center = { x, y }` (fractions of the picture), `overlay = { elements }` drawn over the picture wherever it lands (their `area` is in fractions of the picture: a crop frame, spots), `measure` (sent with the picture's `width`, `height`, `box_width`, `box_height` and the `fit` zoom when they, `measure`, or `to` change), extras |
| `ui.badge "..."` | a small pill: counts, tags, states | `text`, `tone`, extras |
| `ui.terminal { id = "main" }` | a live shell that fills its space | `id`, `cwd`, `engine` (`alacritty`, `ghostty`), `exit` (sent once, with `exited = true` added, as the shell exits; without it Enter starts another) |
| `ui.browser { id = "docs", url = "https://..." }` | a web page in a native view that fills its space (macOS, Windows; Linux offers the system browser); drive it with `command = "browser-back"` (`-forward`, `-reload`, `-stop`) and `browser = "<id>"` on a control | `id`, `url` (http, https, file), `zoom`, `navigate` (sent with `url`, `title`, `loading`, `back`, `forward` added) and `loading` (with `loading`, `url`); a `ui.input { browser = "<id>" }` is its address bar |
| `ui.document { html = state.html }` | HTML shown sealed in a native view that fills its space (macOS, Windows): no script runs and nothing loads but the document (pictures and fonts inline as `data:`), in the app's light or dark look, for a briefing, a shared file's preview or a note; `editable = true` edits it in place | `id`, `html` or `src` (a file, named as `ui.image` names pictures), `link` (sent with `url`, `text`: links are never followed, `#part` scrolls), `scroll` (with `y`, `height`, `content_height`), `scroll_to` (a `y`, an element `id`, or `{ y \| id, seq }`), `select` (with `text`, `x`, `y`, `width`, `height`), `editable`, `change` and `done` (with `html`, the whole document) |
| `ui.markdown "# Notes"` | a markdown document drawn by the host (headings, emphasis, lists, task lists, tables, quotes, `> [!NOTE]` alerts, footnotes, frontmatter, ` ```mermaid `, ` ```dot ` and ` ```graphviz ` fences drawn as diagrams in the theme with the `diagram` feature (Copy as Mermaid and Copy source in their corner; while a fence streams in, the last version that parsed stays; what does not parse shows as code with why), highlighted code with a copy button; LaTeX in a build with `math`), up to 1 MiB; links send `link` instead of opening, a `#heading` link scrolls there, a `[label](cite:source)` link draws a source chip, and pictures show only with `images`, never from the web (a picture titled `emoji`, `![:party:](~/.cache/moonframe/images/emoji/party.png "emoji")`, draws inline at the line's height from the image cache alone, with or without `images`, its alt text standing in until it loads or when it is missing); styled by `markdown`, `markdown-code`, `markdown-quote`, `markdown-alert`, `markdown-find` and `markdown-find-current` rules | `text`, `link` (sent with `url` added), `tooltips` (`{ [url] = text }`: a link's tooltip, over its own title; a link with a title, `[text](url "title")`, shows it while every link to that address has the same title; links in a `> [!NOTE]` alert too), `to`, `grow`, `scroll`, `width`, `height`, `max_width`, `lines` (at most N lines, for a preview), `images`, `image` (a click on a picture on its own line, with `src` and `alt`), `outline` (sent with `headings`: `level`, `text`, `at`, `slug`), `section` (sent with `heading`, the index being read), `reveal = { at, seq }` (scrolls there when it changes), `find`, `find_index`, `found` (sent with `count`), `stream` (fade in what is added), `scrolled` (with `scroll`: sent as the reader scrolls, with `fraction`, 0 at the top to 1 at the end), `scroll_at = { fraction, seq }` (with `scroll`: scrolls there when it changes, without sending `scrolled`) |
| `ui.paths { paths = { { points = { 0, 8, 40, 8, 40, 30 }, arrow = "end", radius = 6 } } }` | lines the view places in pixels from its top-left corner: arrows between dependent bars, connectors in a diagram; without `width` and `height` it fills its parent, so it floats over what it links (`float = "fill"` on a wrapper) | `paths` (each `points` flat x, y pairs, `color`, `stroke`, `dashed`, `radius` rounds its corners, `arrow` `end` `start` `both`), `width`, `height` |
| `ui.map { pins = { { lat = 37.79, lon = -122.40, label = "1" } } }` | a street map from a tile provider with numbered pins and routes over it; drag pans, the wheel, a pinch, a two-finger double tap, a double click and its buttons zoom, and two fingers turn it (it settles on north within a few degrees); without `center`/`zoom` it frames the pins and routes. With `glide`, a new `center`, `zoom` or `bearing` eases the map there. Tiles come only from the listed providers, cached under the cache folder; the provider's credit shows in the corner. Built with the `map` feature (the starter and sealed apps using it have it; elsewhere it marks its spot) | `provider` (`openstreetmap`, the default, and `carto-light` `carto-dark` `carto-voyager` `esri-streets` need no key, nor do `vector-liberty` `vector-positron` `vector-streets` `vector-dark` `vector-graphite`, vector tiles drawn on the GPU by gpui-maps in builds with the `map-vector` feature; `stadia-smooth` `stadia-smooth-dark` `maptiler-streets` `mapbox-streets` `thunderforest-transport` need `key`), `center = { lat, lon }`, `zoom`, `bearing` (the compass direction up the box, degrees clockwise from north; a new one turns the map where it stands, so feed `move`'s back in and a compass shown while it is not 0 sets it to 0 on a click), `pins` (each `lat`, `lon`, `label` up to 3 characters or `icon`, `color`, `shape` `dot` `disc` `balloon`, `selected`, `pulse` (a halo breathes out of it: where the user is), `msg`), `routes` (each `points` flat lat, lon pairs drawn as they are, or `via` stops and `mode` `walk` `bike` `car` routed along streets by routing.openstreetmap.de; `color`, `stroke`, `dashed`; the style's `color` otherwise), `routed` (sent with `route`, `seconds`, `meters` when a `via` route is found or `routed`/`to` changes), `move` (sent with `lat`, `lon`, `zoom`, `bearing` after the user moves it, as they go), `select` (sent with `pin`, its index, on a click), `to`, `width`, `height`, `grow` |
| `ui.video { source = "camera", white = 4095 }` | live frames from another process, from a ring in shared memory, in a 16-bit colour-managed native view that fills its space (macOS, the `video` feature); nothing draws over it, so `guides` draws crop frames and lines in the picture. `source`, `id`, `zoom`, `center`, `filter` (`auto` `nearest` `linear`), `color` (`srgb` `display-p3` `rec709` `rec2020` `linear-srgb` `linear-p3` `none`), `extended`, `black`, `white`, `gain`, `gamma`, `guides`, `probe`, `stats`, `probed`, `width`, `height`. Most apps want `video-viewer`; see `docs/video.md` |
| `ui.diagram { id = "flow", document = { nodes = {...}, edges = {...} } }` | a diagram drawn in Rust in the theme's colors and font: boxes, rounded boxes, pills, diamonds, circles, cylinders, notes and text, joined by orthogonal edges with heads, in groups; or a sequence diagram. Nodes without a place are laid out in layers (`direction` down, up, right or left), and a new document keeps what stayed where it was; placed ones (`x`, `y`, `w`, `h`, as an editor's) stay put. Scroll pans, a pinch or ⌘-scroll zooms. With `mode = "edit"` (the default with `shapes`) a click selects (shift adds, a drag on empty space draws a marquee), a drag moves with snapping guides, corners resize, the blue ports connect, a double click edits the label, and a `tool` draws new nodes; it never changes the document itself but tells the mod each edit (`selected`, `moved`, `resized`, `created`, `connected`, `deleted`, `edited`; `opened` on a double click in `view` mode; `zoom`, `tool`). Built with the `diagram` feature (the starter has it; `app.capabilities.diagram` says so; elsewhere, and on Rune, it marks its spot). The `diagram` component is the same canvas in Luau | `mermaid` or `dot` (Mermaid flowchart, sequence, state, class and ER diagrams, or Graphviz DOT, parsed in Rust; text that does not parse shows why), `shapes` and `links` (the `diagram` component's document) or `document` (`kind` `flow` or `sequence`, `direction`, `nodes` each `id`, `label`, `sublabel`, `kind`, `tone`, `fill`, `group`, `x`/`y`/`w`/`h`; `edges` each `from`, `to`, `label`, `kind` `solid` `dashed` `dotted` `thick` `invisible`, `start`/`end` `none` `arrow` `open` `circle` `cross` `triangle` `diamond` `hollow-diamond`, `tone`; `groups` each `id`, `label`, `parent`; a sequence's `messages` each `from`, `to`, `label`, `kind` `sync` `async` `reply`, and `notes` each `over`, `text`, `before`), `mode`, `tool`, `selected`, `highlight` (ids drawn in the accent), `camera` (`fit`, `zoom` or `zoom_by`, acted on with each new `seq`), `scroll = "focus"` (pans only after a click inside, in a page that scrolls), `export = { format = "png" | "svg", seq }` (writes the diagram in the app's colors to the share folder and sends `{ exported = { path, format } }`, for `app:share`), `copy = { seq }` (the diagram as Mermaid on the clipboard), `id`, `class`, `style` |
| `ui.view { name = "chart", id = "sales", props = {...} }` | a view the app writes in Rust and registers by `name` (apps on the Moonframe library; the starter registers none): what Luau can't draw, such as an editor or a canvas. Kept by mod and `id`, it gets `props` again when they change, and sends this mod's `update` messages of its own | `name`, `id`, `props`, `class`, `style` |

### Controls

`ui.revision_timeline { entries = { { id = "r1", day = "Today", label = "10:30" } }, selected = "r1", select = "pick" }`
is a native, right-aligned Time Machine ruler. Nearby ticks smoothly magnify like the Dock.
Entries are ordered top to bottom; adjacent equal `day`s form groups. Clicking selects a revision;
after focusing it, Up/Down choose revisions, Shift-Up/Down jump days, and Home/End reach either end.
The wheel scrolls long histories. `select` receives `revision` (stable id), `index` (one-based),
`day` and `label`. Also takes `id`, `to`, `width` (240), `height` (420), `grow`, `disabled`,
`class`, `style`, `state`. Up to 10000 unique ids. Style with `revision-timeline` (`color` is
the selected tick); respects `animation_speed`, including 0. Use `revision-timeline` for a gallery demo.

The same element supports compact chat rails: `side = "left"`, `dates = false`,
`labels = "hover"`, `preview = true`. Entries need only `id` and `label`; optional `day`
groups revisions, and `summary` adds up to three lines to the preview card.
`labels` is `all` (day groups and active label), `active` (hover or selection), `hover`, or `none`.
`width` includes room for labels/cards; `tick_width` (default 12, 1–128 px) controls the rail itself.
Also configurable: `tick_height` (2, 0.5–16 px), `spacing` (9, 4–64 px),
`magnification` (4.5, 1–10; 1 disables it), `radius` (4.5, 1–20 neighbours), and
`spread` (1.8, 0–5; 0 keeps centers fixed). `side` defaults to `right`, `dates` to true,
and `preview` to false. Preview cards are noninteractive and stay inside the component's box.

| Element | Use it for | Keys |
| --- | --- | --- |
| `ui.button { "Label", ... }` | an action with a label and/or an icon (`icon` alone is an icon-only button) | `label`, `icon`, `variant`, `small`, `disabled`, `msg`/`to` or `command`, extras |
| `ui.item { "Label", ... }` | a plain selectable list row | `label`, `detail`, `badge`, `selected`, `msg`/`to` or `command`, extras |
| `ui.input { id = "title", value = state.title, change = "type" }` | a text field the host edits (`multiline` for a text area); what the user typed stays across renders, `value` replaces it when the mod passes a new one. A writing field with `prose` (markdown styled as typed), `dim = "sentence"` or `"paragraph"` (focus mode: the rest fades) and `typewriter` (the caret's line stays in the middle), as `prose-editor` draws it. A code editor with `code = "luau"` (any language: colored as typed, with line numbers; read only with `disabled`, not dimmed) | `id`, `value`, `placeholder`, `change` (each edit, with `value` and `caret`, the bytes before the caret), `select` (selection movement, also with `value`, `caret` and `selected`, the bytes selected), `submit` (Enter, or ⌘↩ when multiline), `enter = "submit"` (multiline: Enter submits and ⇧↩ makes a new line, a chat's composer), `history` (earlier entries, oldest first: ↑ on the first line recalls the one before, ↓ on the last line the one after, then back to the draft; each recall sends `change`), `blur` (with `value`, as it loses focus: save a title edited in place), `keys`, `capture` (every chord pressed, with `keys` added, instead of editing: records a shortcut), `to`, `multiline`, `rows`, `max_rows` (grows with its lines up to it), `focus`, `disabled`, `mono`, `secret` (a password: drawn as bullets and never copied or cut, while `change` and `submit` still carry the text; one line), `paste` (sent with `files`, their paths, when files or a picture are pasted into it; a picture is saved to the image cache first and also comes as `image = { path, width, height }`; text pastes as usual), `width`, `grow`, `browser`, `prose`, `dim`, `typewriter`, `code`, `line` (a code editor's caret goes there), `caret` (`"start"`, `"end"` or a byte offset: where the caret goes when the field takes a new `value` or this changes, as after a recalled line), `scrolled` (multiline: sent as the text scrolls, with `fraction`, 0 at the top to 1 with the last line at the bottom), `scroll_at = { fraction, seq }` (multiline: scrolls there when it changes, without sending `scrolled`); a source and its `ui.markdown` preview follow each other with the two |

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
  `scroll_to = "row-7"` reveals the first row or column with
  `key = "row-7"` inside it, however deep (a row in a card in the list),
  once it is drawn; `scroll_to = { key = "row-7", seq = n }` asks again
  for the same key with each new `seq`. Both are the GPUI window's, so far.

- **Measure a scrolling stack** (`row`, `column`): with `scroll` enabled,
  `measure = msg` sends `msg` (to `to`, else the drawing mod) with its
  box's `width` and `height`, its content's `content_width` and
  `content_height`, how far it is scrolled (`x`, `y`), how many of its
  first children end inside its box (`fit`, as laid out before scrolling)
  and the first and last child any part of which shows (`first`, `last`,
  one-based; 0 when none does), each time one of them, `measure`, or `to`
  changes: as it lays out, resizes and scrolls, in whole pixels. A strip
  shows its arrows only
  while `content_width > width`, each unavailable at its end (`x == 0`,
  `x >= content_width - width`); a row of pills (`scroll = "x", scrollbar =
  false`) keeps the first `fit` and opens the rest from Show more; a list
  keeps `first`, to reveal it with `scroll_to` when it comes back.

- **Hear a click outside** (`row`, `column`): `outside = msg` sends `msg`
  (to `to`, else the drawing mod) when a click begins and ends outside its
  box and outside the popovers and overlays drawn inside it; a drag that
  began inside it does not count. Unlike a popover's `dismiss`, nothing
  covers the window: the click still reaches whatever it landed on, so one
  click both closes a floating panel and presses the button beside it. It
  is sent as the button comes up, so closing the panel never moves what
  was clicked from under the pointer. In the GPUI window only, so far:
  Rune draws the stack but does not send it.

- **Frame keys** (`row`, `column`, `panel`): `grow`, `width`, `height`,
  `gap`, `padding`, `surface`, `border`, `scroll` (`true` along its axis, or `"x"`, `"y"`, `"both"`: code that scrolls sideways), `follow = true` (with `scroll`:
  while at its end, stay there as the content grows, as a chat or log
  does), `shrink = false` (keep
  the content's size instead of being squeezed; the default inside a row or
  column that scrolls along its axis), `self_align` (`start`,
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
- **Keys while inside** (`row`, `column`): `keys = { left = <msg>, ... }`,
  named as an overlay's are, sends its messages to the drawing mod while
  focus is inside it (a click anywhere in it gives it focus, and a control
  in it keeps it there) and no text field has it, ahead of the mod's own
  `keys`: a deck whose arrows dismiss and keep only once the reader is in
  it. The innermost row or column with the key takes it.
- **Ends** (`row`, `column`, with `scroll`): `at_end = <msg>` sends `msg`
  to the drawing mod with `at_end = true` or `false` as it first lays out
  and each time it reaches or leaves its end (its bottom; a row that
  scrolls only sideways, its right), within the 24 px `follow` keeps:
  show a chat's "latest message" button while it is `false`.
  `scroll_end = <value>` scrolls to the end each time the value changes,
  so `scroll_end = { seq = n }` with a new `n` asks again; with `follow` it
  stays there as lines arrive.
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
  `dy`, `zoom`, `x`, `y`). On a Mac trackpad, `rotate = <msg>` gets two fingers
  turning (`angle` in degrees clockwise since the last message, `phase`
  `start` `move` `end`), `smart_zoom = <msg>` a two-finger double tap,
  `swipe = <msg>` a finished swipe (`direction` `back` `forward` `up` `down`)
  and `force = <msg>` a force click (`pressure`), each with `x`, `y`;
  `drop = <msg>` gets files dropped from other
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
- **Page text:** `{ to = "app:text", msg = { mod = "notes", reply = "page"
  } }` answers with `text`, what `notes` draws (rows across, columns down,
  with what it mounts; not fields, hidden panes or closed popovers), and
  `selection = { text, mod }`, the text the user last selected in a
  document and the mod that drew it, kept after a click into a field
  clears it on screen. `region = "main"` reads a region's mods instead;
  neither, only the selection. `max` cuts each (64,000 characters): what
  an assistant sends with a question.
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
  draw window buttons or leave room for traffic lights, and its `width`
  and `height` in points as it is resized, so a view can switch to a
  narrow layout under a breakpoint. The main window
  reopens where and as large as it last was, kept within the screen;
  `window = { remember = false }` opens it at its `size`, centered, instead.
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
- **Words in the user's language:** `ui.t("inbox-unread", { count = n })`
  is the message of that ID in the app's `locales/<language>/*.ftl`
  (Fluent), in the language the user prefers, English when it lacks one,
  and the ID itself when English does too. `ui.number(1234.5, { decimals =
  2 })` and `ui.date(seconds, { style = "short", time = true })` write
  numbers and dates the language's way; `ui.locale()` names it.
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
`clip = true` (hide what overflows the box). Selectors: a part (`row`, `item`, `list`, `popover`, `overlay`, `overlay-backdrop`, `tooltip`, `image`, `markdown`, `markdown-code`, `markdown-quote`, `markdown-alert`, `markdown-find`, `markdown-find-current`, ...),
a class (`.tab`), and a state: `:hover`, `:selected`, `:focus`,
`:disabled`, or one the view names with `state = "..."` (`.drawer:open`).
