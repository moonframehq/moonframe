-- Learn's upcoming live sessions, each with Register and its card (the
-- calendar and recording switches, Cancel and Register). Its messages go
-- to the learn mod. Inkwell only.

local switch = use "switch"
local avatar = use "avatar"
local avatar_stack = use "avatar-stack"

-- The learn mod's state (mods/learn/main.luau has its fields).
type State = any

type Props = { state: State, data: any }

return component {
	title = "Learn Sessions",
	category = "Inkwell",
	icon = "calendar",
	description = "The Learn page's upcoming live sessions, each with its Register card.",
	when = "The side of Inkwell's Learn page. Elsewhere use `list`.",
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

		-- Upcoming live sessions, each with Register and its card.
		local sessions = {}
		for index, session in data.sessions do
			if index > 1 then
				table.insert(sessions, ui.row { class = "rule", height = 1 })
			end
			local registered = state.registered[session.id]
			local card = ui.column {
				class = "card-pop",
				ui.row {
					class = "card-pop-head",
					align = "center",
					ui.row { class = "card-pop-tile", width = 38, height = 38, align = "center", justify = "center", ui.icon { name = "monitor" } },
					ui.column {
						gap = "none",
						ui.text { session.title, weight = "medium" },
						ui.text { session.where, class = "small-muted" },
					},
				},
				ui.row {
					class = "card-pop-date",
					align = "center",
					ui.icon { name = "calendar", size = "small", class = "muted" },
					ui.text { session.date, weight = "medium" },
				},
				ui.row {
					class = "card-pop-host",
					align = "center",
					avatar { name = session.host, shape = "circle", size = "large" },
					ui.column {
						gap = "none",
						ui.text { session.host },
						ui.text { `{session.role} · Host`, class = "small-muted" },
					},
				},
				ui.row {
					class = "card-pop-people",
					align = "center",
					avatar_stack {
						people = (function()
							local people = {}
							for _, name in session.people do
								table.insert(people, { name = name })
							end
							return people
						end)(),
						count = session.registered,
						most = 4,
					},
					ui.text { "writers registered", class = "small-muted" },
				},
				ui.row { class = "rule", height = 1 },
				ui.row {
					class = "card-pop-option",
					align = "center",
					style = { margin = { top = 6 } },
					ui.column {
						grow = true,
						gap = "none",
						ui.text { "Add to my calendar" },
						ui.text { "A calendar invite with the video link", class = "small-muted" },
					},
					switch { on = state.calendar, msg = "toggle-calendar" },
				},
				ui.row {
					class = "card-pop-option",
					align = "center",
					ui.column {
						grow = true,
						gap = "none",
						ui.text { "Email me the recording" },
						ui.text { "Sent within 24 hours of the session", class = "small-muted" },
					},
					switch { on = state.recording, msg = "toggle-recording" },
				},
				ui.row {
					class = "card-pop-foot",
					align = "center",
					ui.text { `Free · {session.length}`, class = "muted" },
					ui.spacer(),
					ui.row { class = "pill-button", msg = "cancel-register", ui.text { "Cancel" } },
					ui.row {
						class = "dark-button",
						align = "center",
						msg = "confirm-register",
						ui.icon { name = "calendar", size = "small" },
						ui.text { "Register", weight = "medium" },
					},
				},
			}
			table.insert(sessions, ui.popover {
				open = state.registering == session.id,
				dismiss = "cancel-register",
				side = "left",
				align = "end",
				gap = 12,
				width = 420,
				class = "card-popover",
				content = card,
				ui.row {
					class = "session",
					align = "center",
					selected = state.registering == session.id,
					ui.column {
						class = "date",
						width = 46,
						shrink = false,
						align = "center",
						ui.text { session.month, class = "date-month" },
						ui.text { session.day, class = "date-day" },
					},
					ui.column {
						grow = true,
						gap = "none",
						ui.text { session.title, weight = "medium", truncate = true },
						ui.text { `{session.when} · Host: {session.host}`, class = "small-muted", truncate = true },
					},
					ui.row {
						class = if registered then { "register", "registered" } else "register",
						align = "center",
						shrink = false,
						msg = if registered then nil else { register = session.id },
						tooltip = if registered then "You're registered; the invite is in your inbox" else nil,
						registered and ui.icon { name = "check", size = "small" },
						ui.text { if registered then "Registered" else "Register", weight = "medium" },
					},
				},
			})
		end

		return ui.column { gap = "none", table.unpack(sessions) }
	end,
}
