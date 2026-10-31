-- Review notes: click a note to select it, × removes it, Add note adds one.
local note_card = use "note-card"

local MORE = {
	{ location = "src/ledger.rs:41", text = "This loop allocates per posting; collect into the map directly." },
	{ location = "src/currency.rs:12", text = "Derive Hash here so Currency can key a HashMap." },
	{ location = "tests/book.rs:88", text = "Add a case with two currencies and a zero balance." },
}

return mod {
	init = function(previous)
		return previous
			or {
				selected = 1,
				next = 3,
				notes = {
					{ id = 1, location = "src/ledger.rs:26", text = "Reject mixing currencies here: return a Result instead of silently grouping." },
					{ id = 2, location = "src/ledger.rs:30", text = "Name this `totals_by_currency`; `totals` reads like one number." },
				},
			}
	end,

	update = function(state, msg)
		if msg == "add" then
			local extra = MORE[(state.next - 3) % #MORE + 1]
			table.insert(state.notes, { id = state.next, location = extra.location, text = extra.text })
			state.selected = state.next
			state.next += 1
		elseif type(msg) == "table" and msg.select then
			state.selected = msg.select
		elseif type(msg) == "table" and msg.remove then
			for index, note in state.notes do
				if note.id == msg.remove then
					table.remove(state.notes, index)
					break
				end
			end
		end
	end,

	view = function(state)
		local cards = {}
		for index, note in state.notes do
			cards[index] = note_card {
				id = note.id,
				number = index,
				location = note.location,
				text = note.text,
				selected = note.id == state.selected,
			}
		end
		if #cards == 0 then
			cards[1] = ui.text { "No notes.", tone = "muted", size = "small" }
		end
		table.insert(cards, ui.row { ui.button { "Add note", icon = "plus", small = true, variant = "outline", msg = "add" } })
		return ui.column { gap = "small", width = 360, table.unpack(cards) }
	end,
}
