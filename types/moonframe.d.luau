-- Types for Moonframe modules: the `ui`, `mod`, `style`, and `log`
-- globals the host gives every script. `just types` checks app/ against
-- them in strict mode; editors with luau-lsp read them for completion.
--
-- These mirror what the host accepts (crates/moonframe-core). The host still
-- checks every value at run time; types catch mistakes before it runs.

-- Values that cross between a module and the host: messages and state.
type Data = nil | boolean | number | string | { [any]: any }

type Space = "none" | "small" | "normal" | "large"
type Tone = "normal" | "muted" | "accent" | "success" | "warning" | "danger"
type TextSize = "small" | "normal" | "large" | "title" | "headline" | "display" | "hero"
type Weight = "light" | "normal" | "medium" | "semibold" | "bold"
type Align = "stretch" | "start" | "center" | "end"
type Justify = "start" | "center" | "end" | "between"
type Surface = "background" | "sidebar" | "muted" | "secondary"
type Edge = "none" | "all" | "top" | "bottom" | "left" | "right"
type Border = boolean | Edge
type Direction = "row" | "column"
type Variant = "normal" | "primary" | "ghost" | "outline" | "danger"
type Command = "quit" | "reload" | "toggle-theme" | "show-themes" | "inspect" | "minimize" | "maximize" | "close-window" | "check-for-updates" | "new-window" | "activate" | BrowserCommand
-- Run on the `ui.browser` the control's `browser` key names.
type BrowserCommand = "browser-back" | "browser-forward" | "browser-reload" | "browser-stop" | "browser-open"

-- `#rrggbb`, a palette name, or a theme color, optionally `name/NN`.
type Color = string
-- A space name or pixels.
type Spacing = Space | number
-- One spacing, `{ y, x }`, `{ top, right, bottom, left }`, or named edges.
type Sides = Spacing | { Spacing } | {
	x: Spacing?,
	y: Spacing?,
	top: Spacing?,
	right: Spacing?,
	bottom: Spacing?,
	left: Spacing?,
}
type ShadowLayer = {
	x: number?,
	y: number?,
	blur: number?,
	spread: number?,
	color: Color?,
	inset: boolean?,
}
-- A preset (`"small"`), a CSS string (`"0 4px 12px #0003"`), a layer, or layers.
type Shadow = string | ShadowLayer | { string | ShadowLayer }
type TransitionKind = "slide" | "fade" | "slide-fade" | "rise" | "none"
-- `"slide 220ms ease-out"`, or a table; durations are ms, `"220ms"`, or a speed.
type Transition = string | {
	kind: TransitionKind?,
	duration: (number | string)?,
	curve: string?,
	-- Wait before moving, as CSS's transition-delay.
	delay: (number | string)?,
}

type Style = {
	background: Color?,
	-- A linear gradient over the background: `"180deg, #ffa53a, #ff6200"`
	-- (0 runs up, 90 right, 180 down, the default), or `{ from, to, angle }`.
	gradient: (string | { from: Color, to: Color, angle: number? })?,
	color: Color?,
	border: Border?,
	border_color: Color?,
	-- One radius, or each corner's clockwise from the top left as in CSS:
	-- { 8, 8, 0, 0 } rounds the top only.
	radius: (number | { number })?,
	padding: Sides?,
	gap: Spacing?,
	margin: Sides?,
	shadow: Shadow?,
	size: TextSize?,
	-- Text size in pixels, over `size` (6 to 64): text that must match
	-- what another toolkit draws at a size the app was given.
	font_size: number?,
	weight: Weight?,
	font: ("sans" | "mono")?,
	-- Letter case of a text: `upper` for headings.
	case: ("upper" | "lower" | "none")?,
	opacity: number?,
	-- Pixels, under the element's own `width` and `height`: what a named
	-- state's rule can change (`[".drawer:closed"] = { width = 48 }`).
	width: number?,
	height: number?,
	-- How a pane, popover or overlay shows and hides, a keyed child arrives
	-- and goes, and an element with a `state` moves to its next state's look
	-- (colors, opacity, width and height ease between the two).
	transition: Transition?,
	-- Replaces the transition's delay (ms, or "40ms"): set from an item's
	-- index so a list's items move one after another (a staggered fade).
	delay: (number | string)?,
	-- Hide what overflows the box instead of drawing it outside.
	clip: boolean?,
}

-- What a `ui` function returns; the host reads its fields.
type Element = { type: string }
-- A child: an element, a string (plain text), or false/nil (skipped).
type Child = Element | string | false | nil
-- Class names: `"a b"` or `{ "a", "b" }`.
type Class = string | { string }

-- Where `float` pins an element over its parent.
type FloatAt =
	"top-left"
	| "top-right"
	| "bottom-left"
	| "bottom-right"
	| "top"
	| "bottom"
	| "left"
	| "right"
	| "fill"
-- Out of the flow, over the parent: pinned to an edge or corner
-- (`"top-right"`), set in from it (`{ at = "top-right", inset = 4 }`), or at
-- pixel offsets from its edges (`{ top = -4, right = -4 }`, negative overhangs).
type Float = FloatAt | { at: FloatAt, inset: number? } | {
	top: number?,
	right: number?,
	bottom: number?,
	left: number?,
}

