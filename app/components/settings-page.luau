-- A settings window's body: `settings-nav` on the left; on the right a
-- scrolling page with a title, sections under uppercase headings holding
-- `settings-row`s, and a footer with the save status and buttons.

local settings_nav = use "settings-nav"
local settings_row = use "settings-row"
local switch = use "switch"
local segmented = use "segmented"
local dropdown = use "dropdown"

type Row = {
	title: string,
	description: string?,
	tag: string?,
	tag_tone: string?,
	disabled: boolean?,
	reason: string?,
	stacked: boolean?,
	-- The control: one element, or a list of them.
	control: any,
}

type Section = {
	title: string?,
	description: string?,
	rows: { Row }?,
	-- Elements instead of (or after) rows.
	content: { any }?,
}

type Props = {
	nav: any,
	title: string?,
	subtitle: string?,
	sections: { Section }?,
	status: string?,
	-- `saved` (default, muted), `unsaved` (warning dot) or `error` (danger dot).
	state: string?,
	buttons: { any }?,
	height: number?,
	to: string?,
	[number]: any,
}

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
	title = "Settings page",
	category = "Layout",
	icon = "settings",
	description = "A settings window body: grouped nav on the left, a scrolling page of titled sections and setting rows, a save-status footer.",
	when = "A whole preferences window or settings tab. It is made of `settings-nav` (the page list) and `settings-row` (each setting); use those directly for a different arrangement, `section` for a plain heading, `card` to box a group.",
	props = {
		["nav"] = "props for `settings-nav`: { groups, selected, search, title, footer, event }",
		["title"] = "the page's title (\"Sidebar\")",
		["subtitle"] = "muted line under the title",
		["sections"] = "list of { title = \"Layout\", description, rows = { { title, description, tag, disabled, reason, stacked, control } }, content = { elements } }",
		["status"] = "footer text; default \"All changes saved.\"",
		["state"] = "`saved` (default), `unsaved` (warning dot) or `error` (danger dot)",
		["buttons"] = "footer buttons { { label, msg, variant } }; default a Reload button sending `\"reload\"`",
		["height"] = "pixels; otherwise it fills its parent",
		["to"] = "mod the footer buttons send to, if not the drawing one",
		["[children]"] = "page content after the sections",
	},
	styles = {
		[".settings-page"] = { background = "background", border = "all", border_color = "border", radius = 10, gap = 0 },
		[".settings-page-main"] = { gap = 0 },
		[".settings-page-scroll"] = { padding = { 24, 28 }, gap = 22 },
		[".settings-page-head"] = { gap = 2 },
		[".settings-page-title"] = { size = "title", weight = "bold" },
		[".settings-page-subtitle"] = { color = "muted-foreground" },
		[".settings-page-section"] = { gap = 0 },
		[".settings-page-heading"] = {
			case = "upper",
			size = "small",
			weight = "medium",
			color = "muted-foreground",
			padding = { 0, 0, 6, 0 },
			border = "bottom",
			border_color = "border",
		},
		[".settings-page-section-description"] = { size = "small", color = "muted-foreground", padding = { 6, 0, 0, 0 } },
		[".settings-page-footer"] = { padding = { 8, 14 }, gap = 10, border = "top", border_color = "border", background = "sidebar" },
		[".settings-page-status"] = { size = "small", color = "muted-foreground" },
		[".settings-page-dot"] = { radius = 4, background = "success" },
		[".settings-page-dot-unsaved"] = { background = "warning" },
		[".settings-page-dot-error"] = { background = "danger" },
	},
	usage = [[local settings_page = use "settings-page"
local switch = use "switch"

settings_page {
	nav = { groups = NAV_GROUPS, selected = state.page },
	title = "Sidebar", subtitle = "Changes apply to every open window immediately.",
	sections = {
		{ title = "Layout", rows = {
			{ title = "Show agents", description = "List running agents under their workspace.",
				control = switch { on = state.show_agents, msg = "toggle-agents" } },
		} },
	},
	status = state.dirty and "Unsaved: sidebar" or "All changes saved.",
	state = state.dirty and "unsaved" or "saved",
}]],
	example = {
		height = 520,
		nav = {
			selected = "sidebar",
			groups = {
				{
					title = "Appearance",
					items = {
						{ id = "theme", label = "Theme", icon = "palette" },
						{ id = "sidebar", label = "Sidebar", icon = "panel-left" },
						{ id = "fonts", label = "Fonts", icon = "a-large-small" },
					},
				},
				{
					title = "Behaviour",
					items = {
						{ id = "general", label = "General", icon = "settings" },
						{ id = "notifications", label = "Notifications", icon = "bell" },
					},
				},
				{
					title = "Connect",
					items = {
						{ id = "integrations", label = "Integrations", icon = "square-terminal" },
						{ id = "cloud", label = "Cloud devices", icon = "globe" },
					},
				},
			},
		},
		title = "Sidebar",
		subtitle = "Changes apply to every open window immediately.",
		sections = {
			{
				title = "Layout",
				rows = {
					{
						title = "Layout preset",
						tag = "Local",
						description = "Row shape and density; spacing below starts from the preset.",
						control = dropdown {
							value = "Comfortable Rounded",
							options = { { id = "comfortable", label = "Comfortable Rounded" } },
							toggle = "toggle-preset",
							align = "end",
						},
					},
					{
						title = "Device footer",
						description = "The device picker and its buttons.",
						control = segmented {
							selected = "always",
							small = true,
							options = { { id = "always", label = "Always" }, { id = "auto", label = "Auto" }, { id = "never", label = "Never" } },
						},
					},
					{
						title = "Show agents",
						description = "List running agents under their workspace.",
						control = switch { on = true, msg = "toggle-agents" },
					},
					{
						title = "Show device icons",
						tag = "Shared",
						tag_tone = "shared",
						description = "A mark before each device's name.",
						control = switch { on = false, msg = "toggle-device-icons" },
					},
				} :: { any },
			},
		},
	},

	build = function(props: Props)
		local page: { any } = {}
		if props.title or props.subtitle then
			page[#page + 1] = ui.column {
				class = "settings-page-head",
				shrink = false,
				props.title ~= nil and ui.text { props.title, class = "settings-page-title" },
				props.subtitle ~= nil and ui.text { props.subtitle, class = "settings-page-subtitle" },
			}
		end

		for _, section in ipairs(props.sections or {}) do
			local children: { any } = {}
			if section.title then
				children[#children + 1] = ui.text { section.title, class = "settings-page-heading" }
			end
			if section.description then
				children[#children + 1] = ui.text { section.description, class = "settings-page-section-description" }
			end
			for _, row in ipairs(section.rows or {}) do
				local controls: { any } = {}
				if type(row.control) == "table" and row.control.type == nil then
					controls = row.control
				elseif row.control ~= nil then
					controls = { row.control }
				end
				children[#children + 1] = settings_row {
					title = row.title,
					description = row.description,
					tag = row.tag,
					tag_tone = row.tag_tone,
					disabled = row.disabled,
					reason = row.reason,
					stacked = row.stacked,
					table.unpack(controls),
				}
			end
			for _, element in ipairs(section.content or {}) do
				children[#children + 1] = element
			end
			page[#page + 1] = ui.column { class = "settings-page-section", shrink = false, table.unpack(children) }
		end
		for _, child in positional(props) do
			page[#page + 1] = child
		end

		local state = props.state or "saved"
		local buttons: { any } = {}
		for _, button in ipairs(props.buttons or { { label = "Reload", msg = "reload", variant = "outline" } }) do
			buttons[#buttons + 1] = ui.button {
				button.label,
				small = true,
				variant = (button.variant or "outline") :: any,
				msg = button.msg,
				to = props.to,
			}
		end

		local footer = ui.row {
			class = "settings-page-footer",
			align = "center",
			ui.row {
				class = if state == "saved" then "settings-page-dot" else "settings-page-dot settings-page-dot-" .. state,
				width = 8,
				height = 8,
			},
			ui.text { props.status or "All changes saved.", class = "settings-page-status" },
			ui.spacer(),
			table.unpack(buttons),
		}

		local nav = {}
		for key, value in props.nav or {} do
			nav[key] = value
		end
		nav.scroll = true
		nav.to = nav.to or props.to

		return ui.column {
			class = "settings-page",
			height = props.height,
			grow = props.height == nil,
			ui.row {
				grow = true,
				gap = "none",
				align = "stretch",
				settings_nav(nav),
				ui.column {
					class = "settings-page-main",
					grow = true,
					ui.column { class = "settings-page-scroll", grow = true, scroll = true, table.unpack(page) },
				},
			},
			footer,
		}
	end,
}
