-- Filter chips: small pills that toggle on and off ("Open", "Has agent"),
-- each with an optional icon and count. Several can be on at once.

type Chip = {
	id: string?,
	label: string,
	icon: string?,
	count: any,
	on: boolean?,
	msg: any,
}

type Props = {
	chips: { Chip },
	selected: { string }?,
	event: string?,
	wrap: boolean?,
	to: string?,
}

-- The preview's chips, typed loosely: they mix kinds of entries.
local EXAMPLE: { any } = {
	{ id = "open", label = "Open", count = 12 },
	{ id = "mine", label = "Mine", icon = "user", count = 3 },
	{ id = "agent", label = "Has agent", icon = "agent-claude", count = 5 },
	{ id = "draft", label = "Draft", icon = "git-pull-request-draft" },
	{ id = "failing", label = "CI failing", icon = "circle-x", count = 1 },
}

return component {
	title = "Chips",
	category = "Controls",
	icon = "filter",
	description = "Filter pills that toggle on and off, with an optional icon and count; a click sends { toggle = id }.",
	when = "Filters over a list where any number can be on (Open, Mine, Has agent). For exactly one of a few use `segmented`; for read-only labels use `tags`; for a single on/off setting use `switch`.",
	props = {
		["chips"] = "list of { id, label, icon?, count?, on? (or list ids in `selected`), msg? (sent instead of the default) }",
		["selected"] = "ids of the chips that are on (instead of each chip's `on`)",
		["event"] = "key of the message a click sends, default \"toggle\": { toggle = id }",
		["wrap"] = "wrap onto more lines when narrow (default true)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".chips"] = { gap = 6 },
		[".chips-chip"] = {
			padding = { 3, 10 },
			gap = 5,
			radius = 999,
			border = "all",
			border_color = "border",
			color = "muted-foreground",
			size = "small",
		},
		[".chips-chip:hover"] = { background = "list-hover", color = "foreground" },
		[".chips-chip:selected"] = { background = "accent/15", border_color = "accent/50", color = "accent" },
		[".chips-count"] = { padding = { 0, 5 }, radius = 999, background = "muted-foreground/15" },
		[".chips-count:selected"] = { background = "accent/20" },
	},
	example = {
		selected = { "open", "agent" },
		chips = EXAMPLE,
	},

	build = function(props: Props)
		local event = props.event or "toggle"
		local on = {}
		for _, id in ipairs(props.selected or {}) do
			on[id] = true
		end
		local cells = {}
		for _, chip in props.chips do
			local id = chip.id or chip.label
			local active = chip.on == true or on[id] == true
			cells[#cells + 1] = ui.row {
				class = "chips-chip",
				align = "center",
				selected = active,
				msg = chip.msg or { [event] = id },
				to = props.to,
				chip.icon and ui.icon { name = chip.icon :: any },
				ui.text { chip.label },
				chip.count ~= nil and ui.row {
					class = "chips-count",
					selected = active,
					ui.text { tostring(chip.count) },
				},
			}
		end
		return ui.row { class = "chips", align = "center", wrap = props.wrap ~= false, table.unpack(cells) }
	end,
}
