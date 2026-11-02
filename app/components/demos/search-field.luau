-- Type to filter the files; up and down move the highlight, Enter opens
-- the highlighted one, Esc clears the query.
local search = use "search-field"

local FILES = {
	"crates/moonframe/src/app.rs",
	"crates/moonframe/src/browser.rs",
	"crates/moonframe/src/input.rs",
	"crates/moonframe-core/src/node.rs",
	"crates/moonframe-core/src/style.rs",
	"app/components/tabs.luau",
	"app/components/search-field.luau",
	"app/style.luau",
}

local function matches(query: string): { string }
	local found = {}
	for _, file in FILES do
		if string.find(string.lower(file), string.lower(query), 1, true) then
			table.insert(found, file)
		end
	end
	return found
end

return mod {
	init = function(previous)
		return previous or { query = "", value = nil :: string?, index = 1, opened = nil :: string? }
	end,

	update = function(state, msg)
		local found = matches(state.query)
		if msg.msg == "query" then
			state.query = msg.value
			state.value = nil
			state.index = 1
		elseif msg.msg == "clear" then
			state.query = ""
			-- Passing "" back empties the field.
			state.value = ""
			state.index = 1
		elseif msg.msg == "up" then
			state.index = math.max(1, state.index - 1)
		elseif msg.msg == "down" then
			state.index = math.min(math.max(#found, 1), state.index + 1)
		elseif msg.msg == "open" then
			state.opened = found[state.index]
		elseif msg.pick then
			state.opened = msg.pick
		end
	end,

	view = function(state)
		local found = matches(state.query)
		local rows = {}
		for index, file in found do
			rows[index] = ui.item { file, selected = index == state.index, msg = { pick = file } }
		end
		return ui.column {
			gap = "small",
			search {
				id = "files",
				value = state.value,
				placeholder = "Search files…",
				icon = true,
				shortcut = "cmd-k",
				change = "query",
				submit = "open",
				cancel = "clear",
				previous = "up",
				next = "down",
			},
			ui.column { gap = "none", table.unpack(rows) },
			#found == 0 and ui.text { "No file matches.", tone = "muted", size = "small" },
			state.opened and ui.text { `Opened {state.opened}`, tone = "muted", size = "small" },
		}
	end,
}
