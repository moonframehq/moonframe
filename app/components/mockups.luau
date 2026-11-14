-- Ideas for one change, drawn one under another before any of them is made: each
-- variant is live UI in a lettered card, with a button that copies "apply
-- this one" for the agent that drew them. Stateless; the moonframe-mockups
-- skill writes a throwaway mod around it.

type Variant = {
	name: string,
	-- One line on what this variant tries.
	note: string?,
	-- The mockup itself: one element or a list of them.
	body: any,
}

type Props = {
	-- What the user asked for, shown over the variants and quoted in the pick.
	ask: string?,
	variants: { Variant },
	-- The file the mockups are in, so the agent finds them from the pick.
	file: string?,
	-- Each card's width, in pixels; default 360.
	width: number?,
}

-- What "Pick" copies: enough for the agent that drew them to apply it.
local function pick_text(props: Props, letter: string, variant: Variant): string
	local text = `Apply mockup {letter} ("{variant.name}")`
	if props.ask then
		text ..= ` for: {props.ask}`
	end
	if props.file then
		text ..= `. The mockups are in {props.file}`
	end
	return text .. "."
end

return component {
	title = "Mockups",
	category = "Layout",
	icon = "sparkles",
	description = "Variants of one change one under another, each live in a lettered card with a Pick button that copies which one to apply.",
	when = "Showing the user a few ideas for a change before making it, so they can pick one: `moonframe mockup open` builds a throwaway mod around it. Not for real content; for choosing between options inside the app use `choice-cards`.",
	props = {
		["ask"] = "what the user asked for, shown on top and quoted in the pick",
		["variants"] = "list of { name, note?, body (an element or a list of elements) }",
		["file"] = "the mockup's file, quoted in the pick so the agent finds it",
		["width"] = "each card's width in pixels; default 360",
	},
	styles = {
		[".mockups-card"] = { background = "card", border = "all", border_color = "border", radius = 10, padding = 12 },
		[".mockups-letter"] = { background = "accent/15", color = "accent", radius = 6, padding = { 0, 6 } },
		[".mockups-stage"] = { background = "background", border = "all", border_color = "border", radius = 8, padding = 8 },
	},
	usage = [[local mockups = use "mockups"

mockups { ask = "a busier status bar", file = "~/.config/moonframe/mods/mockup/main.luau", variants = {
	{ name = "Pills", note = "Each value in a pill.", body = ui.row { gap = "small", ui.badge "main", ui.badge "+3" } },
	{ name = "Plain", note = "Muted text, no chrome.", body = ui.text { "main · +3", tone = "muted" } },
} }]],
	example = {
		ask = "a busier status bar",
		width = 240,
		variants = {
			{
				name = "Pills",
				note = "Each value in a pill.",
				body = ui.row { gap = "small", ui.badge "main", ui.badge { "+3", tone = "success" } },
			},
			{
				name = "Plain",
				note = "Muted text, no chrome.",
				body = ui.row {
					gap = "small",
					align = "center",
					ui.icon { name = "git-branch", tone = "muted" },
					ui.text { "main · +3", size = "small", tone = "muted" },
				},
			},
		},
	},

	build = function(props: Props)
		local cards = {}
		for index, variant in props.variants do
			local letter = string.char(64 + index)
			local body = variant.body
			local parts = if typeof(body) == "table" and body.type == nil then body else { body }
			cards[#cards + 1] = ui.column {
				class = "mockups-card",
				width = props.width or 360,
				gap = "small",
				ui.row {
					align = "center",
					gap = "small",
					ui.text { letter, class = "mockups-letter", weight = "medium" },
					ui.text { variant.name, weight = "medium", truncate = true },
					ui.spacer(),
					ui.button {
						"Pick",
						icon = "copy",
						small = true,
						variant = "outline",
						copy = pick_text(props, letter, variant),
						tooltip = "Copies which one to apply, for your agent",
					},
				},
				variant.note and ui.text { variant.note, size = "small", tone = "muted", wrap = true },
				ui.column { class = "mockups-stage", gap = "small", table.unpack(parts) },
			}
		end
		return ui.column {
			grow = true,
			scroll = true,
			gap = "normal",
			style = { padding = 16 },
			props.ask and ui.text { props.ask, weight = "medium" },
			-- One under another: a wrapping row inside a scrolling column is
			-- measured as one line, so the cards past it could not be scrolled to.
			ui.column { shrink = false, gap = "normal", align = "start", table.unpack(cards) },
		}
	end,
}
