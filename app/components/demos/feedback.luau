-- Send feedback: the button opens the dialog; type, then Send or ⌘↩ adds a
-- request that "runs" until you stop it. Nothing is sent anywhere here; the
-- real mod (app/mods/feedback) hands it to the `feedback` provider.
local feedback = use "feedback"

return mod {
	init = function(previous)
		local items: { any } = {
			{
				id = "1",
				text = "Focus the prompt when I open a session",
				status = "done",
				started = 0,
				finished = 140,
				result = "New terminal tabs take focus now (app/mods/editor/main.luau).",
			},
		}
		return previous
			or {
				open = true,
				text = "",
				count = 1,
				items = items,
			}
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif type(msg) == "table" and msg.msg == "type" then
			state.text = msg.value
		elseif msg == "send" or (type(msg) == "table" and msg.msg == "send") then
			state.count += 1
			local text = if type(msg) == "table" then msg.value else state.text
			table.insert(state.items, 1, { id = tostring(state.count), text = text, status = "running", started = 0 })
			state.text = ""
		elseif type(msg) == "table" and msg.cancel then
			for _, item in state.items do
				if item.id == msg.cancel then
					item.status = "cancelled"
				end
			end
		elseif type(msg) == "table" and msg.forget then
			for i, item in state.items do
				if item.id == msg.forget then
					table.remove(state.items, i)
					break
				end
			end
		end
	end,

	view = function(state)
		return ui.column {
			ui.row { ui.button { "Send feedback…", icon = "circle-help", variant = "outline", msg = "open" } },
			feedback {
				open = state.open,
				dismiss = "close",
				value = state.text,
				text = state.text,
				change = "type",
				send = "send",
				note = "A background agent implements it and checks it, then reports back under Feedback & fixes.",
				items = state.items,
			},
		}
	end,
}
