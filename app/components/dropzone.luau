-- Where files are dropped: a dashed rounded box with an icon, "Drop files
-- here or browse" (browse a link the mod answers by opening the system's
-- file panel) and a muted line on what it takes. Files dropped on it from
-- other apps send `drop` with their `paths`. `compact` keeps it to one
-- line, for inside a panel.

type Props = {
	title: string?,
	detail: string?,
	icon: string?,
	compact: boolean?,
	drop: any,
	browse: any,
	to: string?,
	width: number?,
	height: number?,
}

-- The corners' radius, and how many dashes a side holds at most (the rest
-- are clipped): enough for 1,400 pixels across and 400 down.
local RADIUS = 10
local DASH = 5
local SPACE = 4
local ACROSS = 160
local DOWN = 45

local function dashes(count: number, across: boolean)
	local list = table.create(count)
	for index = 1, count do
		list[index] = ui.row {
			class = "dropzone-dash",
			width = if across then DASH else 1,
			height = if across then 1 else DASH,
			shrink = false,
		}
	end
	return list
end

-- One side's dashes, floated along it between the corners and clipped to
-- its length.
local function side(edge: string)
	local across = edge == "top" or edge == "bottom"
	local dash_list = ui.memo(`dropzone-{across}`, {}, function()
		return dashes(if across then ACROSS else DOWN, across)
	end)
	local props: { [any]: any } = { style = { clip = true, gap = SPACE } }
	if across then
		props.float = { [edge] = 0, left = RADIUS, right = RADIUS }
		props.height = 1
	else
		props.float = { [edge] = 0, top = RADIUS, bottom = RADIUS }
		props.width = 1
	end
	for index, dash in dash_list do
		props[index] = dash
	end
	return if across then ui.row(props) else ui.column(props)
end

-- A quarter of a rounded outline in one corner, `top`/`left` or the others.
local function corner(top: boolean, left: boolean)
	return ui.row {
		float = { top = if top then 0 else nil, bottom = if top then nil else 0, left = if left then 0 else nil, right = if left then nil else 0 },
		width = RADIUS,
		height = RADIUS,
		style = { clip = true },
		ui.row {
			class = "dropzone-corner",
			float = { top = if top then 0 else -RADIUS, left = if left then 0 else -RADIUS },
			width = 2 * RADIUS,
			height = 2 * RADIUS,
		},
	}
end

-- "Drop files here or browse", browse a link when the mod takes it.
local function prompt(props: Props, class: string)
	return ui.row {
		align = "center",
		gap = "none",
		wrap = true,
		ui.text { (props.title or "Drop files here") .. (if props.browse ~= nil then " or\u{a0}" else ""), class = class },
		props.browse ~= nil and ui.row {
			class = "dropzone-browse",
			msg = props.browse,
			to = props.to,
			ui.text { "browse", class = class },
		},
	}
end

return component {
	title = "Dropzone",
	category = "Forms",
	icon = "upload",
	description = "A dashed rounded box files are dropped on, with an icon, \"Drop files here or browse\" (browse a link) and a muted line; one line when compact.",
	when = "Adding attachments to a task, a message or a form: files dragged from other apps land on it, and browse opens the system's file panel (`{ to = \"app:pick\", ... }`). Show what was added under it with `file-card`. For a button alone use `ui.button`; for an empty list with one action use `empty-state`.",
	props = {
		["title"] = "the text before the link (default \"Drop files here\")",
		["detail"] = "a muted line on what it takes, e.g. \"Up to 250 MB each\"",
		["icon"] = "icon over the text (default \"upload\")",
		["compact"] = "one line, for inside a panel or under a list",
		["drop"] = "message sent when files are dropped on it, with `paths` added",
		["browse"] = "message the browse link sends (open `app:pick` from it); no link without it",
		["to"] = "mod to send the messages to, if not the drawing one",
		["width"] = "pixels (default: fill its parent)",
		["height"] = "pixels (default its content's)",
	},
	styles = {
		[".dropzone"] = { gap = 0, radius = RADIUS, background = "muted-foreground/4" },
		[".dropzone-dash"] = { background = "muted-foreground/45" },
		[".dropzone-corner"] = { border = "all", border_color = "muted-foreground/45", radius = RADIUS },
		[".dropzone-body"] = { padding = { 22, 20, 22, 20 }, gap = 8 },
		[".dropzone-body-compact"] = { padding = { 10, 14 }, gap = 8 },
		[".dropzone-icon"] = { radius = 999, background = "accent/12", color = "accent" },
		[".dropzone-title"] = { weight = "medium" },
		[".dropzone-title-compact"] = { size = "small", weight = "medium" },
		[".dropzone-browse"] = { color = "accent" },
		[".dropzone-browse:hover"] = { color = "accent/75" },
		[".dropzone-detail"] = { size = "small", color = "muted-foreground" },
	},
	example = { detail = "Up to 250 MB each", browse = "browse", drop = "drop", width = 420 },
	usage = [[local dropzone = use "dropzone"

dropzone { detail = "Up to 250 MB each", drop = "dropped", browse = "browse" }

-- in update:
if msg == "browse" then
	return state, { { to = "app:pick", msg = { kind = "open", multiple = true, reply = "dropped" } } }
elseif type(msg) == "table" and msg.paths then
	-- add msg.paths
end]],

	build = function(props: Props)
		local body
		if props.compact then
			body = ui.row {
				class = { "dropzone-body", "dropzone-body-compact" },
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = (props.icon or "upload") :: any, size = "small", class = "dropzone-detail" },
				prompt(props, "dropzone-title-compact"),
				props.detail and ui.text { props.detail, class = "dropzone-detail", truncate = true },
			}
		else
			body = ui.column {
				class = "dropzone-body",
				grow = true,
				align = "center",
				justify = "center",
				ui.row {
					class = "dropzone-icon",
					width = 36,
					height = 36,
					align = "center",
					justify = "center",
					ui.icon { name = (props.icon or "upload") :: any },
				},
				prompt(props, "dropzone-title"),
				props.detail and ui.text { props.detail, class = "dropzone-detail" },
			}
		end
		return ui.column {
			class = "dropzone",
			width = props.width,
			height = props.height,
			drop = props.drop,
			to = props.to,
			body,
			side("top"),
			side("bottom"),
			side("left"),
			side("right"),
			corner(true, true),
			corner(true, false),
			corner(false, true),
			corner(false, false),
		}
	end,
}
