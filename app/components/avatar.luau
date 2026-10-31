-- A rounded square (or circle) holding a name's initial, on one of six
-- colours picked from the name, so the same name always gets the same
-- colour. An optional status dot sits on its corner.

type Props = {
	name: string?,
	initial: string?,
	size: string?,
	shape: string?,
	color: string?,
	status: string?,
	src: string?,
	tooltip: string?,
}

-- A six-colour space palette: teal, orange, blue, violet, green, pink.
local PALETTE = { "#22a99a", "#e3814f", "#5b8def", "#8b7cf6", "#3fae6a", "#d9649a" }

-- Side length in pixels and text size, by size name.
local SIZES: { [string]: { side: number, text: string } } = {
	small = { side = 18, text = "small" },
	normal = { side = 24, text = "small" },
	large = { side = 32, text = "normal" },
	xlarge = { side = 44, text = "large" },
}

-- A small, stable string hash (djb2), so a name keeps its colour.
local function hash(text: string): number
	local value = 5381
	for index = 1, #text do
		value = (value * 33 + string.byte(text, index)) % 2147483647
	end
	return value
end

local function initial(name: string): string
	local letter = string.match(name, "%w") or "?"
	return string.upper(letter)
end

return component {
	title = "Avatar",
	category = "Data",
	icon = "circle-user",
	description = "A name's initial on a colour picked from the name (six-colour palette), square or round, with an optional status dot on its corner.",
	when = "Stand for a person, space, project or agent by its name: comment authors, workspace switchers, people lists. Use `comment-card` for an authored message (it draws one), `status-dot` for a state alone.",
	props = {
		["name"] = "the name the initial and colour come from",
		["initial"] = "letters to show instead of the name's first letter",
		["size"] = "small | normal | large | xlarge (18, 24, 32, 44 px)",
		["shape"] = "square (rounded, default) | circle",
		["color"] = "a colour instead of the one picked from the name",
		["status"] = "working | blocked | done | idle | unknown: a status dot on the corner",
		["src"] = "an image file (relative to the mod's folder, `~/...` or absolute; not a URL: a provider downloads it to ~/.cache/moonframe/images) shown instead of the initial; a missing file leaves the coloured tile",
		["tooltip"] = "text under the pointer (default the name)",
	},
	styles = {
		[".avatar"] = { radius = 6, color = "#ffffff", weight = "bold" },
		[".avatar-circle"] = { radius = 999 },
		[".avatar-image"] = { radius = 6 },
		[".avatar-image-circle"] = { radius = 999 },
		[".avatar-status"] = { radius = 999, border = "all", border_color = "background" },
		[".avatar-status-working"] = { background = "warning" },
		[".avatar-status-blocked"] = { background = "danger" },
		[".avatar-status-done"] = { background = "success" },
		[".avatar-status-idle"] = { background = "background", border_color = "success" },
		[".avatar-status-unknown"] = { background = "muted-foreground" },
	},
	example = { name = "harbor-docs", size = "xlarge", status = "working" },

	build = function(props: Props)
		local name = props.name or "?"
		local spec = SIZES[props.size or "normal"] or SIZES.normal
		local side = spec.side
		local color = props.color or PALETTE[hash(name) % #PALETTE + 1]
		local classes = { "avatar" }
		if props.shape == "circle" then
			table.insert(classes, "avatar-circle")
		end
		-- The picture, when given, covers the tile; while it is missing the
		-- tile's colour shows through.
		local face = if props.src
			then ui.image {
				src = props.src,
				width = side,
				height = side,
				fit = "cover",
				class = if props.shape == "circle" then "avatar-image avatar-image-circle" else "avatar-image",
			}
			else ui.text { props.initial or initial(name), size = spec.text :: any }
		-- The status dot floats over the bottom-right corner, ringed in the
		-- background colour so it reads on any tile.
		local dot = math.max(7, math.floor(side / 3.2))
		return ui.row {
			class = classes,
			hug = true,
			shrink = false,
			width = side,
			height = side,
			align = "center",
			justify = "center",
			tooltip = props.tooltip or name,
			style = { background = color },
			face,
			props.status and ui.row {
				class = { "avatar-status", `avatar-status-{props.status}` },
				width = dot,
				height = dot,
				float = { right = -2, bottom = -2 },
			},
		}
	end,
}
