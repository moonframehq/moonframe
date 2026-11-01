-- A note drawn from markdown: click a link to see the message it sends;
-- nothing opens by itself.
local markdown = use "markdown"

local NOTE = [[
# Weekly notes

The ledger now keeps **one balance per currency**, so amounts in
different currencies never mix. See [the design](docs/ledger.md) or the
[Luau guide](https://luau.org).

## Next

- [x] Group postings by currency
- [ ] Reject mixed totals with an error
- [ ] Write the migration

| Currency | Balance |
| -------- | ------: |
| EUR      |  412.00 |
| USD      |   98.50 |

> Links send `{ link = true, url = ... }` to the mod.

```rust
pub fn balances(&self) -> HashMap<Currency, i64>
```
]]

return mod {
	init = function(previous)
		return previous or { clicked = nil }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.link then
			state.clicked = msg.url
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			height = 420,
			ui.text {
				if state.clicked then `Clicked: {state.clicked}` else "Click a link",
				tone = "muted",
				size = "small",
			},
			markdown { text = NOTE, link = { link = true }, grow = true, scroll = true },
		}
	end,
}
