-- Inspector sections: click a file to select it; "Show tests" adds a third
-- section.
local section = use "section"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { selected = "src/ledger.rs", tests = false }
	end,

	update = function(state, msg)
		if msg == "tests" then
			state.tests = not state.tests
		elseif typeof(msg) == "table" and msg.open then
			state.selected = msg.open
		end
	end,

	view = function(state)
		local function file(path: string, detail: string)
			return ui.item { path, detail = detail, selected = path == state.selected, msg = { open = path } }
		end
		return ui.column {
			gap = "large",
			width = 360,
			switch { label = "Show tests", on = state.tests, msg = "tests", small = true },
			section {
				title = "CHANGED FILES",
				file("src/ledger.rs", "+12 −3"),
				file("src/currency.rs", "+40"),
			},
			section {
				title = "BRANCH",
				ui.text { "feat/multi-currency", size = "small" },
				ui.text { "2 ahead of main", size = "small", tone = "muted" },
			},
			state.tests and section {
				title = "TESTS",
				file("tests/ledger.rs", "2 passed"),
			},
		}
	end,
}
