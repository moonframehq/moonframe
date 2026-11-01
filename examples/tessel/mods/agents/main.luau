-- The Agents page. A bar with the page's name, "Create an Agent" and a ···
-- menu; under it two big tabs:
--
-- - My Agents: sort, filter by tag, find, and cards or a table of your
--   agents, then the ones to browse with a Launch button. A card opens the
--   agent's page.
-- - Agent Hub: the runs' figures (filtered by agent), then the latest
--   activity. A row folds open to its steps, newest first; a failed step
--   offers Re-run and Manage workflow, which opens the workflow modal.
--
-- Re-runs and fixes are shared with the modal on the `tessel/runs` topic:
-- `{ [run id] = "rerun" | "resolved" }`.

local pixel_icon = use "pixel-icon"
local dropdown = use "dropdown"
local segmented = use "segmented"
local chips = use "chips"
local menu = use "menu"
local dialog = use "dialog"

type State = {
	tab: string,
	layout: string,
	sort: string,
	sort_open: boolean,
	filters_open: boolean,
	tags: { string },
	finding: boolean,
	query: string,
	menu_open: boolean,
	create_open: boolean,
	create_name: string,
	create_description: string,
	created: { any },
	agent_filter: string,
	agent_open: boolean,
	status_filter: string,
	status_open: boolean,
	range: string,
	range_open: boolean,
	expanded: { [string]: any },
	runs: { [string]: string },
}

-- How each run status reads: label, icon, colour.
local STATUS: { [string]: { label: string, icon: string, color: string } } = {
	active = { label = "Active", icon = "play", color = "run-active" },
	completed = { label = "Completed", icon = "circle-check", color = "run-completed" },
	review = { label = "Needs review", icon = "eye", color = "run-review" },
	pending = { label = "Pending", icon = "pause", color = "run-pending" },
	failed = { label = "Failed", icon = "circle-x", color = "run-failed" },
}

-- The figures across the hub, in the order of each agent's `stats`.
local KPIS = {
	{ id = "active", label = "Active", icon = "circle-play", color = "run-active", note = "+2 today", up = true },
	{ id = "completed", label = "Completed", icon = "circle-check", color = "run-completed", note = "+12 this month", up = true },
	{ id = "review", label = "Needs Review", icon = "eye", color = "run-review", note = "2 urgent", up = false },
	{ id = "pending", label = "Pending", icon = "pause", color = "run-pending", note = "+2 today", up = true },
	{ id = "failed", label = "Failed", icon = "circle-x", color = "run-failed", note = "1 today", up = true },
}

local SORTS = {
	{ id = "used", label = "Latest used" },
	{ id = "name", label = "Name" },
	{ id = "owner", label = "Owner" },
}

local RANGES = {
	{ id = "7", label = "Last 7 days" },
	{ id = "30", label = "Last 30 days" },
	{ id = "90", label = "Last 90 days" },
}

local WIDTH = 796
local CARD = 257

