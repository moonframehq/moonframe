-- One step of a run: an icon by what it does, a fold chevron, what happened,
-- and its state; open, its output or the change it made.

local diff_counts = use "diff-counts"
local code_view = use "code-view"

type Props = { item: any, review: any, to: string? }

local ICONS: { [string]: string } = {
	read = "file-text",
	edit = "square-pen",
	write = "file",
	bash = "square-terminal",
	grep = "search",
	find = "search",
	ls = "folder",
}

local function patch_lines(patch: string): { any }
	local rows: { any } = {}
	for line in string.gmatch(patch .. "\n", "(.-)\n") do
		local first = string.sub(line, 1, 1)
		local head = string.sub(line, 1, 3)
		if head ~= "---" and head ~= "+++" then
			local class = if first == "+" then "tool-patch-add" elseif first == "-" then "tool-patch-del" elseif first == "@" then "tool-patch-hunk" else "tool-patch-line"
			table.insert(rows, ui.row { class = class, ui.text { line, wrap = false, class = "tool-patch-text" } })
		end
		if #rows >= 300 then
			break
		end
	end
	return rows
end

return component {
	title = "Timeline Tool",
	category = "Agents",
	icon = "square-terminal",
	description = "One tool call in a run: what it did, its state, and its output when opened.",
	when = "A step of an agent's run in Pilot's conversation.",
	props = { item = "the tool item from `app.pi.thread.items`", review = "message the review button sends", to = "where messages go" },
	styles = {
		[".tool-row"] = { padding = { 7, 0 }, gap = 8, radius = 6 },
		[".tool-row:hover"] = { background = "list-hover" },
		[".tool-icon"] = { color = "muted-foreground" },
		[".tool-title"] = { color = "foreground" },
		[".tool-state"] = { size = "small", color = "muted-foreground" },
		[".tool-dot-done"] = { width = 7, height = 7, radius = 4, background = "success" },
		[".tool-dot-error"] = { width = 7, height = 7, radius = 4, background = "danger" },
		[".tool-output"] = { margin = { 2, 0, 8, 28 }, border = true, radius = 8, padding = { 6, 0 }, background = "secondary" },
		[".tool-patch-text"] = { font = "mono", size = "small" },
		[".tool-patch-line"] = { padding = { 0, 10 } },
		[".tool-patch-add"] = { padding = { 0, 10 }, background = "added" },
		[".tool-patch-del"] = { padding = { 0, 10 }, background = "removed" },
		[".tool-patch-hunk"] = { padding = { 0, 10 }, background = "hunk", color = "muted-foreground" },
	},
	example = { item = { id = "1", tool = "read", title = "Read src/price.js", status = "done" } },
	build = function(props: Props)
		local item = props.item
		local running = item.status == "running"
		local failed = item.status == "error"
		return ui.column {
			gap = "none",
			ui.row {
				class = "tool-row",
				align = "center",
				msg = { expand = item.id },
				to = "provider:pi",
				ui.icon { (ICONS[item.tool] or "settings-2") :: any, class = "tool-icon" },
				ui.icon { (if item.open then "chevron-down" else "chevron-right") :: any, class = "tool-icon" },
				ui.text { item.title, truncate = true, class = "tool-title" },
				(item.added ~= nil) and diff_counts { added = item.added, removed = item.deleted or 0, zero = true },
				if running
					then ui.icon { "loader-circle", spin = true, class = "tool-icon" }
					else ui.row { class = if failed then "tool-dot-error" else "tool-dot-done" },
				ui.text { `{item.tool} · {if running then "running" elseif failed then "failed" else "done"}`, class = "tool-state", wrap = false },
				ui.spacer(),
				props.review and ui.row { class = "quiet-button", msg = props.review, to = props.to, tooltip = "Show in Changes", ui.icon { "git-compare" } },
			},
			item.open and item.patch and ui.column { class = "tool-output", gap = "none", scroll = "x", table.unpack(patch_lines(item.patch)) },
			item.open and not item.patch and item.output and item.output ~= "" and ui.column {
				class = "tool-output",
				code_view { id = `out-{item.id}`, text = item.output, numbers = false, height = 220, language = "text" },
			},
		}
	end,
}