-- Out of the flow, over the parent at fractions of its size: `{ left =
-- 0.1, top = 0.2, width = 0.5, height = 0.3 }`. A crop frame over a
-- picture (in a `ui.image`'s `overlay`, fractions of the picture), a
-- marker on a track.
type Area = {
	left: number?,
	top: number?,
	width: number?,
	height: number?,
}

-- The pointer's shape over a row or column.
type Cursor =
	"default"
	| "pointer"
	| "text"
	| "crosshair"
	| "grab"
	| "grabbing"
	| "move"
	| "ew-resize"
	| "ns-resize"
	| "nwse-resize"
	| "nesw-resize"
	| "col-resize"
	| "row-resize"
	| "not-allowed"

-- Turns a drag into a `value` in `pointer` messages. Without `speed` the
-- value is where the pointer is along `axis` (a slider: `min` at the left
-- or bottom, `max` at the right or top); with it, `value` plus pixels
-- dragged times `speed` (a number scrubbed by dragging its label). It
-- snaps to `step` and stays between `min` and `max`.
type Range = {
	min: number,
	max: number,
	step: number?,
	value: number?,
	speed: number?,
	axis: ("x" | "y")?,
}

-- What a `pointer` message carries, added to its `msg` (a table gets the
-- fields; anything else arrives as `{ msg = msg, ... }`). `x` and `y` are
-- fractions of the box as it was when the button went down (0 to 1 inside,
-- beyond while dragging outside); `dx` and `dy` pixels since then.
type PointerMsg = {
	phase: "down" | "move" | "up" | "hover" | "leave",
	x: number,
	y: number,
	dx: number,
	dy: number,
	width: number,
	height: number,
	button: ("left" | "middle" | "right")?,
	clicks: number?,
	-- With `range`: the value the drag sets.
	value: number?,
	shift: boolean?,
	alt: boolean?,
	ctrl: boolean?,
	cmd: boolean?,
	secondary: boolean?,
}

-- What a `rotate` message carries: `angle` the degrees two fingers turned
-- since the last message, clockwise, how far through the turn it is, and
-- where over the box (`x`, `y`), with the keys held. On macOS.
type RotateMsg = {
	x: number,
	y: number,
	width: number,
	height: number,
	angle: number,
	phase: "start" | "move" | "end",
	shift: boolean?,
	alt: boolean?,
	ctrl: boolean?,
	cmd: boolean?,
	secondary: boolean?,
}

-- What a `swipe` message carries: which way it went, as a browser reads it.
type SwipeMsg = {
	x: number,
	y: number,
	width: number,
	height: number,
	direction: "back" | "forward" | "up" | "down",
	shift: boolean?,
	alt: boolean?,
	ctrl: boolean?,
	cmd: boolean?,
	secondary: boolean?,
}

-- What a `force` message carries: how hard the press is, from 0 to 1.
type ForceMsg = {
	x: number,
	y: number,
	width: number,
	height: number,
	pressure: number,
	shift: boolean?,
	alt: boolean?,
	ctrl: boolean?,
	cmd: boolean?,
	secondary: boolean?,
}

-- What a `wheel` message carries: `dx`, `dy` the pixels scrolled, `zoom`
-- a trackpad pinch's change (0.1 is 10 % larger, 0 for a scroll), and
-- where over the box (`x`, `y`), with the keys held.
type WheelMsg = {
	x: number,
	y: number,
	dx: number,
	dy: number,
	zoom: number,
	width: number,
	height: number,
	shift: boolean?,
	alt: boolean?,
	ctrl: boolean?,
	cmd: boolean?,
	secondary: boolean?,
}

-- Each props type is spelled out rather than combined with `&`, so string
-- literals such as `variant = "ghost"` check against their unions.

-- A stack with `msg` or `command` is clickable as a whole, like a button.
type StackProps = {
	class: Class?,
	style: Style?,
	-- A state of the view's own naming, which `:<state>` rules match.
	state: string?,
	msg: Data,
	to: string?,
	command: Command?,
	-- With a browser command: the `id` of the `ui.browser` (of the mod `to`, else this one).
	browser: string?,
	gap: Space?,
	padding: Space?,
	grow: boolean?,
	width: number?,
	height: number?,
	surface: Surface?,
	border: Border?,
	-- `true` scrolls along its axis; or "x", "y", "both".
	scroll: (boolean | "x" | "y" | "both")?,
	-- Reveal this direct child (one-based) when the index changes.
	scroll_to: number?,
	-- While scrolled to its end, stay there as the content grows (a chat).
	follow: boolean?,
	-- false scrolls with no scrollbar drawn (the GPUI backend; Rune follows
	-- the system's setting).
	scrollbar: boolean?,
	-- false keeps the content's size when the parent is short of space.
	shrink: boolean?,
	-- Where it sits across the parent's axis, over the parent's `align`.
	self_align: Align?,
	-- Keep its own width in a column; follow a row's `align` as usual.
	hug: boolean?,
	max_width: number?,
	max_height: number?,
	-- Out of the flow, over the parent: see `Float`.
	float: Float?,
	-- Out of the flow, at fractions of the parent: see `Area`.
	area: Area?,
	align: Align?,
	justify: Justify?,
	wrap: boolean?,
	-- As a mod's whole view in a row region, each child is a member of it:
	-- it fits or leaves the row by itself.
	spread: boolean?,
	-- Matches `:selected` rules, as on an item: tabs, segments, tiles.
	selected: boolean?,
	-- Extras: shown under the pointer; sent on a right-click; copied on a click.
	tooltip: string?,
	context: Data,
	copy: string?,
	-- Moves the window when dragged and zooms it on a double click, like a
	-- title bar; buttons and clickable stacks inside still take clicks.
	drag: boolean?,
	-- Names it among its siblings (1 to 64 of a-z, 0-9, - and _), so it stays
	-- itself as they come and go: with a `transition` rule it grows in when
	-- it appears and shrinks away when it goes, as tabs open and close.
	key: string?,
	-- Opens the mod named this in a window of its own (or raises it) when
	-- clicked: a tool that stays open beside the main window. Takes no `msg`;
	-- to open it on a page, send `app:window` `{ open = "<mod>", send = ... }`.
	window: string?,
	-- Sent as a button goes down on it, as the pointer moves while held
	-- (wherever it goes), and as it comes up, with a `PointerMsg`'s fields
	-- added: drags, sliders, handles, canvases. The innermost stack with
	-- `pointer` takes the press. To `to` if given.
	pointer: Data,
	-- Also send `phase = "hover"` moves with no button held, and `"leave"`.
	hover: boolean?,
	-- Adds the `value` a drag sets to `pointer` messages: see `Range`.
	range: Range?,
	-- Sent as the wheel or trackpad scrolls or pinches over it, with a
	-- `WheelMsg`'s fields; what is around it does not scroll.
	wheel: Data,
	-- Sent as two fingers turn over it, with a `RotateMsg`'s fields. This
	-- and the next three come from a Mac trackpad; elsewhere never.
	rotate: Data,
	-- Sent on a two-finger double tap over it, with `x`, `y`, `width`,
	-- `height` and the keys held.
	smart_zoom: Data,
	-- Sent when a swipe over it ends, with a `SwipeMsg`'s fields: three
	-- fingers where the system leaves those to apps, or two across, which
	-- also scroll.
	swipe: Data,
	-- Sent once when a press over it goes on to a force click, with a
	-- `ForceMsg`'s fields.
	force: Data,
	-- Sent when files are dropped on it from another app, with `paths`
	-- (and `x`, `y`). Dropped pictures may then show in this mod's `ui.image`.
	drop: Data,
	-- Dragging it carries this data to a row or column with `accept`: a
	-- card to another column, a row to another place in its list. A box of
	-- its size (the `carry` rule) follows the pointer while it dims.
	-- Not with `pointer` or `drag`.
	carry: Data,
	-- Sent as something carried moves over it, leaves it and is dropped on
	-- it, with a `CarryMsg`'s fields added. The innermost target under the
	-- pointer takes the drop. To `to` if given.
	accept: Data,
	cursor: Cursor?,
	[number]: Child,
}

-- What `accept` adds. `phase` is `"over"` as the carried thing moves over
-- the target, `"leave"` as it moves off, and `"drop"` when it lands, after
-- which nothing follows: clear what `over` showed then. `x` and `y` are
-- where the pointer is, as fractions of the target's box (before or after
-- a row's middle, say), `from` is the mod that drew what is carried.
type CarryMsg = {
	phase: "over" | "leave" | "drop",
	carry: Data,
	from: string,
	x: number,
	y: number,
	width: number,
	height: number,
}

type PanelProps = {
	class: Class?,
	style: Style?,
	state: string?,
	gap: Space?,
	padding: Space?,
	grow: boolean?,
	width: number?,
	height: number?,
	surface: Surface?,
	border: Border?,
	-- `true` scrolls along its axis; or "x", "y", "both".
	scroll: (boolean | "x" | "y" | "both")?,
	-- While scrolled to its end, stay there as the content grows (a chat).
	follow: boolean?,
	-- false scrolls with no scrollbar drawn (the GPUI backend; Rune follows
	-- the system's setting).
	scrollbar: boolean?,
	-- false keeps the content's size when the parent is short of space.
	shrink: boolean?,
	-- Where it sits across the parent's axis, over the parent's `align`.
	self_align: Align?,
	-- Keep its own width in a column; follow a row's `align` as usual.
	hug: boolean?,
	max_width: number?,
	max_height: number?,
	-- Out of the flow, over the parent: see `Float`.
	float: Float?,
	-- Out of the flow, at fractions of the parent: see `Area`.
	area: Area?,
	title: string?,
	actions: { Child }?,
	[number]: Child,
}

type PaneProps = {
	class: Class?,
	style: Style?,
	state: string?,
	size: number?,
	min: number?,
	max: number?,
	fixed: boolean?,
	visible: boolean?,
	-- Start what it mounts only once it first shows, and keep it asleep
	-- (neither drawn nor run) while it is hidden.
	lazy: boolean?,
	gap: Space?,
	padding: Space?,
	surface: Surface?,
	-- `true` scrolls along its axis; or "x", "y", "both".
	scroll: (boolean | "x" | "y" | "both")?,
	-- While scrolled to its end, stay there as the content grows (a chat).
	follow: boolean?,
	-- false scrolls with no scrollbar drawn (the GPUI backend; Rune follows
	-- the system's setting).
	scrollbar: boolean?,
	[number]: Child,
}

type SplitProps = {
	id: string?,
	-- false: no hairline between panes; a handle shows only when hovered.
	divider: boolean?,
	[number]: Element | false | nil,
}

type TextProps = {
	class: Class?,
	style: Style?,
	state: string?,
	text: string?,
	tone: Tone?,
	size: TextSize?,
	weight: Weight?,
	truncate: boolean?,
	-- Wrap to at most this many lines, then an ellipsis.
	lines: number?,
	-- false keeps each line whole (code in a container that scrolls sideways).
	wrap: boolean?,
	-- A line through it: a task done.
	strike: boolean?,
	-- Color it as code in this language, a line at a time.
	syntax: string?,
	-- Shown under the pointer; sent on a right-click (to the mod `to`, else
	-- this one); copied on a click.
	tooltip: string?,
	context: Data,
	copy: string?,
	to: string?,
	[number]: string,
}

-- Clicking sends `msg` to an `update` (to the mod `to`, else its own), or
-- runs an app `command`.
type ButtonProps = {
	class: Class?,
	style: Style?,
	state: string?,
	msg: Data,
	to: string?,
	command: Command?,
	-- With a browser command: the `id` of the `ui.browser` (of the mod `to`, else this one).
	browser: string?,
	label: string?,
	-- Drawn before the label; with no label, an icon-only button.
	icon: IconName?,
	variant: Variant?,
	small: boolean?,
	disabled: boolean?,
	tooltip: string?,
	context: Data,
	copy: string?,
	[number]: string,
}

type ItemProps = {
	class: Class?,
	style: Style?,
	state: string?,
	msg: Data,
	to: string?,
	command: Command?,
	-- With a browser command: the `id` of the `ui.browser` (of the mod `to`, else this one).
	browser: string?,
	label: string?,
	detail: string?,
	badge: string?,
	selected: boolean?,
	tooltip: string?,
	context: Data,
	copy: string?,
	[number]: string,
}

type BadgeProps = {
	class: Class?,
	style: Style?,
	state: string?,
	text: string?,
	tone: Tone?,
	-- Shown under the pointer; sent on a right-click (to the mod `to`, else
	-- this one); copied on a click.
	tooltip: string?,
	context: Data,
	copy: string?,
	to: string?,
	[number]: string,
}

-- The bundled icons (Lucide); the host refuses any other name.
type IconName =
	| "a-large-small"
	| "activity"
	| "agent-amp"
	| "agent-claude"
	| "agent-codex"
	| "agent-copilot"
	| "agent-cursor"
	| "agent-droid"
	| "agent-gemini"
	| "agent-grok"
	| "agent-kiro"
	| "agent-opencode"
	| "agent-qwen"
	| "archive"
	| "arrow-down"
	| "arrow-left"
	| "arrow-right"
	| "arrow-up"
	| "arrow-up-down"
	| "arrow-up-right"
	| "asterisk"
	| "at-sign"
	| "ban"
	| "battery"
	| "battery-charging"
	| "battery-full"
	| "battery-low"
	| "battery-medium"
	| "battery-warning"
	| "bell"
	| "bell-off"
	| "bold"
	| "book-open"
	| "bookmark"
	| "bot"
	| "brand-anthropic"
	| "brand-github"
	| "brand-gitlab"
	| "brand-mistral"
	| "brand-ollama"
	| "brand-openai"
	| "bug"
	| "building-2"
	| "calendar"
	| "calendar-check"
	| "calendar-days"
	| "case-sensitive"
	| "chart-column"
	| "chart-gantt"
	| "chart-line"
	| "chart-pie"
	| "check"
	| "chevron-down"
	| "chevron-left"
	| "chevron-right"
	| "chevron-up"
	| "chevrons-left"
	| "chevrons-right"
	| "chevrons-up-down"
	| "circle"
	| "circle-alert"
	| "circle-check"
	| "circle-dashed"
	| "circle-dot"
	| "circle-help"
	| "circle-pause"
	| "circle-play"
	| "circle-question-mark"
	| "circle-stop"
	| "circle-user"
	| "circle-x"
	| "clock"
	| "close"
	| "cloud"
	| "cloud-off"
	| "code"
	| "coffee"
	| "columns-2"
	| "command"
	| "copy"
	| "cpu"
	| "crop"
	| "dash"
	| "database"
	| "delete"
	| "diamond"
	| "dot"
	| "download"
	| "ellipsis"
	| "ellipsis-vertical"
	| "external-link"
	| "eye"
	| "eye-off"
	| "file"
	| "file-archive"
	| "file-text"
	| "filter"
	| "flag"
	| "folder"
	| "folder-closed"
	| "folder-git-2"
	| "folder-open"
	| "frame"
	| "funnel"
	| "gallery-vertical-end"
	| "git-branch"
	| "git-commit-horizontal"
	| "git-compare"
	| "git-merge"
	| "git-pull-request"
	| "git-pull-request-closed"
	| "git-pull-request-draft"
	| "github"
	| "gitlab"
	| "globe"
	| "grip-vertical"
	| "hard-drive"
	| "hash"
	| "heading"
	| "headphones"
	| "heart"
	| "heart-off"
	| "history"
	| "house"
	| "image"
	| "inbox"
	| "info"
	| "inspector"
	| "italic"
	| "key-round"
	| "keyboard"
	| "laptop"
	| "layers"
	| "layout-dashboard"
	| "layout-grid"
	| "link"
	| "list"
	| "list-checks"
	| "list-ordered"
	| "list-tree"
	| "loader"
	| "loader-circle"
	| "lock"
	| "lock-open"
	| "log-out"
	| "mail"
	| "mail-open"
	| "mails"
	| "map"
	| "maximize"
	| "maximize-2"
	| "memory-stick"
	| "menu"
	| "message-square"
	| "messages-square"
	| "mic"
	| "minimize"
	| "minimize-2"
	| "minus"
	| "monitor"
	| "moon"
	| "network"
	| "octagon-alert"
	| "palette"
	| "panel-bottom"
	| "panel-bottom-open"
	| "panel-left"
	| "panel-left-close"
	| "panel-left-open"
	| "panel-right"
	| "panel-right-close"
	| "panel-right-open"
	| "paperclip"
	| "pause"
	| "pencil"
	| "pin"
	| "pin-off"
	| "pipette"
	| "play"
	| "plug"
	| "plus"
	| "power"
	| "redo"
	| "redo-2"
	| "refresh-ccw"
	| "refresh-cw"
	| "repeat"
	| "replace"
	| "reply"
	| "resize-corner"
	| "rocket"
	| "rotate-ccw"
	| "rotate-ccw-clock"
	| "rotate-cw"
	| "rows-2"
	| "scan"
	| "search"
	| "send"
	| "server"
	| "settings"
	| "settings-2"
	| "shield"
	| "sliders-horizontal"
	| "smile-plus"
	| "sort-ascending"
	| "sort-descending"
	| "sparkles"
	| "spline"
	| "split-square-horizontal"
	| "split-square-vertical"
	| "square"
	| "square-check"
	| "square-pen"
	| "square-split-horizontal"
	| "square-split-vertical"
	| "square-terminal"
	| "star"
	| "star-fill"
	| "star-off"
	| "strikethrough"
	| "sun"
	| "tag"
	| "tags"
	| "terminal"
	| "text-quote"
	| "thumbs-down"
	| "thumbs-up"
	| "timer"
	| "trash"
	| "trash-2"
	| "triangle-alert"
	| "undo"
	| "undo-2"
	| "unlock"
	| "upload"
	| "user"
	| "user-plus"
	| "users"
	| "wand-sparkles"
	| "wifi"
	| "wifi-off"
	| "window-close"
	| "window-maximize"
	| "window-minimize"
	| "window-restore"
	| "workflow"
	| "zap"
	| "zoom-in"
	| "zoom-out"

-- Drawn at the size and color of text, so `color` and `size` style it.
type IconProps = {
	class: Class?,
	style: Style?,
	state: string?,
	name: IconName?,
	tone: Tone?,
	size: TextSize?,
	-- Turns without end, for a loader (`loader-circle`); `animation_speed`
	-- paces it and 0 stops it.
	spin: boolean?,
	-- Shown under the pointer; sent on a right-click (to the mod `to`, else
	-- this one); copied on a click.
	tooltip: string?,
	context: Data,
	copy: string?,
	to: string?,
	-- Luau doesn't check literals in list positions against a union, so
	-- write `name = "cpu"` (checked here) rather than `{ "cpu", ... }`.
	[number]: string,
}

type Fit = "contain" | "cover" | "fill" | "scale-down" | "none"

-- A picture from a file: relative to the mod's folder, `~/...`, or absolute.
-- Not a URL: have a provider download it to a cache. A missing file leaves
-- an empty box of `width` x `height`. `radius` in a rule rounds it.
type ImageProps = {
	class: Class?,
	style: Style?,
	state: string?,
	src: string?,
	width: number?,
	height: number?,
	fit: Fit?,
	-- Screen pixels per picture pixel (1 is 100 %), panned so `center` is
	-- in the middle: a viewer. Unset, it fits as `fit` says. Without a
	-- `width` or `height` it fills its parent that way.
	zoom: number?,
	-- The point of the picture in the middle while zoomed: `{ x, y }`
	-- fractions of the picture; `{ 0.5, 0.5 }` when unset.
	center: { number }?,
	-- Sent with `width`, `height` (the picture's own pixels), `box_width`,
	-- `box_height` and `fit` (the zoom that fits it) when they change.
	measure: Data,
	-- Drawn over the picture wherever it lands, so `area` there is in
	-- fractions of the picture: a crop frame, spots, a grid.
	overlay: { Child }?,
	-- Shown under the pointer; sent on a right-click (to the mod `to`, else
	-- this one); copied on a click.
	tooltip: string?,
	context: Data,
	copy: string?,
	to: string?,
	[number]: string,
}

-- A line through `points`, evenly spaced across its box. Its style's
-- `color` strokes it, `background` fills the area to the baseline, and
-- `border_color` draws the baseline, dashed.
type LineProps = {
	class: Class?,
	style: Style?,
	points: { number },
	min: number?,
	max: number?,
	baseline: number?,
	area: boolean?,
	dots: ("last" | "all")?,
	stroke: number?,
	dashed: boolean?,
	width: number?,
	height: number?,
}

-- Time Machine-style vertical ruler, with native Dock-like magnification.
type RevisionTimelineProps = {
	id: string?,
	entries: { { id: string, day: string?, label: string, summary: string? } },
	selected: string?,
	-- Sent with one-based index, revision (id), day and label added.
	select: any?,
	to: string?,
	width: number?,
	height: number?,
	grow: boolean?,
	disabled: boolean?,
	tick_width: number?,
	tick_height: number?,
	spacing: number?,
	-- Ordinary tick's peak width multiplier; 1 disables magnification.
	magnification: number?,
	-- Number of neighbours affected on either side; spread controls vertical expansion.
	radius: number?,
	spread: number?,
	side: ("left" | "right")?,
	labels: ("all" | "active" | "hover" | "none")?,
	dates: boolean?,
	preview: boolean?,
	class: Class?,
	style: Style?,
	state: string?,
}

-- Lines through points the view places itself, in pixels from the box's
-- top-left corner: arrows between dependent bars on a timeline, the
-- connectors of a diagram. Each line is stroked in its `color`, else the
-- style's `color`, else the border color. Without `width` and `height` it
-- fills its parent; it takes no clicks, so it can float over what it links.
type PathsProps = {
	class: Class?,
	style: Style?,
	paths: { Polyline },
	width: number?,
	height: number?,
}

type Polyline = {
	-- Corners in order, flat: { x1, y1, x2, y2, ... }, in pixels.
	points: { number },
	color: string?,
	stroke: number?,
	dashed: boolean?,
	-- Rounds each corner by this many pixels, as an elbow connector does.
	radius: number?,
	arrow: ("end" | "start" | "both")?,
}

-- A box of diagonal stripes in its style's `color` over its `background`.
type HatchProps = {
	class: Class?,
	style: Style?,
	gap: number?,
	width: number?,
	height: number?,
	grow: boolean?,
}

-- Markdown the host renders: headings, emphasis, lists, task lists, tables,
-- quotes, GitHub alerts, footnotes, frontmatter and highlighted code with a
-- copy button, up to 1 MiB; math too in a build with `math`. The `markdown`
-- rule styles the box, `markdown-code` its code, `markdown-quote` its quotes,
-- `markdown-alert` its alerts, `markdown-find` and `markdown-find-current`
-- the matches of `find`. A link never opens by itself: a click sends `link`, a
-- table getting a `url` field and anything else arriving as
-- `{ msg = link, url = url }`; a `#heading` link scrolls to its heading. A
-- `[label](cite:source)` link draws a source chip.
type MarkdownProps = {
	class: Class?,
	style: Style?,
	state: string?,
	text: string?,
	link: Data,
	-- The mod its messages go to; this one when unset.
	to: string?,
	grow: boolean?,
	-- Scroll its own content; give it `grow` or a `height`.
	scroll: boolean?,
	width: number?,
	height: number?,
	max_width: number?,
	-- Show at most this many lines of text (not with `scroll`).
	lines: number?,
	-- Show pictures from files a `ui.image` could show, never from the web.
	images: boolean?,
	-- Sent when a picture on a line of its own is clicked, with `src` and
	-- `alt` added.
	image: Data,
	-- Sent as the headings change, with `headings` added:
	-- { level, text, at (bytes into the text), slug }.
	outline: Data,
	-- Sent as a scrolling document passes a heading, with `heading`, its
	-- index, added.
	section: Data,
	-- Scrolls there when it changes; a new `seq` goes to one place again.
	reveal: { at: number, seq: number? }?,
	-- Text to mark wherever it shows.
	find: string?,
	-- Which match is current, from 1: marked strongly and scrolled to.
	find_index: number?,
	-- Sent as the number of matches changes, with `count` added.
	found: Data,
	-- Fade in what is added to `text`, as an answer streams in.
	stream: boolean?,
	[number]: string,
}

type TerminalEngine = "alacritty" | "ghostty"
-- What keeps the shell running: `auto` (default) is herdr-core, else tmux,
-- else a pty. herdr-core and tmux keep it across launches; a pty ends with
-- the app. One that isn't installed falls back to the next.
type TerminalBackend = "auto" | "herdr-core" | "tmux" | "pty"

-- A live shell (your login shell). The host keeps it by mod and `id`.
type TerminalProps = {
	class: Class?,
	style: Style?,
	state: string?,
	id: string?,
	-- Where it starts: a path or `~/...`; the home folder when unset.
	cwd: string?,
	engine: TerminalEngine?,
	backend: TerminalBackend?,
	-- Sent once, with `exited = true` added, as the shell exits (Ctrl-D,
	-- `exit`). Without it the terminal says so and Enter starts another.
	exit: Data?,
	-- Sent, to this mod only, as the shell reports something, with `event`
	-- added: "prompt", "started" (`command?`), "finished" (`command?`,
	-- `status?`, `took_ms`), "folder" (`path`, `host?`), "title" (`title`)
	-- or "notice" (`title?`, `body`). A table is added to; anything else
	-- arrives as `msg`. Any program can print these, so show them, and
	-- never act on them.
	shell: Data?,
	[number]: string,
}

-- A web page in a native view, kept by mod and `id`. `url` navigates when
-- it changes from the last one given; the user's own navigation stands
-- until then. Only http, https and file addresses.
type BrowserProps = {
	class: Class?,
	style: Style?,
	state: string?,
	id: string?,
	url: string?,
	-- The page's zoom factor, 0.25 to 5.
	zoom: number?,
	-- Sent as the page navigates, its title changes or a load starts or
	-- ends, with `url`, `title` and `loading` added, and `back` and
	-- `forward` (whether it can go there) where the platform says (a
	-- non-table message arrives as `{ msg = ... }`).
	navigate: Data,
	-- Sent as a load starts and finishes, with `loading` and `url` added.
	loading: Data,
	[number]: string,
}

-- A view the app writes in Rust and registers by `name` (an app on the
-- Moonframe library; the starter has none). The host keeps it by mod and
-- `id`, so it lives across reloads and while hidden, and hands it `props`,
-- telling it when they change. What the user does in it comes back to this
-- mod's `update` as messages the view chooses.
type ViewProps = {
	class: Class?,
	style: Style?,
	state: string?,
	name: string?,
	id: string?,
	props: Data,
	[number]: string,
}

-- A text field. The host keeps its text by mod and `id` (else its place in
-- the tree); `value` replaces the text only when it differs from the last
-- `value` passed. Messages carry the text: a table `msg` gets a `value`
-- field, anything else arrives as `{ msg = msg, value = text }`.
type InputProps = {
	class: Class?,
	style: Style?,
	state: string?,
	id: string?,
	value: string?,
	placeholder: string?,
	-- Sent on each edit, with `value` and `caret` (bytes before the caret:
	-- `string.sub(value, 1, caret)` is what comes before it) added.
	change: Data,
	-- Selection movement, with `value` and the caret's byte offset added.
	select: Data,
	-- Sent on Enter, or ⌘/Ctrl-Enter when `multiline`.
	submit: Data,
	-- Sent with `value` when it loses keyboard focus: a title edited in
	-- place is saved as the user clicks away.
	blur: Data,
	-- While focused, these keys send their message instead of editing:
	-- `{ up = "prev", escape = "close", ["shift-tab"] = "back",
	-- ["secondary-k"] = "clear" }` (`secondary` is ⌘ on macOS, Ctrl elsewhere).
	keys: { [string]: any }?,
	-- Takes every chord pressed in it, sent with `keys` added ("cmd-shift-p"),
	-- instead of editing: a field that records a shortcut.
	capture: Data,
	-- The mod the messages go to; the drawing mod when unset.
	to: string?,
	multiline: boolean?,
	-- Visible lines of a multiline field (default 3); it scrolls past them.
	rows: number?,
	-- A multiline field grows from `rows` with its lines up to this many,
	-- then scrolls.
	max_rows: number?,
	disabled: boolean?,
	-- Take keyboard focus when it appears, or when this turns true.
	focus: boolean?,
	-- The monospace font.
	mono: boolean?,
	width: number?,
	grow: boolean?,
	-- Make it the address bar of this mod's `ui.browser` with this id (or
	-- `to`'s): Enter loads the text (a host over https, words as a web
	-- search), focus selects it, and Escape puts back the page's address.
	browser: string?,
	-- A writing field (needs `multiline`): markdown is styled as it is
	-- typed (headings bold, emphasis slanted, markup faded).
	prose: boolean?,
	-- Focus mode (needs `multiline`): the sentence or paragraph the caret is
	-- in stays in full ink and the rest of the text fades.
	dim: ("sentence" | "paragraph")?,
	-- Keep the caret's line in the middle of the field (needs `multiline`).
	typewriter: boolean?,
	-- A code editor for this language (needs `multiline`): `"luau"`,
	-- `"rust"`, ...; colored as typed, with line numbers. With `disabled`
	-- it is read only, not dimmed: its text can be selected and copied.
	code: string?,
	-- A code editor's caret goes to this line (from 1), scrolled into view,
	-- when it changes or the field takes a new `value`.
	line: number?,
}

type RegionProps = {
	name: string,
	direction: Direction?,
	gap: Space?,
	justify: Justify?,
}

-- Which side of its anchor a popover opens on.
type Side = "top" | "bottom" | "left" | "right"
-- The anchor's edge it lines up with: left or right above and below, top or
-- bottom beside.
type Anchor = "start" | "end"

-- Keystrokes an open popover or overlay takes, to the message each sends
-- to this mod's `update`: `{ escape = "close", down = "next", ["cmd-k"] = "find" }`.
-- A name is modifiers then a key, joined by `-`: modifiers `ctrl`, `alt`,
-- `shift`, `cmd` (Super/Windows elsewhere) and `secondary` (cmd on macOS,
-- ctrl elsewhere); keys `a`-`z`, `0`-`9`, a symbol such as `/`, `f1`-`f24`,
-- `escape`, `enter`, `tab`, `space`, `backspace`, `delete`, `insert`, `up`,
-- `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`.
type Keys = { [string]: Data }

-- Content floating beside its anchor (the list part). While `open`, a click
-- outside sends `dismiss`. A `popover` stylesheet rule styles and animates it.
type PopoverProps = {
	class: Class?,
	style: Style?,
	state: string?,
	open: boolean?,
	dismiss: Data,
	-- The mod `dismiss` and `keys` messages go to, if not this one.
	to: string?,
	side: Side?,
	align: Anchor?,
	gap: number?,
	width: number?,
	keys: Keys?,
	-- While open, a click on the anchor reaches it instead of sending
	-- `dismiss`: a field whose suggestions show under it.
	anchor_clicks: boolean?,
	content: Element,
	[number]: Child,
}

type Placement =
	"center"
	| "top"
	| "bottom"
	| "left"
	| "right"
	| "top-start"
	| "top-end"
	| "bottom-start"
	| "bottom-end"

-- Content floating over the whole window: a dialog or palette (`center`,
-- `top`), a sheet along an edge (`bottom`, `left`, `right`), a toast in a
-- corner. Open overlays stack in tree order above popovers. A click on the
-- backdrop or outside the content sends `dismiss`, and so does Escape unless
-- `keys` takes it. An `overlay` rule styles and animates the content, an
-- `overlay-backdrop` rule the dimmed window behind.
type OverlayProps = {
	class: Class?,
	style: Style?,
	state: string?,
	open: boolean?,
	dismiss: Data,
	-- The mod `dismiss` and `keys` messages go to, if not this one.
	to: string?,
	place: Placement?,
	backdrop: boolean?,
	width: number?,
	height: number?,
	keys: Keys?,
	content: Element,
}

-- Rows of one height, of which only those showing are built and drawn, so
-- a list of any length costs a screenful. `row(i)` builds row `i` (from 1)
-- and is called again as the list scrolls; keep it cheap and pure.
type ListProps = {
	class: Class?,
	style: Style?,
	state: string?,
	-- Names it among this mod's lists (default "list"); two need two ids.
	id: string?,
	count: number,
	-- Each row's height in pixels.
	row_height: number,
	row: (index: number) -> Child,
	gap: Space?,
	padding: Space?,
	grow: boolean?,
	width: number?,
	height: number?,
	surface: Surface?,
	border: Border?,
	shrink: boolean?,
	self_align: Align?,
	max_width: number?,
	max_height: number?,
	float: Float?,
}

-- A rectangle or line the host draws in a `ui.video`, in fractions of the
-- picture; a line has no `width` or no `height`.
type VideoGuide = {
	x: number,
	y: number,
	width: number?,
	height: number?,
	-- #rrggbb or #rrggbbaa.
	color: string?,
}

-- `ui.video { source = "camera" }`: frames from a ring in shared memory.
type VideoProps = {
	class: Class?,
	style: Style?,
	state: string?,
	source: string,
	id: string?,
	-- Device pixels per picture pixel, 1 being 100 %; nil fits it.
	zoom: number?,
	center: { number }?,
	filter: ("auto" | "nearest" | "linear")?,
	color: ("srgb" | "display-p3" | "rec709" | "rec2020" | "linear-srgb" | "linear-p3" | "none")?,
	extended: boolean?,
	-- Code values, then what multiplies and encodes them, in this order.
	black: number?,
	white: number?,
	gain: number?,
	gamma: number?,
	guides: { VideoGuide }?,
	-- A point to read, in fractions of the box, as `pointer` gives it.
	probe: { number }?,
	-- Sent each second with `fps`, `shown`, `skipped`, `latency`, `live`,
	-- `width`, `height`, `format`, `frame`, `fit` and `density` added.
	stats: Data?,
	-- Sent with `x`, `y`, `source`, `display` and `channel`? added.
	probed: Data?,
	width: number?,
	height: number?,
	tooltip: string?,
	context: Data?,
	copy: string?,
	to: string?,
	[number]: string,
}

-- Where a `ui.map` gets its tiles. `openstreetmap` and the `carto-*` maps
-- need no key; the others need the provider's API key in `key`.
type MapProvider =
	"openstreetmap"
	| "carto-light"
	| "carto-dark"
	| "carto-voyager"
	| "esri-streets"
	| "stadia-smooth"
	| "stadia-smooth-dark"
	| "maptiler-streets"
	| "mapbox-streets"
	| "thunderforest-transport"

-- A street map from a tile provider, with pins and routes over it. Drag to
-- pan; the wheel, a pinch, a two-finger double tap, a double click and its
-- buttons zoom; two fingers turn it. Without `center` and `zoom` it frames
-- the pins and routes. Its style's `color`
-- strokes routes without their own.
type MapProps = {
	class: Class?,
	style: Style?,
	provider: MapProvider?,
	-- The provider's API key, for those that need one.
	key: string?,
	-- { lat, lon } at the middle.
	center: { number }?,
	-- 0 (the whole world) to about 20 (a street).
	zoom: number?,
	-- A new `center`, `zoom` or `bearing` eases the map there instead of jumping,
	-- unless it is many screens away or animations are off.
	glide: boolean?,
	-- The compass direction up the box, in degrees clockwise from north. A
	-- new one turns the map where it stands. To draw a compass, feed the
	-- `bearing` from `move` back in, show it while that is not 0, and set
	-- this to 0 on a click.
	bearing: number?,
	pins: { MapPin }?,
	routes: { MapRoute }?,
	-- Sent with `lat`, `lon`, `zoom` and `bearing` (0 up to 360) added as
	-- the user moves the map.
	move: any?,
	-- Sent with `pin` (its index, from 1) added when a pin is clicked.
	select: any?,
	-- Sent once a `via` route's way along streets is known, with `route`
	-- (its index, from 1), `seconds` and `meters` added.
	routed: any?,
	to: string?,
	width: number?,
	height: number?,
	grow: boolean?,
}

type MapPin = {
	lat: number,
	lon: number,
	-- Up to three characters, drawn in the pin: "1", "A".
	label: string?,
	-- Drawn in the pin instead of a label.
	icon: IconName?,
	color: string?,
	-- "dot" on the place, "disc" on it (the default with a label or
	-- icon), or "balloon" above it with its tip on the place.
	shape: ("dot" | "disc" | "balloon")?,
	-- Drawn larger, above the others.
	selected: boolean?,
	-- A halo breathes out from it without end, in its color: where the
	-- user is. A still glow while animations are off.
	pulse: boolean?,
	-- Sent when it is clicked, instead of the map's `select`.
	msg: any?,
}

-- Either `points`, drawn as they are, or `via` stops the host routes along
-- streets (OpenStreetMap data, through routing.openstreetmap.de) by `mode`;
-- straight and faint until the way is known.
type MapRoute = {
	-- Corners in order, flat: { lat1, lon1, lat2, lon2, ... }.
	points: { number }?,
	-- Stops in order, flat, two to 25 of them.
	via: { number }?,
	mode: ("walk" | "bike" | "car")?,
	color: string?,
	stroke: number?,
	dashed: boolean?,
}

-- `ui.number`'s options: `decimals` fixes the digits after the point
-- (0 to 20); without it, up to three show and trailing zeros drop.
type NumberOptions = {
	decimals: number?,
}

-- `ui.date`'s options: how long (`style`, default "medium"), whether the
-- date shows (default true), and the time of day (true or "minutes",
-- "seconds"; default none).
type DateOptions = {
	style: ("short" | "medium" | "long")?,
	date: boolean?,
	time: (boolean | "minutes" | "seconds")?,
}

declare ui: {
	row: (props: StackProps) -> Element,
	column: (props: StackProps) -> Element,
	panel: (props: PanelProps) -> Element,
	hsplit: (props: SplitProps) -> Element,
	vsplit: (props: SplitProps) -> Element,
	pane: (props: PaneProps) -> Element,
	text: (props: string | TextProps) -> Element,
	button: (props: ButtonProps) -> Element,
	item: (props: ItemProps) -> Element,
	badge: (props: string | BadgeProps) -> Element,
	icon: (props: IconName | IconProps) -> Element,
	image: (props: string | ImageProps) -> Element,
	line: (props: LineProps) -> Element,
	paths: (props: PathsProps) -> Element,
	revision_timeline: (props: RevisionTimelineProps) -> Element,
	hatch: (props: HatchProps) -> Element,
	map: (props: MapProps) -> Element,
	terminal: (props: string | TerminalProps) -> Element,
	browser: (props: string | BrowserProps) -> Element,
	video: (props: string | VideoProps) -> Element,
	view: (props: string | ViewProps) -> Element,
	input: (props: InputProps) -> Element,
	markdown: (props: string | MarkdownProps) -> Element,
	divider: () -> Element,
	spacer: () -> Element,
	region: (props: string | RegionProps) -> Element,
	-- With an `id`, an instance of the mod with state of its own.
	mount: (name: string | MountProps) -> Element,
	popover: (props: PopoverProps) -> Element,
	overlay: (props: OverlayProps) -> Element,
	list: (props: ListProps) -> Element,
	-- What `build()` returned the last time this view asked for `key` with
	-- the same `deps` (compared with `==`), or `build()` now.
	memo: <T>(key: string, deps: { any }, build: () -> T) -> T,
	-- The message `id` (or `id.attribute`) of the app's `locales/<language>/
	-- *.ftl` in the user's language, with named arguments (strings, numbers
	-- or booleans); English when the language lacks it, `id` itself when
	-- English does too.
	t: (id: string, args: { [string]: any }?) -> string,
	-- `value` the language's way: `1,234.5`, `1 234,5`, `1.234,5`.
	number: (value: number, options: NumberOptions?) -> string,
	-- The moment `seconds` after 1970 (UTC) in the system's time zone, the
	-- language's way: `Nov 10, 2026`, `10 nov. 2026`.
	date: (seconds: number, options: DateOptions?) -> string,
	-- The language the app's words show in, as a BCP 47 tag: `en-US`, `fr`.
	locale: () -> string,
}

-- One commit of `app.git.log`.
type GitCommit = {
	sha: string,
	short: string,
	parents: { string },
	author: string,
	-- Seconds since 1970.
	time: number,
	-- Branches and tags pointing here.
	refs: { string },
	subject: string,
}

-- This machine's load, sampled by the host once a second.
type System = {
	-- Fractions from 0 to 1.
	cpu: number,
	-- Oldest first, ending with `cpu`.
	cpu_history: { number },
	memory: number,
	memory_used: number,
	memory_total: number,
	-- Bytes per second.
	received: number,
	transmitted: number,
}

type Platform = "macos" | "linux" | "windows"
type Titlebar = "system" | "hidden"

-- The window, for a title bar the layout draws itself.
type WindowState = {
	platform: Platform,
	-- `hidden` when the shell asked for it and the system agreed (a Linux
	-- compositor can insist on its own title bar).
	titlebar: Titlebar,
	maximized: boolean,
	fullscreen: boolean,
	-- The frontmost window.
	active: boolean,
	-- The screen it is on: an `id` from `app.displays`.
	display: number?,
}

-- A screen, in `app.displays`. `x`, `y`, `width` and `height` are its
-- place among the others, in points.
type DisplayInfo = {
	id: number,
	primary: boolean,
	x: number,
	y: number,
	width: number,
	height: number,
}

-- How a process or socket provider's link stands, in `app.links`.
type ProviderLink = {
	-- Whether the provider has its program.
	connected: boolean?,
	-- Messages not yet written.
	waiting: number,
	-- Messages dropped since it started: the queue was full or, for a
	-- socket, no one was there to hear them.
	dropped: number,
}

-- How `app:awake` stands, in `app.awake`.
type AwakeState = {
	-- Whether the system agreed to stay awake.
	held: boolean,
	-- Whether the display stays on too.
	display: boolean,
	-- Why it would not, if it would not.
	problem: string?,
}

-- The last run's panic, in `app.crash`.
type CrashReport = {
	-- What it said.
	message: string,
	-- The report's path, for the user to read or send.
	report: string,
}

-- Light, dark, or as the system is (switching as it does).
type AppearanceMode = "system" | "light" | "dark"

-- `app.appearance`: set it with `{ to = "app:appearance", msg = "dark" }`.
type Appearance = {
	-- What the user chose, kept across launches for the app and profile.
	mode: AppearanceMode,
	-- Whether the window is dark now (as `app.dark`).
	dark: boolean,
	-- The theme showing: `light_theme` or `dark_theme`, as the window is.
	theme: string,
	-- The theme light windows show and the one dark windows show, kept
	-- across launches; set with `{ to = "app:theme", msg = { dark = "Nord" } }`.
	-- "Default" is the stock look with the stylesheet's colors.
	light_theme: string,
	dark_theme: string,
	-- Every theme, for a picker of the app's own: "Default" first, once for
	-- light and once for dark, then the rest by name.
	themes: { ThemeInfo },
}

-- A theme as `app.appearance.themes` lists it. Colors are `#rrggbb`.
type ThemeInfo = {
	name: string,
	-- Made for dark windows: only such a theme can be the `dark_theme`.
	dark: boolean,
	background: string,
	foreground: string,
	accent: string,
	-- Six of its colors: red, green, yellow, blue, magenta, cyan.
	colors: { string },
}


-- `app.git`, from providers/git. When `available` is false only `reason`
-- is set (and `repository = false`); every other field is there otherwise.
type GitFile = {
	path: string,
	-- "M", "A", "D", "R" (renamed or copied), "?" (untracked) or "U" (conflict).
	status: string,
	staged: boolean,
	unstaged: boolean,
	-- Lines against HEAD; 0 for binary and untracked files.
	added: number,
	deleted: number,
	-- Renames only.
	old_path: string?,
}

type GitWorktree = {
	path: string,
	-- "" when detached.
	branch: string,
	head: string,
	current: boolean,
	-- Files with changes.
	changed: number,
	ahead: number,
	behind: number,
	detached: boolean,
	locked: boolean,
	prunable: boolean,
	bare: boolean,
}

type GitBranch = {
	name: string,
	-- Short sha.
	head: string,
	-- Unix seconds of its last commit.
	time: number,
	upstream: string?,
	ahead: number,
	behind: number,
	-- The upstream was deleted.
	gone: boolean,
	current: boolean,
}

type GitRemote = {
	name: string,
	url: string,
	host: string,
	-- "github", "gitlab" or "other".
	kind: string,
	owner: string,
	repo: string,
	-- https://host/owner/repo, when the url names a host.
	web_url: string?,
}

type GitDiffLine = {
	-- "add", "del" or "ctx".
	kind: string,
	-- Line numbers in the old and new file; `old` is nil on "add" lines,
	-- `new` on "del" ones.
	old: number?,
	new: number?,
	-- Cut to 300 characters.
	text: string,
}

type GitDiff = {
	path: string,
	binary: boolean,
	-- Lines in all hunks; at most 1200, then `truncated`.
	lines: number,
	truncated: boolean,
	hunks: { { header: string, old_start: number, new_start: number, lines: { GitDiffLine } } },
}

type Git = {
	available: boolean,
	reason: string?,
	repository: boolean,
	-- "HEAD" when detached.
	branch: string,
	-- Files with changes, staged, unstaged or untracked.
	changed: number,
	ahead: number,
	behind: number,
	root: string,
	detached: boolean,
	-- Nil before the first commit.
	head: { sha: string, short: string, subject: string, author: string, email: string, time: number }?,
	-- "origin/main", or nil without one.
	upstream: string?,
	-- Lines in the working tree against HEAD, tracked files only.
	added: number,
	deleted: number,
	-- At most 300, then `files_truncated`.
	files: { GitFile },
	files_truncated: boolean,
	worktrees: { GitWorktree },
	-- Recent local branches, newest first, at most 20.
	branches: { GitBranch },
	remote: GitRemote?,
	stashes: number,
	-- Once a mod sends `provider:git` `{ log = 200 }`: recent commits of every
	-- branch, tag and HEAD, children before parents.
	log: { GitCommit }?,
	-- The file asked for with `{ diff = path }`, until `{ diff = false }`.
	diff: GitDiff?,
}

-- A forge account; `avatar_path` is a cached image file once downloaded.
type ForgePerson = {
	login: string,
	name: string?,
	bot: boolean?,
	avatar_url: string?,
	avatar_path: string?,
}

type ForgeChecks = {
	total: number,
	passed: number,
	failed: number,
	pending: number,
	skipped: number,
}

-- A pull request (GitHub) or merge request (GitLab).
type ForgePr = {
	number: number,
	title: string,
	-- "open", "closed" or "merged".
	state: string,
	draft: boolean,
	url: string,
	branch: string,
	base: string,
	author: ForgePerson?,
	-- Unix seconds.
	updated: number?,
	-- "approved", "changes_requested", "review_required" or "".
	review_decision: string,
	-- "mergeable", "conflicting", "unknown" or "".
	mergeable: string,
	-- The forge's own merge state, lowercase ("clean", "blocked", ...).
	merge_state: string,
	checks: ForgeChecks,
	-- The badge: "draft", "waiting", "ready", "attention", "merged",
	-- "closed" or "held".
	tone: string,
	-- Lines added and removed; on GitLab only for `current_pr`.
	additions: number?,
	deletions: number?,
	-- Files changed; GitHub only.
	files: number?,
	-- `current_pr` only: each check by name, at most 50; `outcome` is
	-- "passed", "failed", "pending" or "skipped".
	check_list: { { name: string, outcome: string, url: string? } }?,
}

type ForgeIssue = {
	number: number,
	title: string,
	url: string,
	author: ForgePerson?,
	updated: number?,
	labels: { { name: string, color: string? } },
	assignees: { string },
}

-- Where a forge's sign-in stands. The app draws it as it likes and
-- sends `provider:github` (or `provider:gitlab`) `{ sign_in = true }`,
-- `{ token = "..." }`, `{ use_cli = true }`, `{ cancel = true }` or
-- `{ sign_out = true }`, from a mod in the provider's `accounts`.
type ForgeAuth = {
	-- "checking", "signed_out", "pending" (a device code is up),
	-- "verifying" or "signed_in".
	status: string,
	-- The host signed in to: the remote's, else github.com or gitlab.com.
	host: string,
	login: string?,
	-- "saved", "env" or "cli".
	source: string?,
	-- Whether a device sign-in is set up (the provider has a client id).
	device: boolean,
	-- Whether the CLI (gh, glab) is signed in and could be used.
	cli: boolean,
	-- While pending: the code to enter, where, the page with it filled
	-- in (GitLab), and when it expires (Unix seconds).
	code: string?,
	url: string?,
	complete_url: string?,
	expires: number?,
	-- What went wrong last, as the user's next step.
	error: string?,
}

-- `app.github` and `app.gitlab`. `auth` and `host` are always there; when
-- `available` is false the rest is not, only `reason`.
type Forge = {
	available: boolean,
	reason: string?,
	host: string,
	-- Whether the checkout's remote belongs to this configured provider,
	-- including self-hosted GitLab, even before sign-in succeeds.
	matches_remote: boolean,
	auth: ForgeAuth,
	repo: { name_with_owner: string, url: string, default_branch: string? },
	viewer: ForgePerson?,
	-- The current branch's newest, open or not, or nil.
	current_pr: ForgePr?,
	-- The newest pull request of each branch the worktrees have out (the
	-- current one first, at most 20), by branch name.
	branches: { [string]: ForgePr }?,
	-- Open ones, most recently updated first, at most 30.
	prs: { ForgePr },
	issues: { ForgeIssue },
}

-- A coding-agent CLI running on this machine.
type Agent = {
	pid: number,
	ppid: number,
	-- "claude", "codex", "gemini", "cursor", "opencode", "aider", "amp",
	-- "copilot", "droid", "kiro", "qwen", "goose", "crush", ...
	name: string,
	-- Its command line, cut to 200 characters.
	command: string,
	cwd: string?,
	tty: string?,
	-- Unix seconds.
	started: number,
	elapsed_seconds: number,
	-- Percent of one core.
	cpu: number,
	-- Always "running": what it is doing is not visible from outside.
	status: string,
}

type Agents = {
	available: boolean,
	reason: string?,
	agents: { Agent },
	count: number,
}

-- One entry of a folder, from the `files` provider.
type FileEntry = {
	name: string,
	dir: boolean,
	-- Ignored by git: editors dim these.
	ignored: boolean,
}

type Files = {
	-- False only when the folder can't be read; then only `reason` comes
	-- with it. Outside any repository it lists home.
	available: boolean,
	reason: string?,
	-- The root folder's name (`~` for home), and its absolute path.
	root: string,
	path: string,
	-- Where a terminal starts: the project of the terminal last used,
	-- else the folder `moonframe` was started from, else `path`.
	cwd: string?,
	-- Entries of the root (key "") and of each open folder (its path from
	-- the root), folders first, each group sorted without regard to case.
	dirs: { [string]: { FileEntry } },
}

-- What this machine has for keeping terminals (the `herdr` provider).
-- The `docs` provider: the folder whose text files it reads and saves for
-- the mod that asks (`{ read = path }`, `{ write = path, text, version }`).
type Docs = {
	available: boolean,
	reason: string?,
	root: string?,
}

-- One limit window of a service's plan (the `usage` provider). Shares are
-- 0 to 1; `pace` is where an even spend would have left the bar of what is
-- left (`usage-bar`'s `pace`), nil early in a window or without a reset.
type UsageWindow = {
	title: string,
	kind: "session" | "daily" | "weekly" | "monthly" | "named",
	used: number,
	left: number,
	severity: "normal" | "warning" | "critical",
	-- "3% used 2h 53m", for a status bar.
	label: string,
	resets_at: number?,
	-- "4d 6h", "2h 53m", "47m".
	resets_in: string?,
	pace: number?,
	-- "11% in reserve · Lasts until reset".
	pace_note: string?,
}

type UsageBalance = {
	label: string,
	amount: number,
	total: number?,
	-- An ISO 4217 code when `currency`, else the service's own unit name.
	unit: string,
	currency: boolean,
	-- "$12.30", "$5.00 of $20.00", "1,250 credits".
	text: string,
	-- amount / total, clamped to 0..1, when total > 0. The label says
	-- whether the amount is spent or remaining; this is not always credit left.
	fraction: number?,
}

type UsageSection =
	{ kind: "limit", window: UsageWindow }
	| { kind: "facts", title: string, facts: { { label: string, value: string } } }
	| { kind: "shares", title: string, shares: { { label: string, share: number } } }

-- One service signed in on the host, or one `show_providers` asks for.
type UsageProvider = {
	id: string,
	name: string,
	-- A bundled logo (`agent-claude`), nil when the app has none for it.
	icon: IconName?,
	dashboard: string?,
	status_page: string?,
	email: string?,
	plan: string?,
	-- Why the latest read failed; with `stale`, the numbers are older ones.
	error: string?,
	stale: boolean,
	-- The sign-in sits behind a macOS Keychain prompt: send `provider:usage`
	-- `{ allow = id }` when the user agrees, and macOS asks.
	access: ("needed" | "denied")?,
	-- The tightest window's share used, and how alarming it is.
	used: number?,
	severity: ("normal" | "warning" | "critical")?,
	windows: { UsageWindow },
	balances: { UsageBalance },
	sections: { UsageSection },
	-- OpenAI's spend per day.
	daily: { { day: string, amount: number } }?,
	-- Claude's other accounts, from cswap.
	accounts: { { name: string, email: string, disabled: boolean, stale: boolean, note: string?, windows: { UsageWindow } } }?,
}

-- Plan usage of the AI services signed in on `host` (the `usage` provider),
-- read every 10 minutes at most. Any message to `provider:usage` refreshes.
type Usage = {
	available: boolean,
	-- "local", "ssh:<target>" or "wsl:<distro>".
	host: string?,
	busy: boolean,
	updated: number?,
	-- Whole minutes since `updated`.
	age: number?,
	-- Why the host itself could not be read.
	error: string?,
	-- The sheet's tabs, in a stable order.
	providers: { UsageProvider },
	-- Ids of those with numbers, closest to a limit first: a status bar's.
	headline: { string },
}

type Herdr = {
	installed: boolean,
	version: string?,
	path: string?,
	tmux: boolean,
	-- herdr-core's installer is running.
	installing: boolean,
	-- Why the last install failed.
	error: string?,
	-- Installed during this run.
	fresh: boolean,
}

-- `app.acp`, from providers/acp: coding agents run for mods over the
-- Agent Client Protocol.
type AcpAgent = {
	-- "claude", "codex", "copilot", "gemini" or "opencode".
	name: string,
	title: string,
	-- Its ACP command is installed.
	installed: boolean,
	-- Only `npx` can start it, downloading its adapter first.
	fetched: boolean,
}

type AcpTool = {
	id: string,
	title: string,
	-- "read", "edit", "delete", "move", "search", "execute", "think", "fetch", "other"...
	kind: string,
	-- "pending", "in_progress", "completed" or "failed".
	status: string,
	-- How long it took, in milliseconds, once it ended.
	ms: number?,
}

type AcpRun = {
	id: string,
	-- The mod that started it: the only one it obeys.
	owner: string,
	agent: string,
	agent_title: string,
	-- The user's words.
	text: string,
	-- The run it follows up (`thread` in its start): the first of a
	-- conversation.
	thread: string?,
	-- "starting", "running", "waiting" (a question for the user), "done",
	-- "failed", "cancelled" or "stopped" (the app quit while it ran).
	status: string,
	-- Unix seconds.
	started: number,
	finished: number?,
	error: string?,
	stop_reason: string?,
	-- The agent's latest message: its answer once it is done.
	message: string?,
	-- The latest tool calls (at most 12), newest last, of `tool_count`.
	tools: { AcpTool },
	tool_count: number,
	plan: { { content: string, status: string } },
	-- A permission question waiting on the user; answer with
	-- `{ answer = id, option = option.id }` (or `option = false`).
	question: { title: string, options: { { id: string, name: string, kind: string } } }?,
	-- It left the app's folders different; undo it with `{ undo = id }`.
	changed: boolean,
	-- The app was taken back to before it: by the user, or because the
	-- change broke it (`warning` says so).
	undone: boolean,
	-- What the user should know beyond the agent's answer.
	warning: string?,
	-- The app is being checked after a run on it (`app = true`).
	checking: boolean?,
	-- Milliseconds from its start: when the agent started, shook hands,
	-- opened its session, first spoke, ended its turn, and how long the
	-- app's check took after it (and before it, `baseline`). `warm` when it
	-- took an agent started ahead (`{ warm = true }`).
	timing: AcpTiming?,
}

type AcpTiming = {
	spawned: number?,
	initialized: number?,
	session: number?,
	first_token: number?,
	ended: number?,
	check: number?,
	baseline: number?,
	warm: boolean,
}

-- The app as the window says an agent changes it.
type AcpTarget = {
	title: string,
	-- Its own folder, the one agents write in (the user's folder over it,
	-- when it is no one's source: `overlay`), and the folder they work in.
	app: string,
	writable: string,
	workdir: string,
	overlay: boolean,
	-- The shell command that checks it headlessly, in about a second.
	check: string,
	-- The folders the window runs on, lowest first: `toolbox`, `package`,
	-- `app`, `user`.
	layers: { { role: string, path: string } },
}

type Acp = {
	available: boolean,
	agents: { AcpAgent },
	-- The agent a start without `agent` gets.
	agent: string,
	-- The folder agents work in.
	workdir: string,
	-- The app folder they change, from `workdir` (`app`, `examples/ledger`),
	-- when it is inside it, and the whole path.
	app: string?,
	writable: string?,
	-- What each mod's runs do without asking (`[settings] allow`), by mod:
	-- "ask", "read", "app" or "all".
	allow: { [string]: string },
	-- The app as the window says an agent changes it.
	target: AcpTarget?,
	-- This app's process: an agent screenshots the window with it.
	pid: number,
	running: number,
	-- Newest first, at most 30.
	runs: { AcpRun },
}

-- `app.speech`, from providers/speech: dictation, for `use "dictation"`.
type SpeechEngine = {
	name: string,
	title: string,
	kind: "apple" | "command" | "remote",
	-- Whether the user's voice leaves this machine.
	remote: boolean,
	ready: boolean,
	-- What the user can do about it, when it is not ready.
	reason: string?,
}

type Speech = {
	-- Some engine in `prefer` is ready; `reason` says why not otherwise.
	available: boolean,
	reason: string?,
	-- The engine a start without `engine` gets.
	engine: string?,
	engines: { SpeechEngine },
	state: "idle" | "recording" | "transcribing",
	-- The mod listening. What it hears goes only to that mod.
	owner: string?,
	-- How loud it is, from 0 to 1, and how long it has listened.
	level: number,
	seconds: number,
}

-- A native provider's answer to an action a mod asked for, arriving in that
-- mod's `update`: `reply` is the `id` the mod sent, else the action's name.
type Reply = {
	reply: any,
	action: string,
	ok: boolean,
	-- What happened, when it worked; why not, when it did not.
	message: string?,
	error: string?,
}

-- `ui.mount { "explorer", id = "files-2", props = { ... } }`.
type MountProps = {
	mod: string?,
	id: string?,
	props: any,
	[number]: string,
}

-- What draws this build.
type Renderer = "gpui" | "rune"

-- What every `view` gets as `app`.
type App = {
	dark: boolean,
	-- Absent where nothing draws, as in `--check`.
	renderer: Renderer?,
	appearance: Appearance,
	window: WindowState,
	-- Every screen, the primary one first.
	displays: { DisplayInfo }?,
	-- Each process or socket provider that has had something to report.
	links: { [string]: ProviderLink }?,
	-- Nil until a mod first asks `app:awake`.
	awake: AwakeState?,
	-- Nil unless the last run panicked.
	crash: CrashReport?,
	-- How each system-wide shortcut mods claimed (`app:hotkey`) stands, by
	-- its keys; nil until a mod first claims one. "taken" when another app
	-- holds them for itself alone.
	hotkeys: { [string]: "claimed" | "taken" | "unsupported" }?,
	-- Cargo feature names mapped to whether this host compiled them in.
	capabilities: { [string]: boolean },
	mods: { string },
	regions: { [string]: { string } },
	-- Nil until the first sample arrives (from the `system` provider).
	system: System?,
	-- Nil until each provider first publishes.
	git: Git?,
	github: Forge?,
	gitlab: Forge?,
	agents: Agents?,
	files: Files?,
	docs: Docs?,
	herdr: Herdr?,
	acp: Acp?,
	speech: Speech?,
	usage: Usage?,
	-- What tab bars can open: each component and mod with a `tab` entry.
	kinds: { TabKind },
	-- The stylesheet's `looks`: which look each kind of component takes.
	looks: { [string]: string },
	-- What each provider in app/providers/ publishes, as `app.<name>`.
	[string]: any,
}

-- What `{ to = "app:notify", msg = ... }` posts. A click on it brings the
-- window forward and sends the posting mod, if it subscribes to
-- `app/notification`, `{ topic = "app/notification", value = NotificationClick }`.
type NotifyMsg = {
	-- At most 256 bytes.
	title: string,
	-- At most 2048 bytes.
	body: string?,
	-- 1 to 128 of a-z A-Z 0-9 - _ . : /, handed back on a click.
	id: string?,
}

type NotificationClick = {
	id: string?,
	clicked: boolean,
}

-- What `{ to = "app:pick", msg = ... }` opens: the system's panel to open
-- files, pick folders, or name a file to save. The answer comes back to
-- the mod as `reply` with `paths` (a list; `path` for a save) or
-- `cancelled = true` added. What the user picks may show in that mod's
-- `ui.image` (and its components').
type PickMsg = {
	kind: ("open" | "folder" | "save")?,
	reply: Data,
	multiple: boolean?,
	-- The panel's button or title, such as "Open scan".
	prompt: string?,
	-- Extensions files to open must have, such as { "tif", "png" }.
	types: { string }?,
	-- A save panel's suggested name and starting folder.
	name: string?,
	folder: string?,
}

-- What `{ to = "app:status-item", msg = ... }` shows: the mod's item in the
-- menu bar (macOS), changed when asked again; `false` takes it away, and
-- `{ popup = { StatusLine } }` drops a menu down from it at once.
type StatusItemMsg = {
	-- A system symbol's name (SF Symbols), such as "tray.and.arrow.down".
	symbol: string?,
	-- Or an image file, found as `app:icon` finds one.
	image: string?,
	-- Drawn in the menu bar's colour (default true).
	template: boolean?,
	-- At most 64 bytes beside the icon, or alone.
	title: string?,
	tooltip: string?,
	items: { StatusLine }?,
}

-- A line of a menu bar item's menu: "-", or an item whose click sends `msg`
-- to the mod (or `to`). Without `msg` it is a heading. `shortcut` only shows.
type StatusLine = "-" | {
	label: string,
	msg: Data?,
	to: string?,
	enabled: boolean?,
	checked: boolean?,
	shortcut: string?,
}

-- What `{ to = "app:hotkey", msg = ... }` claims: `keys` system-wide, each
-- press sending `msg` to the mod (or `to`) whichever app is in front; or,
-- with `release = true`, gives them back. `keys` has two modifiers, not
-- both shift ("ctrl-alt-l"), or is a function key.
type HotkeyMsg = {
	keys: string,
	msg: Data?,
	to: string?,
	release: boolean?,
}

-- What `{ to = "app:edit", msg = ... }` does to the sending mod's input
-- with `id = field`, around its selection, keeping undo and focus; the
-- field then sends its `change`. `wrap` puts `{ before, after }` around the
-- selection (taking them off when already there), `lines` starts each
-- selected line with a prefix (taking it off when all have it), `insert`
-- replaces the selection and the `back` bytes before it: an `@name`
-- completed, an emoji. A toolbar button can send it directly with
-- `to = "app:edit"`.
type EditMsg = {
	field: string,
	wrap: { string }?,
	lines: string?,
	insert: string?,
	back: number?,
	-- For completions: apply only at this empty selection with this exact
	-- preceding text. Give both; `back` is the byte length of `expect`.
	at: number?,
	expect: string?,
}

-- What `{ to = "app:window", msg = ... }` asks: a mod's name opens that
-- mod in a window of its own (or raises it), as a control's `window` does;
-- `{ open = "<mod>", send = <msg> }` first delivers `send` to its `update`,
-- so it opens on a page; `{ close = "<mod>" }` closes it. Any mod may ask
-- for any mod's window; never the shell's.
type WindowMsg = string | { open: string, send: Data? } | { close: string }

-- What the Developer tools (mods/devtools, and only it) ask with
-- `{ to = "app:devtools", msg = ... }`: where they show; a Luau file of the
-- app's folders to read (answered with `text`) or save (with `ok`, or
-- `error`); a mod's state (with `state`, `problem`); or another element of
-- the selection's trail to select. Answers come back as `reply` with fields
-- added; picks arrive on `app/selection`, the place on `app/devtools`.
type DevtoolsMsg =
	{ place: "dock" | "window" | "closed" }
	| { read: string, reply: Data? }
	| { save: string, text: string, reply: Data? }
	| { state: string, reply: Data? }
	| { select: string }

-- A message `update` sends on. `to` is a mod, `"parent"`, `"shell"`,
-- `"provider:<name>"`, `"topic:<name>"`, or the app itself:
-- `"app:command"` (`msg` a `Command`), `"app:appearance"` (`msg` an
-- `AppearanceMode`), `"app:theme"` (`msg` `{ light: string?, dark: string? }`),
-- `"app:sound"` (`msg` `"default"` or a sound file's path),
-- `"app:icon"` (`msg` `"default"` or an image file's path: the app's icon
-- in the Dock, changed at once until the app quits; macOS only),
-- `"app:notify"` (`msg` a `NotifyMsg`), `"app:pick"` (`msg` a `PickMsg`),
-- `"app:edit"` (`msg` an `EditMsg`), `"app:window"` (`msg` a `WindowMsg`),
-- `"app:devtools"` (`msg` a `DevtoolsMsg`), `"app:status-item"` (`msg` a
-- `StatusItemMsg`), `"app:hotkey"` (`msg` a `HotkeyMsg`) and `"app:open"` (`msg` a web address, opened in the system's browser).
type Send = {
	to: string,
	msg: Data,
}

-- `titlebar = "hidden"` puts the layout under the title bar (macOS keeps its
-- traffic lights, at `traffic_lights` from the top left) or drops the system
-- frame (Linux, Windows). Give the layout a `drag = true` row to move it.
type Window = {
	titlebar: Titlebar?,
	traffic_lights: { x: number?, y: number? }?,
	-- The size the window opens at, before the user resizes it.
	size: { width: number?, height: number? }?,
	-- Reopen where and as large as the window last was (the default), kept
	-- within the screen; false opens it at `size`, centered, every launch.
	remember: boolean?,
}

-- A mod's own window (`window` on a mod other than main.luau), when a
-- control's `window` or `app:window` opens it: the size it opens at and
-- the smallest the user can make it, each kept within the screen; 420x560
-- and 280x240 when unset.
type ModWindow = {
	size: { width: number, height: number }?,
	min_size: { width: number, height: number }?,
	-- The screen it opens on: an `id` from `app.displays`. "external" is the
	-- first screen that isn't the primary one, or the primary one if alone.
	display: ("primary" | "external" | number)?,
	-- Opens filling its screen.
	fullscreen: boolean?,
	titlebar: Titlebar?,
	traffic_lights: { x: number?, y: number? }?,
}

-- A menu item in the shell's `menus`: it sends `msg` (to the shell, or to
-- `to`) or runs `command`; `shortcut` is a key as `keys` take them, such as
-- "secondary-t". "-" between items draws a line.
type MenuItem = {
	label: string,
	msg: Data?,
	to: string?,
	command: Command?,
	shortcut: string?,
}

-- `items` holds `MenuItem`s and "-"; typed loosely so items with `msg` and
-- with `command` mix in one list. The host checks each.
type AppMenu = {
	title: string,
	items: { any },
}

-- What the About panel says besides the app's name and icon.
type About = {
	name: string?,
	version: string?,
	description: string?,
	website: string?,
	copyright: string?,
}

-- What a component or mod offers tab bars (`tabbed`): its + menu lists it,
-- and a new tab of it starts with `props`. `report` names the prop a tab
-- passes as a message, whose `url` and `title` the tab keeps (a browser's
-- `navigate`).
type TabEntry = {
	title: string?,
	icon: IconName?,
	props: any,
	report: string?,
}

-- One of `app.kinds`: what a tab bar can open.
type TabKind = {
	-- "component:terminal" or "mod:explorer".
	kind: string,
	title: string,
	icon: string?,
	props: any,
	report: string?,
}

type Mod<State> = {
	-- `props` is what an instance was mounted with (`ui.mount { "name",
	-- id = ..., props = ... }`), nil for a mod in its own place. `restored`
	-- says `previous` was saved by an earlier run, not kept across a reload.
	init: ((previous: State?, props: any?, restored: boolean) -> State)?,
	-- Returns nothing to keep the state it changed in place, or the new
	-- state and optionally a list of `Send`s. Its results are left untyped
	-- so it may return nothing on some paths; the host checks them. A
	-- subscribed topic arrives as `{ topic = "<name>", value = ... }`.
	update: ((state: State, msg: any, props: any?) -> ...any)?,
	view: (state: State, app: App, props: any?) -> Child,
	-- Topics it hears in `update`: names, or prefixes ending in `*`.
	-- `provider/<name>` carries a provider's data as it changes.
	subscribe: { string }?,
	-- Keys it takes anywhere in the window once nothing nearer does (an
	-- open overlay, a terminal; none while a text field has focus):
	-- `{ ["secondary-1"] = "histogram" }`; every mounted copy hears them
	-- too. Keys that type text need ctrl, alt, cmd or secondary.
	keys: Keys?,
	-- Offered as a kind of tab: each tab mounts an instance of it.
	tab: TabEntry?,
	region: string?,
	order: number?,
	title: string?,
	-- Rules for what this mod draws, by selector.
	styles: { [string]: Style }?,
	-- In main.luau, the main window's frame, read when the app starts (a
	-- `Window`); in another mod, the size of its own window (a `ModWindow`).
	window: (Window | ModWindow)?,
	-- Only in main.luau: menus after the app menu (a "View" one merges into
	-- the host's), and what About says.
	menus: { AppMenu }?,
	about: About?,
}

declare function mod<State>(definition: Mod<State>): Mod<State>

-- The fixed-width font terminals and `font = "mono"` text use.
type MonoFont = {
	family: string?,
	size: number?,
	-- 100 (thin) to 900 (black); 400 is regular.
	weight: number?,
	-- Terminal line height as a multiple of the size.
	line_height: number?,
	-- Fonts to look in for characters the family lacks, such as Nerd Font
	-- icons; by default { "Symbols Nerd Font Mono" }, and {} for none.
	fallbacks: { string }?,
	-- false keeps sequences such as `->` from joining.
	ligatures: boolean?,
}

type Stylesheet = {
	font_size: number?,
	font_family: string?,
	mono: MonoFont?,
	radius: number?,
	-- The look each kind of component takes unless its code picks one:
	-- `{ tabs = "cards" }`. Views read it as `app.looks`.
	looks: { [string]: string }?,
	-- Each one hex color, or `{ light = ..., dark = ... }`. Typed `any`:
	-- Luau cannot check a map of unions against a table literal, so the host
	-- checks each entry when it loads the stylesheet.
	colors: { [string]: any }?,
	-- Named durations: milliseconds, or `"220ms"`; `any` as for colors.
	speeds: { [string]: any }?,
	-- Divides every duration: 2 is twice as fast, 0 turns animations off;
	-- otherwise 0.01 to 10.
	animation_speed: number?,
	-- Styles by selector: `panel`, `.class`, `item:selected`.
	rules: { [string]: Style }?,
}

declare function style(sheet: Stylesheet): Stylesheet

-- A component in components/<name>.luau: a ready-made piece of view that
-- mods `use`, listed in the Components gallery with a preview of `example`.
type Component<Props> = {
	title: string?,
	description: string?,
	-- The heading it is listed under in the Components gallery.
	category: string?,
	-- When to reach for it, and what to use instead; the catalogue's lead.
	when: string?,
	-- Each prop and what it does, for the catalogue.
	props: { [string]: string }?,
	-- Default rules for the classes it draws, under every mod's and the
	-- stylesheet's, so `style.luau` restyles it.
	styles: { [string]: Style }?,
	-- Shown beside its title in the browser.
	icon: IconName?,
	-- Luau that uses it; by default made from `example`.
	usage: string?,
	-- Props for the preview.
	example: any,
	-- With `update`, it keeps state: each `use` call with an `id` mounts an
	-- instance, and `build` gets `(props, state, app)`.
	build: (props: Props, state: any?, app: App?) -> Child,
	init: ((previous: any?, props: Props, restored: boolean) -> any)?,
	-- Sends `to = "parent"` reach the module that drew it.
	update: ((state: any, msg: any, props: Props) -> ...any)?,
	subscribe: { string }?,
	tab: TabEntry?,
}

declare function component<Props>(definition: Component<Props>): Component<Props>

-- The component `name`'s `build`: `local stat = use "stat"`, then
-- `stat { value = "12%" }` wherever an element goes.
declare function use(name: string): (props: any) -> Element

-- Prints to the terminal that runs the app.
declare function log(...: any): ()
