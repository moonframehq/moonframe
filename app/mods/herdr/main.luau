-- Offers to install herdr-core when this machine lacks it: a toast in the
-- window's corner, from what the `herdr` provider sees. herdr-core keeps
-- terminals and what mods keep across launches; without it terminals run on
-- tmux, or end with the app. Install runs herdr-core's own installer; ×
-- hides the offer for good (`persist`).
local toast = use "toast"

type State = { dismissed: boolean, asked: boolean }

return mod {
	region = "status-right",
	order = 100,
	persist = { "dismissed" },

	init = function(previous: State?): State
		return { dismissed = previous ~= nil and previous.dismissed or false, asked = false }
	end,

	update = function(state: State, msg)
		if msg == "install" then
			state.asked = true
			return state, { { to = "provider:herdr", msg = "install" } }
		elseif type(msg) == "table" and msg.dismiss then
			state.dismissed = true
			state.asked = false
		end
		return nil
	end,

	view = function(state: State, app)
		local herdr = app.herdr
		local card: any = nil
		if herdr == nil then
			card = nil
		elseif herdr.installing then
			card = toast {
				id = "herdr",
				icon = "loader-circle",
				source = "herdr-core",
				title = "Installing herdr-core…",
				body = "Running its installer into ~/.local/bin",
				dismiss = false,
			}
		elseif herdr.error and state.asked then
			card = toast {
				id = "herdr",
				level = "error",
				source = "herdr-core",
				title = "herdr-core did not install",
				body = herdr.error,
				full = true,
				action = { label = "Try again", msg = "install" },
			}
		elseif herdr.fresh and state.asked then
			card = toast {
				id = "herdr",
				level = "success",
				source = "herdr-core",
				title = `herdr-core {herdr.version or ""} is installed`,
				body = "New terminals run on it, and the app keeps its state there.",
				full = true,
			}
		elseif not herdr.installed and not state.dismissed then
			card = toast {
				id = "herdr",
				source = "Terminals",
				title = "Keep terminals in herdr-core",
				body = (if herdr.tmux
					then "They run on tmux now. "
					else "They end when the app quits now. ")
					.. "herdr-core keeps them running, shows what agents in them are doing, and keeps the app's state.",
				full = true,
				action = { label = "Install", msg = "install" },
			}
		end
		return ui.overlay {
			open = card ~= nil,
			place = "bottom-end",
			content = card or ui.text "",
		}
	end,
}
