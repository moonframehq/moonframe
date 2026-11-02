-- Learn's home: a header with the guide search and My progress (a sheet),
-- the featured course, the guides by category with their progress, and on
-- the right your progress, lessons per week and live sessions. Its
-- messages go to the learn mod. Inkwell only.

local course_card = use "course-card"
local guide_card = use "guide-card"
local heat_grid = use "heat-grid"
local segmented = use "segmented"
local dropdown = use "dropdown"
local sheet = use "sheet"
local learn_search = use "learn-search"
local learn_sessions = use "learn-sessions"

-- The learn mod's state (mods/learn/main.luau has its fields).
type State = any

type Props = { state: State, data: any }

local SORTS: { any } = {
	{ id = "popular", label = "Most popular" },
	{ id = "newest", label = "Newest" },
	{ id = "shortest", label = "Shortest" },
}

local function sort_label(id: string): string
	for _, sort in SORTS do
		if sort.id == id then
			return sort.label
		end
	end
	return "Most popular"
end

local function category_of(data: any, id: string): any
	for _, category in data.categories do
		if category.id == id then
			return category
		end
	end
	return { id = id, label = id, color = "#7c5cff" }
end

local function read_of(state: State, guide: any): number
	return state.read[guide.id] or guide.progress or 0
end

return component {
	title = "Learn Home",
	category = "Inkwell",
	icon = "list-checks",
	description = "The Learn page's home: the featured course, the guides, your progress and live sessions.",
	when = "The home screen of Inkwell's Learn page.",
	props = {
		["state"] = "the learn mod's state",
		["data"] = "`app.inkwell.learn`, the catalogue",
	},
	example = {
		state = { query = "", hi = 1, searching = false, filter = "all", sort = "popular", sorting = false, all = false, completed = 1, lesson = 1, watched = 0.5, playing = false, tab = "overview", bookmarked = false, guide = "welcome", read = {}, registered = {}, calendar = true, recording = true, progress_open = false },
		data = {
			categories = { { id = "growth", label = "Growth", color = "#7c5cff" } },
			guides = { { id = "welcome", title = "Your welcome email", summary = "Greet new readers.", category = "growth", icon = "sparkles", minutes = 5, rank = 1, steps = { "Say hello", "Set expectations" } } },
			help = {},
			course = { title = "Newsletter foundations", meta = "8 lessons", activity = { 1, 2, 3 }, resources = { { title = "Checklist", detail = "PDF", icon = "file-text", color = "#e5484d", action = "download" } }, lessons = { { title = "Write a welcome email", hook = "First impressions", chart = "Opens by day", seconds = 300, time = "5:00", kind = "Video", summary = "Why the first email matters.", takeaways = { "Say what to expect" } } } },
			stats = { lessons = 3, lessons_total = 8, streak = 2, certificates = 0, courses = 1 },
			weeks = { 1, 0, 2 },
			weeks_from = "Feb",
			enrolled = { { title = "Newsletter foundations", total = 8, done = 1, color = "#7c5cff" } },
			sessions = { { id = "live-1", title = "Grow with referrals", where = "Live on Zoom", date = "Thursday, 14 May", month = "May", day = "14", when = "Thu 6pm", host = "Ada Park", role = "Editor", people = { "Ben", "Cy" }, registered = 12, length = "45 min" } },
		},
	},

	build = function(props: Props)
		local state = props.state
		local data = props.data
		local course = data.course
		local lessons = course.lessons
		local total = #lessons
		local next_lesson = math.min(state.completed + 1, total)

		local function stat_box(icon: string, color: string, label: string, value: string, unit: string?)
			return ui.column {
				class = "stat-box",
				grow = true,
				ui.row {
					align = "center",
					gap = "small",
					ui.icon { name = icon :: any, size = "small", style = { color = color } },
					ui.text { label, class = "small-muted" },
				},
				ui.row {
					align = "end",
					style = { gap = 2 },
					ui.text { value, class = "stat-value" },
					unit and ui.text { unit, class = "small-muted" },
				},
			}
		end

		local function weeks_grid(size: number)
			local tips = {}
			for index, count in data.weeks do
				tips[index] = `{count} lesson{if count == 1 then "" else "s"}`
			end
			return heat_grid { rows = 3, size = size, spread = true, columns = data.weeks, tooltips = tips, labels = { data.weeks_from, "This week" } }
		end

		local function stats_row()
			local stats = data.stats
			return ui.row {
				gap = "normal",
				stat_box("book-open", "#7c5cff", "Lessons", `{stats.lessons - 3 + state.completed}`, `/{stats.lessons_total}`),
				stat_box("zap", "#f08a24", "Streak", `{stats.streak}`, "weeks"),
				stat_box("shield", "#14a38b", "Certificates", `{stats.certificates + (if state.completed >= total then 1 else 0)}`),
			}
		end

		-- My progress: a sheet with the figures, the weeks and every course.
		local function progress_sheet()
			local rows = {}
			for index, enrolled in data.enrolled do
				local done = if index == 1 then state.completed else enrolled.done
				local finished = done >= enrolled.total
				table.insert(rows, ui.column {
					class = "course-row",
					ui.row {
						align = "center",
						ui.text { enrolled.title, weight = "medium" },
						ui.spacer(),
						finished and ui.row {
							class = "chip",
							align = "center",
							style = { background = "#14a38b1f", color = "#14a38b" },
							ui.icon { name = "shield", size = "small" },
							ui.text { "Certificate", size = "small" },
						},
					},
					ui.row {
						align = "center",
						gap = "normal",
						heat_grid {
							levels = 2,
							size = 12,
							gap = 3,
							color = enrolled.color,
							columns = (function()
								local cells = {}
								for step = 1, enrolled.total do
									cells[step] = { if step <= done then 2 else 0 }
								end
								return cells
							end)(),
						},
						ui.spacer(),
						ui.text { if finished then enrolled.certificate or "Done" else `{done} of {enrolled.total} lessons`, class = "small-muted" },
					},
				})
			end
			return sheet {
				open = state.progress_open,
				dismiss = "close-progress",
				title = "My progress",
				subtitle = `Across {data.stats.courses} enrolled courses.`,
				icon = "list-checks",
				width = 420,
				footer = ui.row {
					ui.spacer(),
					ui.row {
						class = "dark-button",
						align = "center",
						msg = { lesson = next_lesson, title = course.title },
						ui.icon { name = "play", size = "small" },
						ui.text { if state.completed >= total then "Review the course" else `Continue lesson {next_lesson}`, weight = "medium" },
					},
				},
				ui.column {
					gap = "large",
					stats_row(),
					ui.column {
						gap = "normal",
						ui.row { ui.text { "Lessons per week", weight = "medium" }, ui.spacer(), ui.text { "Last 12 weeks", class = "small-muted" } },
						weeks_grid(18),
					},
					ui.column { gap = "none", ui.text { "Courses", class = "eyebrow" }, table.unpack(rows) },
				},
			}
		end

		local search_field = learn_search { state = state, data = data }

		local head = ui.row {
			class = "learn-head",
			align = "start",
			ui.column {
				gap = "small",
				ui.text { "Learn", class = "learn-title" },
				ui.text { "Guides, courses and live sessions to grow your newsletter.", class = "muted" },
			},
			ui.spacer(),
			search_field,
			ui.row {
				class = "pill-button",
				height = 38,
				align = "center",
				style = { radius = 10 },
				msg = "progress",
				ui.icon { name = "list-checks", size = "small" },
				ui.text { "My progress" },
			},
		}

		local finished = state.completed >= total
		local featured = ui.column {
			style = { padding = { 0, 24 } },
			course_card {
				eyebrow = "Featured course",
				title = course.title,
				meta = course.meta,
				primary = {
					label = if finished then "Review the course" else `Continue lesson {next_lesson}`,
					icon = "play",
					msg = { lesson = next_lesson, title = course.title },
				},
				secondary = { label = "View syllabus", icon = "book-open", msg = { lesson = state.lesson, title = course.title } },
				progress = state.completed / total,
				label = `{state.completed} of {total} lessons`,
				bar = 250,
				aside = heat_grid { rows = 6, size = 15, columns = course.activity },
			},
		}

		-- Guides: the category filter, the order, then three to a row.
		local options: { any } = { { id = "all", label = "All" } }
		for _, category in data.categories do
			table.insert(options, { id = category.id, label = category.label })
		end
		local shown = {}
		for _, guide in data.guides do
			if state.filter == "all" or guide.category == state.filter then
				table.insert(shown, guide)
			end
		end
		local matching = #shown
		table.sort(shown, function(a, b)
			if state.sort == "newest" then
				if (a.new or false) ~= (b.new or false) then
					return a.new == true
				end
			elseif state.sort == "shortest" and a.minutes ~= b.minutes then
				return a.minutes < b.minutes
			end
			return a.rank < b.rank
		end)
		local limit = if state.all then #shown else math.min(6, #shown)
		local grid = {}
		local row = {}
		for index = 1, limit do
			local guide = shown[index]
			local category = category_of(data, guide.category)
			local read = read_of(state, guide)
			table.insert(row, guide_card {
				title = guide.title,
				category = category.label,
				color = category.color,
				icon = guide.icon,
				read = `{guide.minutes} min read`,
				done = read >= 1,
				new = guide.new and read == 0,
				progress = read,
				msg = { guide = guide.id, title = guide.title },
			})
			if #row == 3 or index == limit then
				while #row < 3 do
					table.insert(row, ui.column { grow = true })
				end
				table.insert(grid, ui.row { gap = "large", table.unpack(row) })
				row = {}
			end
		end

		local guides = ui.column {
			class = "guides",
			grow = true,
			ui.column {
				gap = "small",
				ui.text { "Guides", class = "heading" },
				ui.text { "Short, practical reads you can finish between sends.", class = "muted" },
			},
			ui.row {
				align = "center",
				segmented { options = options, selected = state.filter, event = "filter" },
				ui.spacer(),
				dropdown {
					value = sort_label(state.sort),
					options = SORTS,
					selected = state.sort,
					open = state.sorting,
					toggle = "toggle-sort",
					dismiss = "close-sort",
					event = "sort",
					align = "end",
					width = 180,
				},
			},
			#grid == 0 and ui.column {
				class = "placeholder",
				align = "center",
				ui.text { "No guides in this category yet.", class = "muted" },
			},
			ui.column { gap = "large", table.unpack(grid) },
			ui.row {
				align = "center",
				style = { margin = { top = 6 } },
				ui.text { `Showing {limit} of {matching} guides`, class = "muted" },
				ui.spacer(),
				matching > 6 and ui.row {
					class = "pill-button",
					style = { radius = 10 },
					msg = "browse",
					ui.text { if state.all then "Show fewer" else "Browse all guides", weight = "medium" },
				},
			},
		}

		local sessions = learn_sessions { state = state, data = data }

		local side = ui.column {
			class = "side",
			width = 400,
			shrink = false,
			ui.column {
				gap = "small",
				ui.text { "Your progress", class = "heading" },
				ui.text { `Across {data.stats.courses} enrolled courses.`, class = "muted" },
			},
			stats_row(),
			ui.column {
				gap = "normal",
				style = { margin = { top = 4 } },
				ui.row {
					align = "center",
					ui.text { "Lessons per week", weight = "medium" },
					ui.spacer(),
					ui.text { "Last 12 weeks", class = "small-muted" },
				},
				weeks_grid(16),
			},
			ui.row { class = "rule", height = 1, style = { margin = { 10, 0 } } },
			ui.column {
				gap = "small",
				ui.text { "Upcoming live sessions", class = "heading" },
				ui.text { "Join live, ask questions, get the recording.", class = "muted" },
			},
			sessions,
		}

		return ui.column {
			class = "learn",
			grow = true,
			scroll = true,
			ui.column {
				gap = "none",
				shrink = false,
				head,
				featured,
				ui.row { class = "body", guides, side },
			},
			progress_sheet(),
		}
	end,
}
