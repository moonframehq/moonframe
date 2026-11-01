-- A whole sidebar column: an optional header (a brand row, a search field),
-- foldable sections of rows that scroll, and footer links pinned to the
-- bottom. Stateless: the mod keeps which sections are collapsed.

local section = use "sidebar-section"
local status_dot = use "status-dot"

type Section = {
	title: string,
	id: string?,
	count: (number | string)?,
	actions: { any }?,
	collapsed: boolean?,
	toggle: any,
	[number]: any,
}

type FooterItem = {
	icon: string?,
	label: string,
	msg: any,
	to: string?,
	dot: boolean?,
	muted: boolean?,
	tooltip: string?,
}

type Props = {
	header: any,
	sections: { Section }?,
	footer: { FooterItem }?,
	to: string?,
	width: number?,
	height: number?,
	fill: boolean?,
	[number]: any,
}

-- Example rows, in a workspace style: a status dot, a name and a
-- muted branch under it.
local function workspace(id: string, name: string, branch: string, status: string, selected: boolean?)
	return ui.row {
		class = "sidebar-example-row",
		selected = selected,
		align = "center",
		msg = { open = id },
		status_dot { status = status, size = "small" },
		ui.column {
			gap = "none",
			grow = true,
			ui.text { name, weight = "medium", truncate = true },
			ui.text { branch, size = "small", tone = "muted", truncate = true },
		},
	}
end

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

