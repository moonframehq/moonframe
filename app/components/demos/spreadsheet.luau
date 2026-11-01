-- A sheet being filled: "Write next" writes the next cell (outlined, with a
-- caret, its row highlighted); cells not written yet are skeleton bars.
-- Click any cell to select it instead.
local spreadsheet = use "spreadsheet"

local APPS = {
	{ company = "Bayline", apy = "4.35%", fee = "$0", source = "bayline.com" },
	{ company = "Kestrel", apy = "4.60%", fee = "$0", source = "kestrel.app" },
	{ company = "Northgate", apy = "4.10%", fee = "$0", source = "northgate.com" },
	{ company = "Tendril", apy = "3.90%", fee = "$3", source = "tendril.app" },
}
local KEYS = { "apy", "fee" }

local COLUMNS: { any } = {
	{ key = "company", title = "Company", icon = "case-sensitive", width = 130 },
	{ key = "apy", title = "APY", glyph = "%", width = 76 },
	{ key = "fee", title = "Monthly fee", glyph = "$", width = 104 },
	{ key = "source", title = "Source", icon = "link" },
}

return mod {
	init = function(previous)
		return previous or { written = 3, picked = nil }
	end,

	update = function(state, msg)
		if msg == "next" then
			state.written = if state.written >= #APPS * #KEYS then 0 else state.written + 1
			state.picked = nil
		elseif type(msg) == "table" and msg.cell then
			state.picked = msg.cell
		end
	end,

	view = function(state)
		local rows = {}
		for index, app in APPS do
			local row: { [string]: any } = { company = app.company, source = app.source }
			for c, key in KEYS do
				local n = (index - 1) * #KEYS + c
				row[key] = if n <= state.written then app[key] else false
			end
			rows[index] = row
		end
		local at = math.max(1, state.written)
		local row = math.ceil(at / #KEYS)
		local key = KEYS[(at - 1) % #KEYS + 1]
		return ui.column {
			gap = "normal",
			ui.row { ui.button { "Write next", small = true, msg = "next" } },
			spreadsheet {
				columns = COLUMNS,
				rows = rows,
				current = if state.picked then nil else row,
				selected = state.picked or (state.written > 0 and { row = row, key = key } or nil),
				caret = state.picked == nil,
				blank = 2,
			},
		}
	end,
}
