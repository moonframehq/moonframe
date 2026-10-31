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
type TextSize = "small" | "normal" | "large" | "title"
type Weight = "normal" | "medium" | "bold"
type Align = "stretch" | "start" | "center" | "end"
type Justify = "start" | "center" | "end" | "between"
type Surface = "background" | "sidebar" | "muted" | "secondary"
type Edge = "none" | "all" | "top" | "bottom" | "left" | "right"
type Border = boolean | Edge
type Direction = "row" | "column"
type Variant = "normal" | "primary" | "ghost" | "outline" | "danger"
type Command = "quit" | "reload" | "toggle-theme" | "show-themes"

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
type TransitionKind = "slide" | "fade" | "slide-fade" | "none"
-- `"slide 220ms ease-out"`, or a table; durations are ms, `"220ms"`, or a speed.
type Transition = string | {
	kind: TransitionKind?,
	duration: (number | string)?,
	curve: string?,
}

type Style = {
	background: Color?,
	color: Color?,
	border: Border?,
	border_color: Color?,
	radius: number?,
	padding: Sides?,
	gap: Spacing?,
	margin: Sides?,
	shadow: Shadow?,
	size: TextSize?,
	weight: Weight?,
	opacity: number?,
	transition: Transition?,
}

-- What a `ui` function returns; the host reads its fields.
type Element = { type: string }
-- A child: an element, a string (plain text), or false/nil (skipped).
type Child = Element | string | false | nil
-- Class names: `"a b"` or `{ "a", "b" }`.
type Class = string | { string }

-- Each props type is spelled out rather than combined with `&`, so string
-- literals such as `variant = "ghost"` check against their unions.

-- A stack with `msg` or `command` is clickable as a whole, like a button.
type StackProps = {
	class: Class?,
	style: Style?,
	msg: Data,
	to: string?,
	command: Command?,
	gap: Space?,
	padding: Space?,
	grow: boolean?,
	width: number?,
	height: number?,
	surface: Surface?,
	border: Border?,
	scroll: boolean?,
	align: Align?,
	justify: Justify?,
	wrap: boolean?,
	[number]: Child,
}

type PanelProps = {
	class: Class?,
	style: Style?,
	gap: Space?,
	padding: Space?,
	grow: boolean?,
	width: number?,
	height: number?,
	surface: Surface?,
	border: Border?,
	scroll: boolean?,
	title: string?,
	actions: { Child }?,
	[number]: Child,
}

type PaneProps = {
	class: Class?,
	style: Style?,
	size: number?,
	min: number?,
	max: number?,
	fixed: boolean?,
	visible: boolean?,
	gap: Space?,
	padding: Space?,
	surface: Surface?,
	scroll: boolean?,
	[number]: Child,
}

type SplitProps = {
	id: string?,
	[number]: Element | false | nil,
}

type TextProps = {
	class: Class?,
	style: Style?,
	text: string?,
	tone: Tone?,
	size: TextSize?,
	weight: Weight?,
	truncate: boolean?,
	[number]: string,
}

-- Clicking sends `msg` to an `update` (to the mod `to`, else its own), or
-- runs an app `command`.
type ButtonProps = {
	class: Class?,
	style: Style?,
	msg: Data,
	to: string?,
	command: Command?,
	label: string?,
	variant: Variant?,
	small: boolean?,
	disabled: boolean?,
	[number]: string,
}

type ItemProps = {
	class: Class?,
	style: Style?,
	msg: Data,
	to: string?,
	command: Command?,
	label: string?,
	detail: string?,
	badge: string?,
	selected: boolean?,
	[number]: string,
}

type BadgeProps = {
	class: Class?,
	style: Style?,
	text: string?,
	tone: Tone?,
	[number]: string,
}

-- The bundled icons (Lucide); the host refuses any other name.
type IconName =
	| "a-large-small"
	| "arrow-down"
	| "arrow-left"
	| "arrow-right"
	| "arrow-up"
	| "asterisk"
	| "ban"
	| "battery"
	| "battery-charging"
	| "battery-full"
	| "battery-low"
	| "battery-medium"
	| "battery-warning"
	| "bell"
	| "book-open"
	| "bot"
	| "building-2"
	| "calendar"
	| "case-sensitive"
	| "chart-pie"
	| "check"
	| "chevron-down"
	| "chevron-left"
	| "chevron-right"
	| "chevron-up"
	| "chevrons-up-down"
	| "circle-alert"
	| "circle-check"
	| "circle-user"
	| "circle-x"
	| "close"
	| "copy"
	| "cpu"
	| "dash"
	| "delete"
	| "ellipsis"
	| "ellipsis-vertical"
	| "external-link"
	| "eye"
	| "eye-off"
	| "file"
	| "file-text"
	| "folder"
	| "folder-closed"
	| "folder-open"
	| "frame"
	| "gallery-vertical-end"
	| "github"
	| "globe"
	| "hard-drive"
	| "heart"
	| "heart-off"
	| "inbox"
	| "info"
	| "inspector"
	| "layout-dashboard"
	| "loader"
	| "loader-circle"
	| "map"
	| "maximize"
	| "memory-stick"
	| "menu"
	| "mic"
	| "minimize"
	| "minus"
	| "moon"
	| "network"
	| "palette"
	| "panel-bottom"
	| "panel-bottom-open"
	| "panel-left"
	| "panel-left-close"
	| "panel-left-open"
	| "panel-right"
	| "panel-right-close"
	| "panel-right-open"
	| "pause"
	| "play"
	| "plus"
	| "redo"
	| "redo-2"
	| "refresh-cw"
	| "replace"
	| "resize-corner"
	| "rotate-cw"
	| "search"
	| "settings"
	| "settings-2"
	| "sort-ascending"
	| "sort-descending"
	| "square"
	| "square-terminal"
	| "star"
	| "star-fill"
	| "star-off"
	| "sun"
	| "thumbs-down"
	| "thumbs-up"
	| "triangle-alert"
	| "undo"
	| "undo-2"
	| "user"
	| "window-close"
	| "window-maximize"
	| "window-minimize"
	| "window-restore"

