-- An attached file: a thumbnail, or its type's icon on a tinted tile, the
-- name and a muted line ("PDF · 2.4 MB"), and a remove button. While it
-- uploads, a thin bar and the percentage take the muted line's place. A
-- tile for a grid of attachments, or a row for a list.

local file_icon = use "file-icon"

type Props = {
	name: string,
	src: string?,
	detail: string?,
	progress: number?,
	layout: string?,
	remove: any,
	msg: any,
	selected: boolean?,
	to: string?,
	width: number?,
}

-- The tile's thumbnail height for its width.
local THUMB_RATIO = 0.62

-- A small round button that sends `remove`.
local function remove_button(props: Props, class: string)
	return ui.row {
		class = { "file-card-remove", class },
		width = 20,
		height = 20,
		shrink = false,
		align = "center",
		justify = "center",
		msg = props.remove,
		to = props.to,
		tooltip = "Remove",
		ui.icon { name = "close", size = "small" },
	}
end

-- The muted line, or the upload's bar and percentage.
local function status(props: Props): any
	if props.progress then
		local value = math.clamp(props.progress, 0, 1)
		return ui.row {
			align = "center",
			gap = "small",
			ui.row {
				class = "file-card-track",
				grow = true,
				height = 3,
				value > 0 and ui.row { class = "file-card-fill", area = { left = 0, top = 0, width = value, height = 1 } },
			},
			ui.text { `{math.floor(value * 100 + 0.5)}%`, class = "file-card-detail" },
		}
	end
	return props.detail and ui.text { props.detail, class = "file-card-detail", truncate = true }
end

-- The picture, or the type's icon on a tint, `width` x `height`.
local function preview(props: Props, width: number, height: number, class: string, icon_size: string)
	if props.src then
		return ui.image { src = props.src, fit = "cover", width = width, height = height, class = class }
	end
	return ui.row {
		class = { "file-card-tint", class },
		width = width,
		height = height,
		shrink = false,
		align = "center",
		justify = "center",
		style = { size = icon_size :: any },
		file_icon { name = props.name },
	}
end

return component {
	title = "File Card",
	category = "Data",
	icon = "paperclip",
	description = "An attached file: a thumbnail or its type's icon, the name, a muted line, a remove button, and a bar while it uploads; as a tile or a row.",
	when = "Attachments on a task, a comment or a message, in a grid of tiles or a list of rows, while they upload and after. For the area files are dropped on use `dropzone`; for a file in a tree use `tree` with `file-icon`; for a picture to look at closely use `image-viewer`.",
	props = {
		["name"] = "the file's name; its extension picks the icon when there is no thumbnail",
		["src"] = "a picture file for the thumbnail (cropped to fill)",
		["detail"] = "a muted line under the name, e.g. \"PDF · 2.4 MB\"",
		["progress"] = "0 to 1 while it uploads: a bar and the percentage instead of `detail`",
		["layout"] = "\"tile\" (default: thumbnail on top, for a grid) or \"row\" (icon at the left, for a list)",
		["remove"] = "message the × sends; no × without it",
		["msg"] = "message a click sends (open or preview it)",
		["selected"] = "highlight it",
		["to"] = "mod to send the messages to, if not the drawing one",
		["width"] = "pixels (default 168 for a tile; a row grows)",
	},
	styles = {
		[".file-card"] = { radius = 10, border = true, border_color = "border", background = "background", gap = 0 },
		[".file-card:hover"] = { border_color = "muted-foreground/35" },
		[".file-card:selected"] = { border_color = "accent" },
		[".file-card-row"] = { padding = 8, gap = 10 },
		[".file-card-thumb"] = { radius = { 9, 9, 0, 0 } },
		[".file-card-icon"] = { radius = 7 },
		[".file-card-tint"] = { background = "muted-foreground/8" },
		[".file-card-body"] = { padding = { 8, 10, 9, 10 }, gap = 3 },
		[".file-card-name"] = { size = "small", weight = "medium" },
		[".file-card-detail"] = { size = "small", color = "muted-foreground" },
		[".file-card-track"] = { background = "muted-foreground/15", radius = 999 },
		[".file-card-fill"] = { background = "accent", radius = 999 },
		[".file-card-remove"] = { radius = 999, color = "muted-foreground" },
		[".file-card-remove:hover"] = { background = "list-hover", color = "foreground" },
		[".file-card-remove-over"] = { background = "background/85", color = "foreground", shadow = "0 1px 2px #0000001a" },
		[".file-card-remove-over:hover"] = { background = "background" },
	},
	example = { name = "launch-plan.pdf", detail = "PDF · 2.4 MB", remove = "remove" },
	usage = [[local file_card = use "file-card"

ui.row { gap = "normal", wrap = true,
	file_card { name = "mockup.png", src = file.thumbnail, detail = "PNG · 840 KB",
		msg = { open = file.id }, remove = { remove = file.id } },
	file_card { name = "launch-plan.pdf", progress = upload.progress },
}
file_card { name = "notes.md", detail = "Markdown · 3 KB", layout = "row", remove = { remove = "notes" } }]],

	build = function(props: Props)
		local body = ui.column {
			grow = true,
			style = { gap = 3 },
			ui.text { props.name, class = "file-card-name", truncate = true },
			status(props),
		}
		if props.layout == "row" then
			return ui.row {
				class = { "file-card", "file-card-row" },
				width = props.width,
				align = "center",
				selected = props.selected,
				msg = props.msg,
				to = props.to,
				preview(props, 36, 36, "file-card-icon", "large"),
				body,
				props.remove ~= nil and remove_button(props, "file-card-remove-row"),
			}
		end
		local width = props.width or 168
		return ui.column {
			class = "file-card",
			width = width,
			selected = props.selected,
			msg = props.msg,
			to = props.to,
			ui.row {
				shrink = false,
				preview(props, width - 2, math.floor(width * THUMB_RATIO), "file-card-thumb", "headline"),
				props.remove ~= nil and ui.row {
					float = { at = "top-right", inset = 6 },
					remove_button(props, "file-card-remove-over"),
				},
			},
			ui.column { class = "file-card-body", body },
		}
	end,
}
