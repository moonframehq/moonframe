-- The card on the right: the open session's folder as git sees it, and a
-- terminal in it. Files Changed comes from the `projects` provider (the
-- projects package), which opens the session's folder when it is one of
-- the git repositories under the user's project folders; picking a file
-- shows it below the tree.

local tabs = use "tabs"
local menu = use "menu"
local icon_button = use "icon-button"
local segmented = use "segmented"
local diff_counts = use "diff-counts"
local change_tree = use "change-tree"
local empty_state = use "empty-state"
local code = use "code"
local terminal = use "terminal"

type State = {
	tab: string,
	-- The open session and its folder, from `cairn/sessions`.
	current: string?,
	folder: string?,
	picked: string?,
	folded: { [string]: boolean },
	-- Sessions whose terminal is open.
	shells: { [string]: boolean },
	adding: boolean,
	limit: number,
}

local function current_session(list: { any }, id: string?): any
	for _, session in list do
		if session.id == id then
			return session
		end
	end
	return nil
end

-- The projects provider's open project, when it is the session's folder.
local function project(app, folder: string?): any
	local current = app.projects and app.projects.current
	if current and folder and current.path == folder then
		return current
	end
	return nil
end

local function files_tab(state: State, app)
	if not state.folder then
		return empty_state { icon = "list-tree", title = "No changes", message = "Open a session to see what changed in its folder." }
	end
	local current = project(app, state.folder)
	if not current then
		if app.projects == nil then
			return empty_state { icon = "loader-circle", title = "Looking…" }
		end
		return empty_state {
			icon = "folder-git-2",
			title = "Not a project",
			message = `Changes show for git repositories under your project folders (~/code, ~/dev, ~/src, ~/Projects, ~/work, or MOONFRAME_PROJECTS). {state.folder} is not one.`,
		}
	end
	local header = ui.column { class = "bench-tree", shrink = false,
		ui.row { segmented {
			grow = true,
			event = "scope",
			selected = current.scope,
			options = {
				{ id = "uncommitted", label = "Uncommitted" },
				{ id = "branch", label = "Branch" },
				{ id = "last", label = "Last Commit" },
			},
		} },
		ui.row { class = "bench-summary", align = "center",
			ui.text { `{current.count} {if current.count == 1 then "file" else "files"}`, tone = "muted" },
			diff_counts { added = current.added, removed = current.deleted, size = "normal" },
		},
	}
	if current.count == 0 then
		return ui.column { grow = true, gap = "none", header, empty_state { icon = "circle-check", title = "Nothing changed" } }
	end
	local file = app.projects.file
	local shown
	if not state.picked then
		shown = ui.text { "Pick a file to read it.", tone = "muted" }
	elseif not file or file.path ~= state.picked then
		shown = ui.text { "Reading…", tone = "muted" }
	elseif file.missing then
		shown = ui.text { "The file is gone: it was deleted.", tone = "muted" }
	elseif file.binary then
		shown = ui.text { "A binary file.", tone = "muted" }
	else
		shown = code { lines = file.lines, path = file.path, limit = state.limit, total = file.total, more = "more" }
	end
	return ui.column { grow = true, gap = "none",
		header,
		ui.column { class = "bench-list", scroll = true, max_height = 280, shrink = false,
			change_tree { files = current.files, selected = state.picked, counts = true, plain = true, folded = state.folded, pick = "file" },
			current.truncated and ui.text { `and {current.count - #current.files} more`, tone = "muted", size = "small" },
		},
		ui.column { class = "bench-file", grow = true, scroll = true, shown },
	}
end

return mod {
	subscribe = { "cairn/sessions" },

	styles = {
		[".bench-head"] = { padding = { 7, 10, 7, 12 } },
		[".bench-tree"] = { padding = { 4, 10, 6, 10 }, gap = 8 },
		[".bench-summary"] = { padding = { 0, 4 }, gap = 8 },
		[".bench-list"] = { padding = { 0, 8, 8, 8 }, border = "bottom", border_color = "border" },
		[".bench-file"] = { padding = { 10, 10 } },
	},

	init = function(previous: any): State
		local state = previous or { tab = "files", folded = {}, shells = {} }
		if state.tab ~= "files" and state.tab ~= "terminal" then
			state.tab = "files"
		end
		state.shells = state.shells or {}
		state.folded = state.folded or {}
		state.adding = false
		state.limit = 400
		-- Opened again in the projects provider at the first `cairn/sessions`.
		state.folder = nil
		state.current = nil
		state.picked = nil
		-- Gone with the sample sessions and the greeter.
		state.closed = nil
		state.collapsed = nil
		state.greeter = nil
		state.name = nil
		state.doc = nil
		state.shell = nil
		return state
	end,

	update = function(state: State, msg)
		if msg == "add" then
			state.adding = not state.adding
		elseif msg == "add-close" then
			state.adding = false
		elseif msg == "more" then
			state.limit += 1000
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "cairn/sessions" and msg.value then
			local session = current_session(msg.value.list or {}, msg.value.current)
			local folder = session and session.folder ~= "" and session.folder or nil
			state.current = session and session.id
			if folder ~= state.folder then
				state.folder = folder
				state.picked = nil
				state.folded = {}
				if folder then
					return state, { { to = "provider:projects", msg = { select = folder, read = false } } }
				end
			end
		elseif msg.select or msg.tab then
			state.tab = msg.select or msg.tab
			if state.tab == "terminal" and state.current then
				state.shells[state.current] = true
			end
			return state, { { to = "shell", msg = { show_right = true } } }
		elseif msg.scope then
			return state, { { to = "provider:projects", msg = { scope = msg.scope } } }
		elseif msg.file then
			state.picked = msg.file
			state.limit = 400
			return state, { { to = "provider:projects", msg = { read = msg.file } } }
		elseif msg.open_shell then
			if state.current then
				state.shells[state.current] = true
				state.tab = "terminal"
			end
			state.adding = false
		elseif msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold]
		elseif msg.close == "terminal" then
			if state.current then
				state.shells[state.current] = nil
			end
			state.tab = "files"
		end
	end,

	view = function(state: State, app)
		local current = project(app, state.folder)
		local count = if current then current.count else 0
		local shell = state.current ~= nil and state.shells[state.current] == true
		local strip: { any } = { { id = "files", label = "Files Changed", icon = "list-tree", badge = if count > 0 then count else nil, closable = false } }
		local add_items: { any } = {}
		if shell then
			table.insert(strip, { id = "terminal", label = "Terminal", icon = "terminal" })
		elseif state.folder then
			table.insert(add_items, { id = "terminal", label = "Terminal", icon = "terminal", msg = { open_shell = true } })
		end

		local body
		if state.tab == "terminal" and shell and state.folder then
			body = ui.column { grow = true, terminal { id = `bench-{state.current}`, cwd = state.folder } }
		else
			body = files_tab(state, app)
		end

		return ui.column { grow = true, gap = "none",
			ui.row { class = "bench-head", align = "center", shrink = false,
				ui.row { grow = true, width = 0, scroll = "x", gap = "none",
					ui.row { shrink = false, gap = "none",
						tabs {
							variant = "pills", selected = if shell then state.tab else "files", tabs = strip,
							add = if #add_items > 0 then "add" else nil,
							add_menu = { open = state.adding, dismiss = "add-close", width = 180, content = menu { items = add_items } },
						},
					},
				},
				icon_button { icon = "panel-right", tooltip = "Hide the workbench", msg = "toggle-right", to = "shell" },
			},
			body,
		}
	end,
}
