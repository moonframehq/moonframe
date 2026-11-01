-- The Code Review page: a title with the range and Reload, tabs (Analytics,
-- Pull requests, CI, Issues, Settings), and under Analytics two filters,
-- four key figures, pull requests reviewed per day beside issues caught
-- by severity, the repositories behind them, and a row of breakdowns.
-- Its figures are `app.hearth`, sample data from the provider.

local tabs = use "tabs"
local dropdown = use "dropdown"
local bar_chart = use "bar-chart"
local share_bar = use "share-bar"
local avatar = use "avatar"
local pr_row = use "pr-row"
local checks = use "checks"
local settings_row = use "settings-row"
local switch = use "switch"
local segmented = use "segmented"

type State = {
	page: string,
	tab: string,
	range: string,
	repo: string,
	author: string,
	metric: string,
	severity: string,
	open: string?,
	settings: { [string]: boolean },
	strictness: string,
	selected: number?,
}

local TABS: { any } = {
	{ id = "analytics", label = "Analytics", icon = "chart-column", closable = false },
	{ id = "pulls", label = "Pull requests", icon = "git-pull-request", closable = false },
	{ id = "ci", label = "CI", icon = "circle-play", closable = false },
	{ id = "issues", label = "Issues", icon = "circle-alert", closable = false },
	{ id = "settings", label = "Settings", icon = "settings", closable = false },
}

local RANGES: { any } = {
	{ id = "7", label = "Last 7 days" },
	{ id = "14", label = "Last 14 days" },
	{ id = "30", label = "Last 30 days" },
}
local REPOS: { any } = {
	{ id = "all", label = "All repositories" },
	"separator",
	{ id = "emberline/hearth", label = "emberline/hearth" },
	{ id = "emberline/cinder", label = "emberline/cinder" },
	{ id = "emberline/flint-orm", label = "emberline/flint-orm" },
}
local AUTHORS: { any } = {
	{ id = "all", label = "All authors" },
	"separator",
	{ id = "imarchetti", label = "imarchetti" },
	{ id = "treyes", label = "treyes" },
	{ id = "pvale", label = "pvale" },
	{ id = "olind", label = "olind" },
}
local METRICS: { any } = {
	{ id = "prs", label = "PRs reviewed" },
	{ id = "reviews", label = "Reviews" },
	{ id = "comments", label = "Comments" },
}
local SEVERITIES: { any } = {
	{ id = "all", label = "All severities" },
	"separator",
	{ id = "critical", label = "Critical" },
	{ id = "warning", label = "Warning" },
	{ id = "info", label = "Info" },
}
local SEVERITY_COLOR = { critical = "#d9604c", warning = "#e1b64e", info = "#619cf8" }

local SETTINGS: { any } = {
	{ id = "every", title = "Review every pull request", description = "Run a review when a pull request opens or gets new commits." },
	{ id = "drafts", title = "Skip drafts", description = "Wait until a pull request is marked ready." },
	{ id = "summary", title = "Post a summary comment", description = "One comment per review with the issues found, by severity." },
	{ id = "block", title = "Block merging on critical issues", description = "Set a failing check while a critical issue is open." },
	{ id = "ci", title = "Read CI logs", description = "Explain failing jobs in the review." },
}

local function label_of(options: { any }, id: string): string
	for _, option in options do
		if type(option) == "table" and option.id == id then
			return option.label
		end
	end
	return id
end

local function thousands(n: number): string
	local text = tostring(math.floor(n))
	local out = text
	while true do
		local next, count = string.gsub(out, "^(%d+)(%d%d%d)", "%1,%2")
		out = next
		if count == 0 then
			break
		end
	end
	return out
end

