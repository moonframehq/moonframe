-- Listens to the pi provider, so nothing else has to. A message re-runs the
-- view of the mod it reaches, and pi publishes on every streamed word; this
-- mod draws nothing and passes on only what changed: the thread on screen
-- (the topic `pilot/thread`), new events (to the shell) and the Dock's badge.

type State = { seen: number, thread: string?, cwd: string?, prefill: number, editor: number, returned: string?, badge: string }

return mod {
	subscribe = { "provider/pi" },

	init = function(previous: State?): State
		return previous or { seen = 0, prefill = 0, editor = 0, badge = "" }
	end,

	update = function(state: State, msg: any)
		if type(msg) ~= "table" or msg.topic ~= "provider/pi" or not msg.value or not msg.value.events then
			return
		end
		local pi = msg.value
		local sends: { any } = {}

		-- A provider started over counts from one again.
		local newest = pi.events[#pi.events]
		if newest and newest.seq < state.seen then
			state.seen = 0
		end
		local fresh: { any } = {}
		for _, event in pi.events do
			if event.seq > state.seen then
				state.seen = event.seq
				table.insert(fresh, event)
			end
		end
		if #fresh > 0 then
			table.insert(sends, { to = "shell", msg = { events = fresh, selected = pi.selected, prefs = pi.prefs } })
		end

		local thread = pi.thread
		local cwd = thread and thread.cwd
		local prefill = pi.prefill
		local written = thread and thread.editor_text
		local new_prefill = prefill ~= nil and prefill.seq ~= state.prefill
		local new_written = written ~= nil and written.seq ~= state.editor
		local returned = pi.returned
		local new_returned = returned ~= nil and returned.id ~= state.returned
		if pi.selected ~= state.thread or cwd ~= state.cwd or new_prefill or new_written or new_returned then
			state.thread = pi.selected
			state.cwd = cwd
			local text: string? = nil
			if new_prefill then
				state.prefill = prefill.seq
				text = prefill.text
			end
			if new_written then
				state.editor = written.seq
				text = written.text
			end
			local back: any = nil
			if new_returned then
				state.returned = returned.id
				back = returned
				table.insert(sends, { to = "provider:pi", msg = { returned_taken = { thread = pi.selected, id = returned.id } } })
			end
			table.insert(sends, { to = "topic:pilot/thread", msg = { id = pi.selected, cwd = cwd, text = text, seq = state.prefill + state.editor, returned = back } })
		end

		local waiting = 0
		for _, row in pi.threads do
			if row.status == "waiting" or row.unread then
				waiting += 1
			end
		end
		local badge = if waiting > 0 then tostring(waiting) else ""
		if badge ~= state.badge then
			state.badge = badge
			table.insert(sends, { to = "app:dock", msg = { badge = if waiting > 0 then badge else false } })
		end
		return state, sends
	end,

	view = function()
		return ui.column { width = 0, height = 0 }
	end,
}
