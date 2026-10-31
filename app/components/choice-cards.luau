-- A grid of selectable cards, each with an optional preview on top, an
-- icon, a title and a description. The chosen card gets an accent border
-- and a check. For theme, layout and agent pickers.

type Card = {
	id: string?,
	title: string,
	description: string?,
	icon: string?,
	preview: any,
	colors: { string }?,
	disabled: boolean?,
	msg: any,
}

type Props = {
	options: { Card },
	selected: string?,
	width: number?,
	event: string?,
	to: string?,
}

return component {
	title = "Choice Cards",
	category = "Forms",
	icon = "layout-dashboard",
	description = "A wrapping grid of selectable cards (preview, icon, title, description), the chosen one with an accent border and check; sends { select = id }.",
	when = "Pick one of several options that are best seen: a theme, a layout, a template, an agent. For short text options use `segmented` or `radio-group`; for a colour use `swatches`; for a menu use `dropdown`.",
	props = {
		["options"] = "list of { id, title, description?, icon?, preview? (an element drawn on top), colors? (hex list drawn as a strip when there is no preview), disabled?, msg? }",
		["selected"] = "id of the chosen card",
		["width"] = "card width in pixels (default 180); the grid wraps",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".choice-cards"] = { gap = 10 },
		[".choice-cards-card"] = {
			gap = 6,
			padding = 10,
			radius = 10,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".choice-cards-card:hover"] = { border_color = "muted-foreground/50", background = "list-hover" },
		[".choice-cards-card:selected"] = { border_color = "accent", background = "accent/8" },
		[".choice-cards-disabled"] = { opacity = 0.5 },
		[".choice-cards-preview"] = { radius = 6, padding = { 5, 6 }, gap = 3, background = "muted", border = "all", border_color = "border" },
		[".choice-cards-color"] = { radius = 2 },
		[".choice-cards-icon"] = { color = "muted-foreground" },
		[".choice-cards-title"] = { weight = "medium" },
		[".choice-cards-check"] = { color = "accent" },
		[".choice-cards-description"] = { color = "muted-foreground", size = "small" },
	},
	example = {
		selected = "default",
		width = 168,
		options = {
			{
				id = "darkside",
				title = "Darkside",
				description = "Ghostty",
				colors = { "#222324", "#e8341c", "#68c256", "#f2d42c", "#1c98e8", "#8e69c9", "#bababa" },
			},
			{
				id = "default",
				title = "Default",
				description = "Built-in / Saved",
				colors = { "#000000", "#990000", "#00a600", "#999900", "#0000b2", "#b200b2", "#bfbfbf" },
			},
			{
				id = "dawnfox",
				title = "Dawnfox",
				description = "Ghostty",
				colors = { "#575279", "#b4637a", "#618774", "#ea9d34", "#286983", "#907aa9", "#e5e9f0" },
			},
		},
	},
	usage = [[local choice_cards = use "choice-cards"

choice_cards {
	selected = state.layout,
	options = {
		{ id = "rounded", title = "Comfortable", icon = "layout-dashboard", description = "Roomy rounded rows" },
		{ id = "compact", title = "Compact", icon = "menu", description = "Dense rows, more fits" },
		-- preview = any element, drawn on top of the card:
		{ id = "dark", title = "Darkside", description = "Ghostty", colors = { "#222324", "#e8341c", "#68c256" } },
	},
}]],

	build = function(props: Props)
		local event = props.event or "select"
		local width = props.width or 180
		local cells = {}
		for _, card in props.options do
			local id = card.id or card.title
			local chosen = id == props.selected
			local preview = card.preview
			if preview == nil and card.colors then
				local strip = {}
				for index, color in card.colors do
					strip[index] = ui.row {
						class = "choice-cards-color",
						grow = true,
						height = 14,
						style = { background = color },
					}
				end
				preview = ui.row {
					class = "choice-cards-preview",
					align = "center",
					ui.text { "Aa", size = "small", tone = "muted" },
					table.unpack(strip),
				}
			end
			local class = { "choice-cards-card" }
			if card.disabled then
				table.insert(class, "choice-cards-disabled")
			end
			cells[#cells + 1] = ui.column {
				class = class,
				width = width,
				selected = chosen,
				msg = if card.disabled then nil else (card.msg or { [event] = id }),
				to = props.to,
				preview or false,
				ui.row {
					align = "center",
					gap = "small",
					card.icon and ui.icon { name = card.icon :: any, class = "choice-cards-icon" },
					ui.text { card.title, class = "choice-cards-title" },
					ui.spacer(),
					chosen and ui.icon { name = "circle-check", class = "choice-cards-check" },
				},
				card.description and ui.text { card.description, class = "choice-cards-description", truncate = true },
			}
		end
		return ui.row { class = "choice-cards", wrap = true, table.unpack(cells) }
	end,
}
