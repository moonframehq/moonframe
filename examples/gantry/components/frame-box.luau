-- Gantry's section box: a thin square border with brighter corner marks,
-- around any content. Gantry only.

type Props = {
	grow: boolean?,
	width: number?,
	flush: boolean?,
	class: string?,
}

local function corner(at: { top: number?, left: number?, right: number?, bottom: number? }, across: boolean)
	return ui.row {
		class = "frame-box-mark",
		width = if across then 7 else 1,
		height = if across then 1 else 7,
		float = at,
	}
end

return component {
	title = "Frame Box",
	category = "Containers",
	icon = "frame",
	description = "A square bordered box with brighter corner marks.",
	when = "Each section of Gantry's pages. Elsewhere use `card` or `dashboard-card`.",
	props = {
		["[children]"] = "the content",
		["grow"] = "fill the parent's free space",
		["width"] = "pixels",
		["flush"] = "no padding: the content touches the border (cells, tables)",
		["class"] = "another class for the box",
	},
	styles = {
		[".frame-box"] = { border = true, border_color = "border", padding = { 18, 24 }, gap = 16 },
		[".frame-box-flush"] = { padding = 0, gap = 0 },
		[".frame-box-mark"] = { background = "#5a5a5a" },
	},
	example = { width = 240 },

	build = function(props: Props)
		local children: { any } = {
			corner({ top = -1, left = -1 }, true),
			corner({ top = -1, left = -1 }, false),
			corner({ top = -1, right = -1 }, true),
			corner({ top = -1, right = -1 }, false),
			corner({ bottom = -1, left = -1 }, true),
			corner({ bottom = -1, left = -1 }, false),
			corner({ bottom = -1, right = -1 }, true),
			corner({ bottom = -1, right = -1 }, false),
		}
		for _, child in ipairs(props :: any) do
			table.insert(children, child)
		end
		local class = { "frame-box" }
		if props.flush then
			table.insert(class, "frame-box-flush")
		end
		if props.class then
			table.insert(class, props.class)
		end
		return ui.column {
			class = class,
			grow = props.grow,
			width = props.width,
			shrink = if props.width then false else nil,
			table.unpack(children),
		}
	end,
}
