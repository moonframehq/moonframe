-- Offers to install herdr-core when this machine lacks it: a toast in the
-- window's corner, from what the `herdr` provider sees. herdr-core keeps
-- terminals and what mods keep across launches; without it terminals run on
-- tmux, or end with the app. Install runs herdr-core's own installer; ×
-- hides the offer for good: the state is saved, and `init` keeps only that.
-- Whether the offer is up goes out on the `offers/herdr` topic, so other
-- offers in the same corner (mods/skills) wait for it.
local toast = use "toast"

type State = { dismissed: boolean, asked: boolean, herdr: any, showing: boolean }

-- The card for what the provider says, or nil when there is nothing to offer.
local function card_for(state: State, herdr: any): any
	if herdr == nil then
		return nil
	elseif herdr.installing then
		return toast {
			id = "herdr",
			icon = "loader-circle",
			source = "herdr-core",
			title = "Installing herdr-core…",
			body = "Running its installer into ~/.local/bin",
			dismiss = false,
		}
	elseif herdr.error and state.asked then
		return toast {
			id = "herdr",
			level = "error",
			source = "herdr-core",
			title = "herdr-core did not install",
			body = herdr.error,
			full = true,
			action = { label = "Try again", msg = "install" },
		}
	elseif herdr.fresh and state.asked then
		return toast {
			id = "herdr",
			level = "success",
			source = "herdr-core",
			title = `herdr-core {herdr.version or ""} is installed`,
			body = "New terminals run on it now; from the next launch the app keeps its state there too.",
			full = true,
		}
	elseif not herdr.installed and not state.dismissed then
		return toast {
			id = "herdr",
			source = "Terminals",
			title = "Keep terminals in herdr-core",
			body = (if herdr.tmux then "They run on tmux now. " else "They end when the app quits now. ")
				.. "herdr-core keeps them running, shows what agents in them are doing, and keeps the app's state.",
			full = true,
			action = { label = "Install", msg = "install" },
		}
	end
	return nil
end

-- Publishes `offers/herdr` when whether the card shows has changed.
local function announce(state: State): any
	local showing = card_for(state, state.herdr) ~= nil
	if showing == state.showing then
		return nil
	end
	state.showing = showing
	return { { to = "topic:offers/herdr", msg = showing } }
end

return mod {
	region = "status-right",
	order = 100,

	subscribe = { "provider/herdr" },

	init = function(previous: State?)
		local state: State = {
			dismissed = previous ~= nil and previous.dismissed or false,
			asked = false,
			herdr = nil,
			showing = false,
		}
		return state, { { to = "topic:offers/herdr", msg = false } }
	end,

	update = function(state: State, msg)
		local sends: any = nil
		if msg == "install" then
			state.asked = true
			sends = { { to = "provider:herdr", msg = "install" } }
		elseif type(msg) == "table" and msg.dismiss then
			state.dismissed = true
			state.asked = false
		elseif type(msg) == "table" and msg.topic == "provider/herdr" then
			state.herdr = msg.value
		else
			return nil
		end
		local announced = announce(state)
		if announced then
			sends = sends or {}
			table.insert(sends, announced[1])
		end
		return state, sends
	end,

	view = function(state: State, app)
		local card = card_for(state, app.herdr)
		return ui.overlay {
			open = card ~= nil,
			place = "bottom-end",
			content = card or ui.text "",
		}
	end,
}
