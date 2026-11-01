-- Send feedback: the button opens the dialog; type, then Send or ⌘↩ adds a
-- run that "works" on it, and clicking a row opens it. Nothing is sent
-- anywhere here; the real mod (app/mods/feedback) hands it to the `acp`
-- provider, which runs a coding agent.
local feedback = use "feedback"

return mod {
	init = function(previous)
		local runs: { any } = {
			{
				id = "1",
				agent = "claude",
				agent_title = "Claude",
				text = "Focus the prompt when I open a session",
				status = "done",
				started = 0,
				finished = 140,
				tool_count = 5,
				message = "New terminal tabs take focus now (app/mods/editor/main.luau).",
			},
		}
		local expanded: string? = nil
		local attached: any = { element = 'text.status-text "⎇ main +29"', at = "app/mods/status/main.luau:33" }
		return previous or { open = true, text = "", count = 1, runs = runs, expanded = expanded, attached = attached }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
		elseif msg == "detach" then
			state.attached = nil
		elseif type(msg) == "table" and msg.msg == "type" then
			state.text = msg.value
		elseif msg == "send" or (type(msg) == "table" and msg.msg == "send") then
			state.count += 1
			local text = if type(msg) == "table" then msg.value else state.text
			local id = tostring(state.count)
			table.insert(state.runs, 1, {
				id = id,
				agent = "claude",
				agent_title = "Claude",
				text = text,
				status = "running",
				started = 0,
				tool_count = 1,
				tools = { { id = "a", title = "Read app/main.luau", kind = "read", status = "in_progress" } },
			})
			state.expanded = id
			state.text = ""
		elseif type(msg) == "table" and msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
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
				note = "A background agent implements it, then reports back under Feedback & fixes.",
				runs = state.runs,
				expanded = state.expanded,
				-- In a mod, the Inspect button starts Inspect Element and the pick
				-- comes back on the topic `app/inspect`; the gallery starts nothing.
				inspect = true,
				attached = state.attached,
				detach = "detach",
			},
		}
	end,
}
