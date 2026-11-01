-- A small pixel-art sprite on a coloured tile: a grid of digits drawn as
-- square cells, each digit a colour from a short palette, mirrored left to
-- right by default so half a pattern makes a symmetric face. Runs of the
-- same colour in a row are drawn as one cell, so a sprite costs a few dozen
-- boxes at most.

type Props = {
	pattern: { string },
	colors: { string },
	size: number?,
	mirror: boolean?,
	tooltip: string?,
}

local function cells(pattern: { string }, mirror: boolean): { string }
	local rows = {}
	for _, line in pattern do
		table.insert(rows, if mirror then line .. string.reverse(line) else line)
	end
	return rows
end

return component {
	title = "Pixel Icon",
	category = "Data",
	icon = "layers",
	description = "A pixel-art sprite on a rounded tile, from a grid of digits and a palette; mirrored to make a symmetric face.",
	when = "A playful, recognisable mark for an agent, a bot, a project or a workspace, at any size from a list row (18 px) to a header (64 px). For a person's initial use `avatar`; for a generic glyph use `ui.icon`.",
	props = {
		["pattern"] = "rows of digits, top first: 0 is the tile, 1 to 9 are `colors[2]` to `colors[10]` (with `mirror`, only the left half)",
		["colors"] = "the tile's colour first, then one colour per digit",
		["size"] = "the tile's side in pixels (default 40)",
		["mirror"] = "repeat each row reversed, so the pattern is the left half (default true)",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".pixel-icon"] = { radius = 6 },
	},
	example = {
		pattern = { "0011", "0122", "0011", "0002", "0011", "0002", "0022", "0111" },
		colors = { "#6fd3e6", "#f4f4f7", "#c9a7e8" },
		size = 48,
	},

	build = function(props: Props)
		local size = props.size or 40
		local rows = cells(props.pattern, props.mirror ~= false)
		local count = #rows
		local width = 0
		for _, line in rows do
			width = math.max(width, #line)
		end
		-- The sprite fills about three quarters of the tile, on whole pixels.
		local cell = math.max(1, math.floor(size * 0.78 / math.max(count, width)))
		local lines = {}
		for _, line in rows do
			local runs = {}
			local i = 1
			while i <= #line do
				local digit = string.sub(line, i, i)
				local j = i
				while j < #line and string.sub(line, j + 1, j + 1) == digit do
					j += 1
				end
				local color = props.colors[(tonumber(digit) or 0) + 1]
				local span = (j - i + 1) * cell
				table.insert(runs, ui.row {
					width = span,
					height = cell,
					shrink = false,
					style = if digit ~= "0" and color then { background = color } else nil,
				})
				i = j + 1
			end
			table.insert(lines, ui.row { gap = "none", height = cell, shrink = false, table.unpack(runs) })
		end
		return ui.column {
			class = "pixel-icon",
			hug = true,
			shrink = false,
			width = size,
			height = size,
			align = "center",
			justify = "center",
			gap = "none",
			tooltip = props.tooltip,
			style = { background = props.colors[1], radius = math.max(3, math.floor(size / 6)) },
			table.unpack(lines),
		}
	end,
}
