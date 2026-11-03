-- Send feedback: the button opens the dialog; type, then Send or ⌘↩ adds a
-- run that "works" on it, and clicking a row opens it; Reply under an open
-- conversation adds a follow-up to it. Nothing is sent
-- anywhere here; the real mod (app/mods/feedback) hands it to the `acp`
-- provider, which runs a coding agent.
local feedback = use "feedback"

return mod {
	init = function(previous)
		local runs: { any } = {
			{
				id = "2",
				thread = "1",
				agent = "claude",
				agent_title = "Claude",
				text = "The search field is still left of centre in my window",
				status = "done",
				started = 200,
				finished = 262,
				tool_count = 7,
				message = "The empty right box had no width, so the field sat left of centre; both side boxes are 240 px now. The screenshot shows it centred on the bar.",
			},
			{
				id = "1",
				agent = "claude",
				agent_title = "Claude",
				text = "Put the search field in the centre of the top bar",
				status = "done",
				started = 0,
				finished = 38,
				tool_count = 6,
				message = "I centred the search field on the whole top bar, in mods/topbar/main.luau.",
			},
		}
		local expanded: string? = "1"
		local attached: any = { element = 'text.status-text "⎇ main +29"', at = "app/mods/status/main.luau:33" }
		return previous
			or { open = true, text = "", reply = "", count = 2, runs = runs, expanded = expanded, attached = attached }
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
		elseif type(msg) == "table" and msg.follow_type then
			state.reply = msg.value or ""
		elseif msg == "send" or (type(msg) == "table" and (msg.msg == "send" or msg.follow)) then
			state.count += 1
			local follow = type(msg) == "table" and msg.follow or nil
			local text = if follow then msg.value or state.reply elseif type(msg) == "table" then msg.value else state.text
			local id = tostring(state.count)
			table.insert(state.runs, 1, {
				id = id,
				thread = follow,
				agent = "claude",
				agent_title = "Claude",
				text = text,
				status = "running",
				started = 0,
				tool_count = 1,
				tools = { { id = "a", title = "Read app/main.luau", kind = "read", status = "in_progress" } },
			})
			state.expanded = follow or id
			if follow then
				state.reply = ""
			else
				state.text = ""
			end
		elseif type(msg) == "table" and msg.expand then
			state.expanded = if state.expanded == msg.expand then nil else msg.expand
		end
	end,

	view = function(state)
		return ui.column {
			ui.row { ui.button { "Send feedback…", icon = "sparkles", variant = "outline", msg = "open" } },
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
				follow = true,
				reply = state.reply,
				-- In a mod, the Inspect button starts Inspect Element and the pick
				-- comes back on the topic `app/inspect`; the gallery starts nothing.
				inspect = true,
				attached = state.attached,
				detach = "detach",
			},
		}
	end,
}
