-- A live demo of the `browser` component: a mod that keeps the address and
-- title the page reports (its `navigate` messages) and shows them in the bar.
-- Copy it to mods/<name>/main.luau to try it in the window.
local browser = use "browser"

type State = {
	url: string,
	at: string?,
	title: string?,
	loading: boolean,
}

return mod {
	region = "main",
	title = "Browser demo",
	init = function(previous: State?): State
		return previous or { url = "https://example.com", loading = false }
	end,
	update = function(state: State, msg: any)
		if type(msg) ~= "table" then
			return
		end
		if msg.navigated then
			state.at = msg.url
			state.title = msg.title
		elseif msg.loaded ~= nil then
			state.loading = msg.loading
		end
	end,
	view = function(state: State)
		return ui.panel {
			title = state.loading and "Browser (loading)" or "Browser",
			grow = true,
			padding = "none",
			browser {
				id = "demo",
				url = state.url,
				at = state.at,
				title = state.title,
				navigate = { navigated = true },
				loading = { loaded = true },
			},
		}
	end,
}
