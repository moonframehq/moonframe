-- A file at a line: click a line number to highlight it (again to clear);
-- the switch hides the numbers.
local code_view = use "code-view"
local switch = use "switch"

local SOURCE = "impl Book {\n    /// Balance per currency; amounts in different currencies never mix.\n    pub fn balances(&self) -> HashMap<Currency, i64> {\n        let mut totals = HashMap::new();\n        for posting in &self.entries {\n            *totals.entry(posting.currency).or_default() += posting.cents;\n        }\n        totals\n    }\n}"

return mod {
	init = function(previous)
		return previous or { highlight = { 26, 27 }, numbers = true }
	end,

	update = function(state, msg)
		if msg == "toggle-numbers" then
			state.numbers = not state.numbers
		elseif type(msg) == "table" and msg.line then
			local at = table.find(state.highlight, msg.line)
			if at then
				table.remove(state.highlight, at)
			else
				table.insert(state.highlight, msg.line)
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row { switch { on = state.numbers, label = "Line numbers", small = true, msg = "toggle-numbers" } },
			code_view {
				path = "src/ledger.rs",
				start = 24,
				highlight = state.highlight,
				numbers = state.numbers,
				height = 230,
				event = "line",
				text = SOURCE,
			},
		}
	end,
}
