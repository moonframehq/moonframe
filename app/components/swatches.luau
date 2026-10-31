-- A row of colour swatches to pick from, the chosen one ringed, with an
-- optional "none" swatch first. Each swatch's colour is its own inline
-- background; everything else is styled by class.

type Swatch = string | { color: string, label: string?, msg: any }

type Props = {
	colors: { Swatch },
	selected: string?,
	none: boolean?,
	event: string?,
	size: number?,
	to: string?,
}

return component {
	title = "Swatches",
	category = "Forms",
	icon = "palette",
	description = "Colour swatches in a row with a ring on the chosen one and an optional \"none\"; a click sends { pick = color }.",
	when = "Pick a colour from a short palette: a workspace or tag colour, an accent. For themes (many colours each) use `choice-cards` with a swatch row as the preview; to show colours without picking, use `ui.row`s with a background.",
	props = {
		["colors"] = "list of hex strings (\"#ef4444\") or { color, label?, msg? }",
		["selected"] = "the chosen colour, or \"none\"",
		["none"] = "put a \"none\" swatch first; it sends { pick = \"none\" }",
		["event"] = "key of the message a click sends, default \"pick\": { pick = color }",
		["size"] = "swatch size in pixels (default 20)",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".swatches"] = { gap = 4 },
		[".swatches-ring"] = { padding = 2, radius = 999, border = "all", border_color = "#00000000" },
		[".swatches-ring:hover"] = { border_color = "muted-foreground/40" },
		[".swatches-ring:selected"] = { border_color = "foreground" },
		[".swatches-swatch"] = { radius = 999, border = "all", border_color = "foreground/15" },
		[".swatches-none"] = { background = "background", color = "muted-foreground", size = "small" },
	},
	example = {
		none = true,
		selected = "#8b5cf6",
		colors = { "#ef4444", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#3b82f6", "#8b5cf6", "#ec4899", "#64748b" },
	},

	build = function(props: Props)
		local event = props.event or "pick"
		local size = props.size or 20
		local cells = {}
		local function ring(id: string, label: string?, msg: any, inner: any)
			cells[#cells + 1] = ui.row {
				class = "swatches-ring",
				selected = props.selected == id,
				tooltip = label,
				msg = msg or { [event] = id },
				to = props.to,
				inner,
			}
		end
		if props.none then
			ring("none", "None", nil, ui.row {
				class = { "swatches-swatch", "swatches-none" },
				width = size,
				height = size,
				align = "center",
				justify = "center",
				ui.icon "ban",
			})
		end
		for _, entry in props.colors do
			local color: string, label: string?, msg: any
			if type(entry) == "table" then
				color, label, msg = entry.color, entry.label, entry.msg
			else
				color = entry :: string
			end
			ring(color, label or color, msg, ui.row {
				class = "swatches-swatch",
				width = size,
				height = size,
				style = { background = color },
			})
		end
		return ui.row { class = "swatches", align = "center", wrap = true, table.unpack(cells) }
	end,
}
