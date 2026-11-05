-- A note beside its outline, with a find bar: click a heading to jump to it,
-- type to mark what you look for, step through with Enter. A link sends a
-- message; nothing opens by itself.
local markdown = use "markdown"
local outline = use "outline"
local find_bar = use "find-bar"

local NOTE = [[
# Weekly notes

The ledger now keeps **one balance per currency**, so amounts in
different currencies never mix.[^why] See [the design](docs/ledger.md), the
[Luau guide](https://luau.org) or [the next steps](#next).

> [!TIP]
> Alerts draw as callouts, and links in them work too.

## Next

- [x] Group postings by currency
- [ ] Reject mixed totals with an error
- [ ] Write the migration

| Currency | Balance |
| -------- | ------: |
| EUR      |  412.00 |
| USD      |   98.50 |

## Code

```rust
pub fn balances(&self) -> HashMap<Currency, i64>
```

> Links send `{ link = true, url = ... }` to the mod.

[^why]: Summing euros and dollars gives a number that means nothing.
]]

return mod {
	init = function(previous)
		return previous
			or {
				headings = {} :: any,
				query = "",
				match = 1,
				count = 0,
				seq = 0,
				clicked = nil :: string?,
				section = nil :: number?,
				reveal = nil :: any,
			}
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			if msg == "next" and state.count > 0 then
				state.match = state.match % state.count + 1
			elseif msg == "previous" and state.count > 0 then
				state.match = (state.match - 2) % state.count + 1
			end
		elseif msg.link then
			state.clicked = msg.url
		elseif msg.msg == "headings" then
			state.headings = msg.headings
		elseif msg.msg == "reading" then
			state.section = msg.heading
		elseif msg.msg == "found" then
			state.count = msg.count
		elseif msg.msg == "query" then
			state.query = msg.value
			state.match = 1
		elseif msg.heading then
			state.seq += 1
			state.reveal = { at = state.headings[msg.heading].at, seq = state.seq }
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			height = 460,
			find_bar {
				value = state.query, count = state.count, index = state.match,
				change = "query", previous = "previous", next = "next", width = 420,
			},
			ui.text {
				if state.clicked then `Clicked: {state.clicked}` else "Click a link",
				tone = "muted",
				size = "small",
			},
			ui.row {
				grow = true,
				gap = "normal",
				ui.column { width = 150, outline { headings = state.headings, current = state.section } },
				markdown {
					text = NOTE, link = { link = true }, grow = true, scroll = true,
					outline = "headings", section = "reading", reveal = state.reveal,
					find = state.query, find_index = state.match, found = "found",
				},
			},
		}
	end,
}
