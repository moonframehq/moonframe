-- Step through 145 activities by page; the second strip has dozens of
-- pages, so it folds the middle into gaps, and its Items per page menu
-- changes how many show on a page in both.
local pagination = use "pagination"

return mod {
	init = function(previous)
		return previous or { page = 1, per_page = 15, size_open = false, long = 12 }
	end,

	update = function(state, msg)
		if msg == "toggle-size" then
			state.size_open = not state.size_open
		elseif msg == "close-size" then
			state.size_open = false
		elseif type(msg) == "table" and msg.per_page then
			-- A new size starts over from the first page.
			state.per_page = msg.per_page
			state.page = 1
			state.size_open = false
		elseif type(msg) == "table" and msg.page then
			state.page = msg.page
		elseif type(msg) == "table" and msg.long then
			state.long = msg.long
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			-- The summary and the page buttons.
			pagination { page = state.page, total = 145, per_page = state.per_page, noun = "activities", window = 5, ellipsis = false },
			ui.row { height = 110 },
			-- Many pages with gaps, and the page-size menu (it opens upward).
			pagination {
				page = state.long,
				total = 1000,
				per_page = state.per_page,
				summary = false,
				event = "long",
				sizes = { 15, 25, 50 },
				size_open = state.size_open,
				size_toggle = "toggle-size",
				size_dismiss = "close-size",
			},
		}
	end,
}
