-- One bar split into coloured shares with gaps between them: how a whole
-- divides (devices, tiers, plans). It takes its width in pixels, since
-- views cannot measure.

type Share = { share: number, color: string?, label: string? }

type Props = {
	shares: { Share },
	width: number?,
	height: number?,
}

return component {
	title = "Share Bar",
	category = "Data",
	icon = "chart-pie",
	description = "A horizontal bar split into coloured shares with small gaps, each as wide as its share of the whole.",
	when = "How a whole splits into a few parts, above a list of those parts (devices, engagement tiers, plans). For progress toward a target with stages use `progress-stack`; for one fraction use `meter`.",
	props = {
		["shares"] = "list of { share (any unit; they are summed), color?, label? (tooltip) }",
		["width"] = "pixels (default 420)",
		["height"] = "pixels (default 10)",
	},
	styles = {
		[".share-bar"] = { gap = 3 },
		[".share-bar-part"] = { radius = 3 },
	},
	example = {
		width = 360,
		shares = {
			{ share = 51, color = "#a78bfa", label = "Mobile" },
			{ share = 42, color = "#fbbf24", label = "Desktop" },
			{ share = 7, color = "#93c5fd", label = "Tablet" },
		},
	},

	build = function(props: Props)
		local width = props.width or 420
		local height = props.height or 10
		local total = 0
		for _, share in props.shares do
			total += share.share
		end
		local room = width - 3 * math.max(0, #props.shares - 1)
		local parts = {}
		for _, share in props.shares do
			table.insert(parts, ui.row {
				class = "share-bar-part",
				width = math.max(3, room * share.share / math.max(total, 1)),
				height = height,
				tooltip = share.label,
				style = { background = share.color or "accent" },
			})
		end
		return ui.row { class = "share-bar", width = width, shrink = false, table.unpack(parts) }
	end,
}
