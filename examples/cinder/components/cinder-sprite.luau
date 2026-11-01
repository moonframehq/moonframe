-- A pixel drawing from a grid of characters, one per pixel, each mapped to a
-- colour: Cinder's mascot by default. Runs of one colour in a row are drawn
-- as one box, so a 12 by 10 sprite is a few dozen elements.

type Sprite = { rows: { string }, colors: { [string]: string } }

-- Cinder, Cinder's mascot: a little cinder with a flame on its head, two
-- eyes, stubby arms and two feet.
local CINDER: Sprite = {
	rows = {
		"......f.....",
		".....ff.....",
		"....oooo....",
		"..oooooooo..",
		".ookooookoo.",
		"oooooooooooo",
		"oooooooooooo",
		".oooooooooo.",
		"..ooo..ooo..",
		"..oo....oo..",
	},
	colors = { o = "em-cinder", f = "#f2a65a", k = "#141414" },
}

return component {
	title = "Cinder Sprite",
	category = "Data",
	icon = "sparkles",
	description = "A small pixel drawing from rows of characters, one colour per character; Cinder's mascot by default.",
	when = "A mascot or a tiny pixel-art mark drawn from a grid (an empty state, a header). For a bundled glyph use `ui.icon`; for a picture file use `ui.image`.",
	props = {
		rows = "list of strings, one per row; each character is a pixel, `.` or a space is empty",
		colors = "map from character to colour (a theme colour, a palette name or #hex)",
		pixel = "a pixel's side in pixels (default 3)",
		tooltip = "text under the pointer",
	},
	example = { pixel = 6 },
	build = function(props)
		local rows: { string } = props.rows or CINDER.rows
		local colors: { [string]: string } = props.colors or CINDER.colors
		local size: number = props.pixel or 3
		local lines = {}
		for _, line in rows do
			-- One box per run of the same character.
			local runs = {}
			local i = 1
			while i <= #line do
				local char = string.sub(line, i, i)
				local j = i
				while j < #line and string.sub(line, j + 1, j + 1) == char do
					j += 1
				end
				local color = colors[char]
				local count = j - i + 1
				if color and char ~= "." and char ~= " " then
					table.insert(runs, ui.row { width = count * size, height = size, shrink = false, style = { background = color } })
				else
					table.insert(runs, ui.row { width = count * size, height = size, shrink = false })
				end
				i = j + 1
			end
			table.insert(lines, ui.row { gap = "none", shrink = false, table.unpack(runs) })
		end
		return ui.column { class = "cinder-sprite", gap = "none", hug = true, tooltip = props.tooltip, table.unpack(lines) }
	end,
}
