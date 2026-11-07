-- A new thread: what is missing to run pi, if anything, then the composer on
-- its project. The steps of getting started read as one page: add a project,
-- connect a model.

local empty_state = use "empty-state"

local function step(icon: string, title: string, text: string, action: any): any
	return ui.row {
		class = "start-step",
		align = "center",
		ui.icon { icon :: any, class = "start-step-icon" },
		ui.column { grow = true, gap = "none", ui.text { title, weight = "medium" }, ui.text { text, tone = "muted", size = "small", wrap = true } },
		action,
	}
end

return mod {
	styles = {
		[".start-sheet"] = { gap = 14 },
		[".start-title"] = { size = "headline", weight = "semibold", padding = { 0, 32, 4, 32 } },
		[".start-step"] = { border = true, radius = 12, padding = { 12, 14 }, gap = 12, margin = { 0, 32 }, background = "card" },
		[".start-step-icon"] = { color = "accent" },
	},

	view = function(_, app: any, props: any)
		if props == nil then
			return ui.column { width = 0, height = 0 }
		end
		local pi = app.pi
		if pi == nil then
			return ui.column { grow = true, justify = "center", empty_state { icon = "loader-circle", title = "Starting…" } }
		end
		if pi.available == false then
			return ui.column { grow = true, justify = "center", empty_state { icon = "circle-alert", title = "Pilot can't start", message = pi.reason } }
		end
		local steps: { any } = {}
		if #pi.models == 0 then
			table.insert(steps, step("key-round", "Connect a model", "Add an API key or sign in with a provider.", ui.button { "Open settings", msg = { page = "settings" }, to = "shell" }))
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row { drag = true, height = 52, shrink = false },
			ui.column {
				grow = true,
				justify = "center",
				ui.row {
					justify = "center",
					ui.column {
						class = "start-sheet",
						grow = true,
						max_width = 860,
						ui.text { "What should we work on?", class = "start-title" },
						ui.column { gap = "small", shrink = false, table.unpack(steps) },
						ui.mount { "composer", id = "new", props = { draft = true } },
					},
				},
			},
		}
	end,
}
