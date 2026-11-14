-- The sheet the agent fills: its tabs, then a card with the sheet's title
-- and what the agent is doing, a format bar, the grid, and the sheet tabs
-- at the foot. As the scripted demo plays (`app.research.run`), step 1 adds
-- a row per app found, step 2 writes APY, fee, minimum and rating cell by
-- cell, the current row highlighted and the cell being typed outlined.
-- The download button saves the sheet as it stands as a PDF (`app:pdf`, on
-- A4 pages), in the app's cache folder, and says where.

local spreadsheet = use "spreadsheet"
local icon_button = use "icon-button"

type State = { selected: { row: number, key: string }?, saved: string?, failed: string? }

local KEYS = { "apy", "fee", "deposit", "rating" }

local COLUMNS: { any } = {
	{ key = "company", title = "Company", icon = "case-sensitive", width = 136 },
	{ key = "type", title = "Type", icon = "circle-dot", width = 96 },
	{ key = "apy", title = "APY", glyph = "%", width = 76, skeleton = 46 },
	{ key = "fee", title = "Monthly fee", glyph = "$", width = 100, skeleton = 38 },
	{ key = "deposit", title = "Min. deposit", glyph = "$", width = 104, skeleton = 50 },
	{ key = "rating", title = "App rating", icon = "star", width = 94, skeleton = 38 },
	{ key = "source", title = "Source", icon = "link", skeleton = 96 },
}

local ROWS = 26 -- numbered rows drawn, the rest empty

local function escape(text: string): string
	return (string.gsub(string.gsub(string.gsub(text, "&", "&amp;"), "<", "&lt;"), ">", "&gt;"))
end

-- The sheet as far as the agent has written it, as a page to print: what
-- the download button makes a PDF of. Nil until it has found an app.
local function sheet_html(data: any): string?
	local run = data and data.run
	if not run or run.found == 0 then
		return nil
	end
	local head = {}
	for _, column in COLUMNS do
		table.insert(head, `<th>{escape(column.title)}</th>`)
	end
	local rows = {}
	for index = 1, run.found do
		local item = data.apps[index]
		local cells = { `<td><b>{escape(item.name)}</b></td>`, `<td>{escape(item.type)}</td>` }
		for c, key in KEYS do
			local written = run.step > 2 or (run.step == 2 and (index < run.row or (index == run.row and c <= run.cell)))
			table.insert(cells, `<td>{if written then escape(tostring(item[key])) else ""}</td>`)
		end
		table.insert(cells, `<td>{escape(item.source)}</td>`)
		table.insert(rows, `<tr>{table.concat(cells)}</tr>`)
	end
	return `<html><head><title>{escape(data.sheet)}</title><style>`
		.. "body { font: 11pt -apple-system, sans-serif; color: #1f2328; }"
		.. "h1 { font-size: 18pt; margin: 0 0 4pt; } p { color: #59636e; margin: 0 0 14pt; }"
		.. "table { border-collapse: collapse; width: 100%; } tr { break-inside: avoid; }"
		.. "th { text-align: left; background: #f0f2f4; } th, td { padding: 5pt 7pt; border-bottom: 1px solid #d8dee4; }"
		.. `</style></head><body><h1>{escape(data.sheet)}</h1><p>{escape(data.project)} · {run.found} apps</p>`
		.. `<table><thead><tr>{table.concat(head)}</tr></thead><tbody>{table.concat(rows)}</tbody></table></body></html>`
end

