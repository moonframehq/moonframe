-- Commits as a graph: a line per branch running down the left, a dot on each
-- commit, merges and forks joining the lines, and each commit's subject and
-- refs beside its dot. Feed it `app.git.log`, after sending `provider:git`
-- `{ log = 200 }`.

type Commit = {
	sha: string,
	short: string?,
	parents: { string },
	subject: string?,
	author: string?,
	refs: { string }?,
}

type Props = {
	commits: { Commit },
	-- Sent with `sha` added as a commit is clicked.
	select: any,
	selected: string?,
	row_height: number?,
}

local LANE = 14
local ROW = 26
local DOT = 2.5
-- Lines past this share the last lane: a graph wider than this says nothing.
local MAX_LANES = 8
-- Refs shown on a row, so its subject keeps the room.
local MAX_REFS = 2
-- The most lines the host takes in one `ui.paths`.
local MAX_PATHS = 500
local COLORS = { "#6366f1", "#14b8a6", "#f59e0b", "#ec4899", "#3b82f6", "#22c55e", "#ef4444", "#8b5cf6" }

local function color_of(lane: number): string
	return COLORS[(lane - 1) % #COLORS + 1]
end

-- The first lane nothing runs in.
local function free_lane(lanes: { [number]: string }): number
	for lane = 1, MAX_LANES do
		if lanes[lane] == nil then
			return lane
		end
	end
	return MAX_LANES
end

-- The lane already heading for `sha`, if any.
local function lane_to(lanes: { [number]: string }, sha: string): number?
	for lane = 1, MAX_LANES do
		if lanes[lane] == sha then
			return lane
		end
	end
	return nil
end

-- Gives each commit a lane, and each line from a commit to a parent the lane
-- it runs down. A first parent carries on in its child's lane.
local function layout(commits: { Commit })
	local lanes: { [number]: string } = {}
	local placed: { [string]: { row: number, lane: number } } = {}
	local edges = {}
	local widest = 1
	for row, commit in commits do
		local lane = lane_to(lanes, commit.sha) or free_lane(lanes)
		placed[commit.sha] = { row = row, lane = lane }
		widest = math.max(widest, lane)
		-- Every line heading here ends here.
		for other = 1, MAX_LANES do
			if lanes[other] == commit.sha then
				lanes[other] = nil
			end
		end
		lanes[lane] = nil
		for index, parent in commit.parents do
			local next_lane = if index == 1 and lanes[lane] == nil then lane else free_lane(lanes)
			local through: number = lane_to(lanes, parent) or next_lane
			lanes[through] = parent
			widest = math.max(widest, through)
			table.insert(edges, { row = row, lane = lane, through = through, parent = parent })
		end
	end
	return placed, edges, widest
end

local function x_of(lane: number): number
	return (lane - 0.5) * LANE
end

-- The line of one edge: out of the commit into its lane on the next row, down
-- it, and into the parent's lane on the last row. A parent older than the
-- commits shown runs off the bottom.
local function line_of(edge, placed, row_height: number, rows: number): any
	local function y_of(row: number): number
		return (row - 0.5) * row_height
	end
	local points = { x_of(edge.lane), y_of(edge.row) }
	local function point(x: number, y: number)
		table.insert(points, x)
		table.insert(points, y)
	end
	local parent = placed[edge.parent]
	local last = if parent then parent.row else rows + 1
	if edge.through ~= edge.lane then
		point(x_of(edge.through), y_of(edge.row + 1))
	end
	if parent and parent.lane ~= edge.through then
		if last - 1 > edge.row then
			point(x_of(edge.through), y_of(last - 1))
		end
		point(x_of(parent.lane), y_of(last))
	else
		point(x_of(edge.through), if parent then y_of(last) else rows * row_height)
	end
	return { points = points, color = color_of(edge.through), stroke = 1.5, radius = 4 }
end

-- A filled dot, as a tight ring: `ui.paths` draws lines.
local function dot_of(lane: number, y: number): any
	local x = x_of(lane)
	local points = {}
	for step = 0, 8 do
		local angle = step * math.pi / 4
		table.insert(points, x + math.cos(angle) * DOT)
		table.insert(points, y + math.sin(angle) * DOT)
	end
	return { points = points, color = color_of(lane), stroke = 3 }
end

-- A commit's refs worth showing: `origin/main` says nothing beside `main`.
local function shown_refs(refs: { string }): { string }
	local local_names = {}
	for _, name in refs do
		local_names[name] = true
	end
	local shown = {}
	for _, name in refs do
		local short = string.match(name, "^[^/]+/(.+)$")
		if not (short and local_names[short]) and name ~= "origin/HEAD" then
			table.insert(shown, name)
		end
	end
	return shown
end

-- The lines as layers over one another, each as many as a `ui.paths` takes:
-- a commit draws a dot and a line or more, so 250 commits fill one.
local function layers_of(lines: { any }, width: number, height: number): { any }
	local layers = {}
	for first = 1, #lines, MAX_PATHS do
		local last = math.min(first + MAX_PATHS - 1, #lines)
		table.insert(
			layers,
			ui.row {
				float = { top = 0, left = 0 },
				width = width,
				height = height,
				ui.paths { paths = table.move(lines, first, last, 1, {}), width = width, height = height },
			}
		)
	end
	return layers
end

local function row_of(commit: Commit, props: Props, row_height: number)
	local cells: { any } = {}
	local refs = shown_refs(commit.refs or {})
	for index, name in refs do
		if index > MAX_REFS then
			table.insert(cells, ui.text { `+{#refs - MAX_REFS}`, class = "commit-graph-meta" })
			break
		end
		table.insert(cells, ui.text { name, class = "commit-graph-ref", truncate = true })
	end
	table.insert(cells, ui.row { grow = true, ui.text { commit.subject or "", truncate = true } })
	table.insert(cells, ui.text { commit.author or "", class = "commit-graph-meta", truncate = true })
	table.insert(cells, ui.text { commit.short or "", class = "commit-graph-meta commit-graph-sha" })
	local message: any = nil
	if props.select ~= nil then
		message = if type(props.select) == "table" then table.clone(props.select) else { select = props.select }
		message.sha = commit.sha
	end
	return ui.row {
		key = commit.sha,
		class = if commit.sha == props.selected then "commit-graph-row commit-graph-selected" else "commit-graph-row",
		align = "center",
		gap = "small",
		height = row_height,
		msg = message,
		table.unpack(cells),
	}
end

local EXAMPLE: { any } = {
	{ sha = "f", short = "a41f09c", parents = { "e", "d" }, subject = "Merge pull request #12 from worktree-actions", author = "Ada", refs = { "main", "origin/main" } },
	{ sha = "d", short = "9be2d11", parents = { "b" }, subject = "Let mods add and remove worktrees", author = "Ada", refs = { "worktree-actions" } },
	{ sha = "e", short = "71cc0de", parents = { "c" }, subject = "Log how close each module comes to its limits", author = "Grace" },
	{ sha = "c", short = "0d4a7f2", parents = { "b" }, subject = "Name terminal tabs after their folder", author = "Grace" },
	{ sha = "b", short = "e93b120", parents = { "a" }, subject = "Release 0.2.2", author = "Ada", refs = { "v0.2.2" } },
	{ sha = "a", short = "3fa8c55", parents = {}, subject = "Start the app", author = "Ada" },
}

return component {
	title = "Commit graph",
	category = "Developer",
	icon = "git-branch",
	description = "Commits as a graph: a line per branch, a dot on each commit, merges and forks joining the lines, with each commit's subject, refs, author and short id.",
	when = "A repository's history with its branches. For a file's changes use `diff-view`, for a timeline of events `timeline`.",
	props = {
		["commits"] = "`{ sha, parents, short?, subject?, author?, refs? }`, children before parents, as `app.git.log` gives them",
		["select"] = "message sent with `sha` added as a commit is clicked; a string arrives as `{ select = <it>, sha = ... }`",
		["selected"] = "the sha shown as selected",
		["row_height"] = "pixels per commit; 26 by default",
	},
	example = { commits = EXAMPLE },

	styles = {
		[".commit-graph-row"] = { radius = 4, padding = { 0, 6 } },
		[".commit-graph-selected"] = { background = "accent/15" },
		[".commit-graph-meta"] = { size = "small", color = "muted-foreground" },
		[".commit-graph-sha"] = { font = "mono" },
		[".commit-graph-ref"] = {
			size = "small",
			color = "accent",
			background = "accent/12",
			radius = 4,
			padding = { 0, 5 },
		},
	},

	build = function(props: Props)
		local commits: { Commit } = props.commits or {}
		local row_height = props.row_height or ROW
		local placed, edges, widest = layout(commits)
		local lines = {}
		for _, edge in edges do
			table.insert(lines, line_of(edge, placed, row_height, #commits))
		end
		local rows = {}
		for row, commit in commits do
			local spot = placed[commit.sha]
			table.insert(lines, dot_of(spot.lane, (row - 0.5) * row_height))
			table.insert(rows, row_of(commit, props, row_height))
		end
		local width, height = widest * LANE, #commits * row_height
		return ui.row {
			align = "start",
			gap = "none",
			grow = true,
			ui.column { gap = "none", width = width, height = height, table.unpack(layers_of(lines, width, height)) },
			ui.column { gap = "none", grow = true, table.unpack(rows) },
		}
	end,
}
