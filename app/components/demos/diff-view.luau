-- A file's diff: click the header to fold it, use Unified / Split in the
-- header to switch layouts.
local diff_view = use "diff-view"

type Line = { kind: string, old: number?, new: number?, text: string }

local HUNKS: { { header: string, lines: { Line } } } = {
	{
		header = "@@ -1,8 +1,14 @@",
		lines = {
			{ kind = "add", new = 1, text = "use std::collections::HashMap;" },
			{ kind = "add", new = 2, text = "use crate::currency::Currency;" },
			{ kind = "add", new = 3, text = "" },
			{ kind = "ctx", old = 1, new = 4, text = "#[derive(Default)]" },
			{ kind = "ctx", old = 2, new = 5, text = "pub struct Book {" },
			{ kind = "del", old = 3, text = "    entries: Vec<(String, i64)>," },
			{ kind = "add", new = 6, text = "    entries: Vec<Posting>," },
			{ kind = "ctx", old = 4, new = 7, text = "}" },
		},
	},
	{
		header = "@@ -11,4 +17,9 @@ impl Book {",
		lines = {
			{ kind = "del", old = 11, text = "    pub fn balance(&self) -> i64 {" },
			{ kind = "del", old = 12, text = "        self.entries.iter().map(|(_, c)| c).sum()" },
			{ kind = "add", new = 17, text = "    pub fn balances(&self) -> HashMap<Currency, i64> {" },
			{ kind = "add", new = 18, text = "        let mut totals = HashMap::new();" },
			{ kind = "add", new = 19, text = "        for posting in &self.entries {" },
			{ kind = "add", new = 20, text = "            *totals.entry(posting.currency).or_default() += posting.cents;" },
			{ kind = "add", new = 21, text = "        }" },
			{ kind = "add", new = 22, text = "        totals" },
			{ kind = "ctx", old = 13, new = 23, text = "    }" },
		},
	},
}

return mod {
	init = function(previous)
		return previous or { split = false, collapsed = false }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.toggle then
			state.collapsed = not state.collapsed
		elseif msg.layout then
			state.split = msg.layout == "split"
		end
	end,

	view = function(state)
		return diff_view {
			path = "src/ledger.rs",
			status = "M",
			height = 330,
			split = state.split,
			collapsed = state.collapsed,
			switch = "layout",
			hunks = HUNKS,
		}
	end,
}
