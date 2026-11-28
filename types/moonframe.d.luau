-- Types for Moonframe modules: the `ui`, `mod`, `style`, and `log`
-- globals the host gives every script. `just types` checks app/ against
-- them in strict mode; editors with luau-lsp read them for completion.
-- They need Luau's new type solver (luau-lsp's `--flag:LuauSolverV2=true`,
-- or the editor's "new solver" setting): `use` is typed by a type function.
--
-- These mirror what the host accepts (crates/moonframe-core). The host still
-- checks every value at run time; types catch mistakes before it runs.

-- Values that cross between a module and the host: messages and state.
export type Data = nil | boolean | number | string | { [any]: any }

export type Space = "none" | "small" | "normal" | "large"
export type Tone = "normal" | "muted" | "accent" | "success" | "warning" | "danger"
export type TextSize = "small" | "normal" | "large" | "title" | "headline" | "display" | "hero"
export type Weight = "light" | "normal" | "medium" | "semibold" | "bold"
export type Align = "stretch" | "start" | "center" | "end"
export type Justify = "start" | "center" | "end" | "between"
export type Surface = "background" | "sidebar" | "muted" | "secondary"
export type Edge = "none" | "all" | "top" | "bottom" | "left" | "right"
export type Border = boolean | Edge
export type Direction = "row" | "column"
export type Variant = "normal" | "primary" | "ghost" | "outline" | "danger"
export type Command = "quit" | "reload" | "toggle-theme" | "show-themes" | "inspect" | "minimize" | "maximize" | "close-window" | "check-for-updates" | "new-window" | "activate" | BrowserCommand
-- Run on the `ui.browser` the control's `browser` key names.
export type BrowserCommand = "browser-back" | "browser-forward" | "browser-reload" | "browser-stop" | "browser-open"

-- `#rrggbb`, a palette name, or a theme color, optionally `name/NN`.
export type Color = string
-- A space name or pixels.
export type Spacing = Space | number
-- One spacing, `{ y, x }`, `{ top, right, bottom, left }`, or named edges.
-- The list and the named edges share one table type: Luau's new solver
-- cannot match a table literal against a union of two table types.
export type Sides = Spacing | {
	[number]: Spacing,
	x: Spacing?,
	y: Spacing?,
	top: Spacing?,
	right: Spacing?,
	bottom: Spacing?,
	left: Spacing?,
}
export type ShadowLayer = {
	x: number?,
	y: number?,
	blur: number?,
	spread: number?,
	color: Color?,
	inset: boolean?,
}
-- A preset (`"small"`), a CSS string (`"0 4px 12px #0003"`), a layer, or layers.
export type Shadow = string | ShadowLayer | { string | ShadowLayer }
export type TransitionKind = "slide" | "fade" | "slide-fade" | "rise" | "none"
-- `"slide 220ms ease-out"`, or a table; durations are ms, `"220ms"`, or a speed.
export type Transition = string | {
	kind: TransitionKind?,
	duration: (number | string)?,
	curve: string?,
	-- Wait before moving, as CSS's transition-delay.
	delay: (number | string)?,
}

