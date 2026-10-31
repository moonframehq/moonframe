-- Tiled panes: click a title to focus it, the split buttons add a pane to
-- the right or below (up to 4), zoom fills the area with one, × closes it.
local pane_frame = use "pane-frame"

local PROGRAMS: { any } = {
	{ title = "claude", icon = "agent-claude", status = "blocked", lines = { "Edit src/ledger.rs?", "  [y] yes  [n] no" } },
	{ title = "cargo test", icon = "terminal", status = "done", lines = { "running 2 tests", "test result: ok. 2 passed" } },
	{ title = "codex", icon = "agent-codex", status = "working", lines = { "Reading src/currency.rs…" } },
	{ title = "zsh", icon = "square-terminal", status = nil, lines = { "ledger-api main ›" } },
}

return mod {
	init = function(previous)
		-- rows of pane ids; each pane id indexes PROGRAMS.
		return previous or { rows = { { 1, 2 } }, focus = 1, zoomed = nil }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		local count = 0
		for _, row in state.rows do
			count += #row
		end
		-- The first program not already open.
		local function free(): number?
			for id = 1, #PROGRAMS do
				local used = false
				for _, row in state.rows do
					if table.find(row, id) then
						used = true
					end
				end
				if not used then
					return id
				end
			end
			return nil
		end
		local function locate(id: number): (number?, number?)
			for r, row in state.rows do
				local c = table.find(row, id)
				if c then
					return r, c
				end
			end
			return nil, nil
		end

		if msg.focus then
			state.focus = msg.focus
		elseif msg.zoom then
			state.zoomed = if state.zoomed == msg.zoom then nil else msg.zoom
			state.focus = msg.zoom
		elseif msg.split_right or msg.split_down then
			local new = free()
			local r, c = locate(msg.split_right or msg.split_down)
			if new and r and c and count < 4 then
				if msg.split_right then
					table.insert(state.rows[r], c + 1, new)
				else
					table.insert(state.rows, r + 1, { new })
				end
				state.focus = new
				state.zoomed = nil
			end
		elseif msg.close then
			local r, c = locate(msg.close)
			if r and c and count > 1 then
				table.remove(state.rows[r], c)
				if #state.rows[r] == 0 then
					table.remove(state.rows, r)
				end
				if state.zoomed == msg.close then
					state.zoomed = nil
				end
				if state.focus == msg.close then
					state.focus = state.rows[1][1]
				end
			end
		end
	end,

	view = function(state)
		-- Side by side panes are narrow: show the folder only on a lone pane.
		local function frame(id: number, alone: boolean)
			local p = PROGRAMS[id]
			local lines = {}
			for i, line in p.lines do
				lines[i] = ui.text { line, size = "small" }
			end
			return pane_frame {
				title = p.title,
				icon = p.icon,
				subtitle = if alone then "~/code/ledger-api" else nil,
				status = p.status,
				focused = state.focus == id,
				zoomed = state.zoomed == id,
				focus = { focus = id },
				split_right = { split_right = id },
				split_down = { split_down = id },
				zoom = { zoom = id },
				close = { close = id },
				table.unpack(lines),
			}
		end
		local rows = {}
		if state.zoomed then
			rows[1] = ui.row { grow = true, frame(state.zoomed, true) }
		else
			for r, row in state.rows do
				local frames = {}
				for c, id in row do
					frames[c] = frame(id, #row == 1)
				end
				rows[r] = ui.row { grow = true, gap = "small", table.unpack(frames) }
			end
		end
		return ui.column { height = 340, gap = "small", table.unpack(rows) }
	end,
}