return mod {
	region = "main",
	title = "Code Review",

	styles = {
		[".page"] = { padding = { 22, 16, 28, 17 }, gap = 0 },
		[".page-title"] = { font = "sans", size = "title", weight = "bold" },
		[".page-button"] = { radius = 7, border = true, border_color = "border", background = "card", padding = { 5, 11, 5, 9 }, gap = 7 },
		[".page-button:hover"] = { background = "raised" },
		[".page-tabs"] = { margin = { 26, 0, 0, 0 } },
		[".tabs-underline"] = { gap = 14, border_color = "border" },
		[".tabs-underline-tab"] = { padding = { 8, 3, 10, 3 }, color = "muted-foreground", border = "bottom" },
		[".tabs-underline-tab:hover"] = { color = "foreground", border_color = "#00000000" },
		[".tabs-underline-tab:selected"] = { color = "foreground", border_color = "accent" },
		[".filters"] = { margin = { 30, 0, 22, 0 }, gap = 9 },
		[".dropdown-trigger"] = { background = "card", radius = 7, padding = { 6, 8, 6, 10 } },
		[".dropdown-chevron"] = { color = "muted-foreground" },
		[".kpis"] = { gap = 0 },
		[".kpi"] = { padding = { 20, 20, 20, 20 }, gap = 0 },
		[".kpi-split"] = { border = "left", border_color = "border" },
		[".kpi-title"] = { size = "small", color = "#cfc8c0" },
		[".kpi-delta"] = { size = "small", color = "success" },
		[".kpi-value"] = { size = "title", weight = "medium", margin = { 14, 0, 14, 0 } },
		[".kpi-caption"] = { size = "small", color = "muted-foreground" },
		[".grid"] = { gap = 25, margin = { 25, 0, 0, 0 } },
		[".card-head"] = { padding = { 0, 16, 0, 16 }, border = "bottom", border_color = "border" },
		[".card-title"] = { size = "small", color = "#cfc8c0" },
		[".card-body"] = { padding = { 20, 16, 16, 12 } },
		[".card-section"] = { padding = { 20, 16, 14, 16 }, gap = 12, border = "top", border_color = "border" },
		[".card-subtitle"] = { size = "small", color = "#cfc8c0", margin = { 0, 0, 2, 0 } },
		[".repo-row"] = { gap = 9 },
		[".repo-count"] = { color = "#bdb5ad" },
		[".see-all"] = { color = "#cfc8c0", padding = { 4, 8 }, radius = 6, gap = 6 },
		[".see-all:hover"] = { background = "list-hover" },
		[".bar-chart-label"] = { color = "#bdb5ad" },
		[".bar-chart-scale"] = { color = "#bdb5ad" },
		[".bar-chart-plot"] = { border = "none" },
		[".bar-chart-bar"] = { radius = 1 },
		[".break-row"] = { gap = 9 },
		[".break-dot"] = { radius = 3 },
		[".break-count"] = { color = "#bdb5ad" },
		[".break-share"] = { color = "muted-foreground", size = "small" },
		[".author-login"] = { color = "muted-foreground", size = "small" },
		[".list-card"] = { padding = { 4, 0 } },
		[".pr-row"] = { padding = { 10, 16 } },
		[".pr-row:hover"] = { background = "list-hover" },
		[".pr-row:selected"] = { background = "raised" },
		[".pr-row-title"] = { color = "foreground" },
		[".issue-row"] = { padding = { 11, 16 }, gap = 12, border = "bottom", border_color = "border" },
		[".issue-row:hover"] = { background = "list-hover" },
		[".issue-sev"] = { radius = 4, padding = { 1, 6 }, size = "small" },
		[".issue-where"] = { color = "muted-foreground", size = "small" },
		[".checks"] = { border = "none", background = "#00000000" },
		[".settings-row"] = { padding = { 14, 16 } },
		[".placeholder"] = { padding = 40, gap = 10 },
	},

	init = function(previous: State?): State
		local state: State = previous
			or ({
				page = "review",
				tab = "analytics",
				range = "30",
				repo = "all",
				author = "all",
				metric = "prs",
				severity = "all",
				settings = { every = true, drafts = true, summary = true, block = false, ci = true },
				strictness = "balanced",
			} :: any)
		state.open = nil
		state.page = "review"
		return state, { { to = "topic:hearth/tab", msg = state.tab } }
	end,

	update = function(state: State, msg)
		if type(msg) == "string" then
			if string.sub(msg, 1, 5) == "open-" then
				local which = string.sub(msg, 6)
				state.open = if state.open == which then nil else which
			elseif msg == "close" then
				state.open = nil
			elseif msg == "see-issues" then
				state.tab = "issues"
				return state, { { to = "topic:hearth/tab", msg = state.tab } }
			elseif msg == "reload" then
				return state, { { to = "provider:hearth", msg = "reload" } }
			end
			return state
		end
		if type(msg) ~= "table" then
			return state
		end
		if msg.page then
			state.page = msg.page
		elseif msg.tab or msg.select then
			state.tab = msg.tab or msg.select
			return state, { { to = "topic:hearth/tab", msg = state.tab } }
		elseif msg.range then
			state.range = msg.range
			state.open = nil
		elseif msg.repo then
			state.repo = msg.repo
			state.open = nil
		elseif msg.author then
			state.author = msg.author
			state.open = nil
		elseif msg.metric then
			state.metric = msg.metric
			state.open = nil
		elseif msg.severity then
			state.severity = msg.severity
			state.open = nil
		elseif msg.setting then
			state.settings[msg.setting] = not state.settings[msg.setting]
		elseif msg.strictness then
			state.strictness = msg.strictness
		elseif msg.pull then
			state.selected = msg.pull
		end
		return state
	end,

	view = function(state: State, app)
		local data = app.hearth
		if state.page ~= "review" then
			return ui.column {
				class = "placeholder",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "code", size = "title", tone = "muted" },
				ui.text { "Only Code Review is drawn in this example.", tone = "muted" },
			}
		end
		if not data then
			return ui.column { class = "placeholder", ui.text { "Loading the sample reviews…", tone = "muted" } }
		end

		local function select(id: string, options: { any }, selected: string, event: string, width: number, align: string?)
			return dropdown {
				options = options,
				selected = selected,
				open = state.open == id,
				toggle = `open-{id}`,
				dismiss = "close",
				event = event,
				chevron = "chevrons-up-down",
				trigger_width = width,
				width = math.max(width, 200),
				align = align,
			}
		end

		local function card(props: any, ...: any)
			return ui.column { class = "card", gap = "none", grow = props.grow, width = props.width, ... }
		end

		local function head(title: string, control: any?)
			return ui.row {
				class = "card-head",
				height = 50,
				align = "center",
				shrink = false,
				ui.text { title, class = "card-title" },
				ui.spacer(),
				control,
			}
		end

		local function kpi_strip(kpis: { any })
			local cells = {}
			for index, kpi in kpis do
				local arrow = if kpi.up then "arrow-up" else "arrow-down"
				table.insert(cells, ui.column {
					class = if index > 1 then { "kpi", "kpi-split" } else "kpi",
					grow = true,
					ui.row {
						align = "center",
						ui.text { kpi.title, class = "kpi-title", truncate = true },
						ui.spacer(),
						kpi.delta ~= "" and ui.row {
							gap = "none",
							align = "center",
							ui.icon { name = arrow :: any, class = "kpi-delta", size = "small" },
							ui.text { kpi.delta, class = "kpi-delta" },
						},
					},
					ui.text { kpi.value, class = "kpi-value" },
					ui.text { kpi.caption, class = "kpi-caption" },
				})
			end
			return ui.row { class = { "card", "kpis" }, shrink = false, table.unpack(cells) }
		end

		local function repo_list(title: string, rows: { any }, footer: any?)
			local items = { ui.text { title, class = "card-subtitle" } }
			for _, repo in rows do
				table.insert(items, ui.row {
					class = "repo-row",
					align = "center",
					tooltip = `Open {repo.name}`,
					msg = { repo = repo.name },
					ui.icon { name = "brand-github", tone = "muted" },
					ui.text { repo.name },
					ui.spacer(),
					ui.text { repo.count, class = "repo-count" },
				})
			end
			if footer then
				table.insert(items, footer)
			end
			return ui.column { class = "card-section", table.unpack(items) }
		end

		-- The days shown: the last `range` of the thirty.
		local days = {}
		local count = tonumber(state.range) or 30
		for index, day in data.days do
			if index > #data.days - count then
				table.insert(days, day)
			end
		end
		local labelled = count <= 14

		local body
		if state.tab == "analytics" then
			local factor = if state.metric == "reviews" then 4 elseif state.metric == "comments" then 7 else 1
			local noun = label_of(METRICS, state.metric)
			local top = 0
			local pr_buckets = {}
			for index, day in days do
				local value = day.prs * factor
				top = math.max(top, value)
				table.insert(pr_buckets, {
					label = if labelled then string.sub(day.date, 5) else day.label,
					values = { value },
					dim = index == #days,
					tooltip = `{day.date} · {value} {string.lower(noun)}` .. (if index == #days then " (today, so far)" else ""),
				})
			end
			local pr_max = math.ceil(math.max(top, 1) / (5 * factor)) * 5 * factor
			pr_max = math.ceil(pr_max / 14) * 14

			local all = state.severity == "all"
			local series: { any } = {}
			for _, id in { "critical", "warning", "info" } do
				if all or state.severity == id then
					table.insert(series, { id = id, name = label_of(SEVERITIES, id), color = SEVERITY_COLOR[id] })
				end
			end
			local issue_top = 0
			local issue_buckets = {}
			for index, day in days do
				local values = {}
				local total = 0
				for _, item in series do
					table.insert(values, day[item.id])
					total += day[item.id]
				end
				issue_top = math.max(issue_top, total)
				table.insert(issue_buckets, {
					label = if labelled then string.sub(day.date, 5) else day.label,
					values = values,
					dim = index == #days,
					tooltip = if all
						then `{day.date} · {total} issues\n{day.critical} critical · {day.warning} warning · {day.info} info`
						else `{day.date} · {total} {string.lower(label_of(SEVERITIES, state.severity))}`,
				})
			end
			local issue_max = math.ceil(math.max(issue_top, 1) / 28) * 28

			local function breakdown(title: string, rows: { any })
				local sum = 0
				for _, row in rows do
					sum += row.count
				end
				local shares = {}
				local lines = {}
				for _, row in rows do
					table.insert(shares, { share = row.count, color = row.color, label = `{row.label} · {row.count}` })
					table.insert(lines, ui.row {
						class = "break-row",
						align = "center",
						ui.row { class = "break-dot", width = 7, height = 7, style = { background = row.color } },
						ui.text { row.label },
						ui.spacer(),
						ui.text { thousands(row.count), class = "break-count" },
						ui.row { width = 40, justify = "end", ui.text { `{math.floor(row.count / sum * 100 + 0.5)}%`, class = "break-share" } },
					})
				end
				return card(
					{ grow = true },
					head(title),
					ui.column {
						class = "card-body",
						gap = "none",
						style = { gap = 13, padding = { 18, 16, 18, 16 } },
						share_bar { shares = shares, width = 446, height = 8 },
						ui.row { height = 4 },
						table.unpack(lines)
					}
				)
			end

			local authors = {}
			for _, person in data.authors do
				table.insert(authors, ui.row {
					class = "break-row",
					align = "center",
					avatar { name = person.name, shape = "circle", size = "small" },
					ui.text { person.name, truncate = true },
					ui.text { person.login, class = "author-login" },
					ui.spacer(),
					ui.text { person.count, class = "break-count" },
				})
			end

			body = ui.column {
				gap = "none",
				shrink = false,
				ui.row {
					class = "filters",
					align = "center",
					select("repo", REPOS, state.repo, "repo", 190),
					select("author", AUTHORS, state.author, "author", 190),
				},
				kpi_strip(data.kpis),
				ui.row {
					class = "grid",
					align = "stretch",
					card(
						{ grow = true },
						head("Pull requests reviewed", select("metric", METRICS, state.metric, "metric", 142, "end")),
						ui.row {
							class = "card-body",
							bar_chart {
								buckets = pr_buckets,
								series = { { name = noun, color = "#c8825e" } },
								max = pr_max,
								ticks = 5,
								grow = true,
								height = 196,
								bar = 12,
								gap = 2,
								legend = false,
							},
						},
						ui.spacer(),
						repo_list("Top repositories by reviews", data.repos_by_reviews)
					),
					card(
						{ grow = true },
						head("Issues caught", select("severity", SEVERITIES, state.severity, "severity", 142, "end")),
						ui.row {
							class = "card-body",
							bar_chart {
								buckets = issue_buckets,
								series = series,
								max = issue_max,
								ticks = 5,
								grow = true,
								height = 196,
								bar = 14,
								gap = 2,
								legend = false,
							},
						},
						repo_list(
							"Repositories with the most issues",
							data.repos_by_issues,
							ui.row {
								justify = "center",
								ui.row {
									class = "see-all",
									align = "center",
									msg = "see-issues",
									ui.text "See all issues caught",
									ui.icon { name = "arrow-right", size = "small" },
								},
							}
						)
					),
				},
				ui.row {
					class = "grid",
					align = "stretch",
					breakdown("Issues by category", data.categories),
					breakdown("Review outcomes", data.outcomes),
					card(
						{ grow = true },
						head("Top authors"),
						ui.column { class = "card-body", style = { gap = 12, padding = { 16, 16, 16, 16 } }, table.unpack(authors) }
					),
				},
			}
		elseif state.tab == "pulls" then
			local rows = {}
			for _, pull in data.pulls do
				if state.author == "all" or pull.author == state.author then
					table.insert(rows, pr_row {
						number = pull.number,
						title = pull.title,
						branch = pull.branch,
						author = pull.author,
						age = pull.age,
						checks = pull.checks,
						tone = pull.tone,
						draft = pull.draft,
						added = pull.added,
						removed = pull.removed,
						labels = if pull.issues > 1 then { `{pull.issues} issues` } elseif pull.issues == 1 then { "1 issue" } else { "clean" },
						selected = state.selected == pull.number,
						event = "pull",
					})
				end
			end
			body = ui.column {
				gap = "none",
				shrink = false,
				ui.row {
					class = "filters",
					align = "center",
					select("repo", REPOS, state.repo, "repo", 190),
					select("author", AUTHORS, state.author, "author", 190),
				},
				card({}, head(`{#rows} pull requests reviewed`), ui.column { class = "list-card", gap = "none", table.unpack(rows) }),
			}
		elseif state.tab == "ci" then
			local buckets = {}
			for index, day in days do
				local minutes = data.ci_minutes[#data.ci_minutes - #days + index]
				table.insert(buckets, {
					label = if labelled then string.sub(day.date, 5) else day.label,
					values = { minutes },
					dim = index == #days,
					tooltip = `{day.date} · {minutes} runner minutes`,
				})
			end
			body = ui.column {
				gap = "none",
				shrink = false,
				ui.row { height = 30 },
				kpi_strip(data.ci_kpis),
				ui.row {
					class = "grid",
					align = "stretch",
					card(
						{ grow = true },
						head("Runner minutes per day"),
						ui.row {
							class = "card-body",
							bar_chart {
								buckets = buckets,
								series = { { name = "Minutes", color = "#619cf8" } },
								max = 120,
								ticks = 4,
								grow = true,
								height = 196,
								bar = 12,
								gap = 2,
								legend = false,
							},
						}
					),
					card({ width = 420 }, head("Latest runs"), ui.column { class = "list-card", checks { checks = data.runs, title = "Checks" } }),
				},
			}
		elseif state.tab == "issues" then
			local rows = {}
			for _, issue in data.issues do
				if (state.severity == "all" or issue.severity == state.severity) and (state.repo == "all" or issue.repo == state.repo) then
					local color = SEVERITY_COLOR[issue.severity]
					table.insert(rows, ui.row {
						class = "issue-row",
						align = "center",
						tooltip = `{issue.category} · found in #{issue.pr}`,
						ui.row {
							width = 74,
							ui.text { issue.severity, class = "issue-sev", style = { color = color, background = color .. "26" } },
						},
						ui.column {
							gap = "none",
							grow = true,
							ui.text { issue.title, truncate = true },
							ui.text { issue.where, class = "issue-where" },
						},
						ui.row { width = 180, ui.icon { name = "brand-github", tone = "muted", size = "small" }, ui.text { issue.repo, class = "issue-where" } },
						ui.row { width = 64, ui.text { `#{issue.pr}`, class = "issue-where" } },
						ui.row { width = 36, justify = "end", ui.text { issue.age, class = "issue-where" } },
					})
				end
			end
			body = ui.column {
				gap = "none",
				shrink = false,
				ui.row {
					class = "filters",
					align = "center",
					select("repo", REPOS, state.repo, "repo", 190),
					select("severity", SEVERITIES, state.severity, "severity", 190),
				},
				card({}, head(`{#rows} open issues`), table.unpack(rows)),
			}
		else
			local rows = {}
			for index, item in SETTINGS do
				table.insert(rows, settings_row {
					title = item.title,
					description = item.description,
					border = index < #SETTINGS,
					switch { on = state.settings[item.id] == true, msg = { setting = item.id } },
				})
			end
			body = ui.column {
				gap = "none",
				shrink = false,
				ui.row { height = 30 },
				card(
					{},
					head("Reviewer"),
					ui.column {
						gap = "none",
						settings_row {
							title = "Strictness",
							description = "How much the reviewer comments on style and taste.",
							segmented {
								options = { { id = "lenient", label = "Lenient" }, { id = "balanced", label = "Balanced" }, { id = "strict", label = "Strict" } },
								selected = state.strictness,
								event = "strictness",
								small = true,
							},
						},
						table.unpack(rows),
					}
				),
			}
		end

		return ui.column {
			grow = true,
			scroll = true,
			ui.column {
				class = "page",
				shrink = false,
				ui.row {
					align = "center",
					gap = "normal",
					ui.text { "Code Review", class = "page-title" },
					ui.spacer(),
					dropdown {
						icon = "clock",
						options = RANGES,
						selected = state.range,
						open = state.open == "range",
						toggle = "open-range",
						dismiss = "close",
						event = "range",
						align = "end",
						width = 180,
					},
					ui.row {
						class = "page-button",
						align = "center",
						msg = "reload",
						tooltip = "Reload the figures",
						ui.icon { name = "refresh-cw", size = "small" },
						ui.text "Reload",
					},
				},
				ui.column {
					class = "page-tabs",
					tabs { variant = "underline", tabs = TABS, selected = state.tab },
				},
				body,
			},
		}
	end,
}
