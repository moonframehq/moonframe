-- The dashboard: a welcome with the date and Create Workflow, four figures,
-- the active workflows in a table (pause and resume work), and a carousel
-- of templates. Sample data.

type Workflow = {
	id: string,
	name: string,
	color: string,
	trigger: string,
	runs: string,
	success: number,
	last: string,
	paused: boolean,
}

type State = { workflows: { Workflow }, first: number, created: number }

local KPIS = {
	{ label = "Total Runs", value = "12,847", delta = "+12% from last week", good = true },
	{ label = "Success Rate", value = "98.2%", delta = "+0.4% from last week", good = true },
	{ label = "Active Workflows", value = "23+", delta = "+3 from last week", good = true },
	{ label = "AI Tokens", value = "842K", delta = "-2% from last week", good = false },
}

local TEMPLATES = {
	{ name = "Chat Router", detail = "Route chats to the right team", uses = "2.4k", icon = "message-square", color = "#4f6bed", new = true },
	{ name = "Lead Enrichment", detail = "Score and tag every new lead", uses = "1.8k", icon = "user", color = "#f2b632", new = false },
	{ name = "Invoice Reminder", detail = "Chase unpaid invoices politely", uses = "1.1k", icon = "mail", color = "#22a35a", new = false },
	{ name = "Release Notes", detail = "Draft notes from merged changes", uses = "940", icon = "file-text", color = "#f0606a", new = true },
	{ name = "Ticket Triage", detail = "Label and assign support tickets", uses = "870", icon = "inbox", color = "#35c3d8", new = false },
	{ name = "Daily Digest", detail = "One summary of yesterday's runs", uses = "640", icon = "calendar", color = "#a78bfa", new = false },
}

local SHOWN = 4

local function samples(): { Workflow }
	return {
		{ id = "support", name = "Email Support Router", color = "#f0606a", trigger = "Webhook", runs = "12,847", success = 98.2, last = "2 min ago", paused = false },
		{ id = "leads", name = "Lead Scoring Pipeline", color = "#f2b632", trigger = "Schedule", runs = "12,569", success = 96.4, last = "15 min ago", paused = false },
		{ id = "digest", name = "Chat Digest Report", color = "#35c3d8", trigger = "Schedule", runs = "11,846", success = 91.6, last = "30 min ago", paused = false },
		{ id = "weekly", name = "Weekly Report Generator", color = "#f59152", trigger = "Webhook", runs = "10,848", success = 70.5, last = "2hr ago", paused = false },
		{ id = "brand", name = "Brand Mention Monitor", color = "#7cc84a", trigger = "Schedule", runs = "10,560", success = 88.3, last = "3hr ago", paused = false },
		{ id = "router", name = "Weekly Support Router", color = "#f59152", trigger = "Schedule", runs = "4,848", success = 68.7, last = "1 week ago", paused = false },
	}
end

-- Column widths, shared by the header and the rows; the name takes the rest.
local COLUMNS = {
	{ title = "Trigger", width = 120 },
	{ title = "Total Runs", width = 120 },
	{ title = "Success", width = 110 },
	{ title = "Last Run", width = 130 },
	{ title = "Status", width = 90 },
	{ title = "Action", width = 120 },
}

local function cell(width: number, child: any)
	return ui.row { width = width, align = "center", gap = "small", shrink = false, child }
end

