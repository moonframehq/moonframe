-- The Journal layout: today's note as an outline with the days before it,
-- a line to add to today, and the inbox beside it as an append-and-review
-- note (Karpathy's): newest at the top, the oldest picked for review, and
-- keep, make a note, make a task or let it sink. All from the notes
-- provider's live views.

local journal_day = use "journal-day"
local icon_button = use "icon-button"

type State = {
	views: { [string]: any },
	-- The inbox item picked for review: its index among the items.
	pick: number?,
	-- Bumped after each capture, so the field starts empty again.
	round: number,
}

local RAIL: { any } = {
	{ icon = "sun", tooltip = "Journal (⌥⌘2)", layout = "journal" },
	{ icon = "layers", tooltip = "Library (⌥⌘1)", layout = "library" },
	{ icon = "layout-dashboard", tooltip = "Workspace (⌥⌘3)", layout = "workspace" },
}

-- Its views; asked `again` since the mod starts with none.
local function watches(): { any }
	return {
		{ to = "provider:notes", msg = { watch = { id = "days", kind = "journal", days = 4 }, again = true } },
		{ to = "provider:notes", msg = { watch = { id = "inbox", kind = "note", path = "inbox.md" }, again = true } },
	}
end

-- The inbox's top-level items with their line numbers, newest first.
local function items(text: string): { any }
	local out: { any } = {}
	local number = 0
	for line in string.gmatch(text .. "\n", "([^\n]*)\n") do
		number += 1
		local item = string.match(line, "^[-*+] (.*)$")
		if item then
			table.insert(out, { line = number, text = item })
		end
	end
	return out
end

local function rail()
	local buttons = {}
	for _, item in RAIL do
		table.insert(buttons, icon_button { icon = item.icon, tooltip = item.tooltip, active = item.layout == "journal", to = "shell", msg = { layout = item.layout } })
	end
	return ui.column { grow = true, align = "center", gap = "small", ui.row { height = 48, drag = true, shrink = false }, table.unpack(buttons) }
end

local function journal(state: State)
	local days = (state.views.days or {}).rows or {}
	local blocks = {}
	for index, day in days do
		table.insert(blocks, journal_day { day = day })
		if day.today then
			table.insert(
				blocks,
				ui.row {
					class = "search-box",
					style = { margin = { 0, 24 } },
					align = "center",
					ui.icon { "plus", class = "muted" },
					ui.input { id = `today-{state.round}`, placeholder = "Add a line to today…", submit = { capture = "today" }, grow = true, class = "bare-input" },
				}
			)
		end
		if index < #days then
			table.insert(blocks, ui.divider())
		end
	end
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			align = "center",
			ui.icon { "sun", class = "muted" },
			ui.row { grow = true, gap = "none", ui.text { "Journal", class = "card-title" } },
			ui.text { "journal/", class = "muted small" },
		},
		ui.column { grow = true, scroll = true, ui.column { max_width = 760, self_align = "center", grow = true, table.unpack(blocks) } },
	}
end

local function inbox(state: State)
	local note = state.views.inbox or {}
	local list = items(note.text or "")
	local pick = state.pick or #list
	local rows = {}
	for index, item in list do
		table.insert(
			rows,
			ui.row {
				class = "note-row",
				selected = index == pick,
				align = "start",
				msg = { pick = index },
				ui.text { "•", class = "muted" },
				ui.row { grow = true, gap = "none", ui.text { item.text, wrap = true } },
			}
		)
	end
	local picked = list[pick]
	return ui.column {
		class = "card",
		grow = true,
		gap = "none",
		ui.row {
			class = "card-head",
			align = "center",
			ui.icon { "inbox" },
			ui.row { grow = true, gap = "none", ui.text { "Inbox", class = "card-title" } },
			ui.text { `{#list}`, class = "muted small" },
		},
		ui.row {
			padding = "normal",
			ui.row {
				class = "search-box",
				grow = true,
				align = "center",
				ui.icon { "plus", class = "muted" },
				ui.input { id = `inbox-{state.round}`, placeholder = "Write anything…", submit = { capture = "inbox" }, grow = true, class = "bare-input" },
			},
		},
		#rows == 0 and ui.column { padding = "large", ui.text { "Empty. Capture anything above.", class = "muted" } },
		ui.column { grow = true, scroll = true, gap = "small", table.unpack(rows) },
		picked and ui.column {
			padding = "normal",
			gap = "small",
			border = "top",
			ui.text { "Review", class = "section-label" },
			ui.row {
				gap = "small",
				wrap = true,
				ui.button { "Keep", icon = "arrow-up", small = true, msg = { review = "keep" }, tooltip = "Back to the top" },
				ui.button { "Note", icon = "file-text", small = true, msg = { review = "note" }, tooltip = "A note of its own" },
				ui.button { "Task", icon = "square-check", small = true, msg = { review = "task" }, tooltip = "To today, as a task" },
				ui.button { "Done", icon = "check", small = true, variant = "ghost", msg = { review = "drop" }, tooltip = "Remove it" },
				ui.button { "Let sink", small = true, variant = "ghost", msg = "sink", tooltip = "Leave it; look at the next one" },
			},
		},
	}
end

return mod {
	title = "Journal",

	init = function(previous: State?): (State, { any })
		local state: State = previous or { views = {}, round = 1 }
		state.views = {}
		return state, watches()
	end,

	update = function(state: State, msg)
		if msg == "sink" then
			state.pick = math.max(1, (state.pick or 1) - 1)
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.view then
			state.views[msg.view] = msg.data
			return
		end
		if msg.reply ~= nil then
			return
		end
		if msg.pick then
			state.pick = msg.pick
		elseif msg.capture and type(msg.value) == "string" and string.match(msg.value, "%S") then
			state.round += 1
			return state, { { to = "provider:notes", msg = { capture = { text = msg.value, to = msg.capture }, id = "capture" } } }
		elseif msg.review then
			local list = items((state.views.inbox or {}).text or "")
			local index = state.pick or #list
			local item = list[index]
			if item then
				-- Next, the item above this one: it moves down a place when this
				-- one rises to the top, and stays when this one leaves.
				if msg.review == "keep" then
					state.pick = index
				else
					state.pick = if index > 1 then index - 1 else nil
				end
				return state, { { to = "provider:notes", msg = { inbox = { op = msg.review, line = item.line, text = item.text }, id = "review" } } }
			end
		end
	end,

	view = function(state: State, app)
		local notes = app.notes
		if not notes or not notes.available then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { (notes and notes.reason) or "Opening the vault…", class = "muted" } }
		end
		return ui.hsplit {
			id = "journal",
			divider = false,
			ui.pane { size = 64, fixed = true, rail() },
			ui.pane { class = "card-pane", ui.column { grow = true, gap = "none", ui.row { height = 12, drag = true, shrink = false }, journal(state) } },
			ui.pane { size = 360, fixed = true, class = "card-pane-last", ui.column { grow = true, gap = "none", ui.row { height = 12, drag = true, shrink = false }, inbox(state) } },
		}
	end,
}