export type Style = {
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
export type Element = { type: string }
-- A child: an element, a string (plain text), or false/nil (skipped).
export type Child = Element | string | false | nil
-- Class names: `"a b"` or `{ "a", "b" }`.
export type Class = string | { string }

-- Where `float` pins an element over its parent.
export type FloatAt =
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
-- One table type for both tables, as for `Sides`; `at` takes no offsets.
export type Float = FloatAt | {
	at: FloatAt?,
	inset: number?,
	top: number?,
	right: number?,
	bottom: number?,
	left: number?,
}

-- Out of the flow, over the parent at fractions of its size: `{ left =
-- 0.1, top = 0.2, width = 0.5, height = 0.3 }`. A crop frame over a
-- picture (in a `ui.image`'s `overlay`, fractions of the picture), a
-- marker on a track.
export type Area = {
	left: number?,
	top: number?,
	width: number?,
	height: number?,
}

-- The pointer's shape over a row or column.
export type Cursor =
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
export type Range = {
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
export type PointerMsg = {
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
export type RotateMsg = {
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
export type SwipeMsg = {
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
export type ForceMsg = {
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
export type WheelMsg = {
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
export type StackProps = {
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
	-- Reveal this direct child (one-based) when the index changes; or the
	-- first row or column with this `key`, however deep, when the key (or
	-- `seq`, to ask again) changes; or scroll to the `y` (and `x`) its
	-- `measure` told, as far as the content goes: with a `seq`, only a new
	-- `seq` asks again, so `measure`'s `y` can be kept as it comes.
	scroll_to: (number | string | { key: string?, x: number?, y: number?, seq: any? })?,
	-- With `scroll`: sent with `width`, `height`, `content_width`,
	-- `content_height`, `x`, `y`, `fit`, `first` and `last` as they change.
	measure: Data,
	-- Sent (to `to`, else this mod) when a click begins and ends outside its
	-- box and outside the popovers and overlays inside it; the click still
	-- reaches what it landed on. A floating panel that minimizes on a click
	-- elsewhere. The GPUI window only; Rune does not send it yet.
	outside: Data,
	-- While scrolled to its end, stay there as the content grows (a chat).
	follow: boolean?,
	-- With `scroll`: sent with `at_end = true|false` as it first lays out
	-- and each time it reaches or leaves its end.
	at_end: Data,
	-- With `scroll`: scroll to the end each time this value changes.
	scroll_end: Data,
	-- false scrolls with no scrollbar drawn (the GPUI backend; Rune follows
	-- the system's setting).
	scrollbar: boolean?,
	-- false keeps the content's size when the parent is short of space;
	-- the default in a row or column that scrolls along its axis.
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
	-- Keys it takes while focus is inside it (a click in it gives it focus)
	-- and no text field has it, ahead of the mod's own `keys`.
	keys: Keys?,
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
export type CarryMsg = {
	phase: "over" | "leave" | "drop",
	carry: Data,
	from: string,
	x: number,
	y: number,
	width: number,
	height: number,
}

export type PanelProps = {
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
	-- false keeps the content's size when the parent is short of space;
	-- the default in a row or column that scrolls along its axis.
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

export type PaneProps = {
	class: Class?,
	style: Style?,
	state: string?,
	size: number?,
	-- The size a double-click on its divider puts it back to; default: the
	-- size it started with. The app's default when `size` is the user's kept one.
	reset: number?,
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

export type SplitProps = {
	id: string?,
	-- false: no hairline between panes; a handle shows only when hovered.
	divider: boolean?,
	-- Sent once the user lets go of a divider (or moves one by key or a
	-- double-click), with `sizes`: each pane's
	-- whole pixels, in order, 0 for a hidden one. Keep them in state (it
	-- outlives the app) and pass them back as the panes' `size`.
	resize: Data,
	-- The mod `resize` goes to; this one when unset.
	to: string?,
	[number]: Element | false | nil,
}

export type TextProps = {
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
export type ButtonProps = {
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

export type ItemProps = {
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

export type BadgeProps = {
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

-- The bundled icons (Lucide); the host refuses any other name, except an
-- app's own, `app-NAME` (its `icons/app-NAME.svg`), which a mod passes as a
-- string from its data: `ui.icon(name :: any)`. One the app lacks draws a
-- dashed circle, a placeholder.
export type IconName =
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
	| "brain"
	| "brand-anthropic"
	| "brand-github"
	| "brand-gitlab"
	| "brand-google-scholar"
	| "brand-linkedin"
	| "brand-mistral"
	| "brand-ollama"
	| "brand-openai"
	| "brand-slack"
	| "brand-wikipedia"
	| "brand-x"
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
	| "hand"
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
	| "mouse-pointer-2"
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
	| "sticky-note"
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
	| "type"
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
export type IconProps = {
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

export type Fit = "contain" | "cover" | "fill" | "scale-down" | "none"

-- A picture from a file: relative to the mod's folder, `~/...`, or absolute.
-- Not a URL: have a provider download it to a cache. A missing file leaves
-- an empty box of `width` x `height`. `radius` in a rule rounds it.
export type ImageProps = {
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
export type LineProps = {
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
export type RevisionTimelineProps = {
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
export type PathsProps = {
	class: Class?,
	style: Style?,
	paths: { Polyline },
	width: number?,
	height: number?,
}

export type Polyline = {
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
export type HatchProps = {
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
export type MarkdownProps = {
	class: Class?,
	style: Style?,
	state: string?,
	text: string?,
	link: Data,
	-- A link's tooltip by its address, over the link's own title
	-- (`[text](url "title")`, which shows when this names none and every
	-- link to that address has the same title).
	tooltips: { [string]: string }?,
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

export type TerminalEngine = "alacritty" | "ghostty"
-- What keeps the shell running: `auto` (default) is herdr-core, else tmux,
-- else a pty. herdr-core and tmux keep it across launches; a pty ends with
-- the app. One that isn't installed falls back to the next.
export type TerminalBackend = "auto" | "herdr-core" | "tmux" | "pty"

-- A live shell (your login shell). The host keeps it by mod and `id`.
export type TerminalProps = {
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
export type BrowserProps = {
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

-- HTML shown sealed, kept by mod and `id`: its scripts never run and it
-- loads nothing but itself (pictures and fonts inline, as `data:`), and it
-- follows the app's light or dark look. Give `html` or `src`, a file named
-- as `ui.image` names pictures (the app's folders or the image cache, where
-- a provider writes a page too large to publish). Links are told, never followed; `#part`
-- scrolls to it. A table message gets the fields added; any other value
-- arrives as `{ msg = ... }`.
export type DocumentProps = {
	class: Class?,
	style: Style?,
	state: string?,
	id: string?,
	html: string?,
	src: string?,
	-- Sent as a link is clicked, with `url` (as written) and `text` added.
	link: Data,
	-- Sent as it scrolls, at most every 100 ms, with `y`, `height` and
	-- `content_height` (points) added.
	scroll: Data,
	-- When it changes (a new `seq` asks again): a `y` (points down, as
	-- `scroll` told) or an element's `id`, brought to the top.
	scroll_to: (number | string | { y: number?, id: string?, seq: any? })?,
	-- Sent as the selection changes, with `text` ("" once none) and its box
	-- `x`, `y`, `width`, `height`, in points from the box's top left as it
	-- shows.
	select: Data,
	-- Edited in place; a link then opens with cmd (ctrl) and a click.
	editable: boolean?,
	-- While editable: sent with `html`, the whole document, half a second
	-- after an edit; give it back as `html` and nothing reloads. Grown past
	-- the 8 MB `html` takes, it comes with `error` instead.
	change: Data,
	-- While editable: sent with `html` as the document loses the keyboard,
	-- and `edited`, whether it was typed into since the last `done` (or
	-- since it loaded): skip saving a blur that changed nothing.
	done: Data,
	[number]: string,
}

-- A view the app writes in Rust and registers by `name` (an app on the
-- Moonframe library; the starter has none). The host keeps it by mod and
-- `id`, so it lives across reloads and while hidden, and hands it `props`,
-- telling it when they change. What the user does in it comes back to this
-- mod's `update` as messages the view chooses.
-- One node of a `ui.diagram` document. With `x`, `y`, `w` and `h` it stays
-- where it is; without, the diagram is laid out.
export type DiagramNode = {
	id: string,
	label: string?,
	sublabel: string?,
	-- box rounded pill diamond circle (ellipse) cylinder note (sticky) text
	kind: string?,
	tone: Tone?,
	-- `#rrggbb`, over the card color.
	fill: string?,
	group: string?,
	x: number?,
	y: number?,
	w: number?,
	h: number?,
}

-- One edge of a `ui.diagram` document.
export type DiagramEdge = {
	id: string?,
	from: string,
	to: string,
	label: string?,
	kind: ("solid" | "dashed" | "dotted" | "thick" | "invisible")?,
	dashed: boolean?,
	-- none arrow open circle cross triangle diamond hollow-diamond
	start: string?,
	["end"]: string?,
	tone: Tone?,
}

-- A diagram the view lays out: a flow of nodes, edges and groups, or a
-- sequence of messages between participants (its `nodes`).
export type DiagramDocument = {
	kind: ("flow" | "sequence")?,
	direction: ("down" | "up" | "right" | "left")?,
	-- `DiagramNode`s and `DiagramEdge`s.
	nodes: { any }?,
	edges: { any }?,
	groups: { { id: string, label: string?, parent: string?, tone: Tone? } }?,
	messages: { { from: string, to: string, label: string?, kind: ("sync" | "async" | "reply")? } }?,
	notes: { { over: { string }, text: string, before: number? } }?,
}

-- `ui.diagram` (the `diagram` feature): a diagram drawn, laid out and
-- edited in Rust. The mod keeps the document and applies what it hears:
-- `selected`, `moved`, `resized`, `created`, `connected`, `deleted`,
-- `edited`, `opened`, `zoom`, `tool`.
export type DiagramProps = {
	id: string?,
	class: Class?,
	style: Style?,
	-- The Luau `diagram` component's document: every shape placed. Each
	-- is a `DiagramNode` and a `DiagramEdge` (typed `any`: a mod's own
	-- record types would not pass, as Luau's lists are invariant).
	shapes: { any }?,
	links: { any }?,
	-- Or a document to lay out.
	document: DiagramDocument?,
	-- Or Mermaid (flowchart, sequence, state, class, er) or Graphviz DOT
	-- text, laid out the same way; what does not parse shows why.
	mermaid: string?,
	dot: string?,
	-- "edit" changes it (the default with `shapes`), "view" only looks.
	mode: ("view" | "edit")?,
	-- select hand connector, or a node kind to draw.
	tool: string?,
	selected: { string }?,
	-- Nodes and edges drawn in the accent.
	highlight: { string }?,
	-- Asked again with each new `seq`.
	camera: { fit: boolean?, zoom: number?, zoom_by: number?, seq: number? }?,
	-- "focus": scrolling pans only after a click inside (in a page that
	-- scrolls, as a chat does).
	scroll: "focus"?,
	[string]: any,
}

export type ViewProps = {
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
export type InputProps = {
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
	-- Sent on Enter, or ⌘/Ctrl-Enter when `multiline` (and Enter too with
	-- `enter = "submit"`).
	submit: Data,
	-- What Enter does in a multiline field: `"newline"` (default; ⌘/Ctrl-Enter
	-- submits) or `"submit"`, where Shift-Enter makes a new line, as in a
	-- chat's composer.
	enter: ("submit" | "newline")?,
	-- What was sent before, oldest first: Up with the caret on the first
	-- line recalls the entry before the one shown, Down on the last line the
	-- one after, and Down past the newest puts back what was being written.
	-- Each recall replaces the text as an edit, so `change` is sent.
	history: { string }?,
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
	-- A password: drawn as bullets, never copied or cut; one line only.
	secret: boolean?,
	-- Sent with `files` (paths) and, for a picture, `image = { path, width,
	-- height }` when files or a picture are pasted into it; text pastes.
	paste: Data,
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
	-- Where the caret goes when the field takes a new `value`, or when this
	-- changes: "start", "end" (after a recalled line), or a byte offset, as
	-- `change` and `select` count `caret`.
	caret: ("start" | "end" | number)?,
	-- With `multiline`: sent as the text scrolls (the wheel, the scrollbar,
	-- the caret carrying it), with `fraction` added, from 0 (the top) to 1
	-- (the last line at the bottom). Not sent for `scroll_at`.
	scrolled: Data,
	-- With `multiline`: scrolls the text to `fraction` (0 to 1) when it
	-- changes; a new `seq` goes to one place again.
	scroll_at: { fraction: number, seq: number? }?,
}

export type RegionProps = {
	name: string,
	direction: Direction?,
	gap: Space?,
	justify: Justify?,
}

-- Which side of its anchor a popover opens on.
export type Side = "top" | "bottom" | "left" | "right"
-- The anchor's edge it lines up with: left or right above and below, top or
-- bottom beside.
export type Anchor = "start" | "end"

-- Keystrokes an open popover or overlay takes, to the message each sends
-- to this mod's `update`: `{ escape = "close", down = "next", ["cmd-k"] = "find" }`.
-- A name is modifiers then a key, joined by `-`: modifiers `ctrl`, `alt`,
-- `shift`, `cmd` (Super/Windows elsewhere) and `secondary` (cmd on macOS,
-- ctrl elsewhere); keys `a`-`z`, `0`-`9`, a symbol such as `/`, `f1`-`f24`,
-- `escape`, `enter`, `tab`, `space`, `backspace`, `delete`, `insert`, `up`,
-- `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`.
export type Keys = { [string]: Data }

-- Content floating beside its anchor (the list part). While `open`, a click
-- outside sends `dismiss`. A `popover` stylesheet rule styles and animates it.
export type PopoverProps = {
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

export type Placement =
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
export type OverlayProps = {
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

-- Rows, of one height or each its own, of which only those showing are
-- built and drawn, so a list of any length costs a screenful. `row(i)`
-- builds row `i` (from 1) and is called again as the list scrolls; keep it
-- cheap and pure.
export type ListProps = {
	class: Class?,
	style: Style?,
	state: string?,
	-- Names it among this mod's lists (default "list"); two need two ids.
	id: string?,
	count: number,
	-- Every row's height in pixels, or a function giving row `i`'s own
	-- (called for every row each time the view runs; up to 100,000 rows).
	row_height: number | ((index: number) -> number),
	row: (index: number) -> Child,
	-- Sent with `first` and `last`, the rows (from 1) any part of which
	-- shows, and `y`, the pixels it is scrolled down (0 at the top), as
	-- they change while it scrolls and resizes.
	range: Data,
	-- When it changes (a new `seq` asks again): reveal row n, scrolling
	-- only as far as needed (`n` or `{ row = n, seq = s }`), or go back to
	-- a `y` its `range` told (`{ y = px, seq = s }`).
	scroll_to: (number | { row: number?, y: number?, seq: any? })?,
	-- The mod `range` goes to, if not the drawing one.
	to: string?,
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
export type VideoGuide = {
	x: number,
	y: number,
	width: number?,
	height: number?,
	-- #rrggbb or #rrggbbaa.
	color: string?,
}

-- `ui.video { source = "camera" }`: frames from a ring in shared memory.
export type VideoProps = {
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
export type MapProvider =
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
export type MapProps = {
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

export type MapPin = {
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
export type MapRoute = {
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
export type NumberOptions = {
	decimals: number?,
}

-- `ui.date`'s options: how long (`style`, default "medium"), whether the
-- date shows (default true), and the time of day (true or "minutes",
-- "seconds"; default none).
export type DateOptions = {
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
	document: (props: string | DocumentProps) -> Element,
	video: (props: string | VideoProps) -> Element,
	view: (props: string | ViewProps) -> Element,
	diagram: (props: DiagramProps) -> Element,
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
export type GitCommit = {
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
-- What the `system` provider publishes, as `app.system`; declared in
-- app/providers/system/types.luau.
export type System = ProviderSystemPublished

-- `ios` is what MoonSwift reports on an iPhone or iPad.
export type Platform = "macos" | "linux" | "windows" | "ios"
export type Titlebar = "system" | "hidden"

-- The window, for a title bar the layout draws itself.
export type WindowState = {
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
	-- Its content's size in whole points, updated as it is resized: switch
	-- to a narrow layout under a breakpoint. Nil where the renderer does
	-- not say (Rune).
	width: number?,
	height: number?,
}

-- A screen, in `app.displays`. `x`, `y`, `width` and `height` are its
-- place among the others, in points.
export type DisplayInfo = {
	id: number,
	primary: boolean,
	x: number,
	y: number,
	width: number,
	height: number,
}

-- How a process or socket provider's link stands, in `app.links`.
export type ProviderLink = {
	-- Whether the provider has its program.
	connected: boolean?,
	-- Messages not yet written.
	waiting: number,
	-- Messages dropped since it started: the queue was full or, for a
	-- socket, no one was there to hear them.
	dropped: number,
}

-- How `app:awake` stands, in `app.awake`.
export type AwakeState = {
	-- Whether the system agreed to stay awake.
	held: boolean,
	-- Whether the display stays on too.
	display: boolean,
	-- Why it would not, if it would not.
	problem: string?,
}

-- The last run's panic, in `app.crash`.
export type CrashReport = {
	-- What it said.
	message: string,
	-- The report's path, for the user to read or send.
	report: string,
}

-- Light, dark, or as the system is (switching as it does).
export type AppearanceMode = "system" | "light" | "dark"

-- `app.appearance`: set it with `{ to = "app:appearance", msg = "dark" }`.
export type Appearance = {
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
export type ThemeInfo = {
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
export type GitFile = {
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

export type GitWorktree = {
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

export type GitBranch = {
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

export type GitRemote = {
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

export type GitDiffLine = {
	-- "add", "del" or "ctx".
	kind: string,
	-- Line numbers in the old and new file; `old` is nil on "add" lines,
	-- `new` on "del" ones.
	old: number?,
	new: number?,
	-- Cut to 300 characters.
	text: string,
}

export type GitDiff = {
	path: string,
	binary: boolean,
	-- Lines in all hunks; at most 1200, then `truncated`.
	lines: number,
	truncated: boolean,
	hunks: { { header: string, old_start: number, new_start: number, lines: { GitDiffLine } } },
}

-- One commit with what it changed, against its first parent (a first
-- commit against nothing).
export type GitShown = {
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

export type Git = {
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
export type ForgePerson = {
	login: string,
	name: string?,
	bot: boolean?,
	avatar_url: string?,
	avatar_path: string?,
}

export type ForgeChecks = {
	total: number,
	passed: number,
	failed: number,
	pending: number,
	skipped: number,
}

-- A pull request (GitHub) or merge request (GitLab).
export type ForgePr = {
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

export type ForgeIssue = {
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
export type ForgeAuth = {
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
export type Forge = {
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
-- What the `agents` provider publishes, as `app.agents`; declared in
-- app/providers/agents/types.luau.
export type Agent = ProviderAgentsAgent
export type Agents = ProviderAgentsPublished

-- One entry of a folder, from the `files` provider.
export type FileEntry = {
	name: string,
	dir: boolean,
	-- Ignored by git: editors dim these.
	ignored: boolean,
}

export type Files = {
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
export type Docs = {
	available: boolean,
	reason: string?,
	root: string?,
}

-- One limit window of a service's plan (the `usage` provider). Shares are
-- 0 to 1; `pace` is where an even spend would have left the bar of what is
-- left (`usage-bar`'s `pace`), nil early in a window or without a reset.
export type UsageWindow = {
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

export type UsageBalance = {
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

export type UsageSection =
	{ kind: "limit", window: UsageWindow }
	| { kind: "facts", title: string, facts: { { label: string, value: string } } }
	| { kind: "shares", title: string, shares: { { label: string, share: number } } }

-- One service signed in on the host, or one `show_providers` asks for.
export type UsageProvider = {
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
export type Usage = {
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

export type Herdr = {
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
export type AcpAgent = {
	-- "claude", "codex", "copilot", "gemini" or "opencode".
	name: string,
	title: string,
	-- Its ACP command is installed.
	installed: boolean,
	-- Only `npx` can start it, downloading its adapter first.
	fetched: boolean,
}

export type AcpTool = {
	id: string,
	title: string,
	-- "read", "edit", "delete", "move", "search", "execute", "think", "fetch", "other"...
	kind: string,
	-- "pending", "in_progress", "completed" or "failed".
	status: string,
	-- How long it took, in milliseconds, once it ended.
	ms: number?,
}

export type AcpRun = {
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

export type AcpTiming = {
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
export type AcpTarget = {
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

export type Acp = {
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
export type SpeechEngine = {
	name: string,
	title: string,
	kind: "apple" | "command" | "remote",
	-- Whether the user's voice leaves this machine.
	remote: boolean,
	ready: boolean,
	-- What the user can do about it, when it is not ready.
	reason: string?,
}

export type Speech = {
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
export type Reply = {
	reply: any,
	action: string,
	ok: boolean,
	-- What happened, when it worked; why not, when it did not.
	message: string?,
	error: string?,
}

-- `ui.mount { "explorer", id = "files-2", props = { ... } }`.
export type MountProps = {
	mod: string?,
	id: string?,
	props: any,
	[number]: string,
}

-- What draws this build; `swiftui` is what MoonSwift reports.
export type Renderer = "gpui" | "rune" | "swiftui"

-- What every `view` gets as `app`.
export type App = {
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
	-- Nil until each provider first publishes.
	git: Git?,
	github: Forge?,
	gitlab: Forge?,
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
	-- Each provider with a types.luau beside its provider.toml (`system`,
	-- `agents`, ...), nil until it first publishes.
	-- generated: provider fields (just provider-types) start
	agents: ProviderAgentsPublished?,
	system: ProviderSystemPublished?,
	t3: ProviderT3Published?,
	-- generated: provider fields end
	-- What any other provider publishes, as `app.<name>`.
	[string]: any,
}

-- What `{ to = "app:notify", msg = ... }` posts. A click on it brings the
-- window forward and sends the posting mod, if it subscribes to
-- `app/notification`, `{ topic = "app/notification", value = NotificationClick }`.
-- `{ ask = true }` instead posts nothing and asks the user to allow
-- notifications now (once; the first one posted asks otherwise), with
-- `app.notifications` saying the answer.
export type NotifyMsg = {
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
export type NotificationPermission = "allowed" | "denied" | "undecided" | "in-window"

export type NotificationClick = {
	id: string?,
	clicked: boolean,
}

-- A link to one of the app's URL schemes (`schemes` in its moonframe.toml),
-- opened in a browser or another app: a module that subscribes to `app/url`
-- hears `{ topic = "app/url", value = AppUrl }`. Anyone can open one: treat
-- what it carries as untrusted.
export type AppUrl = {
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
export type PickMsg = {
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
export type ShareMsg = {
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
export type StatusItemMsg = {
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
export type StatusLine = "-" | {
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
export type HotkeyMsg = {
	keys: string,
	msg: Data?,
	to: string?,
	release: boolean?,
}

-- What `{ to = "app:pdf", msg = ... }` makes: a page as a PDF file (macOS).
-- One of `html`, `html_file` and `url`. With `reply`, the mod hears it back
-- with `path` (the file written) or `error`.
export type PdfMsg = {
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

-- What `{ to = "app:text", msg = ... }` asks: the text `mod` (or the mods
-- of `region`) draws, rows across and columns down, with what it mounts
-- and without fields, hidden panes or closed popovers; and the text the
-- user last selected in a document, kept after a click into a field clears
-- it. The mod hears `reply` back with `text` and `selection` (`{ text, mod }`,
-- the mod that drew it) added, each cut to `max` characters (64,000).
export type TextMsg = {
	mod: string?,
	region: string?,
	max: number?,
	reply: Data,
}

-- What `{ to = "app:edit", msg = ... }` does to the sending mod's input
-- with `id = field`, around its selection, keeping undo and focus; the
-- field then sends its `change`. `wrap` puts `{ before, after }` around the
-- selection (taking them off when already there), `lines` starts each
-- selected line with a prefix (taking it off when all have it), `insert`
-- replaces the selection and the `back` bytes before it: an `@name`
-- completed, an emoji. A toolbar button can send it directly with
-- `to = "app:edit"`.
export type EditMsg = {
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
export type WindowMsg = string | { open: string, send: Data?, size: { width: number, height: number }? } | { close: string }

-- What the Developer tools (mods/devtools, and only it) ask with
-- `{ to = "app:devtools", msg = ... }`: where they show; a Luau file of the
-- app's folders to read (answered with `text`) or save (with `ok`, or
-- `error`); a mod's state (with `state`, `problem`); or another element of
-- the selection's trail to select. Answers come back as `reply` with fields
-- added; picks arrive on `app/selection`, the place on `app/devtools`.
export type DevtoolsMsg =
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
-- `StatusItemMsg`), `"app:hotkey"` (`msg` a `HotkeyMsg`), `"app:pdf"` (`msg` a `PdfMsg`), `"app:text"` (`msg` a `TextMsg`) and `"app:open"` (`msg` a web address, opened in the system's browser, or one `mailto:` address without headers, opened in its mail app).
export type Send = {
	to: string,
	msg: Data,
}

-- generated: provider types (just provider-types) start
type ProviderAgentsAgent = {
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
type ProviderAgentsPublished = {
	available: boolean,
	reason: string?,
	agents: { ProviderAgentsAgent },
	count: number,
}
type ProviderSystemPublished = {
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
type ProviderSystemMessage = "clear"
type ProviderT3Link = { label: string, title: string, url: string }
type ProviderT3Thread = {
	id: string,
	title: string,
	project: string,
	projectId: string,
	provider: string?,
	status: string,
	group: string,
	updatedAt: string,
	branch: string?,
	worktree: string?,
	pullRequests: { ProviderT3Link },
	issues: { ProviderT3Link },
}
type ProviderT3Result = { status: string, error: string?, threadId: string? }
type ProviderT3Project = { id: string, label: string }
type ProviderT3Published = {
	available: boolean,
	environmentId: string?,
	projects: { ProviderT3Project },
	writer: string?,
	ticket: string?,
	results: { [string]: ProviderT3Result },
	error: string?,
	setup: ("install" | "start" | "pair")?,
	threads: { ProviderT3Thread },
	total: number,
}
type ProviderT3Write = {
	action: "start" | "send",
	target: string,
	text: string,
	environmentId: string,
	ticket: string,
	id: string,
}
type ProviderT3Message = "refresh" | "connect" | ProviderT3Write

-- A message for `update` to return: `send "provider:desk" { watch = id }`
-- is `{ to = "provider:desk", msg = { watch = id } }`, its `msg` typed
-- by the provider's `Message` where it declares one.
declare send:
	((to: "provider:system") -> (msg: ProviderSystemMessage) -> Send)
	& ((to: "provider:t3") -> (msg: ProviderT3Message) -> Send)
	& ((to: string) -> (msg: Data) -> Send)
-- generated: provider types end

-- `titlebar = "hidden"` puts the layout under the title bar (macOS keeps its
-- traffic lights, at `traffic_lights` from the top left) or drops the system
-- frame (Linux, Windows). Give the layout a `drag = true` row to move it.
export type Window = {
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
export type ModWindow = {
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

-- What a mod's `window` takes: a `Window` in main.luau, a `ModWindow` in
-- another mod, in one table type with the fields of both, since Luau's new
-- solver cannot match a table literal against a union of two table types.
-- The host reads the fields that apply where the mod is.
export type WindowFrame = {
	titlebar: Titlebar?,
	traffic_lights: { x: number?, y: number? }?,
	size: { width: number?, height: number? }?,
	remember: boolean?,
	min_size: { width: number, height: number }?,
	display: ("primary" | "external" | number)?,
	fullscreen: boolean?,
}

-- A menu item in the shell's `menus`: it sends `msg` (to the shell, or to
-- `to`) or runs `command`; `shortcut` is a key as `keys` take them, such as
-- "secondary-t". "-" between items draws a line.
export type MenuItem = {
	label: string,
	msg: Data?,
	to: string?,
	command: Command?,
	shortcut: string?,
}

-- `items` holds `MenuItem`s and "-"; typed loosely so items with `msg` and
-- with `command` mix in one list. The host checks each.
export type AppMenu = {
	title: string,
	items: { any },
}

-- What the About panel says besides the app's name and icon.
export type About = {
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
export type TabEntry = {
	title: string?,
	icon: IconName?,
	props: any,
	report: string?,
}

-- One of `app.kinds`: what a tab bar can open.
export type TabKind = {
	-- "component:terminal" or "mod:explorer".
	kind: string,
	title: string,
	icon: string?,
	props: any,
	report: string?,
}

export type Mod<State> = {
	-- `props` is what an instance was mounted with (`ui.mount { "name",
	-- id = ..., props = ... }`), nil for a mod in its own place. `restored`
	-- says `previous` was saved by an earlier run, not kept across a reload.
	-- Returns the state and, as `update` may, a list of `Send`s.
	init: ((previous: State?, props: any?, restored: boolean) -> (State, ...{ Send }))?,
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
	-- Sent with `files` (paths) and, for a picture, `image = { path, width,
	-- height }` when files or a picture are pasted while no text field or
	-- terminal has focus. A paste of text is not.
	paste: Data,
	-- Offered as a kind of tab: each tab mounts an instance of it.
	tab: TabEntry?,
	region: string?,
	order: number?,
	title: string?,
	-- Rules for what this mod draws, by selector.
	styles: { [string]: Style }?,
	-- In main.luau, the main window's frame, read when the app starts (a
	-- `Window`); in another mod, the size of its own window (a `ModWindow`).
	window: WindowFrame?,
	-- Only in main.luau: menus after the app menu (a "View" one merges into
	-- the host's), and what About says.
	menus: { AppMenu }?,
	about: About?,
}

declare function mod<State>(definition: Mod<State>): Mod<State>

-- The fixed-width font terminals and `font = "mono"` text use.
export type MonoFont = {
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

export type Stylesheet = {
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
export type Component<Props> = {
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
	read open: boolean?,
	read dismiss: any,
	read name: string,
	read version: string?,
	read tagline: string?,
	read icon: string?,
	read image: string?,
	read info: { { string } }?,
	read links: { UseAboutLink }?,
	read copyright: string?,
	read copy: string?,
	read inline: boolean?,
	read width: number?,
	read to: string?,
}
type UseAccordionItem = any
type UseAccordionProps = {
	read items: { UseAccordionItem },
	read open: any,
	read event: string?,
	read to: string?,
}
type UseActionTilesTile = any
type UseActionTilesRow = any
type UseActionTilesProps = {
	read title: string?,
	read subtitle: string?,
	read trailing: any,
	read tiles: { UseActionTilesTile }?,
	read columns: number?,
	read rows: { UseActionTilesRow }?,
	read destructive: UseActionTilesRow?,
	read width: number?,
	read event: string?,
	read to: string?,
}
type UseActivityBarItem = any
type UseActivityBarProps = {
	read items: { UseActivityBarItem },
	read bottom: { UseActivityBarItem }?,
	read selected: string?,
	read event: string?,
	read to: string?,
	read height: number?,
	read fill: boolean?,
}
type UseAgentChatProps = {
	read id: string?,
	read chat: string?,
	read agent: string?,
	read cwd: string?,
	read context: string?,
	read placeholder: string?,
	read suggestions: { any }?,
	read title: string?,
	read started: any,
	read link: any,
	read to: string?,
	read view: any,
}
type UseAgentChatState = any
type UseAgentRunTool = any
type UseAgentRunOption = any
type UseAgentRunRun = any
type UseAgentRunProps = {
	read run: UseAgentRunRun,
	read lines: number?,
	read tools: number?,
	read request: boolean?,
	read to: string?,
}
type UseAgentRunLook = any
type UseAheadBehindProps = {
	read ahead: number?,
	read behind: number?,
	read synced: boolean?,
	read tooltip: string?,
}
type UseAudioMeterChannel = any
type UseAudioMeterProps = {
	read channels: { UseAudioMeterChannel },
	read height: number?,
	read bar: number?,
	read floor: number?,
	read warn: number?,
	read danger: number?,
}
type UseAvatarStackProps = {
	read people: { any },
	read count: number?,
	read most: number?,
	read size: string?,
}
type UseAvatarProps = {
	read name: string?,
	read initial: string?,
	read letters: number?,
	read size: string?,
	read shape: string?,
	read color: string?,
	read status: string?,
	read src: string?,
	read tooltip: string?,
}
type UseBannerAction = any
type UseBannerProps = {
	read text: string,
	read detail: string?,
	read icon: string?,
	read tone: string?,
	read solid: boolean?,
	read mono: boolean?,
	read action: any,
	read dismiss: any,
	read to: string?,
}
type UseBarChartSeries = any
type UseBarChartBucket = any
type UseBarChartProps = {
	read buckets: { UseBarChartBucket },
	read series: { UseBarChartSeries }?,
	read height: number?,
	read bar: number?,
	read gap: number?,
	read max: number?,
	read axis: boolean?,
	read legend: boolean?,
	read unit: string?,
	read every: number?,
	read slot: number?,
	read ticks: number?,
	read grid: boolean?,
	read grow: boolean?,
}
type UseBoardCardProps = {
	read title: string,
	read subtitle: string?,
	read badge: string?,
	read badge_tone: string?,
	read amount: string?,
	read date: string?,
	read owner: string?,
	read owner_color: string?,
	read tone: string?,
	read msg: any,
	read context: any,
	read to: string?,
	read selected: boolean?,
	read labels: { any }?,
	read key: string?,
	read carry: any,
	read accept: any,
}
type UseBreadcrumbsCrumb = any
type UseBreadcrumbsProps = {
	read crumbs: { UseBreadcrumbsCrumb },
	read back: any,
	read position: string?,
	read up: any,
	read down: any,
	read actions: { any }?,
	read event: string?,
	read to: string?,
}
type UseBrowserPage = any
type UseBrowserProps = {
	read id: string?,
	read url: string?,
	read page: UseBrowserPage?,
	read navigate: any,
	read loading: any,
	read zoom: number?,
	read share: boolean?,
}
type UseButtonGroupButton = any
type UseButtonGroupProps = {
	read buttons: { UseButtonGroupButton },
	read selected: string?,
	read event: string?,
	read small: boolean?,
	read to: string?,
}
type UseCaffeineAwake = any
type UseCaffeineProps = {
	read awake: UseCaffeineAwake?,
	read scope: string?,
	read reason: string?,
	read variant: string?,
	read size: string?,
	read disabled: boolean?,
}
type UseCardProps = {
	read title: string?,
	read subtitle: string?,
	read actions: { any }?,
	read [number]: any,
}
type UseChangeTreeProps = {
	read files: { string }?,
	read selected: string?,
	read folded: { [string]: boolean? }?,
	read fold: string?,
	read pick: string?,
	read to: string?,
	read indent: number?,
	read counts: boolean?,
	read mark: string?,
	read plain: boolean?,
}
type UseChangeTreeFolder = any
type UseChatMarkdownProps = {
	read text: string?,
	read cursor: boolean?,
	read link: any,
	read to: string?,
	read tooltips: { [string]: string }?,
}
type UseChatMessageProps = {
	read from: string?,
	read author: string?,
	read icon: string?,
	read text: string?,
	read [number]: any,
}
type UseChatQuestionProps = {
	read id: string?,
	read permission: any,
}
type UseChatQuestionState = any
type UseChatWaitingProps = {
	read view: any,
	read chat: string?,
	read plan_open: boolean?,
	read plan_toggle: any,
}
type UseChatWorkingProps = {
	read working: boolean?,
	read waiting: boolean?,
	read step: string?,
	read working_for: string?,
	read activities: { any }?,
	read open: boolean?,
	read toggle: any,
}
type UseCheckboxProps = {
	read checked: boolean?,
	read indeterminate: boolean?,
	read label: string?,
	read description: string?,
	read msg: any,
	read disabled: boolean?,
	read to: string?,
}
type UseChecksCheck = any
type UseChecksProps = {
	read checks: { UseChecksCheck },
	read title: string?,
	read event: string?,
	read to: string?,
	read collapsed: boolean?,
	read toggle: any,
}
type UseChipsChip = any
type UseChipsProps = {
	read chips: { UseChipsChip },
	read selected: { string }?,
	read event: string?,
	read wrap: boolean?,
	read to: string?,
}
type UseChoiceCardsCard = any
type UseChoiceCardsProps = {
	read options: { UseChoiceCardsCard },
	read selected: string?,
	read width: number?,
	read event: string?,
	read to: string?,
}
type UseCodeViewProps = {
	read text: string?,
	read lines: { string }?,
	read start: number?,
	read highlight: { number }?,
	read numbers: boolean?,
	read path: string?,
	read language: string?,
	read height: number?,
	read grow: boolean?,
	read event: string?,
	read to: string?,
	read id: string?,
}
type UseCollapsibleProps = {
	read title: string,
	read open: boolean?,
	read summary: string?,
	read icon: string?,
	read toggle: any,
	read id: string?,
	read to: string?,
	read flat: boolean?,
	read [number]: any,
}
type UseCommandPaletteCommand = any
type UseCommandPaletteState = any
type UseCommandPaletteProps = {
	read id: string?,
	read open: boolean?,
	read commands: { UseCommandPaletteCommand }?,
	read placeholder: string?,
	read look: string?,
	read dismiss: any,
	read event: string?,
	read width: number?,
	read rows: number?,
	read empty: string?,
	read prefix: string?,
}
type UseCommandPaletteShape = any
type UseCommandPaletteRanked = any
type UseCommentCardProps = {
	read author: string,
	read action: string?,
	read age: string?,
	read seconds: number?,
	read body: string?,
	read location: string?,
	read locate: any,
	read badge: string?,
	read avatar: string?,
	read to: string?,
	read actions: { any }?,
	read reactions: { UseCommentCardReaction }?,
	read react: string?,
	read add_reaction: any,
	read picker: any,
	read picker_open: boolean?,
	read [number]: any,
}
type UseCommentCardReaction = any
type UseCommitGraphCommit = any
type UseCommitGraphProps = {
	read commits: { UseCommitGraphCommit },
	read select: any,
	read selected: string?,
	read row_height: number?,
	read compact: boolean?,
}
type UseCommitGraphSpot = any
type UseCommitGraphEdge = any
type UseComposerProps = {
	read id: string?,
	read value: string?,
	read placeholder: string?,
	read agent: string?,
	read icon: string?,
	read change: any,
	read submit: any,
	read attach: any,
	read send: any,
	read actions: { any }?,
	read to: string?,
	read focus: boolean?,
}
type UseConfirmProps = {
	read open: boolean?,
	read title: string,
	read message: string?,
	read detail: string?,
	read items: { any }?,
	read items_title: string?,
	read confirm: any,
	read cancel: any,
	read confirm_label: string?,
	read cancel_label: string?,
	read danger: boolean?,
	read icon: string?,
	read to: string?,
}
type UseContextMenuProps = {
	read open: boolean?,
	read context: any,
	read dismiss: any,
	read items: { any },
	read highlighted: any,
	read expanded: any,
	read expand: string?,
	read submenu_highlighted: any,
	read event: string?,
	read keys: { [string]: any }?,
	read width: number?,
	read side: string?,
	read align: string?,
	read to: string?,
	read [number]: any,
}
type UseContributionGraphProps = {
	read days: { number },
	read start: string,
	read thresholds: { number }?,
	read palette: { string }?,
	read color: string?,
	read empty: string?,
	read size: number?,
	read gap: number?,
	read week_start: string?,
	read weekdays: boolean?,
	read months: boolean?,
	read legend: boolean?,
	read noun: string?,
	read nouns: string?,
	read note: string?,
	read note_msg: any?,
	read selected: string?,
	read event: string?,
	read to: string?,
}
type UseCopyFieldProps = {
	read text: string,
	read label: string?,
	read copied: boolean?,
	read msg: any,
	read button: string?,
	read to: string?,
}
type UseCountBubbleProps = {
	read count: number,
	read max: number?,
	read tone: string?,
	read zero: boolean?,
	read tooltip: string?,
	read float: any,
}
type UseCountryCodeProps = { read code: string }
type UseCourseCardAction = any
type UseCourseCardProps = {
	read title: string,
	read eyebrow: string?,
	read meta: string?,
	read primary: UseCourseCardAction?,
	read secondary: UseCourseCardAction?,
	read progress: number?,
	read label: string?,
	read detail: string?,
	read bar: number?,
	read aside: any,
	read to: string?,
}
type UseCurveEditorPoint = any
type UseCurveEditorProps = {
	read points: { UseCurveEditorPoint },
	read change: any,
	read size: number?,
	read color: string?,
	read values: { number }?,
	read grid: number?,
	read id: string?,
	read disabled: boolean?,
}
type UseCurveEditorState = any
type UseDashboardCardProps = {
	read title: string,
	read icon: string?,
	read meta: any,
	read menu: any,
	read to: string?,
	read grow: boolean?,
	read padding: boolean?,
	read footer: any,
	read [number]: any,
}
type UseDatePickerPreset = any
type UseDatePickerProps = {
	read title: string?,
	read weeks: { any },
	read selected: string?,
	read weekdays: { string }?,
	read event: string?,
	read previous: any,
	read next: any,
	read today: any,
	read presets: { UseDatePickerPreset }?,
	read preset: string?,
	read presets_side: string?,
	read open: boolean?,
	read toggle: any,
	read dismiss: any,
	read value: string?,
	read placeholder: string?,
	read overdue: boolean?,
	read width: number?,
	read align: string?,
	read to: string?,
}
type UseDateTileProps = {
	read month: string,
	read day: string,
	read tone: string?,
	read size: string?,
}
type UseDayStripDay = any
type UseDayStripProps = {
	read days: { UseDayStripDay },
	read selected: string?,
	read event: string?,
	read to: string?,
}
type UseDiagramShape = any
type UseDiagramLink = any
type UseDiagramProps = {
	read id: string,
	read shapes: { UseDiagramShape },
	read links: { UseDiagramLink }?,
	read selected: { string }?,
	read tool: string?,
	read version: number?,
}
type UseDiagramView = any
type UseDiagramDrag = any
type UseDiagramState = any
type UseDiagramBox = any
type UseDialogButton = any
type UseDialogProps = {
	read open: boolean?,
	read dismiss: any,
	read title: string?,
	read subtitle: string?,
	read icon: string?,
	read tone: string?,
	read size: string?,
	read width: number?,
	read height: number?,
	read place: string?,
	read backdrop: boolean?,
	read hint: boolean?,
	read plain: boolean?,
	read close: boolean?,
	read buttons: { UseDialogButton }?,
	read actions: { any }?,
	read note: string?,
	read submit: any,
	read keys: { [string]: any }?,
	read to: string?,
	read class: string?,
	read [number]: any,
}
type UseDictationProps = {
	read id: string?,
	read text: any,
	read failed: any,
	read listening: any,
	read engine: string?,
	read speech: any,
}
type UseDictationState = any
type UseDiffCountsProps = {
	read added: number?,
	read removed: number?,
	read files: number?,
	read size: string?,
	read zero: boolean?,
}
type UseDiffViewLine = any
type UseDiffViewHunk = any
type UseDiffViewProps = {
	read path: string?,
	read status: string?,
	read hunks: { UseDiffViewHunk },
	read split: boolean?,
	read collapsed: boolean?,
	read height: number?,
	read grow: boolean?,
	read header: boolean?,
	read event: string?,
	read to: string?,
	read actions: { any }?,
	read switch: string?,
}
type UseDocumentCardProps = {
	read title: string,
	read text: string?,
	read date: string?,
	read lines: number?,
	read favorite: boolean?,
	read star: any?,
	read selected: boolean?,
	read msg: any?,
	read context: any?,
	read width: number?,
	read height: number?,
	read to: string?,
}
type UseDocumentRowProps = {
	read title: string,
	read excerpt: string?,
	read date: string?,
	read words: number?,
	read favorite: boolean?,
	read selected: boolean?,
	read lines: number?,
	read msg: any?,
	read context: any?,
	read to: string?,
}
type UseDotChartColumn = any
type UseDotChartProps = {
	read columns: { UseDotChartColumn },
	read max: number,
	read rows: number?,
	read ticks: { string }?,
	read labels: { string }?,
	read size: number?,
	read color: string?,
	read shade: string?,
}
type UseDropdownProps = {
	read open: boolean?,
	read label: string?,
	read value: string?,
	read icon: string?,
	read options: { any },
	read selected: string?,
	read toggle: any,
	read dismiss: any,
	read event: string?,
	read highlighted: any,
	read expanded: any,
	read expand: string?,
	read submenu_highlighted: any,
	read keys: { [string]: any }?,
	read width: number?,
	read align: string?,
	read side: string?,
	read placeholder: string?,
	read disabled: boolean?,
	read to: string?,
	read chevron: string?,
	read trigger_width: number?,
}
type UseDropzoneProps = {
	read title: string?,
	read detail: string?,
	read icon: string?,
	read compact: boolean?,
	read drop: any,
	read browse: any,
	read to: string?,
	read width: number?,
	read height: number?,
}
type UseEmojiPickerProps = {
	read recent: { string }?,
	read event: string?,
	read query: string?,
	read search: any,
	read id: string?,
	read focus: boolean?,
	read cancel: any,
	read columns: number?,
	read height: number?,
	read to: string?,
}
type UseEmptyStateProps = {
	read icon: string?,
	read title: string,
	read message: string?,
	read action: string?,
	read msg: any,
}
type UseErrorStateProps = {
	read title: string?,
	read message: string?,
	read details: string?,
	read icon: string?,
	read retry: any,
	read retry_label: string?,
	read secondary: any,
	read block: boolean?,
	read retrying: boolean?,
	read to: string?,
}
type UseFeedbackProps = {
	read open: boolean?,
	read dismiss: any,
	read id: string?,
	read value: string?,
	read text: string?,
	read placeholder: string?,
	read note: string?,
	read error: string?,
	read change: any,
	read send: any,
	read runs: { any }?,
	read expanded: string?,
	read undo: boolean?,
	read follow: boolean?,
	read reply: string?,
	read actions: { any }?,
	read inspect: boolean?,
	read attached: { read element: string, read at: string?, read report: string? }?,
	read detach: any,
	read copy: any,
	read copied: boolean?,
	read inline: boolean?,
	read to: string?,
}
type UseFeedbackStatus = any
type UseFileCardProps = {
	read name: string,
	read src: string?,
	read detail: string?,
	read progress: number?,
	read layout: string?,
	read remove: any,
	read msg: any,
	read selected: boolean?,
	read to: string?,
	read width: number?,
}
type UseFileIconProps = {
	read name: string,
	read dir: boolean?,
	read open: boolean?,
}
type UseFilterBuilderCondition = any
type UseFilterBuilderField = any
type UseFilterBuilderProps = {
	read match: string?,
	read conditions: { UseFilterBuilderCondition }?,
	read fields: { UseFilterBuilderField },
	read open: string?,
	read to: string?,
}
type UseFilterDropdownOption = any
type UseFilterDropdownProps = {
	read icon: string?,
	read label: string?,
	read placeholder: string?,
	read options: { any },
	read selected: any,
	read multiple: boolean?,
	read any: string?,
	read open: boolean?,
	read toggle: any,
	read dismiss: any,
	read event: string?,
	read clear: any,
	read width: number?,
	read plain: boolean?,
	read to: string?,
}
type UseFindBarProps = {
	read id: string?,
	read value: string?,
	read placeholder: string?,
	read count: number?,
	read index: number?,
	read change: any,
	read event: string?,
	read closable: boolean?,
	read focus: boolean?,
	read replace: boolean?,
	read replacement: string?,
	read replace_change: any,
	read replace_one: any,
	read replace_all: any,
	read width: number?,
	read to: string?,
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
	read nodes: { UseFlowDiagramNode },
	read links: { UseFlowDiagramLink }?,
	read groups: { UseFlowDiagramGroup }?,
	read flows: { UseFlowDiagramFlow }?,
	read columns: number?,
	read rows: number?,
	read time: number?,
	read speed: number?,
	read selected: string?,
	read event: string?,
	read to: string?,
	read size: string?,
	read height: number?,
}
type UseForecastChartBar = any
type UseForecastChartProps = {
	read bars: { UseForecastChartBar },
	read max: number,
	read floor: number?,
	read ticks: { string }?,
	read height: number?,
	read bar: number?,
	read event: string?,
	read to: string?,
}
type UseForgeAccountProps = {
	read forge: any,
	read brand: string?,
	read msg: any,
	read to: string?,
}
type UseForgePrProps = {
	read forge: any,
	read branch: string?,
	read pr: any,
	read changes: any,
	read brand: boolean?,
	read open: boolean?,
}
type UseForgeSignInProps = {
	read forge: any,
	read provider: string?,
}
type UseFormGridRow = any
type UseFormGridProps = {
	read rows: { UseFormGridRow },
	read label_width: number?,
	read gap: number?,
}
type UseGanttDay = any
type UseGanttRow = any
type UseGanttProps = {
	read days: { UseGanttDay },
	read rows: { UseGanttRow },
	read deps: { any }?,
	read day_width: number?,
	read row_height: number?,
	read label_width: number?,
	read selected: string?,
	read event: string?,
	read move: string?,
	read resize: string?,
	read moving: any,
	read height: number?,
	read grow: boolean?,
	read to: string?,
}
type UseGanttSpan = any
type UseGhosttyTerminalProps = {
	read id: string?,
	read cwd: string?,
	read backend: string?,
	read exit: any,
	read shell: any,
	read style: any,
}
type UseGroupHeaderProps = {
	read title: string,
	read id: string?,
	read count: (number | string)?,
	read status: string?,
	read icon: string?,
	read detail: string?,
	read collapsed: boolean?,
	read toggle: any,
	read add: any,
	read add_tooltip: string?,
	read actions: { any }?,
	read band: boolean?,
	read to: string?,
}
type UseGuideCardProps = {
	read title: string,
	read category: string?,
	read color: string?,
	read tint: string?,
	read icon: string?,
	read read: string?,
	read progress: number?,
	read done: boolean?,
	read new: boolean?,
	read selected: boolean?,
	read msg: any,
	read to: string?,
	read bar: number?,
}
type UseHeatGridProps = {
	read columns: { any },
	read rows: number?,
	read levels: number?,
	read size: number?,
	read gap: number?,
	read color: string?,
	read empty: string?,
	read spread: boolean?,
	read labels: { string }?,
	read tooltips: { string }?,
}
type UseHistogramChannel = any
type UseHistogramMarker = any
type UseHistogramProps = {
	read channels: { UseHistogramChannel },
	read width: number?,
	read height: number?,
	read max: number?,
	read log: boolean?,
	read markers: { UseHistogramMarker }?,
	read drag: any,
	read shade: boolean?,
	read to: string?,
}
type UseIconButtonProps = {
	read icon: string,
	read tooltip: string?,
	read msg: any,
	read command: any,
	read variant: string?,
	read size: string?,
	read active: boolean?,
	read badge: any,
	read disabled: boolean?,
	read tone: string?,
	read to: string?,
}
type UseImageViewerProps = {
	read src: string,
	read zoom: number?,
	read center: { number }?,
	read change: any,
	read to: string?,
	read overlay: { any }?,
	read controls: boolean?,
	read min: number?,
	read max: number?,
	read width: number?,
	read height: number?,
	read id: string?,
}
type UseImageViewerState = any
type UseInsightNoteProps = {
	read label: string?,
	read icon: string?,
	read text: string,
	read link: string?,
	read after: string?,
	read msg: any,
	read to: string?,
}
type UseKanbanColumnProps = {
	read title: string,
	read color: string?,
	read count: number?,
	read total: string?,
	read add: any,
	read add_label: string?,
	read to: string?,
	read width: number?,
	read accept: any,
	read over: boolean?,
	read marker: number?,
	read [number]: any,
}
type UseKbdProps = {
	read keys: any,
	read small: boolean?,
	read plain: boolean?,
}
type UseKeyHintsHint = any
type UseKeyHintsProps = {
	read hints: { UseKeyHintsHint },
	read caps: boolean?,
	read separator: string?,
	read justify: string?,
}
type UseKeyValuesProps = {
	read rows: { { string } },
	read width: number?,
}
type UseKpiCellProps = {
	read title: string,
	read value: string,
	read icon: string?,
	read tint: string?,
	read delta: string?,
	read good: boolean?,
	read caption: string?,
	read menu: any,
	read to: string?,
}
type UseKpiMeterSegment = any
type UseKpiMeterLegend = any
type UseKpiMeterProps = {
	read title: string,
	read value: string,
	read caption: string?,
	read caption_color: string?,
	read dot: string?,
	read mono: boolean?,
	read segments: { UseKpiMeterSegment }?,
	read legend: { UseKpiMeterLegend }?,
	read width: number?,
	read msg: any,
	read to: string?,
}
type UseLevelRowProps = {
	read label: string,
	read value: string?,
	read fraction: number,
	read color: string?,
	read end_color: string?,
	read width: number?,
	read label_width: number?,
	read height: number?,
	read tooltip: string?,
	read msg: any?,
	read to: string?,
}
type UseLightboxPicture = any
type UseLightboxProps = {
	read open: boolean?,
	read dismiss: any,
	read pictures: { UseLightboxPicture }?,
	read index: number?,
	read event: string?,
	read zoom: number?,
	read center: any,
	read change: any,
	read to: string?,
}
type UseLineChartSeries = any
type UseLineChartProps = {
	read labels: { string },
	read series: { UseLineChartSeries },
	read bars: { number }?,
	read max: number,
	read min: number?,
	read ticks: { string }?,
	read width: number?,
	read height: number?,
	read selected: number?,
	read card: any,
	read card_width: number?,
	read tips: { string }?,
	read event: string?,
	read to: string?,
}
type UseListRow = any
type UseListProps = {
	read rows: { UseListRow },
	read selected: string?,
	read to: string?,
}
type UseLoadingProps = {
	read text: string?,
	read detail: string?,
	read block: boolean?,
	read skeleton: boolean?,
	read count: number?,
	read widths: { number }?,
}
type UseMailRowTag = any
type UseMailRowProps = {
	read from: string,
	read subject: string?,
	read preview: string?,
	read time: string?,
	read account: string?,
	read unread: boolean?,
	read count: number?,
	read starred: boolean?,
	read star: any,
	read attachment: boolean?,
	read tag: UseMailRowTag?,
	read avatar: string?,
	read color: string?,
	read selected: boolean?,
	read msg: any,
	read context: any,
	read to: string?,
}
type UseMarkdownEditorMode = any
type UseMarkdownEditorScrollAt = any
type UseMarkdownEditorState = any
type UseMarkdownEditorProps = {
	read path: string?,
	read root: string?,
	read text: string?,
	read mode: string?,
	read id: string?,
}
type UseMarkdownProps = {
	read text: string?,
	read link: any,
	read tooltips: { [string]: string }?,
	read to: string?,
	read grow: boolean?,
	read scroll: boolean?,
	read max_width: number?,
	read lines: number?,
	read images: boolean?,
	read image: any,
	read outline: any,
	read section: any,
	read reveal: { read at: number, read seq: number? }?,
	read find: string?,
	read find_index: number?,
	read found: any,
	read stream: boolean?,
	read scrolled: any,
	read scroll_at: { read fraction: number, read seq: number? }?,
}
type UseMentionFieldPerson = any
type UseMentionFieldProps = {
	read id: string,
	read value: string?,
	read caret: number?,
	read people: { UseMentionFieldPerson }?,
	read change: any,
	read submit: any,
	read blur: any,
	read placeholder: string?,
	read multiline: boolean?,
	read rows: number?,
	read max_rows: number?,
	read highlighted: number?,
	read event: string?,
	read dismiss: any,
	read most: number?,
	read width: number?,
	read grow: boolean?,
	read focus: boolean?,
	read to: string?,
	read trigger: string?,
	read heading: string?,
}
type UseMenuItem = any
type UseMenuProps = {
	read items: { any },
	read max_height: number?,
	read highlighted: any,
	read event: string?,
	read expanded: any,
	read expand: string?,
	read submenu_highlighted: any,
	read submenu_width: number?,
	read to: string?,
}
type UseMessageHeaderProps = {
	read subject: string?,
	read from: string,
	read email: string?,
	read recipients: any,
	read cc: any,
	read date: string?,
	read avatar: string?,
	read color: string?,
	read actions: { any }?,
	read sender: any,
	read to: string?,
	read [number]: any,
}
type UseMeterProps = {
	read value: number,
	read width: number?,
	read warn: number?,
	read label: string?,
	read percent: boolean?,
}
type UseMetricProps = {
	read label: string,
	read value: string,
	read note: string?,
	read tone: string?,
	read size: string?,
}
type UseMiniBarsBar = any
type UseMiniBarsProps = {
	read bars: { UseMiniBarsBar },
	read highlight: number?,
	read height: number?,
	read bar: number?,
}
type UseMockupsVariant = any
type UseMockupsProps = {
	read ask: string?,
	read variants: { UseMockupsVariant },
	read file: string?,
	read width: number?,
}
type UseMonthGridDay = any
type UseMonthGridProps = {
	read weeks: { { UseMonthGridDay } },
	read selected: string?,
	read weekdays: { string }?,
	read event: string?,
	read to: string?,
}
type UseNavButtonsProps = {
	read can_back: boolean?,
	read can_forward: boolean?,
	read back: any,
	read forward: any,
	read small: boolean?,
	read to: string?,
}
type UseNoteCardProps = {
	read id: any,
	read number: number?,
	read location: string?,
	read text: string,
	read selected: boolean?,
	read event: string?,
	read remove: string?,
	read to: string?,
}
type UseNoticeProps = {
	read id: any,
	read tone: string?,
	read icon: string?,
	read title: string?,
	read text: string?,
	read action: any,
	read dismiss: any,
	read foldable: boolean?,
	read expanded: boolean?,
	read toggle: any,
	read to: string?,
	read [number]: any,
}
type UseNotificationDrawerItem = any
type UseNotificationDrawerFilter = any
type UseNotificationDrawerProps = {
	read open: boolean?,
	read items: { UseNotificationDrawerItem }?,
	read filter: string?,
	read filters: { UseNotificationDrawerFilter }?,
	read title: string?,
	read width: number?,
	read top: number?,
	read backdrop: boolean?,
	read dismiss: any,
	read mark_read: any,
	read mark_read_label: string?,
	read event: string?,
	read remove: any,
	read empty: string?,
	read to: string?,
}
type UseNumberFieldProps = {
	read value: number,
	read change: any,
	read min: number?,
	read max: number?,
	read step: number?,
	read speed: number?,
	read decimals: number?,
	read label: string?,
	read unit: string?,
	read width: number?,
	read buttons: boolean?,
	read id: string?,
	read disabled: boolean?,
	read to: string?,
}
type UseNumberedListProps = {
	read items: { string },
	read start: number?,
}
type UseOnboardingPage = any
type UseOnboardingProps = {
	read open: boolean?,
	read title: string?,
	read icon: string?,
	read badge: string?,
	read pages: { UseOnboardingPage }?,
	read current: number?,
	read back: any,
	read next: any,
	read done: any,
	read dismiss: any,
	read event: string?,
	read done_label: string?,
	read width: number?,
	read to: string?,
	read [number]: any,
}
type UseOutlineHeading = any
type UseOutlineProps = {
	read headings: { UseOutlineHeading }?,
	read current: number?,
	read event: string?,
	read indent: number?,
	read depth: number?,
	read empty: string?,
	read to: string?,
}
type UsePageProps = {
	read title: string,
	read subtitle: string?,
	read icon: string?,
	read actions: { any }?,
	read width: number?,
	read footer: any,
	read height: number?,
	read [number]: any,
}
type UsePagerProps = {
	read count: number,
	read page: number,
	read back: any,
	read next: any,
	read back_label: string?,
	read next_label: string?,
	read event: string?,
	read to: string?,
}
type UsePaginationProps = {
	read page: number,
	read total: number,
	read per_page: number?,
	read noun: string?,
	read summary: boolean?,
	read window: number?,
	read ellipsis: boolean?,
	read event: string?,
	read sizes: { number }?,
	read size_open: boolean?,
	read size_toggle: any,
	read size_dismiss: any,
	read size_event: string?,
	read size_label: string?,
	read to: string?,
}
type UsePaneFrameProps = {
	read title: string,
	read icon: string?,
	read subtitle: string?,
	read status: string?,
	read focused: boolean?,
	read zoomed: boolean?,
	read header: string?,
	read focus: any,
	read split_right: any,
	read split_down: any,
	read zoom: any,
	read close: any,
	read actions: { any }?,
	read leading: { any }?,
	read menu: any,
	read click_focus: boolean?,
	read to: string?,
	read height: number?,
	read [number]: any,
}
type UsePickerOption = any
type UsePickerProps = {
	read options: { any },
	read selected: any,
	read multiple: boolean?,
	read open: boolean?,
	read toggle: any,
	read dismiss: any,
	read event: string?,
	read query: string?,
	read search: any,
	read id: string?,
	read create: any,
	read highlighted: string?,
	read highlight: string?,
	read keys: { [string]: any }?,
	read placeholder: string?,
	read icon: string?,
	read empty: string?,
	read width: number?,
	read align: string?,
	read trigger: any,
	read to: string?,
}
type UsePillTabsItem = any
type UsePillTabsProps = {
	read items: { UsePillTabsItem },
	read selected: string?,
	read event: string?,
	read width: number?,
	read size: number?,
	read to: string?,
}
type UsePillProps = {
	read label: string,
	read icon: string?,
	read tone: string?,
	read color: string?,
	read size: string?,
	read msg: any,
	read to: string?,
	read tooltip: string?,
}
type UsePixelIconProps = {
	read pattern: { string },
	read colors: { string },
	read size: number?,
	read mirror: boolean?,
	read tooltip: string?,
}
type UsePrBadgeProps = {
	read number: number | string,
	read kind: string?,
	read tone: string?,
	read icon: string?,
	read checks: string?,
	read label: string?,
	read tooltip: string?,
	read msg: any,
	read to: string?,
}
type UsePrRowProps = {
	read number: number | string,
	read kind: string?,
	read title: string,
	read tone: string?,
	read draft: boolean?,
	read branch: string?,
	read target: string?,
	read author: string?,
	read age: string?,
	read seconds: number?,
	read checks: string?,
	read added: number?,
	read removed: number?,
	read comments: number?,
	read labels: { string }?,
	read selected: boolean?,
	read event: string?,
	read to: string?,
	read msg: any,
	read context: any,
}
type UsePriorityIconProps = {
	read priority: string?,
	read size: number?,
	read tooltip: string?,
	read label: (string | boolean)?,
}
type UseProgressStackSegment = any
type UseProgressStackProps = {
	read segments: { UseProgressStackSegment },
	read total: number,
	read width: number?,
	read marker: { read value: number, read label: string }?,
	read labels: { string }?,
	read ticks: number?,
}
type UseProgressProps = {
	read value: number?,
	read busy: boolean?,
	read label: string?,
	read detail: string?,
	read tone: string?,
	read width: number?,
	read height: number?,
	read percent: boolean?,
}
type UseProjectCardProps = {
	read name: string,
	read icon: string?,
	read color: string?,
	read description: string?,
	read status: string?,
	read tone: string?,
	read status_color: string?,
	read starred: boolean?,
	read star: any,
	read progress: number?,
	read tasks: string?,
	read due: string?,
	read updated: string?,
	read people: { any }?,
	read selected: boolean?,
	read msg: any,
	read context: any,
	read to: string?,
	read width: number?,
}
type UseProseEditorProps = {
	read id: string?,
	read value: string?,
	read placeholder: string?,
	read change: any?,
	read submit: any?,
	read keys: { [string]: any }?,
	read to: string?,
	read dim: ("sentence" | "paragraph")?,
	read typewriter: boolean?,
	read markdown: boolean?,
	read mono: boolean?,
	read size: ("normal" | "large" | "title")?,
	read measure: number?,
	read focus: boolean?,
}
type UseRadioGroupOption = any
type UseRadioGroupProps = {
	read options: { UseRadioGroupOption },
	read selected: string?,
	read horizontal: boolean?,
	read event: string?,
	read to: string?,
}
type UseRailItem = any
type UseRailProps = {
	read items: { UseRailItem },
	read selected: string?,
	read event: string?,
	read to: string?,
	read header: { any }?,
	read footer: { any }?,
	read height: number?,
	read fill: boolean?,
}
type UseRatioRowProps = {
	read label: string,
	read count: string?,
	read percent: string?,
	read icon: string?,
	read leading: any,
	read share: number?,
	read color: string?,
	read width: number?,
	read msg: any,
	read to: string?,
}
type UseRecipientFieldRecipient = any
type UseRecipientFieldProps = {
	read label: string?,
	read recipients: { any }?,
	read id: string?,
	read value: string?,
	read placeholder: string?,
	read change: any,
	read submit: any,
	read focus: boolean?,
	read keys: { [string]: any }?,
	read suggestions: { any }?,
	read highlighted: number?,
	read pick: string?,
	read remove: string?,
	read dismiss: any,
	read trailing: { any }?,
	read width: number?,
	read to: string?,
}
type UseReconnectingProps = {
	read open: boolean?,
	read host: string?,
	read title: string?,
	read reason: string?,
	read status: string?,
	read retry: any,
	read retry_label: string?,
	read secondary: any,
	read dismiss: any,
	read icon: string?,
	read to: string?,
}
type UseRecordCardProps = {
	read title: string,
	read detail: string?,
	read icon: string?,
	read tone: string?,
	read msg: any,
	read to: string?,
}
type UseRelativeTimeProps = {
	read seconds: number,
	read compact: boolean?,
	read prefix: string?,
	read tooltip: string?,
	read size: string?,
}
type UseReleaseNotesHighlight = any
type UseReleaseNotesProps = {
	read version: string,
	read date: string?,
	read name: string?,
	read badge: string?,
	read summary: string?,
	read highlights: { UseReleaseNotesHighlight }?,
	read fixes: { string }?,
	read fixes_title: string?,
	read collapsed: boolean?,
	read toggle: any,
	read id: any,
	read to: string?,
}
type UseRichTextPart = any
type UseRichTextProps = {
	read parts: { any },
	read size: string?,
	read to: string?,
}
type UseRouteMapStop = any
type UseRouteMapLeg = any
type UseRouteMapProps = {
	read stops: { UseRouteMapStop },
	read legs: { UseRouteMapLeg }?,
	read provider: string?,
	read key: string?,
	read travel: string?,
	read marker: string?,
	read selected: number?,
	read event: string?,
	read routed: any,
	read height: number?,
	read grow: boolean?,
	read to: string?,
}
type UseSearchFieldProps = {
	read id: string?,
	read value: string?,
	read placeholder: string?,
	read change: any,
	read submit: any,
	read cancel: any,
	read previous: any,
	read next: any,
	read to: string?,
	read focus: boolean?,
	read width: number?,
	read grow: boolean?,
	read icon: any,
	read shortcut: any,
}
type UseSectionProps = {
	read title: string,
	read [number]: any,
}
type UseSegmentedOption = any
type UseSegmentedProps = {
	read options: { UseSegmentedOption },
	read selected: string?,
	read event: string?,
	read small: boolean?,
	read grow: boolean?,
	read to: string?,
}
type UseSelectionToolbarProps = {
	read [number]: any,
	read field: string,
	read selected: number?,
	read tools: { string }?,
	read side: any,
}
type UseSeriesChartSeries = any
type UseSeriesChartLabel = any
type UseSeriesChartProps = {
	read series: { UseSeriesChartSeries },
	read width: number?,
	read height: number?,
	read min: number?,
	read max: number?,
	read ticks: { string }?,
	read labels: { UseSeriesChartLabel }?,
	read titles: { string }?,
	read unit: string?,
	read pinned: number?,
	read event: string?,
	read to: string?,
	read legend: boolean?,
}
type UseSettingsNavItem = any
type UseSettingsNavGroup = any
type UseSettingsNavProps = {
	read groups: { UseSettingsNavGroup },
	read selected: string?,
	read title: string?,
	read subtitle: string?,
	read search: any,
	read footer: any,
	read event: string?,
	read width: number?,
	read scroll: boolean?,
	read height: number?,
	read to: string?,
}
type UseSettingsPageRow = any
type UseSettingsPageSection = any
type UseSettingsPageProps = {
	read nav: any,
	read title: string?,
	read subtitle: string?,
	read sections: { UseSettingsPageSection }?,
	read status: string?,
	read state: string?,
	read buttons: { any }?,
	read height: number?,
	read to: string?,
	read [number]: any,
}
type UseSettingsRowProps = {
	read title: string,
	read description: string?,
	read tag: string?,
	read tag_tone: string?,
	read disabled: boolean?,
	read reason: string?,
	read stacked: boolean?,
	read border: boolean?,
	read [number]: any,
}
type UseShareBarShare = any
type UseShareBarProps = {
	read shares: { UseShareBarShare },
	read width: number?,
	read height: number?,
}
type UseSheetProps = {
	read open: boolean?,
	read dismiss: any,
	read side: string?,
	read title: string?,
	read subtitle: string?,
	read icon: string?,
	read width: number?,
	read height: number?,
	read backdrop: boolean?,
	read actions: { any }?,
	read footer: any,
	read keys: { [string]: any }?,
	read to: string?,
	read [number]: any,
}
type UseShortcutFieldProps = {
	read id: string?,
	read value: string?,
	read recording: boolean?,
	read event: string?,
	read clearable: boolean?,
	read placeholder: string?,
	read width: number?,
	read to: string?,
}
type UseShortcutsItem = any
type UseShortcutsGroup = any
type UseShortcutsProps = {
	read open: boolean?,
	read dismiss: any,
	read title: string?,
	read subtitle: string?,
	read groups: { UseShortcutsGroup },
	read columns: number?,
	read search: any,
	read note: string?,
	read width: number?,
	read height: number?,
	read inline: boolean?,
	read to: string?,
}
type UseSideNavItem = any
type UseSideNavSection = any
type UseSideNavAccount = any
type UseSideNavProps = {
	read collapsed: boolean?,
	read selected: string?,
	read sections: { UseSideNavSection }?,
	read footer: { UseSideNavItem }?,
	read account: UseSideNavAccount?,
	read header: any,
	read toggle: any,
	read event: string?,
	read to: string?,
	read stagger: number?,
	read height: number?,
}
type UseSidePanelsProps = {
	read id: string?,
	read sidebar: any,
	read list: any,
	read main: any,
	read left: boolean?,
	read middle: boolean?,
	read sidebar_size: number?,
	read list_size: number?,
	read height: number?,
}
type UseSidebarSectionProps = {
	read title: string,
	read count: (number | string)?,
	read collapsed: boolean?,
	read toggle: any,
	read id: string?,
	read to: string?,
	read actions: { any }?,
	read fixed: boolean?,
	read [number]: any,
}
type UseSidebarSection = any
type UseSidebarFooterItem = any
type UseSidebarProps = {
	read header: any,
	read sections: { UseSidebarSection }?,
	read footer: { UseSidebarFooterItem }?,
	read to: string?,
	read width: number?,
	read height: number?,
	read fill: boolean?,
	read [number]: any,
}
type UseSliderProps = {
	read value: number,
	read min: number?,
	read max: number?,
	read step: number?,
	read change: any,
	read label: string?,
	read unit: string?,
	read decimals: number?,
	read default: number?,
	read width: number?,
	read inline: boolean?,
	read label_width: number?,
	read colors: { string }?,
	read disabled: boolean?,
	read to: string?,
}
type UseSparklineProps = {
	read values: { number },
	read samples: number?,
	read height: number?,
	read hot: number?,
}
type UseSplitViewProps = {
	read id: string?,
	read sidebar: any,
	read main: any,
	read inspector: any,
	read bottom_panel: any,
	read left: boolean?,
	read right: boolean?,
	read bottom: boolean?,
	read sidebar_size: number?,
	read inspector_size: number?,
	read bottom_size: number?,
	read height: number?,
}
type UseSpotlightBarsBar = any
type UseSpotlightBarsProps = {
	read bars: { UseSpotlightBarsBar },
	read selected: number?,
	read height: number?,
	read max: number?,
	read bar: number?,
	read event: string?,
	read to: string?,
}
type UseSpreadsheetColumn = any
type UseSpreadsheetProps = {
	read columns: { UseSpreadsheetColumn },
	read rows: { { [string]: any } },
	read current: number?,
	read selected: { read row: number, read key: string }?,
	read caret: boolean?,
	read blank: number?,
	read numbers: boolean?,
	read row_height: number?,
	read event: string?,
	read to: string?,
	read grow: boolean?,
	read height: number?,
	read id: string?,
}
type UseStatTileProps = {
	read label: string,
	read value: string,
	read unit: string?,
	read icon: string?,
	read delta: string?,
	read trend: string?,
	read good: string?,
	read detail: string?,
	read msg: any,
	read to: string?,
	read selected: boolean?,
	read width: number?,
	read [number]: any,
}
type UseStatProps = {
	read icon: string?,
	read label: string?,
	read value: string,
	read width: number?,
	read tone: any,
}
type UseStatusBarEntry = any
type UseStatusBarProps = {
	read left: { any }?,
	read right: { any }?,
	read separators: boolean?,
	read to: string?,
}
type UseStatusDotProps = {
	read status: string?,
	read size: string?,
	read spin: boolean?,
	read chip: boolean?,
	read label: string?,
	read tooltip: string?,
}
type UseStatusIconProps = {
	read status: string?,
	read progress: number?,
	read color: string?,
	read size: number?,
	read tooltip: string?,
	read label: (string | boolean)?,
}
type UseStepperProps = {
	read value: number,
	read unit: string?,
	read min: number?,
	read max: number?,
	read decrement: any,
	read increment: any,
	read label: string?,
	read width: number?,
	read to: string?,
}
type UseSwapViewProps = {
	read key: string,
	read width: number?,
	read grow: boolean?,
	read [number]: any,
}
type UseSwatchesSwatch = any
type UseSwatchesProps = {
	read colors: { UseSwatchesSwatch },
	read selected: string?,
	read none: boolean?,
	read event: string?,
	read size: number?,
	read to: string?,
}
type UseSwitchProps = {
	read on: boolean?,
	read label: string?,
	read description: string?,
	read msg: any,
	read disabled: boolean?,
	read small: boolean?,
	read to: string?,
}
type UseSystemLoadProps = {
	read cpu: number,
	read memory: number,
	read history: { number }?,
	read samples: number?,
	read compact: boolean?,
	read icons: boolean?,
}
type UseT3ComposerProps = {
	read data: index<App, "t3"> & {},
	read action: "start" | "send",
	read id: string,
	read environmentId: string,
	read target: string?,
	read title: string?,
	read text: string,
	read picker: boolean?,
}
type UseT3SessionsProps = {
	read threads: index<index<App, "t3"> & {}, "threads">,
	read folded: { [string]: boolean }?,
	read selected: string?,
	read to: string?,
}
type UseT3ThreadProps = {
	read thread: index<index<index<App, "t3"> & {}, "threads">, number>,
	read expanded: boolean?,
	read msg: any,
	read to: string?,
}
type UseTabViewTab = any
type UseTabViewProps = {
	read tab: UseTabViewTab?,
	read kind: TabKind?,
}
type UseTabbedTab = any
type UseTabbedState = any
type UseTabbedProps = {
	read id: string?,
	read start: string?,
	read offer: { string }?,
	read variant: string?,
	read show: string?,
}
type UseTabbedReport = any
type UseTableColumn = any
type UseTableProps = {
	read columns: { UseTableColumn },
	read rows: { { [string]: any } }?,
	read selected: any,
	read sort: string?,
	read descending: boolean?,
	read sortable: boolean?,
	read zebra: boolean?,
	read empty: string?,
	read event: string?,
	read to: string?,
	read height: number?,
	read grow: boolean?,
	read checkable: boolean?,
	read checks: any,
	read check: string?,
	read check_all: any,
	read bordered: boolean?,
	read id: string?,
	read groups: { any }?,
	read fold: string?,
	read add_label: string?,
	read reorder: string?,
	read marker: any,
	read measure: any,
	read scroll_to: any,
}
type UseTabsTab = any
type UseTabsProps = {
	read tabs: { UseTabsTab },
	read selected: string?,
	read variant: string?,
	read add: any,
	read add_menu: any,
	read event: string?,
	read close: string?,
	read context: string?,
	read closable: boolean?,
	read grow: boolean?,
	read width: number?,
	read to: string?,
	read in_bar: boolean?,
}
type UseTagsProps = {
	read tags: { string },
	read tone: any,
}
type UseTaskCheckProps = {
	read state: string?,
	read msg: any,
	read to: string?,
	read tooltip: string?,
}
type UseTaskRowProps = {
	read title: string,
	read state: string?,
	read event: boolean?,
	read date: string?,
	read time: string?,
	read due: boolean?,
	read icon: string?,
	read tags: { any }?,
	read carry: any,
	read accept: any,
	read leading: any,
	read toggle: any,
	read msg: any,
	read context: any,
	read to: string?,
	read selected: boolean?,
}
type UseTerminalProps = {
	read id: string?,
	read cwd: string?,
	read backend: string?,
	read exit: any,
	read shell: any,
	read style: any,
}
type UseTextEditorProps = {
	read id: string,
	read value: string?,
	read change: any,
	read blur: any,
	read submit: any,
	read placeholder: string?,
	read rows: number?,
	read max_rows: number?,
	read preview: boolean?,
	read switch: any,
	read tools: { string }?,
	read empty: string?,
	read focus: boolean?,
	read to: string?,
}
type UseThinkingProps = {
	read seconds: number?,
	read label: string?,
	read open: boolean?,
	read msg: any,
	read to: string?,
	read text: string?,
}
type UseThresholdLineProps = {
	read points: { number },
	read threshold: number,
	read label: string?,
	read breach: string?,
	read width: number?,
	read height: number?,
	read min: number?,
	read max: number?,
	read dot: boolean?,
	read stroke: number?,
	read tooltip: string?,
}
type UseTickMeterProps = {
	read value: number,
	read color: string?,
	read ticks: number?,
	read height: number?,
	read tooltip: string?,
}
type UseTimePickerProps = {
	read hour: number?,
	read minute: number?,
	read step: number?,
	read clock: number?,
	read presets: { any }?,
	read event: string?,
	read to: string?,
}
type UseTimelineEntry = any
type UseTimelineProps = {
	read entries: { UseTimelineEntry },
	read selected: any,
	read event: string?,
	read to: string?,
	read mono: boolean?,
	read empty: string?,
}
type UseTitleBarWindow = any
type UseTitleBarProps = {
	read window: UseTitleBarWindow?,
	read inset: number?,
	read leading: { any }?,
	read back: any,
	read forward: any,
	read can_back: boolean?,
	read can_forward: boolean?,
	read title: string?,
	read center: boolean?,
	read actions: { any }?,
	read to: string?,
	read [number]: any,
}
type UseToastProps = {
	read id: any,
	read level: string?,
	read icon: any,
	read source: string?,
	read title: string?,
	read body: string?,
	read full: boolean?,
	read action: any,
	read dismiss: any,
	read event: string?,
	read width: number?,
	read to: string?,
}
type UseToastsItem = any
type UseToastsProps = {
	read items: { UseToastsItem },
	read place: string?,
	read width: number?,
	read max: number?,
	read event: string?,
	read to: string?,
}
type UseToolGroupProps = {
	read group: any,
	read inspect: boolean?,
	read output: any,
	read open: boolean?,
	read msg: any,
	read to: string?,
}
type UseToolbarButton = any
type UseToolbarProps = {
	read buttons: { UseToolbarButton },
}
type UseTreeNode = any
type UseTreeProps = {
	read nodes: { UseTreeNode },
	read selected: string?,
	read guides: boolean?,
	read select: string?,
	read fold: string?,
	read to: string?,
	read indent: number?,
}
type UseUpdatePillProps = {
	read name: string?,
	read version: string?,
	read text: string?,
	read icon: string?,
	read restart: any,
	read restart_label: string?,
	read notes: any,
	read notes_label: string?,
	read dismiss: any,
	read floating: boolean?,
	read place: string?,
	read downloading: boolean?,
	read to: string?,
}
type UseUsageBarProps = {
	read title: string?,
	read left: number,
	read pace: number?,
	read resets: string?,
	read note: string?,
	read width: number?,
	read warn: number?,
	read danger: number?,
}
type UseVectorscopeProps = {
	read points: { number },
	read size: number?,
	read color: string?,
	read targets: boolean?,
}
type UseVerticalBarProps = {
	read value: number,
	read height: number?,
	read width: number?,
	read warn: number?,
	read label: string?,
	read percent: boolean?,
	read tooltip: string?,
}
type UseVideoViewerProps = {
	read source: string,
	read zoom: number?,
	read center: { number }?,
	read change: any,
	read stats: any,
	read probed: any,
	read guides: { any }?,
	read color: string?,
	read extended: boolean?,
	read black: number?,
	read white: number?,
	read gain: number?,
	read gamma: number?,
	read controls: boolean?,
	read max: number?,
	read width: number?,
	read height: number?,
	read id: string?,
}
type UseVideoViewerState = any
type UseWaveformBand = any
type UseWaveformChannel = any
type UseWaveformProps = {
	read channels: { UseWaveformChannel },
	read parade: boolean?,
	read width: number?,
	read height: number?,
	read lines: { number }?,
}
type UseWeekCalendarDay = any
type UseWeekCalendarEvent = any
type UseWeekCalendarProps = {
	read view: string?,
	read views: any,
	read title: string?,
	read today: any,
	read previous: any,
	read next: any,
	read days: { UseWeekCalendarDay }?,
	read weeks: { { UseWeekCalendarDay } }?,
	read weekdays: { string }?,
	read events: { UseWeekCalendarEvent }?,
	read selected: string?,
	read hours: { read from: number?, read to: number? }?,
	read hour_height: number?,
	read column_width: number?,
	read day_width: number?,
	read gutter: number?,
	read now: { read day: string, read hour: number }?,
	read more: number?,
	read event: string?,
	read switch: string?,
	read pick_day: string?,
	read move: string?,
	read over: string?,
	read to: string?,
	read grow: boolean?,
	read height: number?,
	read header: boolean?,
}
type UseWritingStatsProps = {
	read text: string?,
	read show: ("words" | "characters" | "sentences" | "reading")?,
	read wpm: number?,
	read msg: any?,
	read to: string?,
	read tooltip: string?,
}

-- Each component's Props by its name.
type UseProps = {
	["about"]: UseAboutProps,
	["accordion"]: UseAccordionProps,
	["action-tiles"]: UseActionTilesProps,
	["activity-bar"]: UseActivityBarProps,
	["agent-chat"]: UseAgentChatProps,
	["agent-run"]: UseAgentRunProps,
	["ahead-behind"]: UseAheadBehindProps,
	["audio-meter"]: UseAudioMeterProps,
	["avatar-stack"]: UseAvatarStackProps,
	["avatar"]: UseAvatarProps,
	["banner"]: UseBannerProps,
	["bar-chart"]: UseBarChartProps,
	["board-card"]: UseBoardCardProps,
	["breadcrumbs"]: UseBreadcrumbsProps,
	["browser"]: UseBrowserProps,
	["button-group"]: UseButtonGroupProps,
	["caffeine"]: UseCaffeineProps,
	["card"]: UseCardProps,
	["change-tree"]: UseChangeTreeProps,
	["chat-markdown"]: UseChatMarkdownProps,
	["chat-message"]: UseChatMessageProps,
	["chat-question"]: UseChatQuestionProps,
	["chat-waiting"]: UseChatWaitingProps,
	["chat-working"]: UseChatWorkingProps,
	["checkbox"]: UseCheckboxProps,
	["checks"]: UseChecksProps,
	["chips"]: UseChipsProps,
	["choice-cards"]: UseChoiceCardsProps,
	["code-view"]: UseCodeViewProps,
	["collapsible"]: UseCollapsibleProps,
	["command-palette"]: UseCommandPaletteProps,
	["comment-card"]: UseCommentCardProps,
	["commit-graph"]: UseCommitGraphProps,
	["composer"]: UseComposerProps,
	["confirm"]: UseConfirmProps,
	["context-menu"]: UseContextMenuProps,
	["contribution-graph"]: UseContributionGraphProps,
	["copy-field"]: UseCopyFieldProps,
	["count-bubble"]: UseCountBubbleProps,
	["country-code"]: UseCountryCodeProps,
	["course-card"]: UseCourseCardProps,
	["curve-editor"]: UseCurveEditorProps,
	["dashboard-card"]: UseDashboardCardProps,
	["date-picker"]: UseDatePickerProps,
	["date-tile"]: UseDateTileProps,
	["day-strip"]: UseDayStripProps,
	["diagram"]: UseDiagramProps,
	["dialog"]: UseDialogProps,
	["dictation"]: UseDictationProps,
	["diff-counts"]: UseDiffCountsProps,
	["diff-view"]: UseDiffViewProps,
	["document-card"]: UseDocumentCardProps,
	["document-row"]: UseDocumentRowProps,
	["dot-chart"]: UseDotChartProps,
	["dropdown"]: UseDropdownProps,
	["dropzone"]: UseDropzoneProps,
	["emoji-picker"]: UseEmojiPickerProps,
	["empty-state"]: UseEmptyStateProps,
	["error-state"]: UseErrorStateProps,
	["feedback"]: UseFeedbackProps,
	["file-card"]: UseFileCardProps,
	["file-icon"]: UseFileIconProps,
	["filter-builder"]: UseFilterBuilderProps,
	["filter-dropdown"]: UseFilterDropdownProps,
	["find-bar"]: UseFindBarProps,
	["flow-diagram"]: UseFlowDiagramProps,
	["forecast-chart"]: UseForecastChartProps,
	["forge-account"]: UseForgeAccountProps,
	["forge-pr"]: UseForgePrProps,
	["forge-sign-in"]: UseForgeSignInProps,
	["form-grid"]: UseFormGridProps,
	["gantt"]: UseGanttProps,
	["ghostty-terminal"]: UseGhosttyTerminalProps,
	["group-header"]: UseGroupHeaderProps,
	["guide-card"]: UseGuideCardProps,
	["heat-grid"]: UseHeatGridProps,
	["histogram"]: UseHistogramProps,
	["icon-button"]: UseIconButtonProps,
	["image-viewer"]: UseImageViewerProps,
	["insight-note"]: UseInsightNoteProps,
	["kanban-column"]: UseKanbanColumnProps,
	["kbd"]: UseKbdProps,
	["key-hints"]: UseKeyHintsProps,
	["key-values"]: UseKeyValuesProps,
	["kpi-cell"]: UseKpiCellProps,
	["kpi-meter"]: UseKpiMeterProps,
	["level-row"]: UseLevelRowProps,
	["lightbox"]: UseLightboxProps,
	["line-chart"]: UseLineChartProps,
	["list"]: UseListProps,
	["loading"]: UseLoadingProps,
	["mail-row"]: UseMailRowProps,
	["markdown-editor"]: UseMarkdownEditorProps,
	["markdown"]: UseMarkdownProps,
	["mention-field"]: UseMentionFieldProps,
	["menu"]: UseMenuProps,
	["message-header"]: UseMessageHeaderProps,
	["meter"]: UseMeterProps,
	["metric"]: UseMetricProps,
	["mini-bars"]: UseMiniBarsProps,
	["mockups"]: UseMockupsProps,
	["month-grid"]: UseMonthGridProps,
	["nav-buttons"]: UseNavButtonsProps,
	["note-card"]: UseNoteCardProps,
	["notice"]: UseNoticeProps,
	["notification-drawer"]: UseNotificationDrawerProps,
	["number-field"]: UseNumberFieldProps,
	["numbered-list"]: UseNumberedListProps,
	["onboarding"]: UseOnboardingProps,
	["outline"]: UseOutlineProps,
	["page"]: UsePageProps,
	["pager"]: UsePagerProps,
	["pagination"]: UsePaginationProps,
	["pane-frame"]: UsePaneFrameProps,
	["picker"]: UsePickerProps,
	["pill-tabs"]: UsePillTabsProps,
	["pill"]: UsePillProps,
	["pixel-icon"]: UsePixelIconProps,
	["pr-badge"]: UsePrBadgeProps,
	["pr-row"]: UsePrRowProps,
	["priority-icon"]: UsePriorityIconProps,
	["progress-stack"]: UseProgressStackProps,
	["progress"]: UseProgressProps,
	["project-card"]: UseProjectCardProps,
	["prose-editor"]: UseProseEditorProps,
	["radio-group"]: UseRadioGroupProps,
	["rail"]: UseRailProps,
	["ratio-row"]: UseRatioRowProps,
	["recipient-field"]: UseRecipientFieldProps,
	["reconnecting"]: UseReconnectingProps,
	["record-card"]: UseRecordCardProps,
	["relative-time"]: UseRelativeTimeProps,
	["release-notes"]: UseReleaseNotesProps,
	["rich-text"]: UseRichTextProps,
	["route-map"]: UseRouteMapProps,
	["search-field"]: UseSearchFieldProps,
	["section"]: UseSectionProps,
	["segmented"]: UseSegmentedProps,
	["selection-toolbar"]: UseSelectionToolbarProps,
	["series-chart"]: UseSeriesChartProps,
	["settings-nav"]: UseSettingsNavProps,
	["settings-page"]: UseSettingsPageProps,
	["settings-row"]: UseSettingsRowProps,
	["share-bar"]: UseShareBarProps,
	["sheet"]: UseSheetProps,
	["shortcut-field"]: UseShortcutFieldProps,
	["shortcuts"]: UseShortcutsProps,
	["side-nav"]: UseSideNavProps,
	["side-panels"]: UseSidePanelsProps,
	["sidebar-section"]: UseSidebarSectionProps,
	["sidebar"]: UseSidebarProps,
	["slider"]: UseSliderProps,
	["sparkline"]: UseSparklineProps,
	["split-view"]: UseSplitViewProps,
	["spotlight-bars"]: UseSpotlightBarsProps,
	["spreadsheet"]: UseSpreadsheetProps,
	["stat-tile"]: UseStatTileProps,
	["stat"]: UseStatProps,
	["status-bar"]: UseStatusBarProps,
	["status-dot"]: UseStatusDotProps,
	["status-icon"]: UseStatusIconProps,
	["stepper"]: UseStepperProps,
	["swap-view"]: UseSwapViewProps,
	["swatches"]: UseSwatchesProps,
	["switch"]: UseSwitchProps,
	["system-load"]: UseSystemLoadProps,
	["t3-composer"]: UseT3ComposerProps,
	["t3-sessions"]: UseT3SessionsProps,
	["t3-thread"]: UseT3ThreadProps,
	["tab-view"]: UseTabViewProps,
	["tabbed"]: UseTabbedProps,
	["table"]: UseTableProps,
	["tabs"]: UseTabsProps,
	["tags"]: UseTagsProps,
	["task-check"]: UseTaskCheckProps,
	["task-row"]: UseTaskRowProps,
	["terminal"]: UseTerminalProps,
	["text-editor"]: UseTextEditorProps,
	["thinking"]: UseThinkingProps,
	["threshold-line"]: UseThresholdLineProps,
	["tick-meter"]: UseTickMeterProps,
	["time-picker"]: UseTimePickerProps,
	["timeline"]: UseTimelineProps,
	["title-bar"]: UseTitleBarProps,
	["toast"]: UseToastProps,
	["toasts"]: UseToastsProps,
	["tool-group"]: UseToolGroupProps,
	["toolbar"]: UseToolbarProps,
	["tree"]: UseTreeProps,
	["update-pill"]: UseUpdatePillProps,
	["usage-bar"]: UseUsageBarProps,
	["vectorscope"]: UseVectorscopeProps,
	["vertical-bar"]: UseVerticalBarProps,
	["video-viewer"]: UseVideoViewerProps,
	["waveform"]: UseWaveformProps,
	["week-calendar"]: UseWeekCalendarProps,
	["writing-stats"]: UseWritingStatsProps,
}

-- The Props of the component `name`, or `any` for a name it does not
-- know (an app's own components) or one only known at run time.
type function UseOf(name)
	if name:is("singleton") then
		local props = UseProps:readproperty(name)
		if props then
			return props
		end
	end
	return types.any
end

-- The component `name`'s `build`: `local stat = use "stat"`, then
-- `stat { value = "12%" }` wherever an element goes. The `""` in the
-- bound keeps the name's literal type (a bound of only `string` widens
-- it), so `UseOf` can look it up; `keyof<UseProps>` there would do the
-- same but is too complex for the solver at every call.
declare function use<K>(name: K & ("" | string)): (props: UseOf<K>) -> Element
-- generated: component props end

-- Prints to the terminal that runs the app.
declare function log(...: any): ()
