-- A guide of Learn's: its steps, how far you have read, Mark as read and
-- Keep reading, and two more to read beside it. Its messages go to the
-- learn mod. Inkwell only.

local guide_card = use "guide-card"
local numbered_list = use "numbered-list"

-- The learn mod's state (mods/learn/main.luau has its fields).
type State = any

type Props = { state: State, data: any }

local function category_of(data: any, id: string): any
	for _, category in data.categories do
		if category.id == id then
			return category
		end
	end
	return { id = id, label = id, color = "#7c5cff" }
end

local function guide_by_id(data: any, id: string?): any
	for _, guide in data.guides do
		if guide.id == id then
			return guide
		end
	end
	return nil
end

local function read_of(state: State, guide: any): number
	return state.read[guide.id] or guide.progress or 0
end

return component {
	title = "Learn Guide",
	category = "Inkwell",
	icon = "book-open",
	description = "A guide of the Learn page, with two more to read beside it.",
	when = "The guide screen of Inkwell's Learn page.",
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

		local guide = guide_by_id(data, state.guide)
		if not guide then
			return ui.column {
				class = "placeholder",
				align = "center",
				ui.text { "That guide has moved.", class = "muted" },
				ui.button { "Back to Learn", msg = { screen = "home" } },
			}
		end
		local category = category_of(data, guide.category)
		local read = read_of(state, guide)
		local done = read >= 1
		-- Two more to read: the same topic first, then the most popular.
		local picks = {}
		for _, other in data.guides do
			if other.id ~= guide.id and other.category == guide.category and #picks < 2 then
				table.insert(picks, other)
			end
		end
		for _, other in data.guides do
			if other.id ~= guide.id and other.category ~= guide.category and #picks < 2 then
				table.insert(picks, other)
			end
		end
		local related = {}
		for _, other in picks do
			do
				local other_read = read_of(state, other)
				local other_category = category_of(data, other.category)
				table.insert(related, ui.row {
					height = 236,
					guide_card {
						title = other.title,
						category = other_category.label,
						color = other_category.color,
						icon = other.icon,
						read = `{other.minutes} min read`,
						done = other_read >= 1,
						new = other.new and other_read == 0,
						progress = other_read,
						msg = { guide = other.id, title = other.title },
					},
				})
			end
		end
		local bar = 120
		local article = ui.column {
			grow = true,
			scroll = true,
			ui.column {
				class = "lesson-main",
				shrink = false,
				max_width = 820,
				ui.row {
					class = "back",
					hug = true,
					align = "center",
					msg = { screen = "home" },
					ui.icon { name = "arrow-left", size = "small" },
					ui.text { "All guides" },
				},
				ui.row {
					class = "guide-band",
					height = 150,
					align = "center",
					justify = "center",
					style = { background = `{category.color}/10` },
					ui.icon { name = guide.icon :: any, size = "title", style = { color = category.color } },
				},
				ui.text { category.label, class = "eyebrow", style = { color = category.color } },
				ui.text { guide.title, class = "learn-title" },
				ui.row {
					align = "center",
					gap = "large",
					ui.row { align = "center", gap = "small", ui.icon { name = "clock", size = "small", class = "muted" }, ui.text { `{guide.minutes} min read`, class = "muted" } },
					ui.row {
						align = "center",
						gap = "normal",
						ui.row {
							width = bar,
							height = 4,
							style = { background = "muted-foreground/14", radius = 999 },
							read > 0 and ui.row { width = math.max(4, bar * read), height = 4, style = { background = if done then "success" else "accent", radius = 999 } },
						},
						ui.text { if done then "Done" else `{math.floor(read * 100 + 0.5)}% read`, class = if done then nil else "muted", style = if done then { color = "success" } else nil },
					},
				},
				ui.text { guide.summary, class = "prose", size = "large" },
				ui.column {
					gap = "normal",
					style = { margin = { top = 8 } },
					ui.text { "In this guide", weight = "medium" },
					numbered_list { items = guide.steps },
				},
				ui.row {
					class = "cert",
					align = "center",
					style = { margin = { top = 4 } },
					ui.row {
						class = "cert-tile",
						width = 38,
						height = 38,
						shrink = false,
						align = "center",
						justify = "center",
						style = { background = `{category.color}/14`, color = category.color },
						ui.icon { name = "sparkles" },
					},
					ui.column {
						gap = "none",
						ui.text { "Try it before your next send", weight = "medium" },
						ui.text { guide.steps[1], class = "muted" },
					},
				},
				ui.row {
					gap = "normal",
					style = { margin = { top = 10 } },
					ui.row {
						class = "dark-button",
						align = "center",
						msg = if done then { screen = "home" } else { read_all = guide.id },
						ui.icon { name = if done then "arrow-left" else "circle-check", size = "small" },
						ui.text { if done then "Back to guides" else "Mark as read", weight = "medium" },
					},
					not done and ui.row {
						class = "pill-button",
						align = "center",
						msg = { read_more = guide.id, from = read },
						ui.icon { name = "book-open", size = "small" },
						ui.text { "Keep reading" },
					},
				},
			},
		}
		local side = ui.column {
			class = "side",
			width = 380,
			shrink = false,
			style = { padding = 24 },
			ui.column {
				gap = "small",
				ui.text { `More in {category.label}`, class = "heading" },
				ui.text { "Pick up where this one leaves off.", class = "muted" },
			},
			ui.column { gap = "large", table.unpack(related) },
		}
		return ui.row { grow = true, gap = "none", article, side }
	end,
}
