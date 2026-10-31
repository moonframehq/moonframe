-- A strip of tabs in one of six looks. Each tab has a label and may carry
-- an icon, an agent status (a spinner while working, a dot otherwise), a
-- badge and a close button. The mod keeps the tabs and which is selected.

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
	event: string?,
	close: string?,
	context: string?,
	closable: boolean?,
	grow: boolean?,
	to: string?,
}

local VARIANTS = { pills = true, underline = true, outline = true, cards = true, compact = true, classic = true }

return component {
	title = "Tabs",
	category = "Navigation",
	icon = "gallery-vertical-end",
	description = "A tab strip in six looks (pills, underline, outline, cards, compact, classic) with icons, agent status, badges, close and add buttons.",
	when = "Switch between open documents, terminals or agent sessions, or between views of one panel. For 2 to 4 settings choices use `segmented`; for joined action buttons use `button-group`; for onboarding steps use `pager`.",
	props = {
		["tabs"] = "list of { id, label, icon?, status? (\"working\" spinner, \"blocked\", \"done\", \"idle\" dot), badge? (count or text), closable?, tooltip?, msg? (sent instead of the select message) }",
		["selected"] = "id of the selected tab",
		["variant"] = "\"pills\" (default), \"underline\", \"outline\", \"cards\", \"compact\" or \"classic\"",
		["add"] = "message for a + button after the tabs; no button when unset",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["close"] = "key of the message the × sends, default \"close\": { close = id }",
		["context"] = "key of the message a right-click sends, default \"tab_menu\": { tab_menu = id }",
		["closable"] = "show × on every tab, not only the selected one",
		["grow"] = "stretch the tabs to share the width equally",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".tabs"] = { gap = 2, padding = 0, margin = 0 },
		[".tabs-tab"] = { padding = { 5, 10 }, gap = 6, radius = 7, color = "muted-foreground" },
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
	variant = "pills", -- underline, outline, cards, compact, classic
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
		local variant = if props.variant and VARIANTS[props.variant] then props.variant else "pills"
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
			cells[#cells + 1] = ui.row {
				class = { "tabs-tab", "tabs-" .. variant .. "-tab" },
				align = "center",
				justify = if props.grow then "center" else nil,
				grow = props.grow,
				selected = selected,
				msg = tab.msg or { [event] = id },
				context = { [context] = id },
				tooltip = tab.tooltip,
				to = props.to,
				status,
				tab.icon and ui.icon { name = tab.icon :: any, class = "tabs-icon" },
				ui.text { tab.label, class = "tabs-label", truncate = true },
				tab.badge ~= nil and ui.row { class = "tabs-badge", ui.text { tostring(tab.badge) } },
				closable and ui.row {
					class = "tabs-close",
					msg = { [close] = id },
					to = props.to,
					tooltip = "Close",
					ui.icon "close",
				},
			}
		end
		if props.add ~= nil then
			cells[#cells + 1] = ui.row {
				class = "tabs-add",
				align = "center",
				msg = props.add,
				to = props.to,
				tooltip = "New tab",
				ui.icon "plus",
			}
		end
		return ui.row { class = { "tabs", "tabs-" .. variant }, align = "center", table.unpack(cells) }
	end,
}
