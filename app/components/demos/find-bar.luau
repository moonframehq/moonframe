-- A find bar over a short text: type to count the matches, and step through
-- them with Enter and Shift-Enter.
local find_bar = use "find-bar"
local markdown = use "markdown"

local TEXT = [[
The ledger keeps one balance per currency. A ledger entry never mixes
currencies, and the ledger refuses a total that would.
]]

return mod {
	init = function(previous)
		return previous or { query = "ledger", match = 1, count = 0 }
	end,

	update = function(state, msg)
		if msg == "next" and state.count > 0 then
			state.match = state.match % state.count + 1
		elseif msg == "previous" and state.count > 0 then
			state.match = (state.match - 2) % state.count + 1
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			state.match = 1
		elseif type(msg) == "table" and msg.msg == "found" then
			state.count = msg.count
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			find_bar {
				value = state.query, count = state.count, index = state.match,
				change = "query", previous = "previous", next = "next",
			},
			markdown { text = TEXT, find = state.query, find_index = state.match, found = "found" },
		}
	end,
}