-- The children in order. `ipairs` and `#` stop at the first `nil`, which a
-- child such as `cond and ui.text "x"` leaves, and drop what follows it.
local function positional(props: any): { any }
	local keys = {}
	for key, child in props do
		if type(key) == "number" and child then
			table.insert(keys, key)
		end
	end
	table.sort(keys)
	local list = table.create(#keys)
	for _, key in keys do
		table.insert(list, props[key])
	end
	return list
end

return component {
	title = "Sidebar",
	category = "Navigation",
	icon = "panel-left",
	description = "A full sidebar column: header slot, foldable sections of rows with counts and actions, footer links.",
	when = "The whole left column of an app: workspaces, agents, settings categories. One group alone is `sidebar-section`; the narrow collapsed form is `rail`; a VS Code style icon strip beside it is `activity-bar`; nested rows are `tree`.",
	props = {
		["header"] = "element at the top, outside the scroll (a brand row, a search field)",
		["sections"] = "list of { title, id?, count?, actions? (elements), collapsed?, toggle? msg, [rows as elements] }; count defaults to the number of rows",
		["footer"] = "list of { icon, label, msg, to?, dot? (accent dot: an update ready), muted?, tooltip? } pinned to the bottom",
		["to"] = "mod to send section toggles and footer msgs to, if not the drawing one",
		["width"] = "fixed width in pixels; leave unset inside a sized pane",
		["height"] = "fixed height in pixels; leave unset inside a pane, where it fills the height",
		["fill"] = "false to size to its content instead of filling the pane (no scrolling); default true",
		["[children]"] = "extra content after the sections, inside the scroll",
	},
	styles = {
		[".sidebar"] = { background = "sidebar", gap = 0, border = "right", border_color = "border" },
		[".sidebar-header"] = { padding = { 10, 12 }, gap = 8 },
		[".sidebar-body"] = { padding = { 4, 0 }, gap = 6 },
		[".sidebar-footer"] = { border = "top", border_color = "border", padding = { 4, 8, 8, 8 }, gap = 2 },
		[".sidebar-footer-row"] = { padding = { 5, 8 }, gap = 9, radius = 8, color = "muted-foreground" },
		[".sidebar-footer-row:hover"] = { background = "list-hover", color = "foreground" },
		[".sidebar-footer-muted"] = { color = "muted-foreground/70" },
		[".sidebar-footer-dot"] = { background = "accent", radius = 999 },
		[".sidebar-example-row"] = { padding = { 6, 10 }, gap = 10, radius = 8 },
		[".sidebar-example-row:hover"] = { background = "list-hover" },
		[".sidebar-example-row:selected"] = { background = "list-active" },
		[".sidebar-brand"] = { weight = "bold" },
		[".sidebar-brand-mark"] = { background = "accent", color = "accent-foreground", radius = 5 },
	},
	usage = [[local sidebar = use "sidebar"

sidebar {
	header = ui.row { align = "center", gap = "normal",
		ui.text { "Moonframe", weight = "bold" }, ui.spacer(),
		ui.row { msg = "search", ui.icon "search" },
	},
	sections = {
		{ title = "Spaces", id = "spaces", collapsed = state.folded.spaces,
			actions = { ui.button { "new", small = true, variant = "ghost", msg = "new" } },
			ui.item { "ledger-api", detail = "main", selected = true, msg = { open = "ledger-api" } },
			ui.item { "pixel-forge", detail = "main", msg = { open = "pixel-forge" } },
		},
		{ title = "Agents", id = "agents", collapsed = state.folded.agents,
			ui.item { "feat-multi-currency", detail = "claude", msg = { open = "feat" } },
		},
	},
	footer = {
		{ icon = "settings", label = "Settings", msg = "settings", dot = app.update_ready },
		{ icon = "plus", label = "Add machine", msg = "add-machine", muted = true },
	},
}
-- update: msg.toggle is the section id (or title) to fold or unfold.]],
	example = {
		width = 280,
		fill = false,
		header = ui.row {
			align = "center",
			gap = "normal",
			ui.row {
				class = "sidebar-brand-mark",
				width = 20,
				height = 20,
				align = "center",
				justify = "center",
				ui.icon { name = "frame", size = "small" },
			},
			ui.text { "Moonframe", class = "sidebar-brand" },
			ui.spacer(),
			ui.row { msg = "search", tooltip = "Search", ui.icon { name = "search", tone = "muted" } },
			ui.row { msg = "collapse", tooltip = "Collapse sidebar", ui.icon { name = "panel-left", tone = "muted" } },
		},
		sections = loose {
			{
				title = "Spaces",
				id = "spaces",
				actions = { ui.button { "new", small = true, variant = "ghost", msg = "new-space" } },
				workspace("ledger", "ledger-api", "main", "idle"),
				workspace("multi", "feat/multi-currency", "feat/multi-currency", "working", true),
				workspace("rounding", "fix/rounding", "fix/rounding", "blocked"),
				workspace("pixel", "pixel-forge", "main", "done"),
			},
			{
				title = "Agents",
				id = "agents",
				actions = { ui.button { "grouped", small = true, variant = "ghost", msg = "group" } },
				workspace("a1", "ledger-api", "claude · editor", "working"),
				workspace("a2", "harbor-docs", "pi · docs", "blocked"),
				workspace("a3", "fix-rounding", "codex", "done"),
			},
			{ title = "Archived", id = "archived", count = 12, collapsed = true },
		},
		footer = loose {
			{ icon = "settings", label = "Settings", msg = "settings", dot = true },
			{ icon = "plus", label = "Add machine", msg = "add-machine", muted = true },
		},
	},

	build = function(props: Props)
		local sections = {}
		local list: { Section } = props.sections or {}
		for index, entry in ipairs(list) do
			local rows = {}
			for _, row in ipairs(entry) do
				rows[#rows + 1] = row
			end
			sections[index] = section {
				title = entry.title,
				id = entry.id,
				count = if entry.count ~= nil then entry.count else #rows,
				actions = entry.actions,
				collapsed = entry.collapsed,
				toggle = entry.toggle,
				to = props.to,
				table.unpack(rows),
			}
		end
		for _, child in positional(props) do
			sections[#sections + 1] = child
		end

		local footer = nil
		if props.footer and #props.footer > 0 then
			local rows = {}
			for index, item in props.footer do
				rows[index] = ui.row {
					class = item.muted and "sidebar-footer-row sidebar-footer-muted" or "sidebar-footer-row",
					align = "center",
					msg = item.msg,
					to = item.to or props.to,
					tooltip = item.tooltip,
					item.icon ~= nil and ui.icon { name = item.icon :: any },
					ui.text { item.label, truncate = true },
					item.dot == true and ui.row { class = "sidebar-footer-dot", width = 6, height = 6 },
				}
			end
			footer = ui.column { class = "sidebar-footer", shrink = false, table.unpack(rows) }
		end

		local fills = props.fill ~= false or props.height ~= nil
		return ui.column {
			class = "sidebar",
			grow = props.height == nil and props.fill ~= false,
			width = props.width,
			height = props.height,
			props.header ~= nil and ui.column { class = "sidebar-header", shrink = false, props.header },
			ui.column { class = "sidebar-body", grow = fills, scroll = fills, table.unpack(sections) },
			footer or false,
		}
	end,
}
