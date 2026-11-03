-- Attachments: drop files from Finder on either box, or click browse to
-- pick them in the system's panel; each lands as a row under it, where ×
-- removes it. The compact box is the one-line form for inside a panel.
local dropzone = use "dropzone"
local file_card = use "file-card"

-- The last part of a path.
local function base(path: string): string
	return string.match(path, "([^/\\]+)$") or path
end

return mod {
	init = function(previous)
		return previous or { files = {} :: { string }, note = "Nothing added yet" }
	end,

	update = function(state, msg)
		if msg == "browse" then
			return state, { { to = "app:pick", msg = { kind = "open", multiple = true, prompt = "Attach", reply = "added" } } }
		elseif type(msg) == "table" and msg.remove then
			table.remove(state.files, msg.remove)
		elseif type(msg) == "table" and msg.cancelled then
			state.note = "Browse cancelled"
		elseif type(msg) == "table" and msg.paths then
			for _, path in msg.paths do
				table.insert(state.files, path)
			end
			state.note = `{#msg.paths} added`
		end
		return state
	end,

	view = function(state)
		local rows = {}
		for index, path in state.files do
			rows[index] = file_card { name = base(path), detail = path, layout = "row", remove = { remove = index } }
		end
		return ui.column {
			gap = "normal",
			width = 440,
			dropzone { detail = "Up to 250 MB each", drop = "added", browse = "browse" },
			dropzone { compact = true, title = "Drop to attach", drop = "added", browse = "browse" },
			ui.text { state.note, tone = "muted", size = "small" },
			ui.column { gap = "small", table.unpack(rows) },
		}
	end,
}
