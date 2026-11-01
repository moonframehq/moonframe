-- Three guides: clicking one reads a tenth more of it, until it is done.
local guide_card = use "guide-card"

local GUIDES = {
	{ id = "subject", title = "Write subject lines people open", category = "Writing", color = "#7c5cff", icon = "a-large-small", read = "6 min read" },
	{ id = "referral", title = "Grow with a referral program", category = "Growth", color = "#14a38b", icon = "rocket", read = "9 min read" },
	{ id = "pricing", title = "Price your paid tier", category = "Monetization", color = "#f08a24", icon = "tag", read = "12 min read" },
}

return mod {
	init = function(previous)
		return previous or { read = { subject = 1, referral = 0.4, pricing = 0 } }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.open then
			state.read[msg.open] = math.min(1, (state.read[msg.open] or 0) + 0.1)
		end
	end,

	view = function(state)
		local cards = {}
		for _, guide in GUIDES do
			local read = state.read[guide.id] or 0
			table.insert(cards, ui.row {
				width = 230,
				guide_card {
					title = guide.title,
					category = guide.category,
					color = guide.color,
					icon = guide.icon,
					read = guide.read,
					done = read >= 1,
					new = read == 0,
					progress = read,
					msg = { open = guide.id },
				},
			})
		end
		return ui.row { gap = "large", table.unpack(cards) }
	end,
}
