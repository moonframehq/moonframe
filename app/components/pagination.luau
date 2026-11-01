-- The foot of a paged table or list: "Viewing 1 to 15 of 145 activities"
-- on the left; previous, numbered pages (the current one outlined) and
-- next; then an "Items per page" dropdown. The mod keeps the page and the
-- page size; this works out which numbers to show.

local menu = use "menu"

type Props = {
	page: number,
	total: number,
	per_page: number?,
	noun: string?,
	summary: boolean?,
	window: number?,
	ellipsis: boolean?,
	event: string?,
	sizes: { number }?,
	size_open: boolean?,
	size_toggle: any,
	size_dismiss: any,
	size_event: string?,
	size_label: string?,
	to: string?,
}

-- The page numbers to draw, with 0 where a gap (…) goes: a run of `window`
-- numbers around `page`, plus the first and last with gaps when `ellipsis`.
local function numbers(page: number, pages: number, window: number, ellipsis: boolean): { number }
	local list = {}
	if pages <= window then
		for n = 1, pages do
			table.insert(list, n)
		end
		return list
	end
	if not ellipsis then
		local first = math.clamp(page - math.floor(window / 2), 1, pages - window + 1)
		for n = first, first + window - 1 do
			table.insert(list, n)
		end
		return list
	end
	-- With gaps: the first, the last and `window - 2` slots between them.
	local inner = math.max(window - 2, 1)
	if pages <= inner + 2 then
		-- Too few pages for a gap (a `window` of 1 or 2): all of them.
		for n = 1, pages do
			table.insert(list, n)
		end
		return list
	end
	local first = math.clamp(page - math.floor((inner - 1) / 2), 2, pages - inner)
	local last = first + inner - 1
	table.insert(list, 1)
	if first > 2 then
		-- A single hidden number shows itself instead of a gap.
		table.insert(list, if first == 3 then 2 else 0)
	end
	for n = first, last do
		table.insert(list, n)
	end
	if last < pages - 1 then
		table.insert(list, if last == pages - 2 then pages - 1 else 0)
	end
	table.insert(list, pages)
	return list
end

-- 1234567 as "1,234,567".
local function grouped(value: number): string
	local text = tostring(math.floor(value))
	local out = string.reverse((string.gsub(string.reverse(text), "(%d%d%d)", "%1,")))
	return (string.gsub(out, "^,", ""))
end

