-- The editor card: tabs of the files opened from the file tree or the
-- changes, and the chosen one's code, highlighted, with line numbers. The
-- projects provider reads one file at a time; choosing a tab asks for it.

local file_icon = use "file-icon"
local code = use "code"

type State = {
	tabs: { string },
	selected: string?,
	limit: number,
}

local MOST_TABS = 10

local function read(path: string?)
	return { { to = "provider:projects", msg = { read = if path then path else false } } }
end

return mod {
	region = "main",
	title = "Editor",

	styles = {
		[".editor-tabs"] = { padding = { 0, 8, 0, 10 }, gap = 2, border = "bottom", border_color = "border" },
		[".editor-tab"] = { padding = { 5, 6, 5, 10 }, radius = 8, gap = 6, color = "muted-foreground" },
		[".editor-tab:hover"] = { background = "list-hover" },
		[".editor-tab:selected"] = { color = "foreground" },
		[".editor-tab-name"] = { weight = "medium" },
		[".editor-empty"] = { gap = 6 },
	},

	init = function(previous: State?): State
		return previous or { tabs = {}, selected = nil, limit = 400 }
	end,

	update = function(state: State, msg)
		if msg == "more" then
			state.limit += 1000
			return
		elseif msg == "close-current" then
			if not state.selected then
				return
			end
			msg = { close = state.selected }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.open then
			if not table.find(state.tabs, msg.open) then
				table.insert(state.tabs, msg.open)
				if #state.tabs > MOST_TABS then
					table.remove(state.tabs, 1)
				end
			end
			state.selected = msg.open
			state.limit = 400
			return state, read(msg.open)
		elseif msg.tab then
			state.selected = msg.tab
			state.limit = 400
			return state, read(msg.tab)
		elseif msg.close then
			local index = table.find(state.tabs, msg.close)
			if index then
				table.remove(state.tabs, index)
			end
			if state.selected == msg.close then
				state.selected = state.tabs[math.min(index or 1, #state.tabs)]
				return state, read(state.selected)
			end
		elseif msg.project then
			state.tabs = {}
			state.selected = nil
			return state, read(nil)
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local file = projects and projects.file

		local tabs = {}
		for _, path in state.tabs do
			local name = string.match(path, "([^/]+)$") or path
			table.insert(tabs, ui.row {
				class = "editor-tab",
				selected = path == state.selected,
				align = "center",
				msg = { tab = path },
				tooltip = path,
				shrink = false,
				file_icon { name = name },
				ui.text { name, class = "editor-tab-name" },
				ui.button { icon = "close", variant = "ghost", small = true, msg = { close = path } },
			})
		end

		local body
		if not state.selected then
			body = ui.column {
				class = "editor-empty",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "file-text", size = "title", tone = "muted" },
				ui.text { "Open a file from the tree", tone = "muted" },
			}
		elseif not file or file.path ~= state.selected then
			body = ui.text { "Reading…", tone = "muted", style = { padding = 16 } }
		elseif file.missing then
			body = ui.text { "This file is gone.", tone = "muted", style = { padding = 16 } }
		elseif file.binary then
			body = ui.text { "A binary file; nothing to show.", tone = "muted", style = { padding = 16 } }
		else
			body = code {
				lines = file.lines,
				path = file.path,
				limit = state.limit,
				total = file.total,
				more = "more",
			}
		end

		return ui.column {
			class = "card",
			grow = true,
			gap = "none",
			ui.row {
				class = "editor-tabs",
				drag = true,
				height = 50,
				align = "center",
				ui.row { grow = true, scroll = "x", gap = "small", align = "center", table.unpack(tabs) },
			},
			body,
		}
	end,
}