return mod {
	region = "main",
	title = "Dashboard",

	styles = {
		[".dash"] = { padding = { 28, 40, 32, 40 }, gap = 20 },
		[".dash-title"] = { size = "title", weight = "medium" },
		[".dash-muted"] = { color = "muted-foreground" },
		[".dash-small"] = { size = "small", color = "muted-foreground" },
		[".dash-date"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 0, 12 }, gap = 8, color = "#4a4a50" },
		[".dash-create"] = { padding = { 0, 14 }, gap = 6 },
		[".dash-create:hover"] = { background = "#000000" },
		[".kpi"] = { background = "card", radius = 12, border = true, border_color = "border", padding = 5, gap = 0, shadow = "0 1px 2px #00000008" },
		[".kpi-inner"] = { background = "secondary", radius = 9, padding = { 10, 12, 12, 12 }, gap = 6 },
		[".kpi-tile"] = { background = "background", radius = 6, border = true, border_color = "border", shadow = "0 1px 1px #0000000d" },
		[".kpi-label"] = { color = "#6b6b72" },
		[".kpi-value"] = { size = "title", weight = "medium" },
		[".kpi-foot"] = { padding = { 7, 6, 2, 6 }, gap = 6, color = "muted-foreground" },
		[".kpi-good"] = { color = "good", size = "small" },
		[".kpi-bad"] = { color = "bad", size = "small" },
		[".card"] = { background = "card", radius = 12, border = true, border_color = "border", padding = { 14, 12, 12, 12 }, gap = 10, shadow = "0 1px 2px #00000008" },
		[".card-title"] = { size = "large", weight = "medium" },
		[".card-link"] = { color = "muted-foreground" },
		[".card-link:hover"] = { color = "foreground" },
		[".table-head"] = { background = "secondary", radius = 8, padding = { 8, 10 }, color = "muted-foreground", size = "small" },
		[".table-row"] = { padding = { 10, 10 }, border = "bottom", border_color = "border", color = "#4a4a50" },
		[".table-row:hover"] = { background = "#fafafa" },
		[".table-tile"] = { radius = 4, color = "#ffffff" },
		[".table-name"] = { color = "foreground" },
		[".rate-good"] = { color = "good" },
		[".rate-low"] = { color = "amber" },
		[".status"] = { radius = 9, padding = { 1, 8 }, size = "small" },
		[".status-active"] = { background = "#22c55e24", color = "#16a34a" },
		[".status-paused"] = { background = "muted", color = "muted-foreground" },
		[".table-action"] = { color = "#6b6b72", radius = 5, padding = { 2, 4 } },
		[".table-action:hover"] = { color = "foreground", background = "list-hover" },
		[".arrow"] = { radius = 14, color = "muted-foreground" },
		[".arrow:hover"] = { color = "foreground" },
		[".arrow-on"] = { background = "muted", color = "foreground" },
		[".template"] = { background = "card", radius = 10, border = true, border_color = "border", padding = 6, gap = 8, transition = "fade fast ease-out" },
		[".template:hover"] = { border_color = "#dcdce0" },
		[".template-art"] = { background = "secondary", radius = 7 },
		[".template-icon"] = { radius = 10, color = "#ffffff" },
		[".template-new"] = { background = "#4f6bed1f", color = "#4f6bed", radius = 9, padding = { 1, 7 }, size = "small" },
	},

	init = function(previous: State?): State
		return previous or { workflows = samples(), first = 1, created = 0 }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.pause then
			for _, workflow in state.workflows do
				if workflow.id == msg.pause then
					workflow.paused = not workflow.paused
				end
			end
		elseif msg == "next" then
			state.first = math.min(#TEMPLATES - SHOWN + 1, state.first + 1)
		elseif msg == "previous" then
			state.first = math.max(1, state.first - 1)
		elseif msg == "create" then
			state.created += 1
			return state,
				{
					{
						to = "topbar",
						msg = {
							notify = `Workflow "Untitled {state.created}" created`,
							body = "A blank workflow is ready on the Canvas. Add a trigger to start it.",
							level = "success",
						},
					},
				}
		elseif type(msg) == "table" and msg.use then
			return state,
				{
					{
						to = "topbar",
						msg = { notify = `Template "{msg.use}" added`, body = "Find it in your Workflow Library.", level = "info" },
					},
				}
		end
	end,

	view = function(state: State)
		local kpis = {}
		for _, kpi in KPIS do
			table.insert(kpis, ui.column {
				class = "kpi",
				grow = true,
				ui.column {
					class = "kpi-inner",
					ui.row {
						align = "center",
						gap = "normal",
						ui.row {
							class = "kpi-tile",
							width = 22,
							height = 22,
							align = "center",
							justify = "center",
							ui.icon { name = "workflow", size = "small" },
						},
						ui.text { kpi.label, class = "kpi-label" },
					},
					ui.row { padding = "none", ui.row { width = 30 }, ui.text { kpi.value, class = "kpi-value" } },
				},
				ui.row {
					class = "kpi-foot",
					align = "center",
					ui.icon { name = "circle-dashed", size = "small" },
					ui.text { kpi.delta, class = if kpi.good then "kpi-good" else "kpi-bad" },
					ui.spacer(),
					ui.icon { name = "circle-dashed", size = "small" },
				},
			})
		end

		local head = { ui.row { grow = true, ui.text "Workflow Name" } }
		for _, column in COLUMNS do
			table.insert(head, cell(column.width, ui.text(column.title)))
		end

		local rows = {}
		for _, workflow in state.workflows do
			table.insert(rows, ui.row {
				class = "table-row",
				align = "center",
				ui.row {
					grow = true,
					align = "center",
					gap = "normal",
					ui.row {
						class = "table-tile",
						width = 16,
						height = 16,
						align = "center",
						justify = "center",
						style = { background = workflow.color },
						ui.icon { name = "workflow", size = "small" },
					},
					ui.text { workflow.name, class = "table-name", truncate = true },
				},
				cell(120, ui.text(workflow.trigger)),
				cell(120, ui.text(workflow.runs)),
				cell(110, ui.text { string.format("%.1f%%", workflow.success), class = if workflow.success >= 80 then "rate-good" else "rate-low" }),
				cell(130, ui.row { align = "center", gap = "small", ui.icon { name = "clock", size = "small" }, ui.text(workflow.last) }),
				cell(90, ui.row {
					class = { "status", if workflow.paused then "status-paused" else "status-active" },
					hug = true,
					ui.text(if workflow.paused then "Paused" else "Active"),
				}),
				cell(120, ui.row {
					align = "center",
					gap = "small",
					ui.row { class = "table-action", tooltip = "Not in this example", ui.text "View" },
					ui.row { class = "table-action", tooltip = "Not in this example", ui.text "Edit" },
					ui.row {
						class = "table-action",
						tooltip = if workflow.paused then "Resume" else "Pause",
						msg = { pause = workflow.id },
						ui.icon { name = if workflow.paused then "play" else "pause", size = "small" },
					},
				}),
			})
		end

		local templates = {}
		for i = state.first, math.min(#TEMPLATES, state.first + SHOWN - 1) do
			local template = TEMPLATES[i]
			table.insert(templates, ui.column {
				key = `t{i}`,
				class = "template",
				grow = true,
				msg = { use = template.name },
				tooltip = "Use this template",
				ui.row {
					class = "template-art",
					height = 96,
					align = "center",
					justify = "center",
					ui.row {
						class = "template-icon",
						width = 36,
						height = 36,
						align = "center",
						justify = "center",
						style = { background = template.color },
						ui.icon { name = template.icon :: any },
					},
				},
				ui.column {
					padding = "small",
					gap = "none",
					ui.row {
						align = "center",
						gap = "small",
						ui.text { template.name, weight = "medium" },
						ui.spacer(),
						template.new and ui.row { class = "template-new", hug = true, ui.text "New" },
					},
					ui.text { template.detail, class = "dash-small", truncate = true },
					ui.text { `Used {template.uses} times`, class = "dash-small" },
				},
			})
		end
		local can_back = state.first > 1
		local can_next = state.first + SHOWN - 1 < #TEMPLATES

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "dash",
				shrink = false,
				ui.row {
					align = "center",
					gap = "normal",
					ui.column {
						gap = "small",
						ui.text { "Welcome Back, Mira!", class = "dash-title" },
						ui.text { "Automate tasks using AI in minutes. No code required.", class = "dash-muted" },
					},
					ui.spacer(),
					ui.row {
						class = "dash-date",
						height = 34,
						align = "center",
						ui.icon { name = "calendar", size = "small" },
						ui.text "27 April, 2026",
					},
					ui.row {
						class = { "dark-button", "dash-create" },
						height = 34,
						align = "center",
						msg = "create",
						ui.icon { name = "plus", size = "small" },
						ui.text { "Create Workflow", weight = "medium" },
					},
				},
				ui.row { gap = "large", table.unpack(kpis) },
				ui.column {
					class = "card",
					ui.row {
						align = "center",
						padding = "small",
						ui.text { "Active Workflows", class = "card-title" },
						ui.spacer(),
						ui.row { class = "card-link", tooltip = "Not in this example", ui.text "View All" },
					},
					ui.row { class = "table-head", align = "center", table.unpack(head) },
					ui.column { gap = "none", table.unpack(rows) },
				},
				ui.column {
					class = "card",
					ui.row {
						align = "center",
						padding = "small",
						ui.text { "Templates", class = "card-title" },
						ui.spacer(),
						ui.row {
							class = if can_back then "arrow arrow-on" else "arrow",
							width = 28,
							height = 28,
							align = "center",
							justify = "center",
							msg = if can_back then "previous" else nil,
							ui.icon { name = "chevron-left", size = "small" },
						},
						ui.row {
							class = if can_next then "arrow arrow-on" else "arrow",
							width = 28,
							height = 28,
							align = "center",
							justify = "center",
							msg = if can_next then "next" else nil,
							ui.icon { name = "chevron-right", size = "small" },
						},
					},
					ui.row { gap = "normal", table.unpack(templates) },
				},
			},
		}
	end,
}