return mod {
	region = "main",
	title = "Sheet",

	styles = {
		[".bay-body"] = { padding = { 0, 10, 10, 10 }, gap = 0 },
		[".tabs-row"] = { padding = { 0, 4 }, gap = 6 },
		[".doc-tab"] = { padding = { 0, 10 }, radius = 9, gap = 7 },
		[".doc-tab:hover"] = { background = "list-hover" },
		[".doc-tab:selected"] = { background = "background", border = true, border_color = "border", shadow = "0 1px 2px #0000000a" },
		[".doc-close"] = { color = "muted-foreground" },
		[".green-tile"] = { background = "#1fa463", color = "#ffffff", radius = 4 },
		[".doc"] = { background = "background", border = true, border_color = "border", radius = 12, shadow = "0 1px 3px #0000000a" },
		[".doc-head"] = { padding = { 0, 12, 0, 16 }, gap = 10 },
		[".doc-title"] = { size = "large", weight = "bold" },
		[".doc-status"] = { size = "small", color = "accent" },
		[".doc-status-idle"] = { size = "small", color = "muted-foreground" },
		[".doc-action"] = { padding = { 0, 8 }, radius = 7, gap = 6 },
		[".doc-action:hover"] = { background = "list-hover" },
		[".doc-rule"] = { background = "border" },
		[".format"] = { background = "secondary", radius = 10, padding = { 0, 8 }, gap = 2, margin = { 0, 12, 10, 12 } },
		[".format-button"] = { radius = 6, color = "muted-foreground" },
		[".format-button:hover"] = { background = "list-hover", color = "foreground" },
		[".format-glyph"] = { weight = "bold" },
		[".format-rule"] = { background = "border", margin = { 0, 4 } },
		[".grid"] = { border = "top", border_color = "border" },
		[".logo"] = { color = "#ffffff", radius = 5 },
		[".kind"] = { background = "secondary", border = true, border_color = "border", radius = 6, padding = { 1, 7 } },
		[".kind-text"] = { size = "small" },
		[".star"] = { color = "#f0a020" },
		[".foot"] = { padding = { 0, 12 }, gap = 6, border = "top", border_color = "border" },
		[".foot-tab"] = { padding = { 0, 10 }, radius = 8, gap = 7 },
		[".foot-tab:hover"] = { background = "list-hover" },
		[".foot-tab:selected"] = { background = "secondary" },
	},

	init = function(): State
		return { selected = nil }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.cell then
			state.selected = msg.cell
		elseif type(msg) == "table" and msg.saved then
			state.saved, state.failed = msg.path, msg.error
		end
	end,

	view = function(state: State, app)
		local data = app.research
		local run = data and data.run

		local function action(icon: string, label: string?)
			return ui.row {
				class = "doc-action",
				height = 30,
				align = "center",
				tooltip = if label then `{label} (not in this example)` else nil,
				ui.icon { name = icon :: any, size = "small" },
				label and ui.text { label },
			}
		end

		local function format(icon: string?, glyph: string?, tip: string)
			return ui.row {
				class = "format-button",
				width = 28,
				height = 28,
				align = "center",
				justify = "center",
				tooltip = tip,
				icon and ui.icon { name = icon :: any, size = "small" },
				glyph and ui.text { glyph, class = "format-glyph" },
			}
		end

		local rows = {}
		local current, selected, caret = nil, state.selected, false
		local status, busy = "Loading…", false
		local count = 0
		if data and run then
			local s = run.step
			count = run.found
			if s == 2 then
				current = run.row
			end
			for index = 1, run.found do
				local item = data.apps[index]
				local row: { [string]: any } = {
					company = ui.row {
						align = "center",
						gap = "normal",
						ui.row {
							class = "logo",
							width = 20,
							height = 20,
							align = "center",
							justify = "center",
							style = { background = item.color },
							ui.icon { name = item.icon :: any, size = "small" },
						},
						ui.text { item.name, truncate = true },
					},
					type = ui.row { class = "kind", hug = true, ui.text { item.type, class = "kind-text" } },
					source = ui.row {
						align = "center",
						gap = "small",
						ui.icon { name = "link", size = "small", class = "quiet" },
						ui.text { item.source, class = "quiet", truncate = true },
					},
				}
				for c, key in KEYS do
					local written = s > 2 or (s == 2 and (index < run.row or (index == run.row and c <= run.cell)))
					if written then
						if key == "rating" then
							row.rating = ui.row { align = "center", gap = "small", ui.icon { name = "star", size = "small", class = "star" }, ui.text { item.rating } }
						else
							row[key] = item[key]
						end
					else
						row[key] = false
					end
				end
				rows[index] = row
			end
			if s == 2 and run.cell > 0 then
				selected = { row = run.row, key = KEYS[run.cell] }
				caret = run.playing
			end
			local n = #data.apps
			busy = run.playing and not run.done
			if run.done then
				status = "Up to date · every number cited"
			elseif not run.playing then
				status = `Paused · step {s} of 4`
			elseif s == 1 then
				status = `Agent is finding apps · {run.found} found`
			elseif s == 2 then
				status = `Agent is writing · row {run.row} of {n}`
			elseif s == 3 then
				status = `Agent is reading reviews · {run.reviewed} of {n}`
			else
				status = "Agent is summarizing"
			end
		end

		local sources = if data then data.sources else 0
		local sheet = sheet_html(data)
		return ui.column {
			class = "bay-body",
			grow = true,
			ui.row {
				class = "tabs-row",
				height = 48,
				shrink = false,
				align = "center",
				ui.row {
					class = "doc-tab",
					height = 32,
					align = "center",
					ui.icon { name = "layout-dashboard", size = "small", class = "quiet" },
					ui.text { "All", class = "quiet" },
					ui.text { "1", class = "small-muted" },
				},
				ui.row {
					class = "doc-tab",
					height = 32,
					align = "center",
					selected = true,
					ui.row { class = "green-tile", width = 16, height = 16, align = "center", justify = "center", ui.icon { name = "columns-2", size = "small" } },
					ui.text { if data then data.sheet else "Sheet", weight = "medium" },
					ui.icon { name = "close", size = "small", class = "doc-close" },
				},
				icon_button { icon = "plus", tooltip = "New tab (not in this example)", msg = "none", size = "small" },
			},
			ui.column {
				class = "doc",
				grow = true,
				gap = "none",
				ui.row {
					class = "doc-head",
					height = 54,
					shrink = false,
					align = "center",
					ui.row { class = "green-tile", width = 20, height = 20, align = "center", justify = "center", ui.icon { name = "columns-2", size = "small" } },
					ui.text { if data then data.sheet else "Sheet", class = "doc-title" },
					ui.row {
						align = "center",
						gap = "small",
						run and run.done and ui.icon { name = "circle-check", size = "small", tone = "success" },
						run and not run.done and ui.icon {
							name = if busy then "loader-circle" else "pause",
							spin = busy,
							size = "small",
							class = if busy then "doc-status" else "doc-status-idle",
						},
						ui.text { status, class = if busy then "doc-status" else "doc-status-idle" },
					},
					ui.spacer(),
					action("filter", "Filter"),
					action("arrow-up-down", "Sort"),
					action("layers", "Group"),
					ui.row { class = "doc-rule", width = 1, height = 18 },
					ui.row {
						class = "doc-action",
						height = 30,
						align = "center",
						to = if sheet then "app:pdf" else nil,
						msg = if sheet then { html = sheet, pdf = "sorrel/competitors.pdf", paper = "a4", reply = { saved = true } } else nil,
						tooltip = if state.failed then `Not saved: {state.failed}`
							elseif state.saved then `Saved as {state.saved}`
							elseif sheet then "Save the sheet as a PDF"
							else "Nothing to save yet",
						ui.icon { name = if state.saved then "check" else "download", size = "small" },
					},
					action("ellipsis", nil),
				},
				ui.row {
					class = "format",
					height = 38,
					shrink = false,
					align = "center",
					format("undo-2", nil, "Undo"),
					format("redo-2", nil, "Redo"),
					ui.row { class = "format-rule", width = 1, height = 16 },
					format("search", nil, "Find"),
					ui.row { class = "format-rule", width = 1, height = 16 },
					format(nil, "B", "Bold"),
					format(nil, "I", "Italic"),
					format(nil, "U", "Underline"),
					ui.row { class = "format-rule", width = 1, height = 16 },
					format("pencil", nil, "Highlight"),
					format("a-large-small", nil, "Text colour"),
					ui.spacer(),
					action("link", `Sources · {sources}`),
					action("settings-2", "Display"),
				},
				ui.column {
					class = "grid",
					grow = true,
					spreadsheet {
						columns = COLUMNS,
						rows = rows,
						current = current,
						selected = selected,
						caret = caret,
						blank = math.max(0, ROWS - #rows),
						grow = true,
					},
				},
				ui.row {
					class = "foot",
					height = 46,
					shrink = false,
					align = "center",
					icon_button { icon = "plus", tooltip = "New sheet (not in this example)", msg = "none", size = "small" },
					ui.row {
						class = "foot-tab",
						height = 30,
						align = "center",
						selected = true,
						ui.row { class = "green-tile", width = 16, height = 16, align = "center", justify = "center", ui.icon { name = "columns-2", size = "small" } },
						ui.text { "Competitors" },
					},
					ui.row {
						class = "foot-tab",
						height = 30,
						align = "center",
						ui.row { class = "green-tile", width = 16, height = 16, align = "center", justify = "center", ui.icon { name = "columns-2", size = "small" } },
						ui.text { `Sources · {sources}`, class = "quiet" },
					},
					ui.spacer(),
					ui.text { `{count} rows · {#COLUMNS} columns`, class = "small-muted" },
				},
			},
		}
	end,
}
