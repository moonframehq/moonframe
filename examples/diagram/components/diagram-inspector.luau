-- The inspector: what the selection is and how to change it exactly. One
-- shape gives its label, position, size, kind, fill and connections; one
-- link its label, line and ends; several shapes their fill. Every change is
-- sent to the drawing mod as { set = field, value = ... }; text fields send
-- it on Enter and when they lose focus.

type Shape = { id: string, kind: string, x: number, y: number, w: number, h: number, label: string?, fill: string? }
type Link = { id: string, from: string, to: string, label: string?, dashed: boolean? }
type Props = {
	doc: { shapes: { Shape }, links: { Link } },
	selected: { string },
	fills: { string },
}

local button_group = use "button-group"
local form_grid = use "form-grid"
local swatches = use "swatches"

local KINDS = {
	{ id = "box", icon = "square", tooltip = "Box" },
	{ id = "pill", icon = "diamond", tooltip = "Decision" },
	{ id = "ellipse", icon = "circle", tooltip = "Ellipse" },
	{ id = "text", icon = "type", tooltip = "Text" },
	{ id = "sticky", icon = "sticky-note", tooltip = "Sticky note" },
}

local function heading(text: string): any
	return ui.text { text, class = "inspector-heading" }
end

local function field(name: string, value: any, width: number?): any
	return ui.input {
		id = `inspector-{name}`,
		value = tostring(value or ""),
		width = width or 64,
		mono = type(value) == "number",
		submit = { set = name },
		blur = { set = name },
	}
end

local function fill_picker(fills: { string }, selected: string?): any
	local colors = {}
	for _, color in fills do
		table.insert(colors, { color = color, msg = { set = "fill", value = color } })
	end
	return swatches { colors = colors, selected = selected, size = 18 }
end

local function name_of(shapes: { [string]: Shape }, id: string): string
	local s = shapes[id]
	return if s and s.label and s.label ~= "" then s.label else id
end

local function shape_panel(props: Props, s: Shape, by_id: { [string]: Shape }): { any }
	local connections = {}
	for _, l in props.doc.links do
		if l.from == s.id or l.to == s.id then
			table.insert(
				connections,
				ui.item {
					`{name_of(by_id, l.from)} → {name_of(by_id, l.to)}`,
					detail = l.label or (if l.dashed then "dashed" else nil),
					msg = { pick = l.id },
				}
			)
		end
	end
	return {
		heading(string.upper(s.kind)),
		form_grid {
			label_width = 64,
			rows = {
				{ label = "Label", control = field("label", s.label or "", 140) },
				{ label = "Position", control = { field("x", math.floor(s.x + 0.5)), field("y", math.floor(s.y + 0.5)) } },
				{ label = "Size", control = { field("w", math.floor(s.w + 0.5)), field("h", math.floor(s.h + 0.5)) } },
			},
		},
		ui.divider(),
		heading "KIND",
		button_group {
			small = true,
			selected = s.kind,
			buttons = (function()
				local out = {}
				for _, k in KINDS do
					table.insert(out, { id = k.id, icon = k.icon, tooltip = k.tooltip, msg = { set = "kind", value = k.id } })
				end
				return out
			end)(),
		},
		s.kind ~= "text" and ui.divider(),
		s.kind ~= "text" and heading "FILL",
		s.kind ~= "text" and fill_picker(props.fills, s.fill),
		ui.divider(),
		heading(`CONNECTIONS · {#connections}`),
		#connections == 0 and ui.text { "Drag from a blue dot to connect.", class = "inspector-hint" },
		table.unpack(connections),
	}
end

local function link_panel(l: Link, by_id: { [string]: Shape }): { any }
	return {
		heading "CONNECTOR",
		ui.text { `{name_of(by_id, l.from)} → {name_of(by_id, l.to)}`, weight = "medium" },
		form_grid {
			label_width = 64,
			rows = { { label = "Label", control = field("label", l.label or "", 140) } },
		},
		heading "LINE",
		button_group {
			small = true,
			selected = if l.dashed then "dashed" else "solid",
			buttons = {
				{ id = "solid", label = "Solid", msg = { set = "dashed", value = false } },
				{ id = "dashed", label = "Dashed", msg = { set = "dashed", value = true } },
			},
		},
		ui.divider(),
		ui.button { "Delete connector", icon = "trash-2", variant = "ghost", small = true, msg = "delete" },
	}
end

return component {
	title = "Diagram Inspector",
	category = "Forms",
	icon = "sliders-horizontal",
	description = "The diagram editor's right-hand panel: the selection's label, position, size, kind, fill and connections.",
	when = "Beside a `diagram` canvas, to set exactly what dragging sets roughly.",
	props = {
		doc = "the document: { shapes, links }",
		selected = "ids of the chosen shapes and links",
		fills = "colors offered for a shape's fill",
	},
	styles = {
		[".inspector"] = { background = "sidebar", padding = { 12, 12 }, gap = 8, border = "left", border_color = "border" },
		[".inspector-heading"] = { size = "small", weight = "semibold", color = "muted-foreground" },
		[".inspector-hint"] = { size = "small", color = "muted-foreground" },
	},
	example = { doc = { shapes = {}, links = {} }, selected = {}, fills = { "#e2e8f0", "#bfdbfe" } },

	build = function(props: Props)
		local by_id: { [string]: Shape } = {}
		for _, s in props.doc.shapes do
			by_id[s.id] = s
		end
		local links: { [string]: Link } = {}
		for _, l in props.doc.links do
			links[l.id] = l
		end
		local body: { any }
		local selected = props.selected
		if #selected == 1 and by_id[selected[1]] then
			body = shape_panel(props, by_id[selected[1]], by_id)
		elseif #selected == 1 and links[selected[1]] then
			body = link_panel(links[selected[1]], by_id)
		elseif #selected > 1 then
			local fill = nil
			for _, id in selected do
				local s = by_id[id]
				if s then
					fill = if fill == nil or fill == s.fill then s.fill else ""
				end
			end
			body = {
				heading(`{#selected} SELECTED`),
				heading "FILL",
				fill_picker(props.fills, fill),
				ui.divider(),
				ui.button { "Delete", icon = "trash-2", variant = "ghost", small = true, msg = "delete" },
			}
		else
			body = {
				heading "DOCUMENT",
				ui.text { `{#props.doc.shapes} shapes, {#props.doc.links} connectors`, size = "small" },
				ui.divider(),
				ui.text {
					"Pick a tool above (or press its key), then click or drag on the canvas. Drag from a selected shape's blue dot to connect it; double-click a shape to rename it. Scroll to pan, pinch or ⌘-scroll to zoom.",
					class = "inspector-hint",
					wrap = true,
				},
			}
		end
		return ui.column { class = "inspector", width = 260, shrink = false, table.unpack(body) }
	end,
}
