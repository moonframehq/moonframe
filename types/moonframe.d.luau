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
	| "share"
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
	-- With `scroll`: sent as the reader scrolls, with `fraction` added, from
	-- 0 (the top) to 1 (the end at the bottom). Not sent for `scroll_at`.
	scrolled: Data,
	-- With `scroll`: scrolls to `fraction` (0 to 1) when it changes; a new
	-- `seq` goes to one place again. A source and its preview follow each
	-- other with `scrolled` and `scroll_at`.
	scroll_at: { fraction: number, seq: number? }?,
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
	-- With `multiline`: sent as the text scrolls (the wheel, the scrollbar,
	-- the caret carrying it), with `fraction` added, from 0 (the top) to 1
	-- (the last line at the bottom). Not sent for `scroll_at`.
	scrolled: Data,
	-- With `multiline`: scrolls the text to `fraction` (0 to 1) when it
	-- changes; a new `seq` goes to one place again.
	scroll_at: { fraction: number, seq: number? }?,
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

-- Where a `ui.map` gets its tiles. `openstreetmap`, the `carto-*` maps and
-- the `vector-*` maps need no key; the others need the provider's API key in
-- `key`. The `vector-*` maps are OpenFreeMap vector tiles drawn on the GPU by
-- gpui-maps, in builds with the `map-vector` feature.
type MapProvider =
	"openstreetmap"
	| "carto-light"
	| "carto-dark"
	| "carto-voyager"
	| "esri-streets"
	| "vector-liberty"
	| "vector-positron"
	| "vector-streets"
	| "vector-dark"
	| "vector-graphite"
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

-- One commit with what it changed, against its first parent (a first
-- commit against nothing).
type GitShown = {
	sha: string,
	short: string,
	parents: { string },
	author: string,
	email: string,
	-- Seconds since 1970.
	time: number,
	refs: { string },
	subject: string,
	-- The message past its subject, or "".
	body: string,
	-- "<status>\t<added>\t<deleted>\t<path>", status A, D, M or R; at most
	-- 400, then `truncated`, with `count` all of them.
	files: { string },
	count: number,
	truncated: boolean,
	added: number,
	deleted: number,
	-- One of its files, asked for with `{ show_diff = path }`, until
	-- `{ show_diff = false }`.
	diff: GitDiff?,
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
	-- The commit asked for with `{ show = sha }`, until `{ show = false }`.
	commit: GitShown?,
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
	-- The app's name, as its menu bar shows it ("Moonframe", or the
	-- title of the app running as a profile or of the host app). Absent
	-- where nothing draws, as in `--check`.
	title: string?,
	appearance: Appearance,
	window: WindowState,
	-- Every screen, the primary one first.
	displays: { DisplayInfo }?,
	-- Each process or socket provider that has had something to report.
	links: { [string]: ProviderLink }?,
	-- Nil until a mod first asks `app:awake`.
	awake: AwakeState?,
	-- Whether the user lets the app post notifications (`app:notify`);
	-- nil until the system says, read again as a window comes forward.
	notifications: NotificationPermission?,
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
-- `{ ask = true }` instead posts nothing and asks the user to allow
-- notifications now (once; the first one posted asks otherwise), with
-- `app.notifications` saying the answer.
type NotifyMsg = {
	-- At most 256 bytes.
	title: string,
	-- At most 2048 bytes.
	body: string?,
	-- 1 to 128 of a-z A-Z 0-9 - _ . : /, handed back on a click.
	id: string?,
	-- Takes the place of the ones this mod posted with the same `id`
	-- (this run or an earlier one) rather than adding to them. Needs `id`.
	replace: boolean?,
} | { ask: true }

-- Whether the user lets the app post notifications, in
-- `app.notifications`: "in-window" where the system shows none (Linux,
-- Windows, a bare binary) and a toast shows them; "denied" where the user
-- turned them off, toasts showing them too; "undecided" until the user is
-- asked.
type NotificationPermission = "allowed" | "denied" | "undecided" | "in-window"

type NotificationClick = {
	id: string?,
	clicked: boolean,
}

