-- A strip of tabs in one of several looks. Each tab has a label and may
-- carry an icon, an agent status (a spinner while working, a dot otherwise),
-- a badge and a close button. The mod keeps the tabs and which is selected.
-- Tabs are keyed by id, so one grows in as it opens and shrinks away as it
-- closes (the `.tabs-tab` rule's `transition`).

type Tab = {
	id: string?,
	label: string,
	icon: string?,
	status: string?,
	badge: any,
	closable: boolean?,
	tooltip: string?,
	msg: any,
}

type Props = {
	tabs: { Tab },
	selected: string?,
	variant: string?,
	add: any,
	-- A menu the + button opens under itself: { open, dismiss, content, width? }.
	add_menu: any,
	event: string?,
	close: string?,
	context: string?,
	closable: boolean?,
	grow: boolean?,
	width: number?,
	to: string?,
	-- Drawn in a bar that has a line of its own: the strip draws none.
	in_bar: boolean?,
}

-- A look is a pair of classes, `tabs-<look>` on the strip and
-- `tabs-<look>-tab` on each tab: any name works, so a stylesheet can add
-- looks with rules alone. Names are kept plain, as classes need.
local function look(name: string?): string
	if name and string.match(name, "^[%w_-]+$") then
		return name
	end
	return "pills"
end

-- A tab's key, from its id when the id is plain enough for one.
local function key_of(id: string): string?
	if #id <= 64 and string.match(id, "^[%w_-]+$") then
		return id
	end
	return nil
end

-- The curved look's flare where the selected tab meets what is under it:
-- a corner of the tab's color with a rounded notch of the strip's.
local function shoulder(side: string)
	return ui.row {
		class = "tabs-shoulder",
		float = if side == "left" then { bottom = 0, left = -10 } else { bottom = 0, right = -10 },
		width = 10,
		height = 10,
		ui.row { class = "tabs-shoulder-" .. side, width = 10, height = 10 },
	}
end

return component {
	title = "Tabs",
	category = "Navigation",
	icon = "gallery-vertical-end",
	description = "A tab strip in several looks (pills, underline, outline, cards, compact, classic, square, curved, browser) with icons, agent status, badges, close and add buttons; tabs grow in as they open and shrink away as they close.",
	when = "Switch between open documents, terminals or agent sessions, or between views of one panel. For 2 to 4 settings choices use `segmented`; for joined action buttons use `button-group`; for onboarding steps use `pager`.",
	props = {
		["tabs"] = "list of { id, label, icon?, status? (\"working\" spinner, \"blocked\", \"done\", \"idle\" dot), badge? (count or text), closable?, tooltip?, msg? (sent instead of the select message) }",
		["selected"] = "id of the selected tab",
		["variant"] = "\"pills\" (default), \"underline\", \"outline\", \"cards\", \"compact\", \"classic\", \"square\" \"curved\" (the selected tab joins what is under the strip: put the strip right on top of a card of the same color) or \"browser\" (grey fixed-width tabs, the selected one white and raised: give `width`); or any look a stylesheet defines with `.tabs-<look>` and `.tabs-<look>-tab` rules",
		["add"] = "message for a + button after the tabs; no button when unset",
		["add_menu"] = "a menu the + button opens under itself: { open, dismiss (message on a click outside), content, width? }",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["close"] = "key of the message the × sends, default \"close\": { close = id }",
		["context"] = "key of the message a right-click sends, default \"tab_menu\": { tab_menu = id }",
		["closable"] = "show × on every tab, not only the selected one",
		["grow"] = "stretch the tabs to share the width equally",
		["width"] = "each tab's width in pixels: labels truncate and the × sits at the far end, as in a browser",
		["to"] = "mod to send messages to, if not the drawing one",
		["in_bar"] = "drawn in a title bar or toolbar with its own bottom line: the strip draws no line of its own (`.tabs-in-bar`)",
	},
	styles = {
		[".tabs"] = { gap = 2, padding = 0, margin = 0 },
		-- Tabs grow in as they open and shrink away as they close; "none"
		-- stops it.
		[".tabs-tab"] = { padding = { 5, 10 }, gap = 6, radius = 7, color = "muted-foreground", transition = "slide-fade fast ease-out" },
		[".tabs-tab:hover"] = { background = "list-hover", color = "foreground" },
		[".tabs-tab:selected"] = { color = "foreground" },
		[".tabs-label"] = { weight = "medium" },
		[".tabs-icon"] = { size = "small" },
		[".tabs-close"] = { padding = 2, radius = 4, color = "muted-foreground", size = "small" },
		[".tabs-close:hover"] = { background = "muted-foreground/20", color = "foreground" },
		[".tabs-add"] = { padding = { 5, 7 }, radius = 7, color = "muted-foreground" },
		[".tabs-add:hover"] = { background = "list-hover", color = "foreground" },
		[".tabs-badge"] = { padding = { 0, 5 }, radius = 8, background = "muted-foreground/15", color = "muted-foreground", size = "small" },
		[".tabs-dot"] = { radius = 3, background = "muted-foreground/50" },
		[".tabs-dot-blocked"] = { background = "warning" },
		[".tabs-dot-done"] = { background = "success" },
		[".tabs-dot-idle"] = { background = "muted-foreground/40" },
		[".tabs-spinner"] = { color = "accent", size = "small" },

		-- pills: a soft filled pill on the selected tab.
		[".tabs-pills-tab"] = { radius = 999 },
		[".tabs-pills-tab:selected"] = { background = "accent/15", color = "accent" },

		-- underline: a line under the selected tab, over a rule.
		[".tabs-underline"] = { gap = 4, border = "bottom", border_color = "border" },
		[".tabs-underline-tab"] = { radius = 0, padding = { 7, 8 }, border = "bottom", border_color = "#00000000" },
		[".tabs-underline-tab:hover"] = { background = "#00000000", border_color = "muted-foreground/40" },
		[".tabs-underline-tab:selected"] = { border_color = "accent", color = "foreground" },

		-- outline: a bordered box around the selected tab.
		[".tabs-outline-tab"] = { border = "all", border_color = "#00000000" },
		[".tabs-outline-tab:selected"] = { border_color = "border", background = "background" },

		-- cards: tabs as cards on a sunken strip, the selected one raised.
		[".tabs-cards"] = { background = "muted/60", padding = 3, radius = 9, gap = 3 },
		[".tabs-cards-tab"] = { radius = 6 },
		[".tabs-cards-tab:hover"] = { background = "background/50" },
		[".tabs-cards-tab:selected"] = { background = "background", shadow = "0 1 2 #0000001f", color = "foreground" },

		-- compact: small and tight, for dense strips.
		[".tabs-compact"] = { gap = 1 },
		[".tabs-compact-tab"] = { padding = { 2, 7 }, radius = 5, gap = 4, size = "small" },
		[".tabs-compact-tab:selected"] = { background = "list-active" },

		-- classic: flat tabs with dividers, the selected one the page colour.
		[".tabs-classic"] = { gap = 0, background = "sidebar", border = "bottom", border_color = "border" },
		[".tabs-classic-tab"] = { radius = 0, padding = { 7, 12 }, border = "right", border_color = "border" },
		[".tabs-classic-tab:hover"] = { background = "list-hover" },
		[".tabs-classic-tab:selected"] = { background = "background", color = "foreground" },

		-- square: flat tabs with no rounding, the selected one filled.
		[".tabs-square"] = { gap = 0, border = "bottom", border_color = "border" },
		[".tabs-square-tab"] = { radius = 0, padding = { 7, 12 } },
		[".tabs-square-tab:hover"] = { background = "list-hover" },
		[".tabs-square-tab:selected"] = { background = "accent/15", color = "foreground" },

		-- browser: grey tabs of one width, the selected one white and
		-- lifted, as a browser's tab strip in a light title bar.
		[".tabs-browser"] = { gap = 8 },
		[".tabs-browser-tab"] = { radius = 7, padding = { 6, 10 }, gap = 8, background = "foreground/6", color = "muted-foreground" },
		[".tabs-browser-tab:hover"] = { background = "foreground/9", color = "foreground" },
		[".tabs-browser-tab:selected"] = { background = "background", color = "foreground", shadow = "0 1px 2px #0000000f" },

		-- In a bar with a line of its own, the strip's would double it.
		[".tabs-in-bar"] = { border = "none" },

		-- curved: the selected tab is a body in the card's color, square at
		-- its foot, whose shoulders flare out into the card under the strip;
		-- the rest are bare titles in a pill under the pointer, clear of the
		-- foot. Recolor `.tabs-shoulder-left` and `-right` with the strip.
		[".tabs-curved"] = { gap = 4 },
		[".tabs-curved-tab"] = { radius = 8, padding = { 6, 12 }, margin = { bottom = 4 } },
		[".tabs-curved-tab:hover"] = { background = "list-hover" },
		[".tabs-curved-tab:selected"] = {
			background = "card",
			color = "foreground",
			radius = { 8, 8, 0, 0 },
			padding = { 6, 12, 10, 12 },
			margin = { bottom = 0 },
		},
		[".tabs-shoulder"] = { background = "card" },
		[".tabs-shoulder-left"] = { background = "sidebar", radius = { 0, 0, 10, 0 } },
		[".tabs-shoulder-right"] = { background = "sidebar", radius = { 0, 0, 0, 10 } },
	},
	example = {
		selected = "api",
		add = "new-tab",
		tabs = {
			{ id = "api", label = "ledger-api", icon = "square-terminal", status = "working" },
			{ id = "docs", label = "harbor-docs", icon = "agent-codex", status = "blocked", badge = 2 },
			{ id = "forge", label = "pixel-forge", icon = "agent-claude", status = "done" },
			{ id = "notes", label = "notes.md", icon = "file", status = "idle" },
		},
	},
	usage = [[local tabs = use "tabs"

tabs {
	variant = "pills", -- underline, outline, cards, compact, classic, square, curved, browser (with width = 200)
	selected = state.tab, add = "new-tab",
	tabs = {
		{ id = "api", label = "ledger-api", icon = "square-terminal", status = "working" },
		{ id = "docs", label = "harbor-docs", icon = "agent-codex", status = "blocked", badge = 2 },
		{ id = "forge", label = "pixel-forge", icon = "agent-claude", status = "done" },
		{ id = "notes", label = "notes.md", icon = "file", status = "idle" },
	},
}
-- update: msg.select picks a tab, msg.close closes one, msg.tab_menu
-- (right-click) opens a menu, "new-tab" adds one.]],

	build = function(props: Props)
		local variant = look(props.variant)
		local event = props.event or "select"
		local close = props.close or "close"
		local context = props.context or "tab_menu"

		local cells = {}
		for _, tab in props.tabs do
			local id = tab.id or tab.label
			local selected = id == props.selected
			local status: any = false
			if tab.status == "working" then
				status = ui.icon { name = "loader-circle", spin = true, class = "tabs-spinner" }
			elseif tab.status then
				status = ui.row { class = { "tabs-dot", "tabs-dot-" .. tab.status }, width = 6, height = 6 }
			end
			local closable = tab.closable or props.closable or selected
			local joined = selected and variant == "curved"
			cells[#cells + 1] = ui.row {
				key = key_of(id),
				class = { "tabs-tab", "tabs-" .. variant .. "-tab" },
				align = "center",
				justify = if props.grow then "center" else nil,
				grow = props.grow,
				width = props.width,
				shrink = if props.width then false else nil,
				selected = selected,
				msg = tab.msg or { [event] = id },
				context = { [context] = id },
				tooltip = tab.tooltip,
				to = props.to,
				status,
				tab.icon and ui.icon { name = tab.icon :: any, class = "tabs-icon" },
				if props.width
					then ui.row { grow = true, ui.text { tab.label, class = "tabs-label", truncate = true } }
					else ui.text { tab.label, class = "tabs-label", truncate = true },
				tab.badge ~= nil and ui.row { class = "tabs-badge", ui.text { tostring(tab.badge) } },
				closable and ui.row {
					class = "tabs-close",
					msg = { [close] = id },
					to = props.to,
					tooltip = "Close",
					ui.icon "close",
				},
				joined and shoulder("left"),
				joined and shoulder("right"),
			}
		end
		if props.add ~= nil then
			local button = ui.row {
				class = "tabs-add",
				align = "center",
				msg = props.add,
				to = props.to,
				tooltip = "New tab",
				ui.icon "plus",
			}
			local menu = props.add_menu
			-- Keyed too, so a strip with no tabs still notices the first.
			cells[#cells + 1] = ui.row {
				key = "_add",
				if menu
					then ui.popover {
						open = menu.open,
						dismiss = menu.dismiss,
						to = props.to,
						side = "bottom",
						align = "start",
						width = menu.width or 220,
						content = menu.content,
						button,
					}
					else button,
			}
		end
		local classes = { "tabs", "tabs-" .. variant }
		if props.in_bar then
			table.insert(classes, "tabs-in-bar")
		end
		return ui.row { class = classes, align = "center", table.unpack(cells) }
	end,
}
