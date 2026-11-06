-- The stylesheet: colors, corners, and spacing on top of the layout the
-- modules describe. Delete this file and the app falls back to the stock
-- theme; edit it while the window is open to see each change.
return style {
	font_size = 14,
	radius = 8,
	-- The fixed-width font terminals use: family, size, weight (100-900),
	-- line_height, fallbacks, ligatures. Defaults to an installed Nerd Font
	-- Mono (JetBrains preferred), otherwise the platform's monospace font.
	-- "Symbols Nerd Font Mono", when installed, supplies missing icons.
	-- mono = { family = "JetBrainsMono Nerd Font Mono", size = 13, line_height = 1.3 },

	-- Named speeds for transitions; fast (120ms), normal (220ms), and slow
	-- (400ms) exist already and can be redefined.
	speeds = { drawer = "220ms" },
	-- Every animation at once: 2 is twice as fast, 0.5 half, 0 turns them off.
	animation_speed = 1,

	-- The look each kind of component takes, unless its code picks one.
	-- Tabs: pills, underline, outline, cards, compact, classic, square,
	-- curved (the selected tab runs on into the main card, and the title bar
	-- moves over it), or your own: define `.tabs-<name>` and
	-- `.tabs-<name>-tab` rules below.
	looks = { tabs = "pills" },

	colors = {
		-- Names the theme already has re-theme the whole app, buttons included,
		-- in the Default theme; one picked in View > Themes brings its own.
		accent = { light = "#4f46e5", dark = "#818cf8" },
		sidebar = { light = "#f4f4f6", dark = "#141417" },
		card = { light = "#ffffff", dark = "#1e1e22" },
		-- New names can be used anywhere a color goes.
	},

	rules = {
		-- Panels float as rounded cards on the sidebar background.
		panel = { background = "card", radius = 12, margin = { 8, 6 }, border = true },
		-- A terminal takes its panel's color, so it runs on into the header,
		-- and its corners, inside the panel's border, so its background stays
		-- within the card's rounded corners.
		terminal = { background = "card", radius = 11 },
		-- Padding and margin take one value, { y, x }, { top, right, bottom, left },
		-- or named edges such as { x = 14, top = 10 }.
		["panel-header"] = { border = "none", padding = { top = 10, x = 14 }, color = "accent" },
		-- Navigation rows sit inset from the pane's edges, so the hover and
		-- selected backgrounds are rounded pills, not edge-to-edge bars.
		item = { padding = { 6, 10 }, margin = { x = 8 }, radius = 8 },

		-- Side panels slide in and out when the toolbar hides or shows them.
		-- Kinds: slide, fade, slide-fade, none. Curves: linear, ease, ease-in,
		-- ease-out, ease-in-out, cubic-bezier(x1, y1, x2, y2). Durations:
		-- 220ms, 0.22s, or a speed name.
		pane = { transition = "slide drawer ease-out" },

		["item:selected"] = { background = "accent/15", color = "accent", weight = "medium" },
		["item:hover"] = { background = "accent/8" },

		-- Text fields (ui.input): the frame and its states, then what the
		-- editor paints inside it. A focused field ignores its hover rule, and has
		-- no look of its own: the caret shows it.
		input = { background = "background", border = true, border_color = "border", radius = 6, padding = { 5, 8 } },
		["input:hover"] = { border_color = "muted-foreground/50" },
		["input:disabled"] = { opacity = 0.5 },
		["input-placeholder"] = { color = "muted-foreground" },
		["input-selection"] = { background = "accent/25" },
		["input-caret"] = { background = "accent" },

		-- Markdown (ui.markdown): its code, in blocks and inline, and quotes.
		["markdown-code"] = { background = "muted-foreground/8", radius = 6, padding = { 8, 12 }, font = "mono" },
		["markdown-quote"] = { border = "left", border_color = "border", color = "muted-foreground", padding = { 0, 12 } },
		["markdown-alert"] = { border = "left", background = "muted-foreground/6", radius = 6, padding = { 8, 12 } },

		badge = { background = "accent/12", color = "accent" },
		divider = { background = "border" },

		-- The status bar's widgets (mods/usage, mods/system).
		[".status-text"] = { size = "small", color = "muted-foreground" },
		[".status-label"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".usage-widget"] = { padding = { 2, 6 }, radius = 6 },
		[".usage-widget:hover"] = { background = "accent/10" },
		-- The usage mod still draws its status and sheet bars with these classes.
		[".track"] = { background = "muted-foreground/20", radius = 3 },
		[".fill"] = { background = "accent", radius = 3 },
		[".fill-hot"] = { background = "warning" },

		-- The usage sheet: a floating card that slides up and fades in.
		popover = {
			background = "card",
			radius = 12,
			border = true,
			padding = 16,
			shadow = "0 12px 32px -8px #00000040, 0 2px 6px #0000001a",
			transition = "slide-fade fast ease-out",
		},
		-- Overlays (dialogs, palettes, sheets, toasts) float over the window;
		-- a sheet along an edge slides in from it. `overlay-backdrop` is what
		-- dims the window behind one with `backdrop = true`.
		overlay = {
			background = "card",
			shadow = "0 24px 48px -12px #00000059, 0 2px 6px #0000001a",
			transition = "slide-fade fast ease-out",
		},
		["overlay-backdrop"] = { background = "#000000/35" },
		-- Tooltips wait 400ms under a resting pointer, so passing over a
		-- control doesn't flash one, then rise and fade in.
		tooltip = {
			radius = 6,
			shadow = "0 6px 16px -4px #00000033, 0 1px 3px #0000001a",
			transition = "slide-fade fast ease-out 400ms",
		},
		-- What follows the pointer while a row with `carry` is dragged.
		carry = {
			background = "card/90",
			border = "all",
			border_color = "accent/60",
			radius = 8,
			shadow = "0 12px 28px -8px #00000047, 0 2px 6px #0000001a",
		},
		[".sheet-title"] = { weight = "semibold", size = "large" },
		[".muted"] = { size = "small", color = "muted-foreground" },
		[".usage-tabs"] = { margin = { top = 4 } },
		[".usage-tab"] = { padding = { 6, 0 }, radius = 8, color = "muted-foreground" },
		[".usage-tab:hover"] = { background = "accent/8" },
		[".usage-tab-active"] = { background = "accent/12", color = "accent" },
		[".usage-tab-label"] = { size = "small" },

		-- Classes: opt in with `class = "..."` on an element.
		[".toolbar"] = { background = "sidebar", border = "bottom", padding = { 0, 8 } },
		-- Under curved tabs: no line where the selected tab meets the card,
		-- and the card starts right under the strip.
		[".toolbar-joined"] = { border = "none" },
		[".tab-card-joined"] = { margin = { top = 0 }, border = "none" },
		-- A window in the background dims its title bar, as the system's does.
		[".toolbar-inactive"] = { opacity = 0.75 },
		[".toolbar-title"] = { size = "small", color = "muted-foreground" },
		[".tool"] = { padding = 6, radius = 6, color = "muted-foreground" },
		[".tool:hover"] = { background = "accent/10", color = "foreground" },
		[".tool-on"] = { color = "accent" },
		[".tool-on:hover"] = { background = "accent/12", color = "accent" },
		-- The Make it yours sparkles (mods/feedback) where an app without a title
		-- bar of tools floats it over a corner of the window.
		[".feedback-corner"] = { background = "card", border = "all", border_color = "border", radius = 999, padding = 2, shadow = "0 4px 12px -4px #00000033" },
		-- Its button is round, so its hover fits a round corner.
		[".feedback-button"] = { radius = 999 },
		[".hero"] = { color = "accent", size = "title", weight = "bold" },
	},
}
