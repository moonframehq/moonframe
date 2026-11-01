-- The folded sidebar: a column of icons under the traffic lights (the
-- brand, New Chat, Agents, Intelligence, Projects, the latest agent chats)
-- and the account at the foot. The brand unfolds the sidebar; an agent's
-- icon opens its page.

local pixel_icon = use "pixel-icon"

local LOGO = { "0000", "0111", "0001", "0001", "0001", "0001", "0000" }

return mod {
	region = "rail",
	title = "Rail",

	subscribe = { "tessel/nav" },

	styles = {
		[".rail"] = { padding = { 0, 0, 14, 0 }, gap = 6 },
		[".rail-tile"] = { radius = 8, padding = 6, color = "foreground" },
		[".rail-tile:hover"] = { background = "list-hover" },
		[".rail-tile:selected"] = { background = "#0000000d" },
		[".rail-new"] = { color = "brand" },
	},

	init = function(previous)
		return { detail = "" }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "tessel/nav" and type(msg.value) == "table" then
			state.detail = msg.value.detail or ""
		elseif type(msg) == "table" and msg.nav then
			return state, { { to = "topic:tessel/nav", msg = { detail = msg.nav } } }
		end
	end,

	view = function(state, app)
		local data = app.tessel
		local tiles: { any } = {}
		if data and data.agent_chats and data.agents then
			for _, chat in data.agent_chats do
				for _, agent in data.agents do
					if agent.id == chat.agent then
						table.insert(tiles, ui.row {
							class = "rail-tile",
							selected = state.detail == agent.id,
							tooltip = agent.name,
							msg = { nav = agent.id },
							pixel_icon { pattern = agent.icon.pattern, colors = agent.icon.colors, size = 20 },
						})
					end
				end
			end
		end
		local user = data and data.user
		local items: { any } = {
			ui.row { height = 40, shrink = false, drag = true },
			ui.row {
				class = "rail-tile",
				tooltip = "Unfold the sidebar",
				msg = "show-left",
				to = "shell",
				pixel_icon { pattern = LOGO, colors = { "#f2542d", "#ffffff" }, size = 24 },
			},
			ui.row { class = "rail-tile rail-new", tooltip = "New Chat", msg = "show-left", to = "shell", ui.icon "plus" },
			ui.row {
				class = "rail-tile",
				tooltip = "Agents",
				selected = state.detail == "",
				msg = { nav = "" },
				ui.icon "bot",
			},
			ui.row { class = "rail-tile", tooltip = "Intelligence", msg = "show-left", to = "shell", ui.icon "sparkles" },
			ui.row { class = "rail-tile", tooltip = "Projects", msg = "show-left", to = "shell", ui.icon "folder" },
			ui.row { height = 4 },
		}
		for _, tile in tiles do
			table.insert(items, tile)
		end
		table.insert(items, ui.spacer())
		if user then
			table.insert(items, ui.row {
				width = 30,
				height = 30,
				align = "center",
				justify = "center",
				tooltip = user.name,
				style = { background = user.color, radius = 999, color = "#ffffff" },
				ui.text { string.sub(user.name, 1, 1), weight = "bold" },
			})
		end
		return ui.column { class = "rail", grow = true, align = "center", table.unpack(items) }
	end,
}
