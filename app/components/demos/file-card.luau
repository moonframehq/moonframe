-- A task's attachments as tiles and as rows: click one to select it, × to
-- remove it; "Upload more" moves the upload along until it is done.
local file_card = use "file-card"
local segmented = use "segmented"

local FILES: { any } = {
	{ id = "scan", name = "whiteboard.jpg", src = "demos/scan.jpg", detail = "JPG · 1.2 MB" },
	{ id = "plan", name = "launch-plan.pdf", detail = "PDF · 2.4 MB" },
	{ id = "notes", name = "notes.md", detail = "Markdown · 3 KB" },
	{ id = "budget", name = "budget.xlsx", progress = 0.35 },
}

return mod {
	init = function(previous)
		if previous then
			return previous
		end
		local files: { any } = {}
		for index, file in FILES do
			files[index] = table.clone(file)
		end
		return { layout = "tile", files = files, selected = nil }
	end,

	update = function(state, msg)
		if msg == "upload" then
			for _, file in state.files do
				if file.progress then
					file.progress += 0.25
					if file.progress >= 1 then
						file.progress = nil
						file.detail = "XLSX · 88 KB"
					end
				end
			end
		elseif type(msg) == "table" and msg.select then
			state.layout = msg.select
		elseif type(msg) == "table" and msg.open then
			state.selected = msg.open
		elseif type(msg) == "table" and msg.remove then
			for index, file in state.files do
				if file.id == msg.remove then
					table.remove(state.files, index)
					break
				end
			end
		end
	end,

	view = function(state)
		local cards = {}
		local uploading = false
		for index, file in state.files do
			uploading = uploading or file.progress ~= nil
			cards[index] = file_card {
				name = file.name,
				src = file.src,
				detail = file.detail,
				progress = file.progress,
				layout = state.layout,
				selected = state.selected == file.id,
				msg = { open = file.id },
				remove = { remove = file.id },
				width = if state.layout == "row" then 340 else 160,
			}
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				segmented {
					selected = state.layout,
					small = true,
					options = { { id = "tile", label = "Tiles" }, { id = "row", label = "Rows" } },
				},
				uploading and ui.button { "Upload more", small = true, variant = "outline", msg = "upload" },
			},
			if state.layout == "row"
				then ui.column { gap = "small", table.unpack(cards) }
				else ui.row { gap = "normal", wrap = true, align = "start", table.unpack(cards) },
		}
	end,
}
