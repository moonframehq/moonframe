-- Upcoming view: toggle between multi-day Kanban-style columns (Board) and
-- a day-grouped vertical list, matching Woday's Upcoming experience.

local nav_buttons = use "nav-buttons"
local pill = use "pill"
local segmented = use "segmented"
local task_check = use "task-check"

type Props = {
	planner: any,
	mode: string?,
	selected_id: string?,
	tags_map: { [string]: string },
	notes_map: { [string]: any },
}

local RANK: { [string]: number } = { open = 1, important = 2, progress = 3, done = 4 }

local function sort_items(items: { any })
	table.sort(items, function(a: any, b: any)
		if a.kind ~= b.kind then
			return a.kind == "event"
		end
		if a.kind == "task" then
			local ra = RANK[a.state] or 9
			local rb = RANK[b.state] or 9
			if ra ~= rb then
				return ra < rb
			end
		end
		local ha: number = a.start_hour or 99
		local hb: number = b.start_hour or 99
		if ha ~= hb then
			return ha < hb
		end
		local ta = tostring(a.title or "")
		local tb = tostring(b.title or "")
		if ta ~= tb then
			return ta < tb
		end
		return tostring(a.id or "") < tostring(b.id or "")
	end)
end

return component {
	title = "Daybook Upcoming",
	category = "Daybook",
	icon = "calendar",
	description = "Upcoming tasks and events in multi-day Board columns or a vertical List.",
	styles = {
		[".up-head"] = { gap = 8 },
		[".up-title"] = { size = "title", weight = "bold" },
		[".up-sub"] = { color = "muted-foreground" },
		[".up-today-btn"] = { radius = 14, padding = { 5, 12 }, background = "list-hover", color = "muted-foreground" },
		[".up-today-btn:hover"] = { background = "accent/12", color = "foreground" },
		[".up-board"] = { gap = 14, padding = { 4, 2, 8, 2 } },
		[".up-col"] = { gap = 8, padding = { 0, 2 } },
		[".up-col-head"] = { padding = { 4, 4, 6, 4 }, gap = 6, border = "bottom", border_color = "border/60" },
		[".up-col-day"] = { weight = "semibold", size = "normal" },
		[".up-col-today"] = { color = "danger", weight = "semibold" },
		[".up-col-date"] = { size = "small", color = "muted-foreground" },
		[".up-card"] = { background = "surface-soft", radius = 12, padding = { 10, 12 }, gap = 6, border = "all", border_color = "#00000000" },
		[".up-card:hover"] = { background = "list-hover", border_color = "border" },
		[".up-card:selected"] = { background = "card", border_color = "foreground/70", shadow = "0 2px 8px #00000014" },
		[".up-card-title"] = { weight = "medium" },
		[".up-card-done"] = { color = "muted-foreground" },
		[".up-meta"] = { gap = 6 },
		[".up-time"] = { size = "small", color = "muted-foreground" },
		[".up-time-due"] = { size = "small", color = "danger", weight = "medium" },
		[".up-dot"] = { background = "danger", radius = 4 },
		[".up-note-chip"] = { background = "background", border = true, border_color = "border", radius = 6, padding = { 1, 6 }, gap = 4 },
		[".up-note-chip:hover"] = { border_color = "foreground/35" },
		[".up-list-day"] = { gap = 16, padding = { 6, 0, 12, 0 }, border = "bottom", border_color = "border/50" },
		[".up-list-gutter"] = { padding = { 8, 0, 0, 0 }, gap = 2 },
		[".up-list-cards"] = { gap = 6 },
		[".up-empty"] = { size = "small", color = "muted-foreground", padding = { 8, 4 } },
	},
	example = { planner = { items = {}, day_details = {}, today = "2026-10-31" }, tags_map = {}, notes_map = {} },

	build = function(props: Props)
		local planner = props.planner
		local mode = props.mode or "board"
		local details: { any } = planner.day_details or {}
		local tags_map = props.tags_map or {}
		local notes_map = props.notes_map or {}

		local by_date: { [string]: { any } } = {}
		for _, item in planner.items or {} do
			if item.kind ~= "note" and item.date then
				by_date[item.date] = by_date[item.date] or {}
				table.insert(by_date[item.date], item)
			end
		end
		for _, bucket in by_date do
			sort_items(bucket)
		end

		local function item_card(item: any): Element
			local done = item.state == "done"
			local lead: Element = if item.kind == "event"
				then ui.row {
					width = 22,
					height = 22,
					align = "center",
					justify = "center",
					shrink = false,
					ui.row { class = "up-dot", width = 8, height = 8 },
				}
				else task_check { state = item.state, msg = { toggle = item.id } }

			local meta: { Element } = {}
			if item.habit then
				table.insert(meta, ui.icon { name = "zap", size = "small", class = "up-time" })
			end
			if item.time then
				table.insert(meta, ui.text { item.time, class = if item.due then "up-time-due" else "up-time" })
			end
			if item.days_left and not done then
				table.insert(meta, pill { label = item.days_left, tone = "neutral" })
			end
			local linked = item.note_id and notes_map[item.note_id]
			if linked then
				table.insert(meta, ui.row {
					class = "up-note-chip",
					align = "center",
					hug = true,
					msg = { open_note = linked.id },
					ui.text { linked.emoji or "📝", size = "small" },
					ui.text { linked.title, size = "small", truncate = true },
				})
			end
			for _, tag_name in item.tags or {} do
				table.insert(meta, pill { label = tag_name, color = tags_map[tag_name] })
			end

			return ui.row {
				class = "up-card",
				align = "start",
				selected = props.selected_id == item.id,
				msg = { inspect = item.id },
				context = if item.kind == "task" then { cycle = item.id } else nil,
				lead,
				ui.column {
					grow = true,
					gap = "small",
					ui.text {
						item.title,
						class = if done then { "up-card-title", "up-card-done" } else "up-card-title",
						strike = done,
						lines = 2,
					},
					#meta > 0 and ui.row { class = "up-meta", align = "center", wrap = true, table.unpack(meta) },
				},
			}
		end

		local subtitle = "Next 14 days"
		if mode == "board" and #details >= 4 then
			subtitle = `{details[1].short_date} – {details[math.min(5, #details)].short_date}`
		end

		local header = ui.row {
			class = "up-head",
			align = "start",
			ui.column {
				gap = "none",
				ui.text { "Upcoming", class = "up-title" },
				ui.text { subtitle, class = "up-sub" },
			},
			ui.spacer(),
			ui.row {
				align = "center",
				gap = "small",
				segmented {
					selected = mode,
					event = "upcoming_mode",
					small = true,
					options = {
						{ id = "board", icon = "columns-2", tooltip = "Board columns" },
						{ id = "list", icon = "list", tooltip = "List view" },
					},
				},
				ui.row {
					class = "up-today-btn",
					msg = { select = planner.today },
					ui.text { "Today", size = "small", weight = "medium" },
				},
				nav_buttons { back = { shift = -1 }, forward = { shift = 1 }, to = "provider:planner", small = true },
			},
		}

		local content: Element
		if mode == "board" then
			local cols: { Element } = {}
			local limit = math.min(7, #details)
			for i = 1, limit do
				local d = details[i]
				local list = by_date[d.id] or {}
				local cards: { Element } = {}
				for _, item in list do
					table.insert(cards, item_card(item))
				end
				if #cards == 0 then
					table.insert(cards, ui.text { "Nothing scheduled", class = "up-empty" })
				end
				local day_name = if d.today then "Today" else d.full_weekday
				table.insert(cols, ui.column {
					class = "up-col",
					width = 236,
					shrink = false,
					ui.row {
						class = "up-col-head",
						align = "center",
						ui.text { day_name, class = if d.today then "up-col-today" else "up-col-day" },
						ui.spacer(),
						ui.text { d.short_date, class = "up-col-date" },
					},
					ui.column {
						grow = true,
						scroll = true,
						gap = "small",
						table.unpack(cards),
					},
				})
			end
			content = ui.row {
				class = "up-board",
				grow = true,
				scroll = "x",
				align = "stretch",
				table.unpack(cols),
			}
		else
			local sections: { Element } = {}
			for _, d in details do
				local list = by_date[d.id] or {}
				if #list > 0 or d.today then
					local cards: { Element } = {}
					for _, item in list do
						table.insert(cards, item_card(item))
					end
					if #cards == 0 then
						table.insert(cards, ui.text { "Nothing scheduled today", class = "up-empty" })
					end
					table.insert(sections, ui.row {
						class = "up-list-day",
						align = "start",
						ui.column {
							class = "up-list-gutter",
							width = 96,
							shrink = false,
							ui.text { if d.today then "Today" else d.full_weekday, class = if d.today then "up-col-today" else "up-col-day" },
							ui.text { d.short_date, class = "up-col-date" },
						},
						ui.column {
							class = "up-list-cards",
							grow = true,
							table.unpack(cards),
						},
					})
				end
			end
			content = ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				if #sections == 0 then ui.text { "Nothing coming up", class = "up-empty" } else nil,
				table.unpack(sections),
			}
		end

		return ui.column { grow = true, gap = "normal", header, content }
	end,
}
