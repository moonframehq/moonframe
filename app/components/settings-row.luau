-- One setting: a title (with an optional source tag such as "Local") and a
-- muted description on the left, its control on the right (the children),
-- and a line under it. `disabled` greys it and says why.

local dropdown = use "dropdown"

type Props = {
	title: string,
	description: string?,
	tag: string?,
	-- `local` (default, warning-coloured) or `shared` (muted) for the tag.
	tag_tone: string?,
	disabled: boolean?,
	reason: string?,
	-- Draw the control under the text instead of beside it (wide controls).
	stacked: boolean?,
	border: boolean?,
	[number]: any,
}

return component {
	title = "Settings row",
	category = "Forms",
	icon = "settings-2",
	description = "A setting: title, source tag and muted description on the left, its control on the right, a line under it.",
	when = "Each line of a settings page or preferences card, holding a `switch`, `segmented`, `dropdown` or button. Group rows under `settings-page` sections; for read-only properties use `key-values`; for a labelled switch on its own use `switch` with `label`.",
	props = {
		["title"] = "the setting's name",
		["description"] = "muted line under the title",
		["tag"] = "a small chip after the title: where the value comes from (\"Local\", \"Shared\")",
		["tag_tone"] = "`local` (default, warning colour) or `shared` (muted)",
		["disabled"] = "grey the row out",
		["reason"] = "why it's disabled, shown under the description",
		["stacked"] = "put the control under the text (for wide controls such as a list of choices)",
		["border"] = "the line under the row; default true",
		["[children]"] = "the control(s)",
	},
	styles = {
		[".settings-row"] = { padding = { 10, 0 }, gap = 16, border = "bottom", border_color = "border" },
		[".settings-row-flat"] = { border = "none" },
		[".settings-row-disabled"] = { opacity = 0.55 },
		[".settings-row-text"] = { gap = 2 },
		[".settings-row-head"] = { gap = 8 },
		[".settings-row-title"] = {},
		[".settings-row-description"] = { size = "small", color = "muted-foreground" },
		[".settings-row-reason"] = { size = "small", color = "warning" },
		[".settings-row-tag"] = {
			size = "small",
			color = "warning",
			border = "all",
			border_color = "warning/40",
			radius = 5,
			padding = { 0, 6 },
		},
		[".settings-row-tag-shared"] = { color = "muted-foreground", border_color = "border" },
		[".settings-row-control"] = { gap = 8 },
	},
	usage = [[local settings_row = use "settings-row"
local switch = use "switch"

settings_row { title = "Show agents", description = "List running agents under their workspace.",
	switch { on = state.show_agents, msg = "toggle-agents" },
}]],
	example = {
		title = "Layout preset",
		tag = "Local",
		description = "Row shape and density; spacing below starts from the preset.",
		dropdown {
			value = "Comfortable Rounded",
			selected = "comfortable",
			options = { { id = "comfortable", label = "Comfortable Rounded" }, { id = "compact", label = "Compact" } },
			toggle = "toggle-preset",
			dismiss = "close-preset",
			align = "end",
		},
	},

	build = function(props: Props)
		local classes = { "settings-row" }
		if props.border == false then
			classes[#classes + 1] = "settings-row-flat"
		end
		if props.disabled then
			classes[#classes + 1] = "settings-row-disabled"
		end

		local tag: any = false
		if props.tag then
			tag = ui.row {
				class = if props.tag_tone == "shared" then "settings-row-tag settings-row-tag-shared" else "settings-row-tag",
				ui.text { props.tag },
			}
		end

		local text = ui.column {
			class = "settings-row-text",
			grow = true,
			ui.row {
				class = "settings-row-head",
				align = "center",
				ui.text { props.title, class = "settings-row-title" },
				tag,
			},
			props.description ~= nil and ui.text { props.description, class = "settings-row-description" },
			props.disabled == true and props.reason ~= nil and ui.text { props.reason, class = "settings-row-reason" },
		}

		local control: any = false
		if #props > 0 then
			control = ui.row { class = "settings-row-control", shrink = false, align = "center", table.unpack(props) }
		end

		if props.stacked then
			classes[#classes + 1] = "settings-row-stacked"
			return ui.column { class = classes, shrink = false, text, control }
		end
		return ui.row { class = classes, shrink = false, align = "center", text, control }
	end,
}