-- Drawn at the size and color of text, so `color` and `size` style it.
type IconProps = {
	class: Class?,
	style: Style?,
	name: IconName?,
	tone: Tone?,
	size: TextSize?,
	-- Luau doesn't check literals in list positions against a union, so
	-- write `name = "cpu"` (checked here) rather than `{ "cpu", ... }`.
	[number]: string,
}

type TerminalEngine = "alacritty" | "ghostty"

-- A live shell (your login shell). The host keeps it by mod and `id`.
type TerminalProps = {
	class: Class?,
	style: Style?,
	id: string?,
	-- Where it starts: a path or `~/...`; the home folder when unset.
	cwd: string?,
	engine: TerminalEngine?,
	[number]: string,
}

type RegionProps = {
	name: string,
	direction: Direction?,
	gap: Space?,
	justify: Justify?,
}

type Side = "top" | "bottom"
type Anchor = "start" | "end"

-- Content floating beside its anchor (the list part). While `open`, a click
-- outside sends `dismiss`. A `popover` stylesheet rule styles and animates it.
type PopoverProps = {
	class: Class?,
	style: Style?,
	open: boolean?,
	dismiss: Data,
	side: Side?,
	align: Anchor?,
	gap: number?,
	width: number?,
	content: Element,
	[number]: Child,
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
	terminal: (props: string | TerminalProps) -> Element,
	divider: () -> Element,
	spacer: () -> Element,
	region: (props: string | RegionProps) -> Element,
	mount: (name: string) -> Element,
	popover: (props: PopoverProps) -> Element,
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

-- What every `view` gets as `app`.
type App = {
	dark: boolean,
	mods: { string },
	regions: { [string]: { string } },
	-- Nil until the first sample arrives (from the `system` provider).
	system: System?,
	-- What each provider in app/providers/ publishes, as `app.<name>`.
	[string]: any,
}

-- A message `update` sends on.
type Send = {
	to: string,
	msg: Data,
}

type Mod<State> = {
	init: ((previous: State?) -> State)?,
	-- Returns nothing to keep the state it changed in place, or the new
	-- state and optionally a list of `Send`s. Its results are left untyped
	-- so it may return nothing on some paths; the host checks them.
	update: ((state: State, msg: any) -> ...any)?,
	view: (state: State, app: App) -> Child,
	region: string?,
	order: number?,
	title: string?,
	-- Rules for what this mod draws, by selector.
	styles: { [string]: Style }?,
}

declare function mod<State>(definition: Mod<State>): Mod<State>

type Stylesheet = {
	font_size: number?,
	font_family: string?,
	radius: number?,
	-- Each one hex color, or `{ light = ..., dark = ... }`. Typed `any`:
	-- Luau cannot check a map of unions against a table literal, so the host
	-- checks each entry when it loads the stylesheet.
	colors: { [string]: any }?,
	-- Named durations: milliseconds, or `"220ms"`; `any` as for colors.
	speeds: { [string]: any }?,
	animation_speed: number?,
	-- Styles by selector: `panel`, `.class`, `item:selected`.
	rules: { [string]: Style }?,
}

declare function style(sheet: Stylesheet): Stylesheet

-- A component in components/<name>.luau: a ready-made piece of view that
-- mods `use`, listed in the Components menu with a preview of `example`.
type Component<Props> = {
	title: string?,
	description: string?,
	-- The Components menu's submenu.
	category: string?,
	-- Shown beside its title in the browser.
	icon: IconName?,
	-- Luau that uses it; by default made from `example`.
	usage: string?,
	-- Props for the preview.
	example: any,
	build: (props: Props) -> Child,
}

declare function component<Props>(definition: Component<Props>): Component<Props>

-- The component `name`'s `build`: `local stat = use "stat"`, then
-- `stat { value = "12%" }` wherever an element goes.
declare function use(name: string): (props: any) -> Element

-- Prints to the terminal that runs the app.
declare function log(...: any): ()
