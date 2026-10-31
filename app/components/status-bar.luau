-- A thin bar along the window's bottom edge, styled as a footer:
-- items on the left, items pushed right, optional thin separators. Each item
-- is an element, a string, or a small table { icon, text, tone, msg }.

type Entry = {
	icon: string?,
	text: string?,
	tone: string?,
	msg: any,
	to: string?,
	tooltip: string?,
	mono: boolean?,
}

type Props = {
	left: { any }?,
	right: { any }?,
	separators: boolean?,
	to: string?,
}

local function entry(item: any, to: string?)
	if typeof(item) == "string" then
		return ui.text { item, class = "status-bar-text" }
	end
	if typeof(item) ~= "table" or item.type ~= nil then
		return item
	end
	local e = item :: Entry
	local classes = { "status-bar-item" }
	if e.tone then
		classes[#classes + 1] = `status-bar-{e.tone}`
	end
	if e.mono then
		classes[#classes + 1] = "status-bar-mono"
	end
	if e.msg ~= nil then
		classes[#classes + 1] = "status-bar-button"
	end
	return ui.row {
		class = classes,
		align = "center",
		shrink = false,
		msg = e.msg,
		to = e.to or (if e.msg ~= nil then to else nil),
		tooltip = e.tooltip,
		e.icon ~= nil and ui.icon { name = e.icon :: any },
		e.text ~= nil and ui.text { e.text },
	}
end

local function group(items: { any }?, props: Props)
	local out = {}
	for index, item in ipairs(items or {}) do
		if props.separators and index > 1 then
			out[#out + 1] = ui.column { class = "status-bar-separator", width = 1, height = 12 }
		end
		out[#out + 1] = entry(item, props.to)
	end
	return out
end

-- Lists in the preview mix optional fields; typed loosely for the checker.
local function loose(list: { any }): { any }
	return list
end

return component {
	title = "Status bar",
	category = "Status bar",
	icon = "panel-bottom",
	description = "A thin footer bar with left and right items (elements, text, or { icon, text, tone, msg }) and optional separators.",
	when = "The window's bottom bar: machine, branch, agent counts on the left; CPU, theme, shortcuts on the right. Put `stat`, `meter` or `sparkline` in it for live readings. For a coloured strip announcing something use `banner`; for a top row use `title-bar`; for an extensible bar, give it a `ui.region` as an item.",
	props = {
		["left"] = "items on the left: elements, strings, or { icon, text, tone? (muted, accent, success, warning, danger), msg?, to?, tooltip?, mono? }",
		["right"] = "items pushed to the right, same shapes",
		["separators"] = "thin vertical rules between items",
		["to"] = "mod to send the items' msgs to, if not the drawing one",
	},
	styles = {
		[".status-bar"] = {
			background = "sidebar",
			border = "top",
			border_color = "border",
			padding = { 0, 12 },
			gap = 14,
			size = "small",
			color = "muted-foreground",
		},
		[".status-bar-group"] = { gap = 14 },
		[".status-bar-item"] = { gap = 5, radius = 5 },
		[".status-bar-button"] = { padding = { 1, 5 } },
		[".status-bar-button:hover"] = { background = "list-hover", color = "foreground" },
		[".status-bar-mono"] = { font = "mono" },
		[".status-bar-muted"] = { color = "muted-foreground/70" },
		[".status-bar-accent"] = { color = "accent" },
		[".status-bar-success"] = { color = "success" },
		[".status-bar-warning"] = { color = "warning" },
		[".status-bar-danger"] = { color = "danger" },
		[".status-bar-separator"] = { background = "border" },
	},
	usage = [[local status_bar = use "status-bar"

status_bar {
	left = {
		{ icon = "laptop", text = "This computer" },
		{ icon = "git-branch", text = app.git and app.git.branch or "main", mono = true },
		{ text = "4 agents" },
		{ text = "· 1 needs you", tone = "danger", msg = "show-blocked" },
	},
	right = {
		{ icon = "cpu", text = "CPU 27%" },
		{ icon = "sun", text = "Theme", msg = "toggle-theme" },
		{ icon = "keyboard", text = "Shortcuts", msg = "shortcuts" },
	},
}]],
	example = {
		left = loose {
			{ icon = "git-branch", text = "feat/multi-currency", mono = true },
			{ icon = "bot", text = "4 agents" },
			{ text = "1 needs you", tone = "danger", msg = "show-blocked" },
		},
		right = loose {
			{ icon = "cpu", text = "27%" },
			{ icon = "moon", text = "Theme", msg = "toggle-theme" },
			{ icon = "keyboard", text = "Shortcuts", msg = "shortcuts" },
		},
	},

	build = function(props: Props)
		return ui.row {
			class = "status-bar",
			height = 28,
			shrink = false,
			align = "center",
			ui.row { class = "status-bar-group", align = "center", table.unpack(group(props.left, props)) },
			ui.spacer(),
			ui.row { class = "status-bar-group", align = "center", table.unpack(group(props.right, props)) },
		}
	end,
}
