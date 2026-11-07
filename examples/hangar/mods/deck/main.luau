-- The deck: the chosen bay's terminal, kept while you visit others, and what
-- every terminal's shell says it is doing. That goes out on `hangar/activity`
-- for the bays to show. Shell events are for showing: a program can print
-- anything, so nothing here runs or removes because of one.

local empty_state = use "empty-state"
local status_dot = use "status-dot"

-- A command ending in a bay not on show is news from this long on.
local LONG_COMMAND_MS = 10000
local MAX_TITLE = 256
local MAX_BODY = 2048

type Bay = { path: string, branch: string }

type State = {
	bay: Bay?,
	-- Each bay's terminal id, kept so its shell is found again.
	terminals: { [string]: string },
	next_terminal: number,
	-- A command runs there, and what happened while it was out of sight.
	running: { [string]: boolean },
	attention: { [string]: string },
	-- The bay on show whose shell exited, still showing what it printed.
	ended: string?,
}

local function activity_of(state: State): { [string]: string }
	local shown = state.bay and state.bay.path
	local activity = {}
	for path in state.running do
		activity[path] = "working"
	end
	for path, what in state.attention do
		if path ~= shown then
			activity[path] = what
		end
	end
	return activity
end

local function announce(state: State, extra: any?): { any }
	local sends: { any } = { { to = "topic:hangar/activity", msg = activity_of(state) } }
	if extra then
		table.insert(sends, extra)
	end
	return sends
end

-- `text` in `limit` bytes at most, cut between characters.
local function clip(text: string, limit: number): string
	if #text <= limit then
		return text
	end
	local at = limit + 1
	while at > 1 and bit32.band(string.byte(text, at), 0xC0) == 0x80 do
		at -= 1
	end
	return string.sub(text, 1, at - 1)
end

-- Which bay a notification is about. Never the command: its line can hold a
-- token, and notifications outlive the terminal in the system's own list.
local function folder_of(path: string): string
	return string.match(path, "([^/\\]+)$") or path
end

local function notify(title: string, body: string?): any
	return { to = "app:notify", msg = { title = clip(title, MAX_TITLE), body = body and clip(body, MAX_BODY) } }
end

return mod {
	region = "main",
	title = "Deck",
	subscribe = { "hangar/bay" },

	styles = {
		[".deck"] = { background = "card", radius = 12, border = true, clip = true },
		[".deck-head"] = { padding = { 10, 14 }, border = "bottom", border_color = "border" },
		[".deck-branch"] = { weight = "medium" },
		[".deck-path"] = { size = "small", color = "muted-foreground" },
	},

	init = function(previous: any): State
		return {
			terminals = previous and previous.terminals or {},
			next_terminal = previous and previous.next_terminal or 1,
			running = {},
			attention = {},
		}
	end,

	update = function(state: State, msg: any)
		if msg == "restart" then
			-- A new id, as the old one's terminal stays until it is no
			-- longer drawn, which also gives back its slot.
			local path = state.ended
			if path then
				state.terminals[path] = `bay-{state.next_terminal}`
				state.next_terminal += 1
				state.ended = nil
				return state
			end
			return
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "hangar/bay" then
			state.bay = msg.value or nil
			local bay = state.bay
			-- Out of sight an ended terminal goes, and coming back starts
			-- a shell by itself.
			if state.ended ~= (bay and bay.path) then
				state.ended = nil
			end
			if bay then
				state.attention[bay.path] = nil
				if not state.terminals[bay.path] then
					state.terminals[bay.path] = `bay-{state.next_terminal}`
					state.next_terminal += 1
				end
			end
			return state, announce(state)
		end
		local path = msg.bay
		if not path then
			return
		end
		local hidden = not state.bay or state.bay.path ~= path
		if msg.exited then
			state.running[path] = nil
			if not hidden then
				state.ended = path
			end
			return state, announce(state)
		elseif msg.event == "started" then
			state.running[path] = true
			return state, announce(state)
		elseif msg.event == "finished" then
			state.running[path] = nil
			if not hidden then
				return state, announce(state)
			end
			state.attention[path] = "done"
			local long = msg.took_ms >= LONG_COMMAND_MS
			local failed = msg.status ~= nil and msg.status ~= 0
			local title = if failed then "Command failed" else "Command finished"
			return state, announce(state, if long then notify(title, folder_of(path)) else nil)
		elseif msg.event == "notice" then
			if hidden then
				state.attention[path] = "blocked"
			end
			return state, announce(state, notify(msg.title or "Hangar", msg.body))
		end
		return
	end,

	view = function(state: State)
		local bay = state.bay
		if not bay then
			return ui.column {
				grow = true,
				class = "deck",
				empty_state { icon = "git-branch", title = "No bay", description = "Open Hangar in a git repository to see its worktrees." },
			}
		end
		return ui.column {
			grow = true,
			gap = "none",
			class = "deck",
			ui.row {
				align = "center",
				gap = "small",
				class = "deck-head",
				drag = true,
				status_dot { status = if state.running[bay.path] then "working" else "idle", spin = true },
				ui.text { bay.branch, class = "deck-branch" },
				ui.row { grow = true, ui.text { bay.path, class = "deck-path", truncate = true } },
				state.ended == bay.path and ui.button { "New shell", small = true, msg = "restart" } or nil,
			},
			ui.terminal {
				id = state.terminals[bay.path],
				cwd = bay.path,
				shell = { bay = bay.path },
				exit = { bay = bay.path },
			},
		}
	end,
}
