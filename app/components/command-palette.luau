-- A command palette: a search field over a list of commands that filters as
-- the user types, with ↑↓ to move, ↩ to run and Esc to close. It keeps its
-- own query and highlight (an instance), so the mod only says which commands
-- there are and whether it is open, and hears back { run = id } when one is
-- picked and its `dismiss` message when it closes.
-- Its look comes from `look`, else the stylesheet's `looks = { palette = ... }`,
-- else "spotlight". Built in: spotlight, classic, compact, cards. Any other
-- name becomes the classes `.command-palette-<look>` and
-- `.command-palette-<look>-row`, so a stylesheet can define one with rules.

local kbd = use "kbd"
local key_hints = use "key-hints"

type Command = {
	id: string,
	label: string,
	icon: string?,
	-- Muted text after the label (a path, a branch), or under it in `cards`.
	detail: string?,
	shortcut: any,
	-- The heading it is listed under while the query is empty.
	group: string?,
	-- More words it matches on, not shown.
	keywords: string?,
	disabled: boolean?,
}

type State = {
	query: string,
	-- Index of the highlighted command among those that match.
	highlighted: number,
}

type Props = {
	id: string?,
	open: boolean?,
	commands: { Command }?,
	placeholder: string?,
	look: string?,
	-- Sent to the parent when it closes without running anything.
	dismiss: any,
	-- Key of the message sent to the parent when a command runs: { run = id }.
	event: string?,
	width: number?,
	-- The most rows shown before the list scrolls.
	rows: number?,
	-- Text when nothing matches.
	empty: string?,
	-- Words before the field, as a scope ("Go to file"); none by default.
	prefix: string?,
}

-- What each built-in look draws, beyond its classes.
type Shape = {
	-- An icon before the field.
	search_icon: boolean,
	-- Group headings while the query is empty.
	headings: boolean,
	-- A line of key hints under the list.
	footer: boolean,
	-- Icons in a tinted tile, the detail under the label.
	tiles: boolean,
	-- The group as a muted tag on each row while filtering.
	group_tags: boolean,
	-- Each row's height in pixels, for the list's height.
	row_height: number,
}

local SHAPES: { [string]: Shape } = {
	spotlight = { search_icon = true, headings = true, footer = true, tiles = false, group_tags = true, row_height = 36 },
	classic = { search_icon = false, headings = false, footer = false, tiles = false, group_tags = true, row_height = 26 },
	compact = { search_icon = true, headings = false, footer = false, tiles = false, group_tags = false, row_height = 24 },
	cards = { search_icon = true, headings = true, footer = true, tiles = true, group_tags = false, row_height = 48 },
}

-- How well `query` matches `text`, as a subsequence: higher is better, nil
-- when it does not. Consecutive letters and word starts count more.
local function score(text: string, query: string): number?
	if query == "" then
		return 0
	end
	local haystack = string.lower(text)
	local total, at, run = 0, 1, 0
	for i = 1, #query do
		local char = string.sub(query, i, i)
		local found = string.find(haystack, char, at, true)
		if not found then
			return nil
		end
		run = if found == at then run + 1 else 0
		local before = if found > 1 then string.sub(haystack, found - 1, found - 1) else " "
		local word_start = string.find(before, "[%s%p]") ~= nil
		total += 1 + run * 2 + (if word_start then 3 else 0)
		at = found + 1
	end
	-- Shorter labels for the same letters rank first.
	return total - #haystack * 0.01
end

-- The commands that match the query, best first; in their given order (and
-- groups) while the query is empty.
local function matches(commands: { Command }, query: string): { Command }
	local q = string.lower(query)
	if q == "" then
		return commands
	end
	local ranked = {}
	for index, command in commands do
		local best = score(command.label, q)
		local extra = score(`{command.label} {command.keywords or ""} {command.group or ""}`, q)
		if extra and (not best or extra - 5 > best) then
			best = extra - 5
		end
		if best then
			table.insert(ranked, { command = command, score = best, index = index })
		end
	end
	table.sort(ranked, function(a, b)
		if a.score ~= b.score then
			return a.score > b.score
		end
		return a.index < b.index
	end)
	local out = {}
	for _, entry in ranked do
		table.insert(out, entry.command)
	end
	return out
