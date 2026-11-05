-- A picture over the whole window, to look at closely: it zooms and pans as
-- an `image-viewer` does, ← and → step through a set, and Escape closes it.
-- A click on a picture in a `ui.markdown` with `image` set can open it.

local image_viewer = use "image-viewer"
local icon_button = use "icon-button"

type Picture = { src: string, title: string? }

type Props = {
	open: boolean?,
	dismiss: any,
	pictures: { Picture }?,
	index: number?,
	event: string?,
	zoom: number?,
	center: any,
	change: any,
	to: string?,
}

return component {
	title = "Lightbox",
	category = "Overlays",
	icon = "image",
	description = "A picture over the whole window with its title and place in the set: zoom and pan, ← and → through the set, Escape to close.",
	when = "Opening a picture from a document, a gallery or a message to see it large. For a viewer that stays in a pane use `image-viewer`; for a picture at a fixed size use `ui.image`.",
	props = {
		["open"] = "show it",
		["dismiss"] = "sent by ×, Escape and a click on the backdrop",
		["pictures"] = "list of { src, title? }",
		["index"] = "which one shows, from 1 (default 1)",
		["event"] = "key of the message ← and → send, default \"picture\": { picture = index }",
		["zoom"] = "as `image-viewer`; nil fits it",
		["center"] = "as `image-viewer`",
		["change"] = "as `image-viewer`: sent as the view changes, with `zoom` and `center`",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".lightbox"] = { background = "background", radius = 12, gap = 0, clip = true, shadow = "large" },
		[".lightbox-bar"] = { padding = { 6, 8, 6, 14 }, gap = 8, border = "bottom", border_color = "border" },
		[".lightbox-title"] = { weight = "medium" },
		[".lightbox-place"] = { size = "small", color = "muted-foreground" },
	},
	example = { open = false, pictures = { { src = "demos/scan.jpg", title = "Scan" } } },
	usage = [[local lightbox = use "lightbox"

lightbox {
	open = state.viewing ~= nil, dismiss = "close-picture",
	pictures = state.pictures, index = state.viewing,
	zoom = state.zoom, center = state.center, change = "viewed",
}
-- update: msg.picture steps to that one (and clears zoom and center);
-- msg.msg == "viewed" keeps msg.zoom and msg.center.]],

	build = function(props: Props)
		local pictures: { Picture } = props.pictures or {}
		local count = #pictures
		local index = math.clamp(props.index or 1, 1, math.max(count, 1))
		local picture = pictures[index]
		if not props.open or not picture then
			return ui.overlay { open = false, content = ui.column {} }
		end
		local event = props.event or "picture"
		local step = function(by: number)
			return { [event] = (index - 1 + by) % count + 1 }
		end
		local keys: any = { escape = props.dismiss }
		if count > 1 then
			keys.left = step(-1)
			keys.right = step(1)
		end
		return ui.overlay {
			open = true,
			place = "center",
			backdrop = true,
			dismiss = props.dismiss,
			to = props.to,
			keys = keys,
			content = ui.column {
				class = "lightbox",
				width = 960,
				height = 680,
				ui.row {
					class = "lightbox-bar",
					align = "center",
					ui.row { grow = true, ui.text { picture.title or "", class = "lightbox-title", truncate = true } },
					count > 1 and ui.text { `{index} of {count}`, class = "lightbox-place" },
					count > 1 and icon_button { icon = "chevron-left", tooltip = "Previous", size = "small", msg = step(-1), to = props.to },
					count > 1 and icon_button { icon = "chevron-right", tooltip = "Next", size = "small", msg = step(1), to = props.to },
					icon_button { icon = "close", tooltip = "Close", size = "small", msg = props.dismiss, to = props.to },
				},
				image_viewer {
					id = "lightbox",
					src = picture.src,
					zoom = props.zoom,
					center = props.center,
					change = props.change,
					controls = true,
				},
			},
		}
	end,
}
