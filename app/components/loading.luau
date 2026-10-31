-- Something is on its way: a spinner and a line of text, inline or centred
-- in the space it gets, or `skeleton` placeholder bars shaped like the
-- content to come.

type Props = {
	text: string?,
	detail: string?,
	block: boolean?,
	skeleton: boolean?,
	count: number?,
	-- Pixel widths of the skeleton bars, cycled; the last bar is shorter.
	widths: { number }?,
}

return component {
	title = "Loading",
	category = "Feedback",
	icon = "loader-circle",
	description = "A spinner with text, inline or centred as a block, or skeleton bars standing in for content.",
	when = "While data loads: inline beside a label, centred in an empty pane (`block`), or as `skeleton` bars where rows will appear. For known progress use `progress`; for a lost connection use `reconnecting`; for a failure use `error-state`; for nothing to show use `empty-state`.",
	props = {
		["text"] = "what is loading; default \"Loading…\"",
		["detail"] = "a muted second line (block only)",
		["block"] = "centre it in the space it gets, larger, instead of a small inline row",
		["skeleton"] = "draw placeholder bars instead of a spinner",
		["count"] = "number of skeleton bars; default 3",
		["widths"] = "skeleton bar widths in pixels, cycled; default full width with a shorter last bar",
	},
	styles = {
		[".loading"] = { gap = 6, color = "muted-foreground" },
		[".loading-icon"] = { color = "muted-foreground", size = "small" },
		[".loading-text"] = { size = "small", color = "muted-foreground" },
		[".loading-block"] = { gap = 8, padding = 24 },
		[".loading-block-icon"] = { color = "accent", size = "title" },
		[".loading-block-text"] = { weight = "medium" },
		[".loading-detail"] = { size = "small", color = "muted-foreground" },
		[".loading-skeleton"] = { gap = 8, padding = { 4, 0 } },
		[".loading-bar"] = { background = "muted-foreground/15", radius = 5 },
	},
	usage = [[local loading = use "loading"

loading { text = "Fetching pull requests…" }
loading { block = true, text = "Connecting to devbox", detail = "ssh penso@devbox.local" }
loading { skeleton = true, count = 4 }]],
	example = { block = true, text = "Fetching pull requests…", detail = "github.com/penso/moonframe" },

	build = function(props: Props)
		if props.skeleton then
			local bars = {}
			local count = props.count or 3
			for index = 1, count do
				local width: number? = nil
				if props.widths and #props.widths > 0 then
					width = props.widths[(index - 1) % #props.widths + 1]
				elseif index == count and count > 1 then
					width = 160
				end
				bars[index] = ui.row { class = "loading-bar", height = 12, width = width }
			end
			return ui.column { class = "loading-skeleton", table.unpack(bars) }
		end

		local text = props.text or "Loading…"
		if props.block then
			return ui.column {
				class = "loading-block",
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "loader-circle", spin = true, class = "loading-block-icon" },
				ui.text { text, class = "loading-block-text" },
				props.detail ~= nil and ui.text { props.detail, class = "loading-detail" },
			}
		end
		return ui.row {
			class = "loading",
			align = "center",
			ui.icon { name = "loader-circle", spin = true, class = "loading-icon" },
			ui.text { text, class = "loading-text" },
		}
	end,
}
