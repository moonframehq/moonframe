-- A live demo of the `browser` component: a mod that keeps what the page
-- last reported (its `navigate` message) and hands it back to the bar,
-- which shows the address, title, history and loading from it. The address
-- field navigates on its own. Copy it to mods/<name>/main.luau to try it in
-- the window.
local browser = use "browser"

type State = {
	url: string,
	page: any,
}

return mod {
	region = "main",
	title = "Browser demo",
	init = function(previous: State?): State
		return previous or { url = "https://example.com" }
	end,
	update = function(state: State, msg: any)
		if type(msg) == "table" and msg.navigated then
			state.page = msg
		end
	end,
	view = function(state: State)
		local page = state.page
		return ui.panel {
			title = if page and page.loading then "Browser (loading)" else "Browser",
			grow = true,
			padding = "none",
			browser {
				id = "demo",
				url = state.url,
				page = page,
				navigate = { navigated = true },
			},
		}
	end,
}