return component {
	title = "Pagination",
	category = "Navigation",
	icon = "chevrons-right",
	description = "Page buttons for a long table or list: a Viewing x to y of n summary, previous and next, numbered pages with the current one outlined, gaps, and an items-per-page dropdown.",
	when = "Under a table or list too long to show at once (an audit log, orders, invoices), when the user steps through it by page. For a list that scrolls use `ui.list`; for steps of an onboarding flow use `pager`; for history use `nav-buttons`.",
	props = {
		["page"] = "the current page, from 1",
		["total"] = "how many items there are in all",
		["per_page"] = "items on a page (default 15)",
		["noun"] = "what the items are, for the summary (default \"items\")",
		["summary"] = "false hides the \"Viewing 1 to 15 of 145 items\" text",
		["window"] = "most page buttons shown (default 7)",
		["ellipsis"] = "false shows a sliding run of `window` pages instead of first … run … last",
		["event"] = "key of the message a page button sends, default \"page\": { page = n }",
		["sizes"] = "page sizes for the Items per page dropdown, e.g. { 15, 30, 50 }; no dropdown without it",
		["size_open"] = "true while the page-size menu shows",
		["size_toggle"] = "message the page-size button sends (open or close)",
		["size_dismiss"] = "message sent on a click outside the page-size menu",
		["size_event"] = "key of the message a size sends, default \"per_page\": { per_page = n }",
		["size_label"] = "text before the page-size button (default \"Items per page\")",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".pagination"] = { gap = 16 },
		[".pagination-summary"] = { color = "muted-foreground" },
		[".pagination-strong"] = { color = "foreground", weight = "medium" },
		[".pagination-pages"] = { gap = 8 },
		[".pagination-button"] = {
			radius = 7,
			border = "all",
			border_color = "border",
			background = "background",
			size = "small",
			weight = "medium",
		},
		[".pagination-button:hover"] = { background = "list-hover" },
		[".pagination-button:selected"] = { border_color = "accent", color = "foreground" },
		[".pagination-disabled"] = { opacity = 0.4 },
		[".pagination-gap"] = { color = "muted-foreground" },
		[".pagination-size"] = { gap = 10 },
		[".pagination-size-label"] = { color = "muted-foreground" },
		[".pagination-size-button"] = { radius = 7, border = "all", border_color = "border", background = "background", padding = { 0, 8, 0, 10 }, gap = 6, size = "small", weight = "medium" },
		[".pagination-size-button:hover"] = { background = "list-hover" },
		[".pagination-chevron"] = { color = "muted-foreground" },
	},
	example = {
		page = 1,
		total = 145,
		per_page = 15,
		noun = "activities",
		window = 5,
		ellipsis = false,
		sizes = { 15, 30, 50 },
	},
	usage = [[local pagination = use "pagination"

pagination {
	page = state.page, total = #rows, per_page = state.per_page, noun = "activities",
	sizes = { 15, 30, 50 }, size_open = state.size_open,
	size_toggle = "toggle-size", size_dismiss = "close-size",
}
-- update: msg.page goes to that page; msg.per_page sets the size (and
-- closes the menu, back to page 1); "toggle-size" / "close-size" open and
-- close it.]],

	build = function(props: Props)
		local per_page = math.max(props.per_page or 15, 1)
		local total = math.max(props.total, 0)
		local pages = math.max(math.ceil(total / per_page), 1)
		local page = math.clamp(props.page, 1, pages)
		local event = props.event or "page"

		local function button(content: any, target: number?, selected: boolean?)
			local enabled = target ~= nil and target ~= page
			return ui.row {
				class = if target == nil then { "pagination-button", "pagination-disabled" } else "pagination-button",
				width = 32,
				height = 32,
				shrink = false,
				align = "center",
				justify = "center",
				selected = selected == true,
				msg = if enabled then { [event] = target } else nil,
				to = props.to,
				content,
			}
		end

		local cells = {}
		table.insert(cells, button(ui.icon { name = "chevron-left" }, if page > 1 then page - 1 else nil))
		for _, n in numbers(page, pages, math.max(props.window or 7, 3), props.ellipsis ~= false) do
			if n == 0 then
				table.insert(cells, ui.row { width = 20, justify = "center", ui.text { "…", class = "pagination-gap" } })
			else
				table.insert(cells, button(ui.text { tostring(n) }, n, n == page))
			end
		end
		table.insert(cells, button(ui.icon { name = "chevron-right" }, if page < pages then page + 1 else nil))

		local from = if total == 0 then 0 else (page - 1) * per_page + 1
		local to = math.min(page * per_page, total)
		local summary = if props.summary == false
			then nil
			else ui.row {
				gap = "small",
				align = "center",
				ui.text { "Viewing", class = "pagination-summary" },
				ui.text { grouped(from), class = "pagination-strong" },
				ui.text { "to", class = "pagination-summary" },
				ui.text { grouped(to), class = "pagination-strong" },
				ui.text { `of {grouped(total)} {props.noun or "items"}`, class = "pagination-summary" },
			}

		local size: any = false
		if props.sizes then
			local items = {}
			for index, n in props.sizes do
				items[index] = { id = tostring(n), label = tostring(n), checked = n == per_page, msg = { [props.size_event or "per_page"] = n } }
			end
			size = ui.row {
				class = "pagination-size",
				align = "center",
				ui.text { props.size_label or "Items per page", class = "pagination-size-label" },
				ui.popover {
					open = props.size_open == true,
					dismiss = props.size_dismiss,
					to = props.to,
					side = "top",
					align = "end",
					gap = 4,
					width = 120,
					keys = { escape = props.size_dismiss },
					content = menu { items = items, to = props.to },
					ui.row {
						class = "pagination-size-button",
						height = 32,
						align = "center",
						msg = props.size_toggle,
						to = props.to,
						ui.text { tostring(per_page) },
						ui.icon { name = "chevron-down", class = "pagination-chevron" },
					},
				},
			}
		end

		return ui.row {
			class = "pagination",
			grow = true,
			align = "center",
			summary,
			ui.spacer(),
			ui.row { class = "pagination-pages", align = "center", table.unpack(cells) },
			size,
		}
	end,
}
