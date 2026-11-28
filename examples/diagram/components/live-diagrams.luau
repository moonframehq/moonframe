-- The diagrams agents show (and the project's own), from `app.diagrams`:
-- a list to pick one from, and the picked one drawn read-only with the
-- agent's highlight and note.

type Props = {
	diagrams: { any },
	-- The `key` of the one shown, if any.
	shown: string?,
}

-- A row key for `key` (`agent/<file>`): each character a key may not
-- hold as its hex code, `-` too, so two names never meet. One longer than
-- a key may be (64) goes by its place instead: `long-<n>`, which no
-- encoded key (they start `agent` or `project`) can be.
local function row_key(key: string, place: number): string
	local encoded = string.gsub(key, "[^%w_]", function(c)
		return string.format("-%02x", string.byte(c))
	end)
	return if #encoded <= 64 then encoded else `long-{place}`
end

return component {
	title = "Live Diagrams",
	category = "Data",
	icon = "workflow",
	description = "The diagrams agents show through `moonframe mcp diagrams`, and the project's `.moonframe/diagrams`, as a list that picks one.",
	when = "Beside a diagram editor or in a side panel, for `app.diagrams`. Each row sends `{ live = key }`; draw the picked one with `ui.diagram { document = d.document, highlight = d.highlight }`.",
	props = {
		diagrams = "`app.diagrams.diagrams`",
		shown = "the `key` of the diagram on show",
	},
	styles = {
		[".live-diagram"] = { padding = { 4, 8 }, gap = 8, radius = 6, margin = { 0, 2 } },
		[".live-diagram:hover"] = { background = "list-hover" },
		[".live-diagram:selected"] = { background = "accent/15", color = "accent" },
		[".live-diagram-detail"] = { size = "small", color = "muted-foreground" },
	},
	example = { diagrams = {} },

	build = function(props: Props)
		local rows: { any } = {}
		for i, d in props.diagrams do
			local detail = if d.error then "doesn't parse" elseif d.agent then d.agent else d.from
			table.insert(
				rows,
				ui.row {
					class = "live-diagram",
					key = row_key(d.key, i),
					selected = d.key == props.shown,
					align = "center",
					msg = { live = d.key },
					tooltip = d.note or d.error,
					ui.icon { name = (if d.from == "agent" then "sparkles" else "file-text") :: any, size = "small", tone = "muted" },
					ui.row { grow = true, ui.text { d.title, truncate = true } },
					ui.text { detail, class = "live-diagram-detail" },
				}
			)
		end
		return ui.column { gap = "none", table.unpack(rows) }
	end,
}
