-- Send feedback: a ? in the title bar opens a dialog where the user writes
-- what should change in the app. The `feedback` provider hands it to a
-- coding agent working on the app's own files (the window reloads as they
-- change) and reports back; the dialog lists what came of each request.
-- The icon spins while an agent works, and shows a dot when one finished
-- since the dialog was last open. The shell mounts it in its title bar.
local feedback = use "feedback"

type State = {
	open: boolean,
	text: string,
	-- The newest time a request finished, and the newest the user has seen
	-- in the dialog.
	latest: number,
	seen: number,
	-- The main tab's title, from `tabs/main`: what they are looking at.
	tab: string?,
}

-- Where the agent works, with the home folder as ~.
local function short(path: string): string
	local home = string.match(path, "^(/Users/[^/]+)") or string.match(path, "^(/home/[^/]+)")
	if home then
		return "~" .. string.sub(path, #home + 1)
	end
	return path
end

-- The newest time any request finished.
local function newest(items: { any }?): number
	local latest = 0
	local list: { any } = items or {}
	for _, item in list do
		if item.finished and item.finished > latest then
			latest = item.finished
		end
	end
	return latest
end

return mod {
	title = "Feedback",
	subscribe = { "tabs/main", "provider/feedback" },

	init = function(previous: State?): State
		local seen = previous and previous.seen or 0
		return { open = false, text = "", latest = seen, seen = seen, tab = nil }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "tabs/main" then
			state.tab = msg.value and msg.value.title
		elseif type(msg) == "table" and msg.topic == "provider/feedback" then
			state.latest = newest(msg.value and msg.value.items)
			if state.open then
				state.seen = state.latest
			end
		elseif msg == "open" or msg == "close" then
			-- What the dialog showed counts as seen.
			state.open = msg == "open"
			state.seen = state.latest
		elseif type(msg) == "table" and msg.msg == "type" then
			state.text = msg.value
		elseif type(msg) == "table" and msg.send then
			-- The Send button can't read the field, so it carries no `value`;
			-- ⌘↩ does.
			local text = msg.value or state.text
			if string.match(text or "", "^%s*$") then
				return nil
			end
			state.text = ""
			state.open = false
			return state, { { to = "provider:feedback", msg = { send = text, context = msg.context } } }
		elseif type(msg) == "table" and msg.cancel then
			return state, { { to = "provider:feedback", msg = { cancel = msg.cancel } } }
		elseif type(msg) == "table" and msg.forget then
			return state, { { to = "provider:feedback", msg = { dismiss = msg.forget } } }
		end
		return nil
	end,

	view = function(state: State, app)
		local data = app.feedback
		local items = data and data.available and data.items or nil
		local running = data and data.running or 0
		local unseen = state.latest > state.seen
		local window = app.window
		local git = app.git

		-- What goes with the request: what the user is looking at.
		local shape = window.platform
		if window.fullscreen then
			shape ..= ", fullscreen"
		elseif window.maximized then
			shape ..= ", maximized"
		end
		local context = {
			["main tab"] = state.tab,
			window = shape,
			branch = git and git.available and git.branch or nil,
			["changed files"] = git and git.available and git.changed or nil,
		}

		local note
		if data == nil then
			note = "Starting…"
		elseif not data.available then
			note = `Feedback can't be sent here: {data.reason}.`
		else
			note = `A background agent ({data.agent}) in {short(data.workdir)} implements it and checks it; your window reloads with the change, and it reports back under Feedback & fixes. It gets what you're looking at, your window and the build.`
		end

		return ui.row {
			gap = "none",
			-- The dialog is beside the button, not in it, so clicks in the
			-- dialog never reach the button.
			ui.row {
				class = "tool",
				align = "center",
				justify = "center",
				msg = "open",
				tooltip = if running > 0 then "Send feedback (an agent is working)" else "Send feedback",
				ui.icon { name = if running > 0 then "loader-circle" else "circle-help", spin = running > 0 },
				unseen and running == 0 and ui.row { class = "feedback-dot", float = { top = 3, right = 3 } },
			},
			feedback {
				open = state.open,
				dismiss = "close",
				value = state.text,
				text = if data and data.available then state.text else "",
				change = "type",
				send = { send = true, context = context },
				note = note,
				items = items,
			},
		}
	end,

	styles = {
		[".feedback-dot"] = { width = 6, height = 6, radius = 3, background = "accent" },
	},
}
