-- The bar across the top: how the engine stands, the cup, and two ways to
-- disturb it. This is a mod of its own so that the cup's hold on the machine
-- is separate from the scan's: each mod holds for itself.
local caffeine = use "caffeine"
local pill = use "pill"

return mod {
	region = "top",

	update = function(state, msg)
		if msg == "crash" then
			return state, { { to = "provider:scanner", msg = "crash" } }
		elseif msg == "restart" then
			return state, { { to = "app:restart", msg = "scanner" } }
		end
	end,

	view = function(_, app)
		local scanner: any = (app :: any).scanner or {}
		local links: any = app.links or {}
		local link: any = links.scanner or {}
		local awake: any = (app :: any).awake
		local crashes = scanner.crashes or 0
		return ui.row {
			class = "top-bar",
			grow = true,
			drag = true,
			align = "center",
			ui.text { "Lightbox", weight = "semibold" },
			pill {
				label = if link.connected then "Engine running" else "Engine down",
				tone = if link.connected then "success" else "danger",
			},
			if crashes > 0
				then pill { label = `Recovered from {crashes} crash{if crashes == 1 then "" else "es"}`, tone = "warning" }
				else nil,
			ui.spacer(),
			ui.text { if awake and awake.held then "Staying awake" else "May sleep", size = "small", tone = "muted" },
			caffeine { awake = awake, scope = "system", reason = "Lightbox, by hand", disabled = scanner.running },
			ui.button { "Restart engine", msg = "restart" },
			ui.button { "Crash engine", msg = "crash" },
		}
	end,
}
