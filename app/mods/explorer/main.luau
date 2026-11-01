-- The project's files as a tree in the left sidebar, as in an editor:
-- folders first, chevrons and folder icons, file icons by type, and
-- git-ignored entries dimmed. The `files` provider lists the root and each
-- open folder; opening one tells it which to list. Choosing a file tells the
-- editor and the inspector, by returning sends from `update`.

-- Icon and color for a file, by its whole name first, then its extension.
local BY_NAME = {
	["Cargo.toml"] = { "settings-2", "#dea584" },
	["Cargo.lock"] = { "lock", "#dea584" },
	["LICENSE"] = { "file-text", "#d4c55c" },
	["README.md"] = { "book-open", "#519aba" },
	["justfile"] = { "terminal", "#89e051" },
	[".gitignore"] = { "git-branch", "#f14e32" },
}
local BY_EXTENSION = {
	luau = { "moon", "#5b8cff" },
	lua = { "moon", "#5b8cff" },
	rs = { "code", "#dea584" },
	toml = { "settings", "#9da5b4" },
	lock = { "lock", "#9da5b4" },
	md = { "book-open", "#519aba" },
	json = { "code", "#cbcb41" },
	sh = { "terminal", "#89e051" },
	swift = { "code", "#f05138" },
	wit = { "plug", "#7fd3ed" },
	png = { "frame", "#a074c4" },
	jpg = { "frame", "#a074c4" },
	svg = { "frame", "#a074c4" },
	icns = { "frame", "#a074c4" },
	yml = { "settings", "#cb4b16" },
	yaml = { "settings", "#cb4b16" },
}
local FILE = { "file", "#9da5b4" }
local FOLDER = "#dcb67a"
-- Pixels each level of the tree steps in.
local INDENT = 12
-- Each row's height: the tree is a `ui.list`, which builds only the rows
-- that show, so a large project costs what a screenful does.
local ROW_HEIGHT = 24

type State = {
	-- Open folders, by path from the root.
	open: { [string]: boolean? },
	-- The root folded away.
	collapsed: boolean,
	-- The chosen file, by path.
	selected: string?,
}

local function file_icon(name: string)
	local known = BY_NAME[name]
	if known then
		return known
	end
	local extension = string.match(name, "%.([%w]+)$")
	return extension and BY_EXTENSION[string.lower(extension)] or FILE
end

-- The explorer's box: a panel in the sidebar; in a tab, which frames it
-- already (`props.framed`), a bare column, so no second border. The list
-- in it scrolls itself.
local function frame(props: any, children: { any })
	if props and props.framed then
		return ui.column { grow = true, gap = "none", padding = "small", table.unpack(children) }
	end
	return ui.panel {
		title = "Explorer",
		grow = true,
		gap = "none",
		padding = "small",
		table.unpack(children),
	}
end

-- One line of the tree, as data: a file or folder at a depth, or a folder
-- the provider has not listed yet.
type Line = {
	path: string,
	depth: number,
	entry: any?,
	open: boolean?,
}

-- Draws one line of the tree.
local function line(state: State, at: Line)
	local entry = at.entry
	if not entry then
		return ui.row {
			class = "tree-row",
			ui.row { width = INDENT * at.depth + 16 },
			ui.text { "…", tone = "muted", size = "small" },
		}
	end
	local icon, color, lead
	if entry.dir then
		icon, color = if at.open then "folder-open" else "folder", FOLDER
		lead = ui.icon {
			name = if at.open then "chevron-down" else "chevron-right",
			class = "tree-chevron",
			size = "small",
		}
	else
		local known = file_icon(entry.name)
		icon, color = known[1], known[2]
		-- Where a folder's chevron goes, so names line up.
		lead = ui.row { width = 14 }
	end
	local classes = { "tree-row" }
	if at.path == state.selected then
		table.insert(classes, "tree-selected")
	end
	if entry.ignored then
		table.insert(classes, "tree-ignored")
	end
	return ui.row {
		class = classes,
		align = "center",
		msg = if entry.dir then { toggle = at.path } else { open = at.path },
		ui.row { width = INDENT * at.depth },
		lead,
		ui.icon { name = icon :: any, style = { color = color } },
		ui.text { entry.name, truncate = true },
	}
end

-- The open folders, one path per line: what the provider lists.
local function open_list(open: { [string]: boolean? }): string
	local paths = {}
	for path in open do
		table.insert(paths, path)
	end
	table.sort(paths)
	return table.concat(paths, "\n")
end

return mod {
	region = "left",
	title = "Explorer",
	-- Offered by tab bars' + menu: each tab is an explorer of its own.
	tab = { title = "Files", icon = "folder" },

	styles = {
		[".tree-row"] = { padding = { 2, 6 }, radius = 4, gap = 4 },
		[".tree-row:hover"] = { background = "list-hover" },
		[".tree-selected"] = { background = "accent/15" },
		[".tree-selected:hover"] = { background = "accent/20" },
		[".tree-ignored"] = { opacity = 0.5 },
		[".tree-root"] = { padding = { 4, 6 }, radius = 4, gap = 4 },
		[".tree-root:hover"] = { background = "list-hover" },
		[".tree-root-label"] = { size = "small", weight = "bold" },
		[".tree-chevron"] = { color = "muted-foreground" },
	},

	init = function(previous: State?): State
		return {
			open = previous and previous.open or {},
			collapsed = previous and previous.collapsed or false,
			selected = previous and previous.selected,
		}
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.toggle then
			state.open[msg.toggle] = if state.open[msg.toggle] then nil else true
			return state, { { to = "provider:files", msg = { open = open_list(state.open) } } }
		elseif msg.collapse ~= nil then
			state.collapsed = not state.collapsed
		elseif msg.open then
			state.selected = msg.open
			return state, {
				{ to = "editor", msg = { open = msg.open } },
				{ to = "inspector", msg = { select = msg.open } },
			}
		end
	end,

	view = function(state: State, app, props)
		local files = app.files
		if not files then
			return frame(props, { ui.text { "Loading files…", tone = "muted" } })
		end
		if not files.available then
			return frame(props, { ui.text { files.reason or "No files to show.", tone = "muted" } })
		end

		-- The open part of the tree, flat and in order; only the lines that
		-- show are drawn.
		local lines: { Line } = {}
		local function add(dir: string, depth: number)
			local entries = files.dirs[dir]
			if not entries then
				-- Opened, and the provider has not listed it yet.
				table.insert(lines, { path = dir, depth = depth })
				return
			end
			for _, entry in entries do
				local path = if dir == "" then entry.name else `{dir}/{entry.name}`
				local open = entry.dir and state.open[path] or false
				table.insert(lines, { path = path, depth = depth, entry = entry, open = open })
				if open then
					add(path, depth + 1)
				end
			end
		end

		local root = ui.row {
			class = "tree-root",
			align = "center",
			msg = { collapse = true },
			ui.icon {
				name = if state.collapsed then "chevron-right" else "chevron-down",
				class = "tree-chevron",
				size = "small",
			},
			ui.text { string.upper(files.root), class = "tree-root-label" },
		}
		if state.collapsed then
			return frame(props, { root })
		end
		add("", 0)
		return frame(props, {
			root,
			ui.list {
				id = "tree",
				count = #lines,
				row_height = ROW_HEIGHT,
				row = function(index)
					return line(state, lines[index])
				end,
			},
		})
	end,
}
