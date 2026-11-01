-- Content that changes with a view, and moves when it does: the old page
-- fades out and folds away as the new one fades in and slides into its
-- place. Wrap a list's header and rows in it, keyed by the view shown, so
-- Inbox → All → Last 7 Days reads as a change of page, not a redraw.
--
-- It is a keyed column in a row: when `key` changes the old column leaves
-- and the new one arrives, each by the `.swap-view-page` transition, along
-- the row, so the two share its width while they move.

type Props = {
	key: string,
	width: number?,
	grow: boolean?,
	[number]: any,
}

-- Keys are 1 to 64 of a-z, 0-9, `-` and `_`.
local function keyed(value: string): string
	local key = string.gsub(string.lower(value), "[^a-z0-9_%-]", "-")
	if key == "" then
		return "page"
	end
	return string.sub(key, 1, 64)
end

return component {
	title = "Swap View",
	category = "Containers",
	icon = "copy",
	description = "Content keyed by a view: when the key changes, the old page fades and folds away as the new one fades and slides in.",
	when = "The body under a view switcher (`pill-tabs`, `segmented`, `tabs`) whose content is replaced, such as a list's header and rows per filter. Give `width` so the pages can be measured while they move. For panels that open and close use `side-panels`; for items joining a list give the rows a `key` and a transition instead.",
	props = {
		["[children]"] = "the page for the current key",
		["key"] = "names the page shown: a new key swaps it",
		["width"] = "the page's width in pixels (default 260); the moving pages keep it, clipped",
		["grow"] = "take the free space along the parent's axis",
	},
	styles = {
		[".swap-view-page"] = { transition = "slide-fade 260ms cubic-bezier(0.32, 0.72, 0, 1)" },
	},
	usage = [[local swap_view = use "swap-view"

swap_view { key = state.view, width = 260,
	ui.text { TITLES[state.view], weight = "bold" },
	-- ...the rows of that view
}]],
	example = {
		key = "inbox",
		width = 220,
		ui.text { "Inbox", weight = "bold" },
		ui.text { "1 sheet · 150 words", tone = "muted", size = "small" },
	},

	build = function(props: Props)
		local page: any = {
			key = keyed(props.key),
			class = "swap-view-page",
			width = props.width or 260,
			shrink = false,
			gap = "normal",
		}
		for _, child in ipairs(props) do
			table.insert(page, child)
		end
		return ui.row {
			class = "swap-view",
			gap = "none",
			align = "start",
			grow = props.grow,
			ui.column(page),
		}
	end,
}