-- A made-up sprite for an agent you create, from its name.
local function sprite_for(name: string): any
	local palettes = {
		{ "#fde68a", "#7c2d12", "#f97316" },
		{ "#bae6fd", "#1e3a8a", "#f472b6" },
		{ "#d9f99d", "#14532d", "#a855f7" },
		{ "#fecdd3", "#4c0519", "#0ea5e9" },
	}
	local seed = 7
	for i = 1, #name do
		seed = (seed * 31 + string.byte(name, i)) % 100003
	end
	local rows = {}
	for _ = 1, 8 do
		local line = ""
		for _ = 1, 4 do
			seed = (seed * 1103 + 12345) % 100003
			local n = seed % 5
			line ..= if n < 2 then "0" elseif n < 4 then "1" else "2"
		end
		table.insert(rows, line)
	end
	return { pattern = rows, colors = palettes[seed % #palettes + 1] }
end

local function publish_runs(state: State)
	return { { to = "topic:tessel/runs", msg = state.runs } }
end

return mod {
	title = "Agents",

	subscribe = { "tessel/runs" },

	styles = {
		[".top"] = { padding = { 0, 14, 0, 16 }, gap = 8 },
		[".top-title"] = { weight = "medium" },
		[".more"] = { radius = 8, padding = { 5, 7 }, color = "muted-foreground" },
		[".more:hover"] = { background = "list-hover", color = "foreground" },
		[".page"] = { padding = { 22, 0, 48, 0 }, gap = 0 },
		[".big-tabs"] = { gap = 24, padding = { 0, 0, 22, 0 } },
		[".big-tab"] = { size = "title", weight = "medium", color = "#b9b9b7" },
		[".big-tab:hover"] = { color = "muted-foreground" },
		[".big-tab:selected"] = { color = "foreground" },
		[".tools"] = { gap = 8, padding = { 0, 0, 12, 0 } },
		[".ghost"] = { radius = 7, padding = { 4, 8 }, gap = 5, color = "muted-foreground" },
		[".ghost:hover"] = { background = "list-hover", color = "foreground" },
		[".find"] = { border = true, border_color = "border", radius = 7, padding = { 0, 8 }, gap = 6, background = "card" },
		[".find-icon"] = { color = "muted-foreground" },
		[".section-title"] = { size = "large", weight = "medium", padding = { 34, 0, 12, 0 } },
		[".fade"] = { transition = "fade normal ease-out" },
		-- Cards
		[".grid"] = { gap = 12 },
		[".grid-row"] = { gap = 12 },
		[".card"] = { background = "card", border = true, border_color = "border", radius = 9, gap = 0 },
		[".card:hover"] = { border_color = "#d6d6d4" },
		[".card-body"] = { padding = { 10, 10, 8, 10 }, gap = 2 },
		[".card-title"] = { size = "large", weight = "medium", padding = { 10, 0, 0, 0 } },
		[".card-desc"] = { color = "muted-foreground", size = "small" },
		[".card-band"] = { background = "band", border = "top", border_color = "border", radius = { 0, 0, 9, 9 }, padding = { 6, 10 }, gap = 12 },
		[".meta"] = { gap = 5, color = "muted-foreground", size = "small" },
		[".meta-strong"] = { color = "foreground", size = "small" },
		[".tag"] = { background = "secondary", radius = 5, padding = { 1, 6 }, gap = 3, size = "small", color = "foreground" },
		[".tag-hash"] = { color = "muted-foreground", size = "small" },
		[".launch"] = { background = "secondary", radius = 7, padding = { 3, 10 }, weight = "medium", size = "small" },
		[".launch:hover"] = { background = "#e6e6e4" },
		-- Table
		[".thead"] = { padding = { 0, 12, 6, 12 }, size = "small", color = "muted-foreground" },
		[".tline"] = { background = "card", border = true, border_color = "border", radius = 8, padding = { 0, 12 }, gap = 10 },
		[".tline:hover"] = { border_color = "#d6d6d4", background = "band" },
		[".tlines"] = { gap = 6 },
		-- Hub
		[".kpis"] = { gap = 8, padding = { 0, 0, 30, 0 } },
		[".kpi"] = { background = "card", border = true, border_color = "border", radius = 9, gap = 0 },
		[".kpi-top"] = { padding = { 10, 10, 4, 10 }, gap = 0 },
		[".kpi-label"] = { size = "small", color = "muted-foreground" },
		[".kpi-value"] = { size = "title", weight = "medium" },
		[".kpi-band"] = { background = "band", border = "top", border_color = "border", radius = { 0, 0, 9, 9 }, padding = { 5, 10 }, gap = 5, size = "small", color = "muted-foreground" },
		[".activity-head"] = { padding = { 0, 0, 10, 0 } },
		[".activity-title"] = { color = "muted-foreground", size = "large" },
		[".run"] = { padding = { 0, 4 }, gap = 10, radius = 8 },
		[".run:hover"] = { background = "list-hover" },
		[".run-open"] = { border = true, border_color = "border", radius = 10, background = "card", padding = { 0, 0, 12, 0 }, margin = { 2, 0, 8, 0 } },
		[".run-name"] = { weight = "medium", size = "small" },
		[".run-summary"] = { color = "muted-foreground", size = "small" },
		[".chev"] = { color = "muted-foreground" },
		[".status"] = { radius = 6, padding = { 1, 6 }, gap = 4, size = "small" },
		[".age"] = { gap = 4, size = "small", color = "muted-foreground" },
		[".steps"] = { padding = { 4, 14, 0, 26 }, gap = 0 },
		[".step"] = { gap = 10, radius = 8, padding = { 0, 8 } },
		[".step-fail"] = { background = "run-failed/8" },
		[".step-review"] = { background = "run-review/7" },
		[".step-text"] = { size = "small" },
		[".step-muted"] = { size = "small", color = "muted-foreground" },
		[".step-error"] = { size = "small", color = "run-failed" },
		[".step-line"] = { background = "border" },
		[".step-button"] = { background = "card", border = true, border_color = "border", radius = 7, padding = { 3, 9 }, gap = 5, size = "small", weight = "medium" },
		[".step-button:hover"] = { background = "band" },
		[".step-link"] = { radius = 7, padding = { 3, 9 }, gap = 5, size = "small", weight = "medium" },
		[".step-link:hover"] = { background = "list-hover" },
		[".empty"] = { padding = { 30, 0 }, color = "muted-foreground" },
		[".menu-pop"] = { padding = 4 },
	},

	init = function(previous: any): State
		local p = previous or {}
		return {
			tab = p.tab or "mine",
			layout = p.layout or "grid",
			sort = p.sort or "used",
			sort_open = false,
			filters_open = false,
			tags = p.tags or {},
			finding = false,
			query = "",
			menu_open = false,
			create_open = false,
			create_name = "",
			create_description = "",
			created = p.created or {},
			agent_filter = p.agent_filter or "all",
			agent_open = false,
			status_filter = p.status_filter or "all",
			status_open = false,
			range = p.range or "30",
			range_open = false,
			expanded = p.expanded or { r4 = true },
			runs = {},
		}
	end,

	update = function(state: State, msg)
		if type(msg) == "string" then
			if msg == "create" then
				state.create_open = true
				state.menu_open = false
			elseif msg == "close-create" then
				state.create_open = false
			elseif msg == "save-create" then
				local name = state.create_name
				if name ~= "" then
					local id = "made-" .. tostring(#state.created + 1)
					table.insert(state.created, {
						id = id,
						mine = true,
						name = name,
						description = if state.create_description ~= "" then state.create_description else "A new agent",
						used = "now",
						used_min = 0,
						owner = "me",
						tag = "New",
						icon = sprite_for(name),
					})
					state.create_open = false
					state.create_name = ""
					state.create_description = ""
					state.tab = "mine"
				end
			elseif msg == "find" then
				state.tab = "mine"
				state.finding = true
			elseif msg == "stop-find" then
				state.finding = false
				state.query = ""
			elseif msg == "toggle-sort" then
				state.sort_open = not state.sort_open
			elseif msg == "close-sort" then
				state.sort_open = false
			elseif msg == "toggle-filters" then
				state.filters_open = not state.filters_open
			elseif msg == "close-filters" then
				state.filters_open = false
			elseif msg == "toggle-menu" then
				state.menu_open = not state.menu_open
			elseif msg == "close-menu" then
				state.menu_open = false
			elseif msg == "toggle-agent" then
				state.agent_open = not state.agent_open
			elseif msg == "close-agent" then
				state.agent_open = false
			elseif msg == "toggle-status" then
				state.status_open = not state.status_open
			elseif msg == "close-status" then
				state.status_open = false
			elseif msg == "toggle-range" then
				state.range_open = not state.range_open
			elseif msg == "close-range" then
				state.range_open = false
			end
			return state
		end
		if type(msg) ~= "table" then
			return state
		end
		if msg.topic == "tessel/runs" then
			state.runs = if type(msg.value) == "table" then msg.value else {}
		elseif msg.tab then
			state.tab = msg.tab
		elseif msg.layout then
			state.layout = msg.layout
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "create-name" then
			state.create_name = msg.value
		elseif msg.msg == "create-description" then
			state.create_description = msg.value
		elseif msg.sort then
			state.sort = msg.sort
			state.sort_open = false
		elseif msg.tag then
			local kept = {}
			local found = false
			for _, tag in state.tags do
				if tag == msg.tag then
					found = true
				else
					table.insert(kept, tag)
				end
			end
			if not found then
				table.insert(kept, msg.tag)
			end
			state.tags = kept
		elseif msg.clear_tags then
			state.tags = {}
			state.filters_open = false
		elseif msg.agent_pick then
			state.agent_filter = msg.agent_pick
			state.agent_open = false
		elseif msg.status_pick then
			state.status_filter = msg.status_pick
			state.status_open = false
		elseif msg.range_pick then
			state.range = msg.range_pick
			state.range_open = false
		elseif msg.menu_pick then
			state.menu_open = false
		elseif msg.fold then
			state.expanded[msg.fold] = if state.expanded[msg.fold] then nil else true
		elseif msg.open then
			return state, { { to = "topic:tessel/nav", msg = { detail = msg.open } } }
		elseif msg.rerun then
			state.runs[msg.rerun] = "rerun"
			return state, publish_runs(state)
		elseif msg.manage then
			return state, { { to = "workflow", msg = { open = msg.manage } } }
		end
		return state
	end,

	view = function(state: State, app)
		local data = app.tessel
		local people = data and data.people or {}

		local agents: { any } = {}
		local by_id: { [string]: any } = {}
		if data and data.agents then
			for _, agent in data.agents do
				table.insert(agents, agent)
				by_id[agent.id] = agent
			end
		end
		for _, agent in state.created do
			table.insert(agents, agent)
			by_id[agent.id] = agent
		end

		-- ── Pieces ─────────────────────────────────────────────────────

		local function owner_face(id: string, size: number)
			local person = people[id] or { name = id, color = "#888888" }
			if person.official then
				return pixel_icon { pattern = { "0000", "0111", "0001", "0001", "0001", "0000" }, colors = { "#f2542d", "#ffffff" }, size = size }
			end
			return ui.row {
				width = size,
				height = size,
				shrink = false,
				align = "center",
				justify = "center",
				style = { background = person.color, radius = 999, color = "#ffffff" },
				ui.text { string.upper((string.match(person.name, "^" .. utf8.charpattern) or "")), size = "small", weight = "bold" },
			}
		end

		local function owner_label(id: string): string
			local person = people[id]
			if not person then
				return id
			end
			if id == "me" or person.official then
				return "by " .. person.name
			end
			return person.name
		end

		local function tag(name: string)
			return ui.row { class = "tag", align = "center", hug = true, ui.text { "#", class = "tag-hash" }, ui.text { name } }
		end

		local function used(agent: any)
			return ui.row { class = "meta", align = "center", ui.icon { name = "clock", size = "small" }, ui.text { agent.used } }
		end

		local function owner(agent: any)
			return ui.row {
				class = "meta",
				align = "center",
				owner_face(agent.owner, 16),
				ui.text { owner_label(agent.owner), class = "meta-strong" },
			}
		end

		local function card(agent: any)
			local launch = not agent.mine
			return ui.column {
				class = "card",
				width = CARD,
				msg = { open = agent.id },
				ui.column {
					class = "card-body",
					ui.row {
						align = "start",
						pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 40 },
						ui.spacer(),
						launch and ui.row { class = "launch", msg = { open = agent.id }, ui.text "Launch" },
					},
					ui.text { agent.name, class = "card-title", truncate = true },
					ui.text { agent.description, class = "card-desc", truncate = true },
				},
				ui.row {
					class = "card-band",
					align = "center",
					not launch and used(agent),
					owner(agent),
					tag(agent.tag),
				},
			}
		end

		local function line(agent: any)
			local launch = not agent.mine
			return ui.row {
				class = "tline",
				key = agent.id,
				height = 58,
				align = "center",
				msg = { open = agent.id },
				pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 40 },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { agent.name, weight = "medium", truncate = true },
					ui.text { agent.description, class = "card-desc", truncate = true },
				},
				ui.row { width = 130, tag(agent.tag) },
				ui.row { width = 130, owner(agent) },
				ui.row {
					width = 70,
					justify = "end",
					if launch then ui.row { class = "launch", msg = { open = agent.id }, ui.text "Launch" } else used(agent),
				},
			}
		end

		local function thead(last: string)
			return ui.row {
				class = "thead",
				gap = "normal",
				ui.row { grow = true, ui.text "Agent name" },
				ui.row { width = 130, ui.text "Type" },
				ui.row { width = 130, ui.text "By" },
				ui.row { width = 70, justify = "end", ui.text { last } },
			}
		end

		local function grid(list: { any })
			local rows = {}
			local i = 1
			while i <= #list do
				local cells = {}
				for j = i, math.min(i + 2, #list) do
					table.insert(cells, card(list[j]))
				end
				table.insert(rows, ui.row { class = "grid-row", table.unpack(cells) })
				i += 3
			end
			return ui.column { class = "grid", table.unpack(rows) }
		end

		local function collection(list: { any }, last: string, key: string)
			if #list == 0 then
				return ui.row { class = "empty", ui.text "No agents match." }
			end
			if state.layout == "list" then
				local lines = {}
				for _, agent in list do
					table.insert(lines, line(agent))
				end
				return ui.column {
					key = key .. "-list",
					class = "fade",
					gap = "none",
					thead(last),
					ui.column { class = "tlines", table.unpack(lines) },
				}
			end
			return ui.column { key = key .. "-grid", class = "fade", gap = "none", grid(list) }
		end

		local function status_pill(status: string)
			local look = STATUS[status] or STATUS.pending
			return ui.row {
				class = "status",
				align = "center",
				hug = true,
				style = { color = look.color },
				ui.icon { name = look.icon :: any, size = "small" },
				ui.text { look.label },
			}
		end

		-- ── Top bar ────────────────────────────────────────────────────

		local top = ui.row {
			class = "top",
			drag = true,
			height = 56,
			shrink = false,
			align = "center",
			ui.text { "Agents", class = "top-title" },
			ui.spacer(),
			ui.row {
				class = "soft-button",
				align = "center",
				msg = "create",
				ui.icon { name = "plus", size = "small", tone = "muted" },
				ui.text { "Create an Agent", weight = "medium" },
			},
			ui.popover {
				open = state.menu_open,
				dismiss = "close-menu",
				side = "bottom",
				align = "end",
				width = 220,
				content = menu {
					event = "menu_pick",
					items = {
						{ id = "feedback", label = "Send Feedback", icon = "message-square" },
						{ id = "about", label = "About Tessel Agents", icon = "book-open" },
						{ id = "usage", label = "Trial Usage", icon = "info" },
					},
				},
				ui.row { class = "more", msg = "toggle-menu", tooltip = "More", ui.icon "ellipsis" },
			},
		}

		local tabs = ui.row {
			class = "big-tabs",
			align = "end",
			ui.row { class = "big-tab", selected = state.tab == "mine", msg = { tab = "mine" }, ui.text "My Agents" },
			ui.row { class = "big-tab", selected = state.tab == "hub", msg = { tab = "hub" }, ui.text "Agent Hub" },
		}

		local body: { any } = {}

		if state.tab == "mine" then
			-- ── My Agents ──────────────────────────────────────────────
			local tagset: { [string]: boolean } = {}
			for _, t in state.tags do
				tagset[t] = true
			end
			local all_tags: { string } = {}
			local seen: { [string]: boolean } = {}
			for _, agent in agents do
				if not seen[agent.tag] then
					seen[agent.tag] = true
					table.insert(all_tags, agent.tag)
				end
			end
			local query = string.lower(state.query)
			local mine, browse = {}, {}
			for _, agent in agents do
				local keep = (#state.tags == 0 or tagset[agent.tag])
					and (query == "" or string.find(string.lower(agent.name), query, 1, true) ~= nil)
				if keep then
					table.insert(if agent.mine then mine else browse, agent)
				end
			end
			table.sort(mine, function(a: any, b: any)
				if state.sort == "name" then
					return a.name < b.name
				elseif state.sort == "owner" then
					if a.owner == b.owner then
						return a.name < b.name
					end
					return a.owner < b.owner
				end
				return a.used_min < b.used_min
			end)

			local tag_chips = {}
			for _, t in all_tags do
				table.insert(tag_chips, { id = t, label = "# " .. t, msg = { tag = t } })
			end

			table.insert(body, ui.row {
				class = "tools",
				align = "center",
				dropdown {
					open = state.sort_open,
					toggle = "toggle-sort",
					dismiss = "close-sort",
					selected = state.sort,
					event = "sort",
					width = 180,
					options = SORTS,
				},
				ui.popover {
					open = state.filters_open,
					dismiss = "close-filters",
					side = "bottom",
					align = "start",
					width = 280,
					content = ui.column {
						class = "menu-pop",
						gap = "normal",
						ui.text { "Filter by type", size = "small", tone = "muted" },
						chips { chips = tag_chips, selected = state.tags },
						ui.row {
							ui.spacer(),
							ui.button { "Clear", small = true, variant = "ghost", msg = { clear_tags = true } },
						},
					},
					ui.row {
						class = "ghost",
						align = "center",
						msg = "toggle-filters",
						ui.icon { name = "plus", size = "small" },
						ui.text { if #state.tags > 0 then `Filters · {#state.tags}` else "Filters" },
					},
				},
				ui.spacer(),
				if state.finding
					then ui.row {
						class = "find",
						width = 200,
						height = 30,
						align = "center",
						ui.icon { name = "search", size = "small", class = "find-icon" },
						ui.input {
							id = "find",
							value = state.query,
							placeholder = "Find an agent",
							change = "query",
							keys = { escape = "stop-find" },
							focus = true,
							grow = true,
							class = "bare-input",
						},
						ui.row { class = "ghost", msg = "stop-find", tooltip = "Close", ui.icon { name = "close", size = "small" } },
					}
					else ui.row {
						class = "ghost",
						align = "center",
						msg = "find",
						ui.icon { name = "search", size = "small" },
						ui.text "Find",
					},
				segmented {
					small = true,
					event = "layout",
					selected = state.layout,
					options = {
						{ id = "grid", icon = "layout-dashboard", tooltip = "Cards" },
						{ id = "list", icon = "list", tooltip = "Table" },
					},
				},
			})
			table.insert(body, collection(mine, "Last used", "mine"))
			if #browse > 0 then
				table.insert(body, ui.text { "Browse Agents", class = "section-title" })
				table.insert(body, collection(browse, "", "browse"))
			end
		else
			-- ── Agent Hub ──────────────────────────────────────────────
			local agent_options: { any } = { { id = "all", label = "All" }, "separator" }
			for _, agent in agents do
				if agent.stats then
					table.insert(agent_options, { id = agent.id, label = agent.name })
				end
			end
			local totals = { 0, 0, 0, 0, 0 }
			for _, agent in agents do
				if agent.stats and (state.agent_filter == "all" or state.agent_filter == agent.id) then
					for i = 1, 5 do
						totals[i] += agent.stats[i] or 0
					end
				end
			end

			local agent_label = if state.agent_filter == "all"
				then "All"
				else (by_id[state.agent_filter] and by_id[state.agent_filter].name or "All")

			table.insert(body, ui.row {
				class = "tools",
				align = "center",
				dropdown {
					label = "Agent:",
					value = agent_label,
					open = state.agent_open,
					toggle = "toggle-agent",
					dismiss = "close-agent",
					selected = state.agent_filter,
					event = "agent_pick",
					width = 240,
					options = agent_options,
				},
				ui.spacer(),
				dropdown {
					icon = "calendar",
					open = state.range_open,
					toggle = "toggle-range",
					dismiss = "close-range",
					selected = state.range,
					event = "range_pick",
					align = "end",
					width = 180,
					options = RANGES,
				},
			})

			local cells = {}
			for i, kpi in KPIS do
				table.insert(cells, ui.column {
					class = "kpi",
					grow = true,
					ui.column {
						class = "kpi-top",
						ui.row {
							align = "center",
							ui.text { kpi.label, class = "kpi-label" },
							ui.spacer(),
							ui.icon { name = kpi.icon :: any, style = { color = kpi.color } },
						},
						ui.text { tostring(totals[i]), class = "kpi-value" },
					},
					ui.row {
						class = "kpi-band",
						align = "center",
						ui.text { if kpi.up then "↗" else "↘" },
						ui.text { kpi.note },
					},
				})
			end
			table.insert(body, ui.row { class = "kpis", table.unpack(cells) })

			local status_options: { any } = { { id = "all", label = "All" }, "separator" }
			for _, id in { "active", "completed", "review", "pending", "failed" } do
				table.insert(status_options, { id = id, label = STATUS[id].label, icon = STATUS[id].icon })
			end
			table.insert(body, ui.row {
				class = "activity-head",
				align = "center",
				ui.text { "Latest Activity", class = "activity-title" },
				ui.spacer(),
				dropdown {
					label = "Status:",
					value = if state.status_filter == "all" then "All" else STATUS[state.status_filter].label,
					open = state.status_open,
					toggle = "toggle-status",
					dismiss = "close-status",
					selected = state.status_filter,
					event = "status_pick",
					align = "end",
					width = 200,
					options = status_options,
				},
			})

			local function step_icon(status: string)
				if status == "done" then
					return ui.icon { name = "circle-check", size = "small", style = { color = "run-completed" } }
				elseif status == "failed" then
					return ui.icon { name = "circle-x", size = "small", style = { color = "run-failed" } }
				elseif status == "running" then
					return ui.icon { name = "loader-circle", size = "small", spin = true, tone = "muted" }
				elseif status == "review" then
					return ui.icon { name = "eye", size = "small", style = { color = "run-review" } }
				end
				return ui.icon { name = "circle-dashed", size = "small", tone = "muted" }
			end

			-- A run's steps as they stand after a re-run or a fix.
			local function steps_of(run: any): { any }
				local fix = state.runs[run.id]
				local out = {}
				local after_fix = false
				for _, step in run.steps do
					local status = step.status
					if fix == "rerun" and status == "failed" then
						status = "running"
					elseif fix == "resolved" and status == "failed" then
						status = "done"
						after_fix = true
					elseif after_fix and status == "waiting" then
						status = "running"
						after_fix = false
					end
					table.insert(out, { step = step, status = status })
				end
				return out
			end

			local function run_status(run: any): string
				local fix = state.runs[run.id]
				if fix and run.status == "failed" then
					return "active"
				end
				return run.status
			end

			local function run_head(run: any, agent: any, open: boolean)
				local status = run_status(run)
				return ui.row {
					class = "run",
					height = 40,
					align = "center",
					msg = { fold = run.id },
					ui.icon { name = if open then "chevron-down" else "chevron-right", size = "small", class = "chev" },
					agent and pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 18 },
					ui.row { width = 180, ui.text { agent and agent.name or run.agent, class = "run-name", truncate = true } },
					ui.row { grow = true, ui.text { run.summary, class = "run-summary", truncate = true } },
					ui.row { width = 116, justify = "end", status_pill(status) },
					ui.row {
						class = "age",
						width = 46,
						align = "center",
						justify = "end",
						ui.icon { name = "clock", size = "small" },
						ui.text { if status ~= run.status then "now" else run.age },
					},
				}
			end

			local function run_steps(run: any)
				local list = steps_of(run)
				local rows = {}
				-- Newest first, as a feed.
				for i = #list, 1, -1 do
					local item = list[i]
					local step = item.step
					local status = item.status
					local first = i == #list
					local last = i == 1
					local tall = status == "failed" or (status == "review" and step.action ~= nil)
					local height = if tall then 58 else 32
					local text: any
					if status == "failed" then
						text = ui.column {
							gap = "none",
							grow = true,
							ui.text { step.line, class = "step-error", weight = "medium" },
							ui.text { step.error or "", class = "step-muted" },
						}
					elseif status == "waiting" then
						text = ui.row { grow = true, ui.text { step.line, class = "step-muted", truncate = true } }
					else
						text = ui.row { grow = true, ui.text { step.line, class = "step-text", truncate = true } }
					end
					table.insert(rows, ui.row {
						class = if status == "failed" then "step step-fail" elseif tall then "step step-review" else "step",
						height = height,
						align = "center",
						ui.column {
							width = 16,
							height = height,
							align = "center",
							gap = "none",
							ui.row { width = 1, grow = true, class = if first then nil else "step-line" },
							step_icon(status),
							ui.row { width = 1, grow = true, class = if last then nil else "step-line" },
						},
						text,
						ui.spacer(),
						status == "failed" and ui.row {
							class = "step-button",
							align = "center",
							msg = { rerun = run.id },
							ui.icon { name = "refresh-cw", size = "small" },
							ui.text "Re-run",
						},
						tall and ui.row {
							class = "step-link",
							align = "center",
							msg = { manage = run.id },
							ui.icon { name = "workflow", size = "small" },
							ui.text "Manage workflow",
						},
					})
				end
				return ui.column { class = "steps", table.unpack(rows) }
			end

			local runs = {}
			if data and data.runs then
				for _, run in data.runs do
					local status = run_status(run)
					if (state.agent_filter == "all" or state.agent_filter == run.agent)
						and (state.status_filter == "all" or state.status_filter == status)
					then
						local agent = by_id[run.agent]
						local open = state.expanded[run.id] == true
						if open then
							table.insert(runs, ui.column {
								key = run.id,
								class = "run-open",
								gap = "none",
								run_head(run, agent, true),
								run_steps(run),
							})
						else
							table.insert(runs, ui.column { key = run.id, gap = "none", run_head(run, agent, false) })
						end
					end
				end
			end
			if #runs == 0 then
				table.insert(body, ui.row { class = "empty", ui.text "Nothing ran with these filters." })
			else
				table.insert(body, ui.column { gap = "none", table.unpack(runs) })
			end
		end

		return ui.column {
			grow = true,
			gap = "none",
			top,
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.row {
					justify = "center",
					shrink = false,
					ui.column {
						class = "page",
						width = WIDTH,
						shrink = false,
						tabs,
						table.unpack(body),
					},
				},
			},
			dialog {
				open = state.create_open,
				dismiss = "close-create",
				submit = "save-create",
				title = "Create an Agent",
				subtitle = "Describe what it does; you can build its workflow next.",
				icon = "sparkles",
				size = "small",
				buttons = {
					{ label = "Cancel", msg = "close-create", variant = "ghost" },
					{ label = "Create", msg = "save-create", variant = "primary", disabled = state.create_name == "" },
				},
				ui.column {
					gap = "normal",
					ui.text { "Name", size = "small", tone = "muted" },
					ui.input { id = "create-name", value = state.create_name, placeholder = "Invoice Chaser", change = "create-name", focus = state.create_open },
					ui.text { "What it does", size = "small", tone = "muted" },
					ui.input {
						id = "create-description",
						value = state.create_description,
						placeholder = "Reminds clients of unpaid invoices every Monday",
						change = "create-description",
						multiline = true,
						rows = 3,
					},
				},
			},
		}
	end,
}