end

local function clamp(index: number, count: number): number
	if count == 0 then
		return 1
	end
	return math.clamp(index, 1, count)
end

local EXAMPLE: { any } = {
	{ id = "new-tab", label = "New terminal tab", icon = "terminal", shortcut = "cmd-t", group = "Terminal" },
	{ id = "split", label = "Split pane right", icon = "columns-2", shortcut = "cmd-d", group = "Terminal" },
	{ id = "file", label = "Go to file…", icon = "file-text", shortcut = "cmd-p", group = "Navigate" },
	{ id = "branch", label = "Switch branch…", icon = "git-branch", detail = "main", group = "Git" },
	{ id = "commit", label = "Commit all changes", icon = "git-commit-horizontal", detail = "14 files", group = "Git" },
	{ id = "theme", label = "Toggle light and dark", icon = "sun", shortcut = "cmd-shift-l", group = "View" },
	{ id = "reload", label = "Reload window", icon = "refresh-cw", shortcut = "cmd-r", group = "View" },
}

return component {
	title = "Command Palette",
	category = "Overlays",
	icon = "command",
	description = "A search field over commands that filters as you type, with ↑↓, ↩ and Esc; grouped while empty, ranked while filtering, in several looks.",
	when = "Run any of the app's or a mod's commands from the keyboard (⌘K, ⌘⇧P), or jump to a file, branch or workspace by name. For a menu under a button use `dropdown`; for right-click actions use `context-menu`; for a list of shortcuts to read use `shortcuts`; for a search box over a list in a pane use `search-field`.",
	props = {
		["id"] = "keeps its query and highlight; one per id (default \"main\")",
		["open"] = "show it; reopening starts with an empty query",
		["commands"] = "list of { id, label, icon?, detail?, shortcut? (kbd keys, \"cmd-t\"), group?, keywords?, disabled? }",
		["placeholder"] = "grey text in the empty field (default \"Type a command…\")",
		["look"] = "spotlight (default), classic, compact or cards, or a look the stylesheet defines; else the stylesheet's `looks.palette`",
		["dismiss"] = "message sent to the parent on Esc or a click outside",
		["event"] = "key of the message sent to the parent when a command runs, default \"run\": { run = id }",
		["width"] = "pixels (default per look, 560 for spotlight)",
		["rows"] = "most rows shown before the list scrolls (default 8)",
		["empty"] = "text when nothing matches (default \"No matching commands\")",
		["prefix"] = "a scope chip before the field, such as \"Go to file\"",
	},
	styles = {
		[".command-palette"] = { padding = 0, radius = 12, border = "all", border_color = "border", background = "background" },
		[".command-palette-field"] = { padding = { 12, 14 }, gap = 10, border = "bottom", border_color = "border" },
		[".command-palette-field-icon"] = { color = "muted-foreground" },
		[".command-palette-input"] = { border = "none", background = "#00000000", padding = 0 },
		[".command-palette-prefix"] = { padding = { 1, 8 }, radius = 5, background = "accent/12", color = "accent", size = "small" },
		[".command-palette-list"] = { padding = 6, gap = 0 },
		[".command-palette-heading"] = { padding = { 8, 10, 4, 10 }, size = "small", color = "muted-foreground", weight = "medium" },
		[".command-palette-row"] = { padding = { 0, 10 }, radius = 7, gap = 10 },
		[".command-palette-row:hover"] = { background = "list-hover" },
		[".command-palette-row:selected"] = { background = "accent/15" },
		[".command-palette-disabled"] = { opacity = 0.45 },
		[".command-palette-icon"] = { color = "muted-foreground" },
		[".command-palette-detail"] = { size = "small", color = "muted-foreground" },
		[".command-palette-tag"] = { size = "small", color = "muted-foreground" },
		[".command-palette-empty"] = { padding = { 18, 12 }, color = "muted-foreground", size = "small" },
		[".command-palette-footer"] = { padding = { 6, 14 }, border = "top", border_color = "border", background = "muted/40" },

		-- spotlight: roomy, a large field, a footer of hints.
		[".command-palette-spotlight"] = { radius = 14, shadow = "0 24px 48px -12px #00000055" },
		[".command-palette-spotlight-field"] = { padding = { 16, 18 } },
		[".command-palette-spotlight-input"] = { size = "large" },

		-- classic: an editor's palette, square and tight, hanging from the top.
		[".command-palette-classic"] = { radius = 6, shadow = "0 8px 24px -6px #00000044" },
		[".command-palette-classic-field"] = { padding = 6, border = "none" },
		[".command-palette-classic-input"] = {
			border = "all",
			border_color = "accent",
			radius = 4,
			padding = { 4, 8 },
			background = "background",
		},
		[".command-palette-classic-list"] = { padding = { 0, 0, 4, 0 } },
		[".command-palette-classic-row"] = { radius = 0, padding = { 0, 12 } },
		[".command-palette-classic-row:selected"] = { background = "list-active" },

		-- compact: small type, dense rows, a narrow card.
		[".command-palette-compact"] = { radius = 8, shadow = "0 8px 20px -6px #00000044" },
		[".command-palette-compact-field"] = { padding = { 8, 10 }, gap = 8 },
		[".command-palette-compact-input"] = { size = "small" },
		[".command-palette-compact-list"] = { padding = 4 },
		[".command-palette-compact-row"] = { radius = 5, padding = { 0, 8 }, gap = 8, size = "small" },

		-- cards: two-line rows with icon tiles.
		[".command-palette-cards"] = { radius = 16, shadow = "0 24px 56px -12px #00000066" },
		[".command-palette-cards-field"] = { padding = { 16, 18 } },
		[".command-palette-cards-input"] = { size = "large" },
		[".command-palette-cards-list"] = { padding = 8, gap = 2 },
		[".command-palette-cards-row"] = { radius = 10, padding = { 0, 8 }, gap = 12 },
		[".command-palette-cards-row:selected"] = { background = "accent/12" },
		[".command-palette-tile"] = { radius = 8, background = "muted", color = "foreground" },
		[".command-palette-tile:selected"] = { background = "accent", color = "accent-foreground" },
	},
	usage = [[local command_palette = use "command-palette"

-- in view (the mod opens it from a shortcut or a menu):
command_palette {
	open = state.palette, dismiss = "close-palette",
	look = "spotlight", -- classic, compact, cards
	commands = {
		{ id = "new-tab", label = "New terminal tab", icon = "terminal", shortcut = "cmd-t", group = "Terminal" },
		{ id = "branch", label = "Switch branch…", icon = "git-branch", detail = "main", group = "Git" },
	},
}
-- in update: msg.run is the chosen command's id (close it there);
-- "close-palette" closes it.]],
	example = { id = "example", open = true, commands = EXAMPLE },

	init = function(previous: State?)
		local state: State = previous or { query = "", highlighted = 1 }
		return state
	end,

	update = function(state: State, msg, props: Props)
		local commands: { Command } = props.commands or {}
		local found = matches(commands, state.query)
		local function close(sends: { any })
			state.query = ""
			state.highlighted = 1
			return state, sends
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.msg == "query" then
			state.query = msg.value or ""
			state.highlighted = 1
		elseif msg.msg == "previous" then
			state.highlighted = if state.highlighted <= 1 then #found else state.highlighted - 1
		elseif msg.msg == "next" then
			state.highlighted = if state.highlighted >= #found then 1 else state.highlighted + 1
		elseif msg.msg == "run" or msg.pick then
			local command = if msg.pick
				then nil
				else found[clamp(state.highlighted, #found)]
			if msg.pick then
				for _, candidate in commands do
					if candidate.id == msg.pick then
						command = candidate
					end
				end
			end
			if not command or command.disabled then
				return
			end
			return close { { to = "parent", msg = { [props.event or "run"] = command.id } } }
		elseif msg.msg == "close" then
			return close(if props.dismiss ~= nil then { { to = "parent", msg = props.dismiss } } else {})
		end
	end,

	build = function(props: Props, kept: any, app)
		local state: State = kept
		local look = props.look or (app and app.looks and app.looks.palette) or "spotlight"
		local shape = SHAPES[look] or SHAPES.spotlight
		local function cls(part: string): { string }
			return { `command-palette-{part}`, `command-palette-{look}-{part}` }
		end

		local found = matches(props.commands or {}, state.query)
		local highlighted = clamp(state.highlighted, #found)
		local grouped = shape.headings and state.query == ""

		local rows = {}
		local heading: string? = nil
		for index, command in found do
			if grouped and command.group and command.group ~= heading then
				heading = command.group
				table.insert(rows, ui.row { class = cls "heading", shrink = false, ui.text { command.group :: string } })
			end
			local on = index == highlighted
			local icon: any = command.icon
				and ui.icon { name = command.icon :: any, class = cls "icon", size = if shape.tiles then "normal" else nil }
			if shape.tiles then
				icon = ui.row {
					class = "command-palette-tile",
					selected = on,
					width = 32,
					height = 32,
					justify = "center",
					align = "center",
					shrink = false,
					command.icon and ui.icon { name = command.icon :: any },
				}
			end
			local label: any
			if shape.tiles then
				label = ui.column {
					grow = true,
					gap = "none",
					ui.text { command.label, truncate = true, weight = "medium" },
					(command.detail or not grouped and command.group) and ui.text {
						command.detail or command.group :: string,
						class = cls "detail",
						truncate = true,
					},
				}
			else
				label = ui.row {
					grow = true,
					gap = "normal",
					align = "center",
					ui.text { command.label, truncate = true },
					command.detail and ui.text { command.detail, class = cls "detail", truncate = true },
				}
			end
			local row_class = { "command-palette-row", `command-palette-{look}-row` }
			if command.disabled then
				table.insert(row_class, "command-palette-disabled")
			end
			table.insert(
				rows,
				ui.row {
					class = row_class,
					height = shape.row_height,
					align = "center",
					shrink = false,
					selected = on and not command.disabled,
					msg = if command.disabled then nil else { pick = command.id },
					icon,
					label,
					shape.group_tags and not grouped and command.group and ui.text { command.group, class = cls "tag" },
					command.shortcut and kbd { keys = command.shortcut, small = true, plain = not shape.tiles },
				}
			)
		end

		local most = props.rows or 8
		local list: any = if #found == 0
			then ui.row {
				class = cls "empty",
				justify = "center",
				ui.text { props.empty or "No matching commands" },
			}
			else ui.column {
				class = cls "list",
				scroll = true,
				max_height = most * shape.row_height + 40,
				table.unpack(rows),
			}

		local field = ui.row {
			class = cls "field",
			align = "center",
			shrink = false,
			shape.search_icon and ui.icon { name = "search", class = cls "field-icon" },
			props.prefix and ui.row { class = "command-palette-prefix", ui.text { props.prefix } },
			ui.input {
				id = "query",
				class = cls "input",
				value = state.query,
				placeholder = props.placeholder or (if look == "classic" then "> " else "Type a command…"),
				change = "query",
				submit = "run",
				focus = props.open,
				grow = true,
				keys = { up = "previous", down = "next", escape = "close", ["ctrl-p"] = "previous", ["ctrl-n"] = "next" },
			},
			look == "compact" and kbd { keys = "escape", small = true },
		}

		local widths = { spotlight = 560, classic = 600, compact = 420, cards = 600 }
		return ui.overlay {
			open = props.open,
			dismiss = { msg = "close" },
			place = "top",
			backdrop = look ~= "classic" and look ~= "compact",
			width = props.width or widths[look] or 560,
			class = { "command-palette", `command-palette-{look}` } :: any,
			content = ui.column {
				gap = "none",
				field,
				list,
				shape.footer and ui.row {
					class = cls "footer",
					shrink = false,
					key_hints {
						hints = {
							{ keys = "up down", label = "move" },
							{ keys = "enter", label = "run" },
							{ keys = "escape", label = "close" },
						},
					},
					ui.spacer(),
					ui.text { `{#found} commands`, class = cls "tag" },
				},
			},
		}
	end,
}