-- A link to one of the app's URL schemes (`schemes` in its moonframe.toml),
-- opened in a browser or another app: a module that subscribes to `app/url`
-- hears `{ topic = "app/url", value = AppUrl }`. Anyone can open one: treat
-- what it carries as untrusted.
type AppUrl = {
	-- The whole link, as opened.
	url: string,
	-- Lowercase, one the app claims.
	scheme: string,
	-- `notes://host/path` has one; `notes:/path` has none.
	host: string?,
	path: string,
	-- Each parameter decoded, the first of a name.
	query: { [string]: string },
	fragment: string?,
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

-- What `{ to = "app:share", msg = ... }` offers to the system's share menu
-- (macOS), at the pointer. Files must be the app's (its folders, the image
-- cache `~/.cache/<app>/images` or the share cache `~/.cache/<app>/share`)
-- or picked by the user for the mod. With `reply`, the mod hears it
-- back with `service` (the name of the one picked) or `cancelled = true`.
type ShareMsg = {
	-- A web address: https:// or http://.
	url: string?,
	-- At most 4096 bytes, beside the rest.
	text: string?,
	-- At most 20, named as `ui.image` names pictures.
	files: { string }?,
	reply: Data?,
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

-- What `{ to = "app:pdf", msg = ... }` makes: a page as a PDF file (macOS).
-- One of `html`, `html_file` and `url`. With `reply`, the mod hears it back
-- with `path` (the file written) or `error`.
type PdfMsg = {
	-- The page's text: no script, nothing from the network but `data:`.
	html: string?,
	-- An `.html` file under the app's cache or state folders, shown as `html`.
	html_file: string?,
	-- An https:// page on the internet, in a session of its own, never
	-- this machine or its network (macOS 14 or later).
	url: string?,
	-- Where: relative to the app's cache folder, or under its cache or state.
	pdf: string,
	-- The system's paper unless given.
	paper: ("a4" | "letter")?,
	reply: Data?,
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
-- so it opens on a page; `size` opens it at that size over its `window`
-- key's, and grows it to that size if it is open (never shrinking it);
-- `{ close = "<mod>" }` closes it. Any mod may ask for any mod's window;
-- never the shell's.
type WindowMsg = string | { open: string, send: Data?, size: { width: number, height: number }? } | { close: string }

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
-- `"app:devtools"` (`msg` a `DevtoolsMsg`), `"app:share"` (`msg` a `ShareMsg`), `"app:status-item"` (`msg` a
-- `StatusItemMsg`), `"app:hotkey"` (`msg` a `HotkeyMsg`), `"app:pdf"` (`msg` a `PdfMsg`) and `"app:open"` (`msg` a web address, opened in the system's browser).
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

-- generated: component props (just component-types) start
type UseAboutLink = any
type UseAboutProps = {
	open: boolean?,
	dismiss: any,
	name: string,
	version: string?,
	tagline: string?,
	icon: string?,
	image: string?,
	info: { { string } }?,
	links: { UseAboutLink }?,
	copyright: string?,
	copy: string?,
	inline: boolean?,
	width: number?,
	to: string?,
}
type UseAccordionItem = any
type UseAccordionProps = {
	items: { UseAccordionItem },
	open: any,
	event: string?,
	to: string?,
}
type UseActionTilesTile = any
type UseActionTilesRow = any
type UseActionTilesProps = {
	title: string?,
	subtitle: string?,
	trailing: any,
	tiles: { UseActionTilesTile }?,
	columns: number?,
	rows: { UseActionTilesRow }?,
	destructive: UseActionTilesRow?,
	width: number?,
	event: string?,
	to: string?,
}
type UseActivityBarItem = any
type UseActivityBarProps = {
	items: { UseActivityBarItem },
	bottom: { UseActivityBarItem }?,
	selected: string?,
	event: string?,
	to: string?,
	height: number?,
	fill: boolean?,
}
type UseAgentRunTool = any
type UseAgentRunOption = any
type UseAgentRunRun = any
type UseAgentRunProps = {
	run: UseAgentRunRun,
	lines: number?,
	tools: number?,
	request: boolean?,
	to: string?,
}
type UseAheadBehindProps = {
	ahead: number?,
	behind: number?,
	synced: boolean?,
	tooltip: string?,
}
type UseAudioMeterChannel = any
type UseAudioMeterProps = {
	channels: { UseAudioMeterChannel },
	height: number?,
	bar: number?,
	floor: number?,
	warn: number?,
	danger: number?,
}
type UseAvatarStackProps = {
	people: { any },
	count: number?,
	most: number?,
	size: string?,
}
type UseAvatarProps = {
	name: string?,
	initial: string?,
	letters: number?,
	size: string?,
	shape: string?,
	color: string?,
	status: string?,
	src: string?,
	tooltip: string?,
}
type UseBannerAction = any
type UseBannerProps = {
	text: string,
	detail: string?,
	icon: string?,
	tone: string?,
	solid: boolean?,
	mono: boolean?,
	action: any,
	dismiss: any,
	to: string?,
}
type UseBarChartSeries = any
type UseBarChartBucket = any
type UseBarChartProps = {
	buckets: { UseBarChartBucket },
	series: { UseBarChartSeries }?,
	height: number?,
	bar: number?,
	gap: number?,
	max: number?,
	axis: boolean?,
	legend: boolean?,
	unit: string?,
	every: number?,
	slot: number?,
	ticks: number?,
	grid: boolean?,
	grow: boolean?,
}
type UseBoardCardProps = {
	title: string,
	subtitle: string?,
	badge: string?,
	badge_tone: string?,
	amount: string?,
	date: string?,
	owner: string?,
	owner_color: string?,
	tone: string?,
	msg: any,
	context: any,
	to: string?,
	selected: boolean?,
	labels: { any }?,
	key: string?,
	carry: any,
	accept: any,
}
type UseBreadcrumbsCrumb = any
type UseBreadcrumbsProps = {
	crumbs: { UseBreadcrumbsCrumb },
	back: any,
	position: string?,
	up: any,
	down: any,
	actions: { any }?,
	event: string?,
	to: string?,
}
type UseBrowserPage = any
type UseBrowserProps = {
	id: string?,
	url: string?,
	page: UseBrowserPage?,
	navigate: any,
	loading: any,
	zoom: number?,
	share: boolean?,
}
type UseButtonGroupButton = any
type UseButtonGroupProps = {
	buttons: { UseButtonGroupButton },
	selected: string?,
	event: string?,
	small: boolean?,
	to: string?,
}
type UseCaffeineAwake = any
type UseCaffeineProps = {
	awake: UseCaffeineAwake?,
	scope: string?,
	reason: string?,
	variant: string?,
	size: string?,
	disabled: boolean?,
}
type UseCardProps = {
	title: string?,
	subtitle: string?,
	actions: { any }?,
	[number]: any,
}
type UseChangeTreeProps = {
	files: { string }?,
	selected: string?,
	folded: { [string]: boolean? }?,
	fold: string?,
	pick: string?,
	to: string?,
	indent: number?,
	counts: boolean?,
	mark: string?,
	plain: boolean?,
}
type UseChangeTreeFolder = any
type UseChatMarkdownProps = {
	text: string?,
	cursor: boolean?,
	link: any,
	to: string?,
}
type UseChatMessageProps = {
	from: string?,
	author: string?,
	icon: string?,
	text: string?,
	[number]: any,
}
type UseCheckboxProps = {
	checked: boolean?,
	indeterminate: boolean?,
	label: string?,
	description: string?,
	msg: any,
	disabled: boolean?,
	to: string?,
}
type UseChecksCheck = any
type UseChecksProps = {
	checks: { UseChecksCheck },
	title: string?,
	event: string?,
	to: string?,
	collapsed: boolean?,
	toggle: any,
}
type UseChipsChip = any
type UseChipsProps = {
	chips: { UseChipsChip },
	selected: { string }?,
	event: string?,
	wrap: boolean?,
	to: string?,
}
type UseChoiceCardsCard = any
type UseChoiceCardsProps = {
	options: { UseChoiceCardsCard },
	selected: string?,
	width: number?,
	event: string?,
	to: string?,
}
type UseCodeViewProps = {
	text: string?,
	lines: { string }?,
	start: number?,
	highlight: { number }?,
	numbers: boolean?,
	path: string?,
	language: string?,
	height: number?,
	grow: boolean?,
	event: string?,
	to: string?,
	id: string?,
}
type UseCollapsibleProps = {
	title: string,
	open: boolean?,
	summary: string?,
	icon: string?,
	toggle: any,
	id: string?,
	to: string?,
	flat: boolean?,
	[number]: any,
}
type UseCommandPaletteCommand = any
type UseCommandPaletteState = any
type UseCommandPaletteProps = {
	id: string?,
	open: boolean?,
	commands: { UseCommandPaletteCommand }?,
	placeholder: string?,
	look: string?,
	dismiss: any,
	event: string?,
	width: number?,
	rows: number?,
	empty: string?,
	prefix: string?,
}
type UseCommandPaletteShape = any
type UseCommentCardProps = {
	author: string,
	action: string?,
	age: string?,
	seconds: number?,
	body: string?,
	location: string?,
	locate: any,
	badge: string?,
	avatar: string?,
	to: string?,
	actions: { any }?,
	reactions: { UseCommentCardReaction }?,
	react: string?,
	add_reaction: any,
	picker: any,
	picker_open: boolean?,
	[number]: any,
}
type UseCommentCardReaction = any
type UseCommitGraphCommit = any
type UseCommitGraphProps = {
	commits: { UseCommitGraphCommit },
	select: any,
	selected: string?,
	row_height: number?,
	compact: boolean?,
}
type UseComposerProps = {
	id: string?,
	value: string?,
	placeholder: string?,
	agent: string?,
	icon: string?,
	change: any,
	submit: any,
	attach: any,
	send: any,
	actions: { any }?,
	to: string?,
	focus: boolean?,
}
type UseConfirmProps = {
	open: boolean?,
	title: string,
	message: string?,
	detail: string?,
	items: { any }?,
	items_title: string?,
	confirm: any,
	cancel: any,
	confirm_label: string?,
	cancel_label: string?,
	danger: boolean?,
	icon: string?,
	to: string?,
}
type UseContextMenuProps = {
	open: boolean?,
	context: any,
	dismiss: any,
	items: { any },
	highlighted: any,
	expanded: any,
	expand: string?,
	submenu_highlighted: any,
	event: string?,
	keys: { [string]: any }?,
	width: number?,
	side: string?,
	align: string?,
	to: string?,
	[number]: any,
}
type UseContributionGraphProps = {
	days: { number },
	start: string,
	thresholds: { number }?,
	palette: { string }?,
	color: string?,
	empty: string?,
	size: number?,
	gap: number?,
	week_start: string?,
	weekdays: boolean?,
	months: boolean?,
	legend: boolean?,
	noun: string?,
	nouns: string?,
	note: string?,
	note_msg: any?,
	selected: string?,
	event: string?,
	to: string?,
}
type UseCopyFieldProps = {
	text: string,
	label: string?,
	copied: boolean?,
	msg: any,
	button: string?,
	to: string?,
}
type UseCountBubbleProps = {
	count: number,
	max: number?,
	tone: string?,
	zero: boolean?,
	tooltip: string?,
	float: any,
}
type UseCountryCodeProps = { code: string }
type UseCourseCardAction = any
type UseCourseCardProps = {
	title: string,
	eyebrow: string?,
	meta: string?,
	primary: UseCourseCardAction?,
	secondary: UseCourseCardAction?,
	progress: number?,
	label: string?,
	detail: string?,
	bar: number?,
	aside: any,
	to: string?,
}
type UseCurveEditorPoint = any
type UseCurveEditorProps = {
	points: { UseCurveEditorPoint },
	change: any,
	size: number?,
	color: string?,
	values: { number }?,
	grid: number?,
	id: string?,
	disabled: boolean?,
}
type UseCurveEditorState = any
type UseDashboardCardProps = {
	title: string,
	icon: string?,
	meta: any,
	menu: any,
	to: string?,
	grow: boolean?,
	padding: boolean?,
	footer: any,
	[number]: any,
}
type UseDatePickerPreset = any
type UseDatePickerProps = {
	title: string?,
	weeks: { any },
	selected: string?,
	weekdays: { string }?,
	event: string?,
	previous: any,
	next: any,
	today: any,
	presets: { UseDatePickerPreset }?,
	preset: string?,
	presets_side: string?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	value: string?,
	placeholder: string?,
	overdue: boolean?,
	width: number?,
	align: string?,
	to: string?,
}
type UseDateTileProps = {
	month: string,
	day: string,
	tone: string?,
	size: string?,
}
type UseDayStripDay = any
type UseDayStripProps = {
	days: { UseDayStripDay },
	selected: string?,
	event: string?,
	to: string?,
}
type UseDialogButton = any
type UseDialogProps = {
	open: boolean?,
	dismiss: any,
	title: string?,
	subtitle: string?,
	icon: string?,
	tone: string?,
	size: string?,
	width: number?,
	height: number?,
	place: string?,
	backdrop: boolean?,
	hint: boolean?,
	plain: boolean?,
	close: boolean?,
	buttons: { UseDialogButton }?,
	actions: { any }?,
	note: string?,
	submit: any,
	keys: { [string]: any }?,
	to: string?,
	class: string?,
	[number]: any,
}
type UseDictationProps = {
	id: string?,
	text: any,
	failed: any,
	listening: any,
	engine: string?,
	speech: any,
}
type UseDictationState = any
type UseDiffCountsProps = {
	added: number?,
	removed: number?,
	files: number?,
	size: string?,
	zero: boolean?,
}
type UseDiffViewLine = any
type UseDiffViewHunk = any
type UseDiffViewProps = {
	path: string?,
	status: string?,
	hunks: { UseDiffViewHunk },
	split: boolean?,
	collapsed: boolean?,
	height: number?,
	grow: boolean?,
	header: boolean?,
	event: string?,
	to: string?,
	actions: { any }?,
	switch: string?,
}
type UseDocumentCardProps = {
	title: string,
	text: string?,
	date: string?,
	lines: number?,
	favorite: boolean?,
	star: any?,
	selected: boolean?,
	msg: any?,
	context: any?,
	width: number?,
	height: number?,
	to: string?,
}
type UseDocumentRowProps = {
	title: string,
	excerpt: string?,
	date: string?,
	words: number?,
	favorite: boolean?,
	selected: boolean?,
	lines: number?,
	msg: any?,
	context: any?,
	to: string?,
}
type UseDotChartColumn = any
type UseDotChartProps = {
	columns: { UseDotChartColumn },
	max: number,
	rows: number?,
	ticks: { string }?,
	labels: { string }?,
	size: number?,
	color: string?,
	shade: string?,
}
type UseDropdownProps = {
	open: boolean?,
	label: string?,
	value: string?,
	icon: string?,
	options: { any },
	selected: string?,
	toggle: any,
	dismiss: any,
	event: string?,
	highlighted: any,
	expanded: any,
	expand: string?,
	submenu_highlighted: any,
	keys: { [string]: any }?,
	width: number?,
	align: string?,
	side: string?,
	placeholder: string?,
	disabled: boolean?,
	to: string?,
	chevron: string?,
	trigger_width: number?,
}
type UseDropzoneProps = {
	title: string?,
	detail: string?,
	icon: string?,
	compact: boolean?,
	drop: any,
	browse: any,
	to: string?,
	width: number?,
	height: number?,
}
type UseEmojiPickerProps = {
	recent: { string }?,
	event: string?,
	query: string?,
	search: any,
	id: string?,
	focus: boolean?,
	cancel: any,
	columns: number?,
	height: number?,
	to: string?,
}
type UseEmptyStateProps = {
	icon: string?,
	title: string,
	message: string?,
	action: string?,
	msg: any,
}
type UseErrorStateProps = {
	title: string?,
	message: string?,
	details: string?,
	icon: string?,
	retry: any,
	retry_label: string?,
	secondary: any,
	block: boolean?,
	retrying: boolean?,
	to: string?,
}
type UseFeedbackProps = {
	open: boolean?,
	dismiss: any,
	id: string?,
	value: string?,
	text: string?,
	placeholder: string?,
	note: string?,
	error: string?,
	change: any,
	send: any,
	runs: { any }?,
	expanded: string?,
	undo: boolean?,
	follow: boolean?,
	reply: string?,
	actions: { any }?,
	inspect: boolean?,
	attached: { element: string, at: string?, report: string? }?,
	detach: any,
	copy: any,
	copied: boolean?,
	inline: boolean?,
	to: string?,
}
type UseFileCardProps = {
	name: string,
	src: string?,
	detail: string?,
	progress: number?,
	layout: string?,
	remove: any,
	msg: any,
	selected: boolean?,
	to: string?,
	width: number?,
}
type UseFileIconProps = {
	name: string,
	dir: boolean?,
	open: boolean?,
}
type UseFilterBuilderCondition = any
type UseFilterBuilderField = any
type UseFilterBuilderProps = {
	match: string?,
	conditions: { UseFilterBuilderCondition }?,
	fields: { UseFilterBuilderField },
	open: string?,
	to: string?,
}
type UseFilterDropdownOption = any
type UseFilterDropdownProps = {
	icon: string?,
	label: string?,
	placeholder: string?,
	options: { any },
	selected: any,
	multiple: boolean?,
	any: string?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	event: string?,
	clear: any,
	width: number?,
	plain: boolean?,
	to: string?,
}
type UseFindBarProps = {
	id: string?,
	value: string?,
	placeholder: string?,
	count: number?,
	index: number?,
	change: any,
	event: string?,
	closable: boolean?,
	focus: boolean?,
	replace: boolean?,
	replacement: string?,
	replace_change: any,
	replace_one: any,
	replace_all: any,
	width: number?,
	to: string?,
}
type UseFlowDiagramMeter = any
type UseFlowDiagramNode = any
type UseFlowDiagramLink = any
type UseFlowDiagramGroup = any
type UseFlowDiagramFlow = any
type UseFlowDiagramPoint = any
type UseFlowDiagramSize = any
type UseFlowDiagramStep = any
type UseFlowDiagramProps = {
	nodes: { UseFlowDiagramNode },
	links: { UseFlowDiagramLink }?,
	groups: { UseFlowDiagramGroup }?,
	flows: { UseFlowDiagramFlow }?,
	columns: number?,
	rows: number?,
	time: number?,
	speed: number?,
	selected: string?,
	event: string?,
	to: string?,
	size: string?,
	height: number?,
}
type UseForecastChartBar = any
type UseForecastChartProps = {
	bars: { UseForecastChartBar },
	max: number,
	floor: number?,
	ticks: { string }?,
	height: number?,
	bar: number?,
	event: string?,
	to: string?,
}
type UseForgeAccountProps = {
	forge: any,
	brand: string?,
	msg: any,
	to: string?,
}
type UseForgePrProps = {
	forge: any,
	branch: string?,
	pr: any,
	changes: any,
	brand: boolean?,
	open: boolean?,
}
type UseForgeSignInProps = {
	forge: any,
	provider: string?,
}
type UseFormGridRow = any
type UseFormGridProps = {
	rows: { UseFormGridRow },
	label_width: number?,
	gap: number?,
}
type UseGanttDay = any
type UseGanttProps = {
	days: { UseGanttDay },
	rows: { any },
	deps: { any }?,
	day_width: number?,
	row_height: number?,
	label_width: number?,
	selected: string?,
	event: string?,
	move: string?,
	resize: string?,
	moving: any,
	height: number?,
	grow: boolean?,
	to: string?,
}
type UseGanttSpan = any
type UseGhosttyTerminalProps = {
	id: string?,
	cwd: string?,
	backend: string?,
	exit: any,
	shell: any,
	style: any,
}
type UseGroupHeaderProps = {
	title: string,
	id: string?,
	count: (number | string)?,
	status: string?,
	icon: string?,
	detail: string?,
	collapsed: boolean?,
	toggle: any,
	add: any,
	add_tooltip: string?,
	actions: { any }?,
	band: boolean?,
	to: string?,
}
type UseGuideCardProps = {
	title: string,
	category: string?,
	color: string?,
	tint: string?,
	icon: string?,
	read: string?,
	progress: number?,
	done: boolean?,
	new: boolean?,
	selected: boolean?,
	msg: any,
	to: string?,
	bar: number?,
}
type UseHeatGridProps = {
	columns: { any },
	rows: number?,
	levels: number?,
	size: number?,
	gap: number?,
	color: string?,
	empty: string?,
	spread: boolean?,
	labels: { string }?,
	tooltips: { string }?,
}
type UseHistogramChannel = any
type UseHistogramMarker = any
type UseHistogramProps = {
	channels: { UseHistogramChannel },
	width: number?,
	height: number?,
	max: number?,
	log: boolean?,
	markers: { UseHistogramMarker }?,
	drag: any,
	shade: boolean?,
	to: string?,
}
type UseIconButtonProps = {
	icon: string,
	tooltip: string?,
	msg: any,
	command: any,
	variant: string?,
	size: string?,
	active: boolean?,
	badge: any,
	disabled: boolean?,
	tone: string?,
	to: string?,
}
type UseImageViewerProps = {
	src: string,
	zoom: number?,
	center: { number }?,
	change: any,
	to: string?,
	overlay: { any }?,
	controls: boolean?,
	min: number?,
	max: number?,
	width: number?,
	height: number?,
	id: string?,
}
type UseImageViewerState = any
type UseInsightNoteProps = {
	label: string?,
	icon: string?,
	text: string,
	link: string?,
	after: string?,
	msg: any,
	to: string?,
}
type UseKanbanColumnProps = {
	title: string,
	color: string?,
	count: number?,
	total: string?,
	add: any,
	add_label: string?,
	to: string?,
	width: number?,
	accept: any,
	over: boolean?,
	marker: number?,
	[number]: any,
}
type UseKbdProps = {
	keys: any,
	small: boolean?,
	plain: boolean?,
}
type UseKeyHintsHint = any
type UseKeyHintsProps = {
	hints: { UseKeyHintsHint },
	caps: boolean?,
	separator: string?,
	justify: string?,
}
type UseKeyValuesProps = {
	rows: { { string } },
	width: number?,
}
type UseKpiCellProps = {
	title: string,
	value: string,
	icon: string?,
	tint: string?,
	delta: string?,
	good: boolean?,
	caption: string?,
	menu: any,
	to: string?,
}
type UseKpiMeterSegment = any
type UseKpiMeterLegend = any
type UseKpiMeterProps = {
	title: string,
	value: string,
	caption: string?,
	caption_color: string?,
	dot: string?,
	mono: boolean?,
	segments: { UseKpiMeterSegment }?,
	legend: { UseKpiMeterLegend }?,
	width: number?,
	msg: any,
	to: string?,
}
type UseLevelRowProps = {
	label: string,
	value: string?,
	fraction: number,
	color: string?,
	end_color: string?,
	width: number?,
	label_width: number?,
	height: number?,
	tooltip: string?,
	msg: any?,
	to: string?,
}
type UseLightboxPicture = any
type UseLightboxProps = {
	open: boolean?,
	dismiss: any,
	pictures: { UseLightboxPicture }?,
	index: number?,
	event: string?,
	zoom: number?,
	center: any,
	change: any,
	to: string?,
}
type UseLineChartSeries = any
type UseLineChartProps = {
	labels: { string },
	series: { UseLineChartSeries },
	bars: { number }?,
	max: number,
	min: number?,
	ticks: { string }?,
	width: number?,
	height: number?,
	selected: number?,
	card: any,
	card_width: number?,
	tips: { string }?,
	event: string?,
	to: string?,
}
type UseListRow = any
type UseListProps = {
	rows: { UseListRow },
	selected: string?,
	to: string?,
}
type UseLoadingProps = {
	text: string?,
	detail: string?,
	block: boolean?,
	skeleton: boolean?,
	count: number?,
	widths: { number }?,
}
type UseMailRowTag = any
type UseMailRowProps = {
	from: string,
	subject: string?,
	preview: string?,
	time: string?,
	account: string?,
	unread: boolean?,
	count: number?,
	starred: boolean?,
	star: any,
	attachment: boolean?,
	tag: UseMailRowTag?,
	avatar: string?,
	color: string?,
	selected: boolean?,
	msg: any,
	context: any,
	to: string?,
}
type UseMarkdownEditorMode = any
type UseMarkdownEditorState = any
type UseMarkdownEditorProps = {
	path: string?,
	root: string?,
	text: string?,
	mode: string?,
	id: string?,
}
type UseMarkdownProps = {
	text: string?,
	link: any,
	to: string?,
	grow: boolean?,
	scroll: boolean?,
	max_width: number?,
	lines: number?,
	images: boolean?,
	image: any,
	outline: any,
	section: any,
	reveal: { at: number, seq: number? }?,
	find: string?,
	find_index: number?,
	found: any,
	stream: boolean?,
	scrolled: any,
	scroll_at: { fraction: number, seq: number? }?,
}
type UseMentionFieldPerson = any
type UseMentionFieldProps = {
	id: string,
	value: string?,
	caret: number?,
	people: { UseMentionFieldPerson }?,
	change: any,
	submit: any,
	blur: any,
	placeholder: string?,
	multiline: boolean?,
	rows: number?,
	max_rows: number?,
	highlighted: number?,
	event: string?,
	dismiss: any,
	most: number?,
	width: number?,
	grow: boolean?,
	focus: boolean?,
	to: string?,
	trigger: string?,
	heading: string?,
}
type UseMenuItem = any
type UseMenuProps = {
	items: { any },
	max_height: number?,
	highlighted: any,
	event: string?,
	expanded: any,
	expand: string?,
	submenu_highlighted: any,
	submenu_width: number?,
	to: string?,
}
type UseMessageHeaderProps = {
	subject: string?,
	from: string,
	email: string?,
	recipients: any,
	cc: any,
	date: string?,
	avatar: string?,
	color: string?,
	actions: { any }?,
	sender: any,
	to: string?,
	[number]: any,
}
type UseMeterProps = {
	value: number,
	width: number?,
	warn: number?,
	label: string?,
	percent: boolean?,
}
type UseMetricProps = {
	label: string,
	value: string,
	note: string?,
	tone: string?,
	size: string?,
}
type UseMiniBarsBar = any
type UseMiniBarsProps = {
	bars: { UseMiniBarsBar },
	highlight: number?,
	height: number?,
	bar: number?,
}
type UseMockupsVariant = any
type UseMockupsProps = {
	ask: string?,
	variants: { UseMockupsVariant },
	file: string?,
	width: number?,
}
type UseMonthGridDay = any
type UseMonthGridProps = {
	weeks: { { UseMonthGridDay } },
	selected: string?,
	weekdays: { string }?,
	event: string?,
	to: string?,
}
type UseNavButtonsProps = {
	can_back: boolean?,
	can_forward: boolean?,
	back: any,
	forward: any,
	small: boolean?,
	to: string?,
}
type UseNoteCardProps = {
	id: any,
	number: number?,
	location: string?,
	text: string,
	selected: boolean?,
	event: string?,
	remove: string?,
	to: string?,
}
type UseNoticeProps = {
	id: any,
	tone: string?,
	icon: string?,
	title: string?,
	text: string?,
	action: any,
	dismiss: any,
	foldable: boolean?,
	expanded: boolean?,
	toggle: any,
	to: string?,
	[number]: any,
}
type UseNotificationDrawerItem = any
type UseNotificationDrawerFilter = any
type UseNotificationDrawerProps = {
	open: boolean?,
	items: { UseNotificationDrawerItem }?,
	filter: string?,
	filters: { UseNotificationDrawerFilter }?,
	title: string?,
	width: number?,
	top: number?,
	backdrop: boolean?,
	dismiss: any,
	mark_read: any,
	mark_read_label: string?,
	event: string?,
	remove: any,
	empty: string?,
	to: string?,
}
type UseNumberFieldProps = {
	value: number,
	change: any,
	min: number?,
	max: number?,
	step: number?,
	speed: number?,
	decimals: number?,
	label: string?,
	unit: string?,
	width: number?,
	buttons: boolean?,
	id: string?,
	disabled: boolean?,
	to: string?,
}
type UseNumberedListProps = {
	items: { string },
	start: number?,
}
type UseOnboardingPage = any
type UseOnboardingProps = {
	open: boolean?,
	title: string?,
	icon: string?,
	badge: string?,
	pages: { UseOnboardingPage }?,
	current: number?,
	back: any,
	next: any,
	done: any,
	dismiss: any,
	event: string?,
	done_label: string?,
	width: number?,
	to: string?,
	[number]: any,
}
type UseOutlineHeading = any
type UseOutlineProps = {
	headings: { UseOutlineHeading }?,
	current: number?,
	event: string?,
	indent: number?,
	depth: number?,
	empty: string?,
	to: string?,
}
type UsePageProps = {
	title: string,
	subtitle: string?,
	icon: string?,
	actions: { any }?,
	width: number?,
	footer: any,
	height: number?,
	[number]: any,
}
type UsePagerProps = {
	count: number,
	page: number,
	back: any,
	next: any,
	back_label: string?,
	next_label: string?,
	event: string?,
	to: string?,
}
type UsePaginationProps = {
	page: number,
	total: number,
	per_page: number?,
	noun: string?,
	summary: boolean?,
	window: number?,
	ellipsis: boolean?,
	event: string?,
	sizes: { number }?,
	size_open: boolean?,
	size_toggle: any,
	size_dismiss: any,
	size_event: string?,
	size_label: string?,
	to: string?,
}
type UsePaneFrameProps = {
	title: string,
	icon: string?,
	subtitle: string?,
	status: string?,
	focused: boolean?,
	zoomed: boolean?,
	header: string?,
	focus: any,
	split_right: any,
	split_down: any,
	zoom: any,
	close: any,
	actions: { any }?,
	leading: { any }?,
	menu: any,
	click_focus: boolean?,
	to: string?,
	height: number?,
	[number]: any,
}
type UsePickerOption = any
type UsePickerProps = {
	options: { any },
	selected: any,
	multiple: boolean?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	event: string?,
	query: string?,
	search: any,
	id: string?,
	create: any,
	highlighted: string?,
	highlight: string?,
	keys: { [string]: any }?,
	placeholder: string?,
	icon: string?,
	empty: string?,
	width: number?,
	align: string?,
	trigger: any,
	to: string?,
}
type UsePillTabsItem = any
type UsePillTabsProps = {
	items: { UsePillTabsItem },
	selected: string?,
	event: string?,
	width: number?,
	size: number?,
	to: string?,
}
type UsePillProps = {
	label: string,
	icon: string?,
	tone: string?,
	color: string?,
	size: string?,
	msg: any,
	to: string?,
	tooltip: string?,
}
type UsePixelIconProps = {
	pattern: { string },
	colors: { string },
	size: number?,
	mirror: boolean?,
	tooltip: string?,
}
type UsePrBadgeProps = {
	number: number | string,
	kind: string?,
	tone: string?,
	icon: string?,
	checks: string?,
	label: string?,
	tooltip: string?,
	msg: any,
	to: string?,
}
type UsePrRowProps = {
	number: number | string,
	kind: string?,
	title: string,
	tone: string?,
	draft: boolean?,
	branch: string?,
	target: string?,
	author: string?,
	age: string?,
	seconds: number?,
	checks: string?,
	added: number?,
	removed: number?,
	comments: number?,
	labels: { string }?,
	selected: boolean?,
	event: string?,
	to: string?,
	msg: any,
	context: any,
}
type UsePriorityIconProps = {
	priority: string?,
	size: number?,
	tooltip: string?,
	label: (string | boolean)?,
}
type UseProgressStackSegment = any
type UseProgressStackProps = {
	segments: { UseProgressStackSegment },
	total: number,
	width: number?,
	marker: { value: number, label: string }?,
	labels: { string }?,
	ticks: number?,
}
type UseProgressProps = {
	value: number?,
	busy: boolean?,
	label: string?,
	detail: string?,
	tone: string?,
	width: number?,
	height: number?,
	percent: boolean?,
}
type UseProjectCardProps = {
	name: string,
	icon: string?,
	color: string?,
	description: string?,
	status: string?,
	tone: string?,
	status_color: string?,
	starred: boolean?,
	star: any,
	progress: number?,
	tasks: string?,
	due: string?,
	updated: string?,
	people: { any }?,
	selected: boolean?,
	msg: any,
	context: any,
	to: string?,
	width: number?,
}
type UseProseEditorProps = {
	id: string?,
	value: string?,
	placeholder: string?,
	change: any?,
	submit: any?,
	keys: { [string]: any }?,
	to: string?,
	dim: ("sentence" | "paragraph")?,
	typewriter: boolean?,
	markdown: boolean?,
	mono: boolean?,
	size: ("normal" | "large" | "title")?,
	measure: number?,
	focus: boolean?,
}
type UseRadioGroupOption = any
type UseRadioGroupProps = {
	options: { UseRadioGroupOption },
	selected: string?,
	horizontal: boolean?,
	event: string?,
	to: string?,
}
type UseRailItem = any
type UseRailProps = {
	items: { UseRailItem },
	selected: string?,
	event: string?,
	to: string?,
	header: { any }?,
	footer: { any }?,
	height: number?,
	fill: boolean?,
}
type UseRatioRowProps = {
	label: string,
	count: string?,
	percent: string?,
	icon: string?,
	leading: any,
	share: number?,
	color: string?,
	width: number?,
	msg: any,
	to: string?,
}
type UseRecipientFieldRecipient = any
type UseRecipientFieldProps = {
	label: string?,
	recipients: { any }?,
	id: string?,
	value: string?,
	placeholder: string?,
	change: any,
	submit: any,
	focus: boolean?,
	keys: { [string]: any }?,
	suggestions: { any }?,
	highlighted: number?,
	pick: string?,
	remove: string?,
	dismiss: any,
	trailing: { any }?,
	width: number?,
	to: string?,
}
type UseReconnectingProps = {
	open: boolean?,
	host: string?,
	title: string?,
	reason: string?,
	status: string?,
	retry: any,
	retry_label: string?,
	secondary: any,
	dismiss: any,
	icon: string?,
	to: string?,
}
type UseRecordCardProps = {
	title: string,
	detail: string?,
	icon: string?,
	tone: string?,
	msg: any,
	to: string?,
}
type UseRelativeTimeProps = {
	seconds: number,
	compact: boolean?,
	prefix: string?,
	tooltip: string?,
	size: string?,
}
type UseReleaseNotesHighlight = any
type UseReleaseNotesProps = {
	version: string,
	date: string?,
	name: string?,
	badge: string?,
	summary: string?,
	highlights: { UseReleaseNotesHighlight }?,
	fixes: { string }?,
	fixes_title: string?,
	collapsed: boolean?,
	toggle: any,
	id: any,
	to: string?,
}
type UseRichTextPart = any
type UseRichTextProps = {
	parts: { any },
	size: string?,
	to: string?,
}
type UseRouteMapStop = any
type UseRouteMapLeg = any
type UseRouteMapProps = {
	stops: { UseRouteMapStop },
	legs: { UseRouteMapLeg }?,
	provider: string?,
	key: string?,
	travel: string?,
	marker: string?,
	selected: number?,
	event: string?,
	routed: any,
	height: number?,
	grow: boolean?,
	to: string?,
}
type UseSearchFieldProps = {
	id: string?,
	value: string?,
	placeholder: string?,
	change: any,
	submit: any,
	cancel: any,
	previous: any,
	next: any,
	to: string?,
	focus: boolean?,
	width: number?,
	grow: boolean?,
	icon: any,
	shortcut: any,
}
type UseSectionProps = {
	title: string,
	[number]: any,
}
type UseSegmentedOption = any
type UseSegmentedProps = {
	options: { UseSegmentedOption },
	selected: string?,
	event: string?,
	small: boolean?,
	grow: boolean?,
	to: string?,
}
type UseSelectionToolbarProps = {
	[number]: any,
	field: string,
	selected: number?,
	tools: { string }?,
	side: any,
}
type UseSeriesChartSeries = any
type UseSeriesChartLabel = any
type UseSeriesChartProps = {
	series: { UseSeriesChartSeries },
	width: number?,
	height: number?,
	min: number?,
	max: number?,
	ticks: { string }?,
	labels: { UseSeriesChartLabel }?,
	titles: { string }?,
	unit: string?,
	pinned: number?,
	event: string?,
	to: string?,
	legend: boolean?,
}
type UseSettingsNavItem = any
type UseSettingsNavGroup = any
type UseSettingsNavProps = {
	groups: { UseSettingsNavGroup },
	selected: string?,
	title: string?,
	subtitle: string?,
	search: any,
	footer: any,
	event: string?,
	width: number?,
	scroll: boolean?,
	height: number?,
	to: string?,
}
type UseSettingsPageRow = any
type UseSettingsPageSection = any
type UseSettingsPageProps = {
	nav: any,
	title: string?,
	subtitle: string?,
	sections: { UseSettingsPageSection }?,
	status: string?,
	state: string?,
	buttons: { any }?,
	height: number?,
	to: string?,
	[number]: any,
}
type UseSettingsRowProps = {
	title: string,
	description: string?,
	tag: string?,
	tag_tone: string?,
	disabled: boolean?,
	reason: string?,
	stacked: boolean?,
	border: boolean?,
	[number]: any,
}
type UseShareBarShare = any
type UseShareBarProps = {
	shares: { UseShareBarShare },
	width: number?,
	height: number?,
}
type UseSheetProps = {
	open: boolean?,
	dismiss: any,
	side: string?,
	title: string?,
	subtitle: string?,
	icon: string?,
	width: number?,
	height: number?,
	backdrop: boolean?,
	actions: { any }?,
	footer: any,
	keys: { [string]: any }?,
	to: string?,
	[number]: any,
}
type UseShortcutFieldProps = {
	id: string?,
	value: string?,
	recording: boolean?,
	event: string?,
	clearable: boolean?,
	placeholder: string?,
	width: number?,
	to: string?,
}
type UseShortcutsItem = any
type UseShortcutsGroup = any
type UseShortcutsProps = {
	open: boolean?,
	dismiss: any,
	title: string?,
	subtitle: string?,
	groups: { UseShortcutsGroup },
	columns: number?,
	search: any,
	note: string?,
	width: number?,
	height: number?,
	inline: boolean?,
	to: string?,
}
type UseSideNavItem = any
type UseSideNavSection = any
type UseSideNavAccount = any
type UseSideNavProps = {
	collapsed: boolean?,
	selected: string?,
	sections: { UseSideNavSection }?,
	footer: { UseSideNavItem }?,
	account: UseSideNavAccount?,
	header: any,
	toggle: any,
	event: string?,
	to: string?,
	stagger: number?,
	height: number?,
}
type UseSidePanelsProps = {
	id: string?,
	sidebar: any,
	list: any,
	main: any,
	left: boolean?,
	middle: boolean?,
	sidebar_size: number?,
	list_size: number?,
	height: number?,
}
type UseSidebarSectionProps = {
	title: string,
	count: (number | string)?,
	collapsed: boolean?,
	toggle: any,
	id: string?,
	to: string?,
	actions: { any }?,
	fixed: boolean?,
	[number]: any,
}
type UseSidebarSection = any
type UseSidebarFooterItem = any
type UseSidebarProps = {
	header: any,
	sections: { UseSidebarSection }?,
	footer: { UseSidebarFooterItem }?,
	to: string?,
	width: number?,
	height: number?,
	fill: boolean?,
	[number]: any,
}
type UseSliderProps = {
	value: number,
	min: number?,
	max: number?,
	step: number?,
	change: any,
	label: string?,
	unit: string?,
	decimals: number?,
	default: number?,
	width: number?,
	inline: boolean?,
	label_width: number?,
	colors: { string }?,
	disabled: boolean?,
	to: string?,
}
type UseSparklineProps = {
	values: { number },
	samples: number?,
	height: number?,
	hot: number?,
}
type UseSplitViewProps = {
	id: string?,
	sidebar: any,
	main: any,
	inspector: any,
	bottom_panel: any,
	left: boolean?,
	right: boolean?,
	bottom: boolean?,
	sidebar_size: number?,
	inspector_size: number?,
	bottom_size: number?,
	height: number?,
}
type UseSpotlightBarsBar = any
type UseSpotlightBarsProps = {
	bars: { UseSpotlightBarsBar },
	selected: number?,
	height: number?,
	max: number?,
	bar: number?,
	event: string?,
	to: string?,
}
type UseSpreadsheetColumn = any
type UseSpreadsheetProps = {
	columns: { UseSpreadsheetColumn },
	rows: { { [string]: any } },
	current: number?,
	selected: { row: number, key: string }?,
	caret: boolean?,
	blank: number?,
	numbers: boolean?,
	row_height: number?,
	event: string?,
	to: string?,
	grow: boolean?,
	height: number?,
	id: string?,
}
type UseStatTileProps = {
	label: string,
	value: string,
	unit: string?,
	icon: string?,
	delta: string?,
	trend: string?,
	good: string?,
	detail: string?,
	msg: any,
	to: string?,
	selected: boolean?,
	width: number?,
	[number]: any,
}
type UseStatProps = {
	icon: string?,
	label: string?,
	value: string,
	width: number?,
	tone: any,
}
type UseStatusBarEntry = any
type UseStatusBarProps = {
	left: { any }?,
	right: { any }?,
	separators: boolean?,
	to: string?,
}
type UseStatusDotProps = {
	status: string?,
	size: string?,
	spin: boolean?,
	chip: boolean?,
	label: string?,
	tooltip: string?,
}
type UseStatusIconProps = {
	status: string?,
	progress: number?,
	color: string?,
	size: number?,
	tooltip: string?,
	label: (string | boolean)?,
}
type UseStepperProps = {
	value: number,
	unit: string?,
	min: number?,
	max: number?,
	decrement: any,
	increment: any,
	label: string?,
	width: number?,
	to: string?,
}
type UseSwapViewProps = {
	key: string,
	width: number?,
	grow: boolean?,
	[number]: any,
}
type UseSwatchesSwatch = any
type UseSwatchesProps = {
	colors: { UseSwatchesSwatch },
	selected: string?,
	none: boolean?,
	event: string?,
	size: number?,
	to: string?,
}
type UseSwitchProps = {
	on: boolean?,
	label: string?,
	description: string?,
	msg: any,
	disabled: boolean?,
	small: boolean?,
	to: string?,
}
type UseSystemLoadProps = {
	cpu: number,
	memory: number,
	history: { number }?,
	samples: number?,
	compact: boolean?,
	icons: boolean?,
}
type UseTabViewProps = {
	tab: any,
	kind: any,
}
type UseTabbedTab = any
type UseTabbedState = any
type UseTabbedProps = {
	id: string?,
	start: string?,
	offer: { string }?,
	variant: string?,
	show: string?,
}
type UseTableColumn = any
type UseTableProps = {
	columns: { UseTableColumn },
	rows: { { [string]: any } }?,
	selected: any,
	sort: string?,
	descending: boolean?,
	sortable: boolean?,
	zebra: boolean?,
	empty: string?,
	event: string?,
	to: string?,
	height: number?,
	grow: boolean?,
	checkable: boolean?,
	checks: any,
	check: string?,
	check_all: any,
	bordered: boolean?,
	id: string?,
	groups: { any }?,
	fold: string?,
	add_label: string?,
	reorder: string?,
	marker: any,
}
type UseTabsTab = any
type UseTabsProps = {
	tabs: { UseTabsTab },
	selected: string?,
	variant: string?,
	add: any,
	add_menu: any,
	event: string?,
	close: string?,
	context: string?,
	closable: boolean?,
	grow: boolean?,
	width: number?,
	to: string?,
	in_bar: boolean?,
}
type UseTagsProps = {
	tags: { string },
	tone: any,
}
type UseTaskCheckProps = {
	state: string?,
	msg: any,
	to: string?,
	tooltip: string?,
}
type UseTaskRowProps = {
	title: string,
	state: string?,
	event: boolean?,
	date: string?,
	time: string?,
	due: boolean?,
	icon: string?,
	tags: { any }?,
	carry: any,
	accept: any,
	leading: any,
	toggle: any,
	msg: any,
	context: any,
	to: string?,
	selected: boolean?,
}
type UseTerminalProps = {
	id: string?,
	cwd: string?,
	backend: string?,
	exit: any,
	shell: any,
	style: any,
}
type UseTextEditorProps = {
	id: string,
	value: string?,
	change: any,
	blur: any,
	submit: any,
	placeholder: string?,
	rows: number?,
	max_rows: number?,
	preview: boolean?,
	switch: any,
	tools: { string }?,
	empty: string?,
	focus: boolean?,
	to: string?,
}
type UseThinkingProps = {
	seconds: number?,
	label: string?,
	open: boolean?,
	msg: any,
	to: string?,
	text: string?,
}
type UseThresholdLineProps = {
	points: { number },
	threshold: number,
	label: string?,
	breach: string?,
	width: number?,
	height: number?,
	min: number?,
	max: number?,
	dot: boolean?,
	stroke: number?,
	tooltip: string?,
}
type UseTickMeterProps = {
	value: number,
	color: string?,
	ticks: number?,
	height: number?,
	tooltip: string?,
}
type UseTimePickerProps = {
	hour: number?,
	minute: number?,
	step: number?,
	clock: number?,
	presets: { any }?,
	event: string?,
	to: string?,
}
type UseTimelineEntry = any
type UseTimelineProps = {
	entries: { UseTimelineEntry },
	selected: any,
	event: string?,
	to: string?,
	mono: boolean?,
	empty: string?,
}
type UseTitleBarWindow = any
type UseTitleBarProps = {
	window: UseTitleBarWindow?,
	inset: number?,
	leading: { any }?,
	back: any,
	forward: any,
	can_back: boolean?,
	can_forward: boolean?,
	title: string?,
	center: boolean?,
	actions: { any }?,
	to: string?,
	[number]: any,
}
type UseToastProps = {
	id: any,
	level: string?,
	icon: any,
	source: string?,
	title: string?,
	body: string?,
	full: boolean?,
	action: any,
	dismiss: any,
	event: string?,
	width: number?,
	to: string?,
}
type UseToastsItem = any
type UseToastsProps = {
	items: { UseToastsItem },
	place: string?,
	width: number?,
	max: number?,
	event: string?,
	to: string?,
}
type UseToolbarButton = any
type UseToolbarProps = {
	buttons: { UseToolbarButton },
}
type UseTreeNode = any
type UseTreeProps = {
	nodes: { UseTreeNode },
	selected: string?,
	guides: boolean?,
	select: string?,
	fold: string?,
	to: string?,
	indent: number?,
}
type UseUpdatePillProps = {
	name: string?,
	version: string?,
	text: string?,
	icon: string?,
	restart: any,
	restart_label: string?,
	notes: any,
	notes_label: string?,
	dismiss: any,
	floating: boolean?,
	place: string?,
	downloading: boolean?,
	to: string?,
}
type UseUsageBarProps = {
	title: string?,
	left: number,
	pace: number?,
	resets: string?,
	note: string?,
	width: number?,
	warn: number?,
	danger: number?,
}
type UseVectorscopeProps = {
	points: { number },
	size: number?,
	color: string?,
	targets: boolean?,
}
type UseVerticalBarProps = {
	value: number,
	height: number?,
	width: number?,
	warn: number?,
	label: string?,
	percent: boolean?,
	tooltip: string?,
}
type UseVideoViewerProps = {
	source: string,
	zoom: number?,
	center: { number }?,
	change: any,
	stats: any,
	probed: any,
	guides: { any }?,
	color: string?,
	extended: boolean?,
	black: number?,
	white: number?,
	gain: number?,
	gamma: number?,
	controls: boolean?,
	max: number?,
	width: number?,
	height: number?,
	id: string?,
}
type UseVideoViewerState = any
type UseWaveformBand = any
type UseWaveformChannel = any
type UseWaveformProps = {
	channels: { UseWaveformChannel },
	parade: boolean?,
	width: number?,
	height: number?,
	lines: { number }?,
}
type UseWeekCalendarDay = any
type UseWeekCalendarEvent = any
type UseWeekCalendarProps = {
	view: string?,
	views: any,
	title: string?,
	today: any,
	previous: any,
	next: any,
	days: { UseWeekCalendarDay }?,
	weeks: { { UseWeekCalendarDay } }?,
	weekdays: { string }?,
	events: { UseWeekCalendarEvent }?,
	selected: string?,
	hours: { from: number?, to: number? }?,
	hour_height: number?,
	column_width: number?,
	day_width: number?,
	gutter: number?,
	now: { day: string, hour: number }?,
	more: number?,
	event: string?,
	switch: string?,
	pick_day: string?,
	move: string?,
	over: string?,
	to: string?,
	grow: boolean?,
	height: number?,
	header: boolean?,
}
type UseWritingStatsProps = {
	text: string?,
	show: ("words" | "characters" | "sentences" | "reading")?,
	wpm: number?,
	msg: any?,
	to: string?,
	tooltip: string?,
}

-- The component `name`'s `build`: `local stat = use "stat"`, then
-- `stat { value = "12%" }` wherever an element goes.
declare use:
	((name: "about") -> (props: UseAboutProps) -> Element)
	& ((name: "accordion") -> (props: UseAccordionProps) -> Element)
	& ((name: "action-tiles") -> (props: UseActionTilesProps) -> Element)
	& ((name: "activity-bar") -> (props: UseActivityBarProps) -> Element)
	& ((name: "agent-run") -> (props: UseAgentRunProps) -> Element)
	& ((name: "ahead-behind") -> (props: UseAheadBehindProps) -> Element)
	& ((name: "audio-meter") -> (props: UseAudioMeterProps) -> Element)
	& ((name: "avatar-stack") -> (props: UseAvatarStackProps) -> Element)
	& ((name: "avatar") -> (props: UseAvatarProps) -> Element)
	& ((name: "banner") -> (props: UseBannerProps) -> Element)
	& ((name: "bar-chart") -> (props: UseBarChartProps) -> Element)
	& ((name: "board-card") -> (props: UseBoardCardProps) -> Element)
	& ((name: "breadcrumbs") -> (props: UseBreadcrumbsProps) -> Element)
	& ((name: "browser") -> (props: UseBrowserProps) -> Element)
	& ((name: "button-group") -> (props: UseButtonGroupProps) -> Element)
	& ((name: "caffeine") -> (props: UseCaffeineProps) -> Element)
	& ((name: "card") -> (props: UseCardProps) -> Element)
	& ((name: "change-tree") -> (props: UseChangeTreeProps) -> Element)
	& ((name: "chat-markdown") -> (props: UseChatMarkdownProps) -> Element)
	& ((name: "chat-message") -> (props: UseChatMessageProps) -> Element)
	& ((name: "checkbox") -> (props: UseCheckboxProps) -> Element)
	& ((name: "checks") -> (props: UseChecksProps) -> Element)
	& ((name: "chips") -> (props: UseChipsProps) -> Element)
	& ((name: "choice-cards") -> (props: UseChoiceCardsProps) -> Element)
	& ((name: "code-view") -> (props: UseCodeViewProps) -> Element)
	& ((name: "collapsible") -> (props: UseCollapsibleProps) -> Element)
	& ((name: "command-palette") -> (props: UseCommandPaletteProps) -> Element)
	& ((name: "comment-card") -> (props: UseCommentCardProps) -> Element)
	& ((name: "commit-graph") -> (props: UseCommitGraphProps) -> Element)
	& ((name: "composer") -> (props: UseComposerProps) -> Element)
	& ((name: "confirm") -> (props: UseConfirmProps) -> Element)
	& ((name: "context-menu") -> (props: UseContextMenuProps) -> Element)
	& ((name: "contribution-graph") -> (props: UseContributionGraphProps) -> Element)
	& ((name: "copy-field") -> (props: UseCopyFieldProps) -> Element)
	& ((name: "count-bubble") -> (props: UseCountBubbleProps) -> Element)
	& ((name: "country-code") -> (props: UseCountryCodeProps) -> Element)
	& ((name: "course-card") -> (props: UseCourseCardProps) -> Element)
	& ((name: "curve-editor") -> (props: UseCurveEditorProps) -> Element)
	& ((name: "dashboard-card") -> (props: UseDashboardCardProps) -> Element)
	& ((name: "date-picker") -> (props: UseDatePickerProps) -> Element)
	& ((name: "date-tile") -> (props: UseDateTileProps) -> Element)
	& ((name: "day-strip") -> (props: UseDayStripProps) -> Element)
	& ((name: "dialog") -> (props: UseDialogProps) -> Element)
	& ((name: "dictation") -> (props: UseDictationProps) -> Element)
	& ((name: "diff-counts") -> (props: UseDiffCountsProps) -> Element)
	& ((name: "diff-view") -> (props: UseDiffViewProps) -> Element)
	& ((name: "document-card") -> (props: UseDocumentCardProps) -> Element)
	& ((name: "document-row") -> (props: UseDocumentRowProps) -> Element)
	& ((name: "dot-chart") -> (props: UseDotChartProps) -> Element)
	& ((name: "dropdown") -> (props: UseDropdownProps) -> Element)
	& ((name: "dropzone") -> (props: UseDropzoneProps) -> Element)
	& ((name: "emoji-picker") -> (props: UseEmojiPickerProps) -> Element)
	& ((name: "empty-state") -> (props: UseEmptyStateProps) -> Element)
	& ((name: "error-state") -> (props: UseErrorStateProps) -> Element)
	& ((name: "feedback") -> (props: UseFeedbackProps) -> Element)
	& ((name: "file-card") -> (props: UseFileCardProps) -> Element)
	& ((name: "file-icon") -> (props: UseFileIconProps) -> Element)
	& ((name: "filter-builder") -> (props: UseFilterBuilderProps) -> Element)
	& ((name: "filter-dropdown") -> (props: UseFilterDropdownProps) -> Element)
	& ((name: "find-bar") -> (props: UseFindBarProps) -> Element)
	& ((name: "flow-diagram") -> (props: UseFlowDiagramProps) -> Element)
	& ((name: "forecast-chart") -> (props: UseForecastChartProps) -> Element)
	& ((name: "forge-account") -> (props: UseForgeAccountProps) -> Element)
	& ((name: "forge-pr") -> (props: UseForgePrProps) -> Element)
	& ((name: "forge-sign-in") -> (props: UseForgeSignInProps) -> Element)
	& ((name: "form-grid") -> (props: UseFormGridProps) -> Element)
	& ((name: "gantt") -> (props: UseGanttProps) -> Element)
	& ((name: "ghostty-terminal") -> (props: UseGhosttyTerminalProps) -> Element)
	& ((name: "group-header") -> (props: UseGroupHeaderProps) -> Element)
	& ((name: "guide-card") -> (props: UseGuideCardProps) -> Element)
	& ((name: "heat-grid") -> (props: UseHeatGridProps) -> Element)
	& ((name: "histogram") -> (props: UseHistogramProps) -> Element)
	& ((name: "icon-button") -> (props: UseIconButtonProps) -> Element)
	& ((name: "image-viewer") -> (props: UseImageViewerProps) -> Element)
	& ((name: "insight-note") -> (props: UseInsightNoteProps) -> Element)
	& ((name: "kanban-column") -> (props: UseKanbanColumnProps) -> Element)
	& ((name: "kbd") -> (props: UseKbdProps) -> Element)
	& ((name: "key-hints") -> (props: UseKeyHintsProps) -> Element)
	& ((name: "key-values") -> (props: UseKeyValuesProps) -> Element)
	& ((name: "kpi-cell") -> (props: UseKpiCellProps) -> Element)
	& ((name: "kpi-meter") -> (props: UseKpiMeterProps) -> Element)
	& ((name: "level-row") -> (props: UseLevelRowProps) -> Element)
	& ((name: "lightbox") -> (props: UseLightboxProps) -> Element)
	& ((name: "line-chart") -> (props: UseLineChartProps) -> Element)
	& ((name: "list") -> (props: UseListProps) -> Element)
	& ((name: "loading") -> (props: UseLoadingProps) -> Element)
	& ((name: "mail-row") -> (props: UseMailRowProps) -> Element)
	& ((name: "markdown-editor") -> (props: UseMarkdownEditorProps) -> Element)
	& ((name: "markdown") -> (props: UseMarkdownProps) -> Element)
	& ((name: "mention-field") -> (props: UseMentionFieldProps) -> Element)
	& ((name: "menu") -> (props: UseMenuProps) -> Element)
	& ((name: "message-header") -> (props: UseMessageHeaderProps) -> Element)
	& ((name: "meter") -> (props: UseMeterProps) -> Element)
	& ((name: "metric") -> (props: UseMetricProps) -> Element)
	& ((name: "mini-bars") -> (props: UseMiniBarsProps) -> Element)
	& ((name: "mockups") -> (props: UseMockupsProps) -> Element)
	& ((name: "month-grid") -> (props: UseMonthGridProps) -> Element)
	& ((name: "nav-buttons") -> (props: UseNavButtonsProps) -> Element)
	& ((name: "note-card") -> (props: UseNoteCardProps) -> Element)
	& ((name: "notice") -> (props: UseNoticeProps) -> Element)
	& ((name: "notification-drawer") -> (props: UseNotificationDrawerProps) -> Element)
	& ((name: "number-field") -> (props: UseNumberFieldProps) -> Element)
	& ((name: "numbered-list") -> (props: UseNumberedListProps) -> Element)
	& ((name: "onboarding") -> (props: UseOnboardingProps) -> Element)
	& ((name: "outline") -> (props: UseOutlineProps) -> Element)
	& ((name: "page") -> (props: UsePageProps) -> Element)
	& ((name: "pager") -> (props: UsePagerProps) -> Element)
	& ((name: "pagination") -> (props: UsePaginationProps) -> Element)
	& ((name: "pane-frame") -> (props: UsePaneFrameProps) -> Element)
	& ((name: "picker") -> (props: UsePickerProps) -> Element)
	& ((name: "pill-tabs") -> (props: UsePillTabsProps) -> Element)
	& ((name: "pill") -> (props: UsePillProps) -> Element)
	& ((name: "pixel-icon") -> (props: UsePixelIconProps) -> Element)
	& ((name: "pr-badge") -> (props: UsePrBadgeProps) -> Element)
	& ((name: "pr-row") -> (props: UsePrRowProps) -> Element)
	& ((name: "priority-icon") -> (props: UsePriorityIconProps) -> Element)
	& ((name: "progress-stack") -> (props: UseProgressStackProps) -> Element)
	& ((name: "progress") -> (props: UseProgressProps) -> Element)
	& ((name: "project-card") -> (props: UseProjectCardProps) -> Element)
	& ((name: "prose-editor") -> (props: UseProseEditorProps) -> Element)
	& ((name: "radio-group") -> (props: UseRadioGroupProps) -> Element)
	& ((name: "rail") -> (props: UseRailProps) -> Element)
	& ((name: "ratio-row") -> (props: UseRatioRowProps) -> Element)
	& ((name: "recipient-field") -> (props: UseRecipientFieldProps) -> Element)
	& ((name: "reconnecting") -> (props: UseReconnectingProps) -> Element)
	& ((name: "record-card") -> (props: UseRecordCardProps) -> Element)
	& ((name: "relative-time") -> (props: UseRelativeTimeProps) -> Element)
	& ((name: "release-notes") -> (props: UseReleaseNotesProps) -> Element)
	& ((name: "rich-text") -> (props: UseRichTextProps) -> Element)
	& ((name: "route-map") -> (props: UseRouteMapProps) -> Element)
	& ((name: "search-field") -> (props: UseSearchFieldProps) -> Element)
	& ((name: "section") -> (props: UseSectionProps) -> Element)
	& ((name: "segmented") -> (props: UseSegmentedProps) -> Element)
	& ((name: "selection-toolbar") -> (props: UseSelectionToolbarProps) -> Element)
	& ((name: "series-chart") -> (props: UseSeriesChartProps) -> Element)
	& ((name: "settings-nav") -> (props: UseSettingsNavProps) -> Element)
	& ((name: "settings-page") -> (props: UseSettingsPageProps) -> Element)
	& ((name: "settings-row") -> (props: UseSettingsRowProps) -> Element)
	& ((name: "share-bar") -> (props: UseShareBarProps) -> Element)
	& ((name: "sheet") -> (props: UseSheetProps) -> Element)
	& ((name: "shortcut-field") -> (props: UseShortcutFieldProps) -> Element)
	& ((name: "shortcuts") -> (props: UseShortcutsProps) -> Element)
	& ((name: "side-nav") -> (props: UseSideNavProps) -> Element)
	& ((name: "side-panels") -> (props: UseSidePanelsProps) -> Element)
	& ((name: "sidebar-section") -> (props: UseSidebarSectionProps) -> Element)
	& ((name: "sidebar") -> (props: UseSidebarProps) -> Element)
	& ((name: "slider") -> (props: UseSliderProps) -> Element)
	& ((name: "sparkline") -> (props: UseSparklineProps) -> Element)
	& ((name: "split-view") -> (props: UseSplitViewProps) -> Element)
	& ((name: "spotlight-bars") -> (props: UseSpotlightBarsProps) -> Element)
	& ((name: "spreadsheet") -> (props: UseSpreadsheetProps) -> Element)
	& ((name: "stat-tile") -> (props: UseStatTileProps) -> Element)
	& ((name: "stat") -> (props: UseStatProps) -> Element)
	& ((name: "status-bar") -> (props: UseStatusBarProps) -> Element)
	& ((name: "status-dot") -> (props: UseStatusDotProps) -> Element)
	& ((name: "status-icon") -> (props: UseStatusIconProps) -> Element)
	& ((name: "stepper") -> (props: UseStepperProps) -> Element)
	& ((name: "swap-view") -> (props: UseSwapViewProps) -> Element)
	& ((name: "swatches") -> (props: UseSwatchesProps) -> Element)
	& ((name: "switch") -> (props: UseSwitchProps) -> Element)
	& ((name: "system-load") -> (props: UseSystemLoadProps) -> Element)
	& ((name: "tab-view") -> (props: UseTabViewProps) -> Element)
	& ((name: "tabbed") -> (props: UseTabbedProps) -> Element)
	& ((name: "table") -> (props: UseTableProps) -> Element)
	& ((name: "tabs") -> (props: UseTabsProps) -> Element)
	& ((name: "tags") -> (props: UseTagsProps) -> Element)
	& ((name: "task-check") -> (props: UseTaskCheckProps) -> Element)
	& ((name: "task-row") -> (props: UseTaskRowProps) -> Element)
	& ((name: "terminal") -> (props: UseTerminalProps) -> Element)
	& ((name: "text-editor") -> (props: UseTextEditorProps) -> Element)
	& ((name: "thinking") -> (props: UseThinkingProps) -> Element)
	& ((name: "threshold-line") -> (props: UseThresholdLineProps) -> Element)
	& ((name: "tick-meter") -> (props: UseTickMeterProps) -> Element)
	& ((name: "time-picker") -> (props: UseTimePickerProps) -> Element)
	& ((name: "timeline") -> (props: UseTimelineProps) -> Element)
	& ((name: "title-bar") -> (props: UseTitleBarProps) -> Element)
	& ((name: "toast") -> (props: UseToastProps) -> Element)
	& ((name: "toasts") -> (props: UseToastsProps) -> Element)
	& ((name: "toolbar") -> (props: UseToolbarProps) -> Element)
	& ((name: "tree") -> (props: UseTreeProps) -> Element)
	& ((name: "update-pill") -> (props: UseUpdatePillProps) -> Element)
	& ((name: "usage-bar") -> (props: UseUsageBarProps) -> Element)
	& ((name: "vectorscope") -> (props: UseVectorscopeProps) -> Element)
	& ((name: "vertical-bar") -> (props: UseVerticalBarProps) -> Element)
	& ((name: "video-viewer") -> (props: UseVideoViewerProps) -> Element)
	& ((name: "waveform") -> (props: UseWaveformProps) -> Element)
	& ((name: "week-calendar") -> (props: UseWeekCalendarProps) -> Element)
	& ((name: "writing-stats") -> (props: UseWritingStatsProps) -> Element)
	& ((name: string) -> (props: any) -> Element)
-- generated: component props end

-- Prints to the terminal that runs the app.
declare function log(...: any): ()
