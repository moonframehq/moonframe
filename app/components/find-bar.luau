-- A find bar over a document: the query, "3 of 12", previous and next, and
-- a close button; with `replace`, a second line to replace matches. Enter
-- goes to the next match, Shift-Enter to the previous, Escape closes. It only
-- asks: the mod keeps the query and the match and hands them to what it
-- searches, such as a `ui.markdown`'s `find` and `find_index`.

local icon_button = use "icon-button"

type Props = {
	id: string?,
	value: string?,
	placeholder: string?,
	count: number?,
	index: number?,
	change: any,
	event: string?,
	closable: boolean?,
	focus: boolean?,
	replace: boolean?,
	replacement: string?,
	replace_change: any,
	replace_one: any,
	replace_all: any,
	width: number?,
	to: string?,
}

return component {
	title = "Find Bar",
	category = "Forms",
	icon = "search",
	description = "A find box with \"3 of 12\", previous and next, and close; Enter and Shift-Enter step through matches, and `replace` adds a replace line.",
	when = "Finding text in the document or pane on screen (⌘F). For filtering a list use `search-field`; for running commands use `command-palette`.",
	props = {
		["id"] = "keeps the text across renders (default \"find\")",
		["value"] = "the query; applied when it changes, so `\"\"` clears it",
		["placeholder"] = "grey text while empty (default \"Find\")",
		["count"] = "how many matches there are",
		["index"] = "which one is current, from 1",
		["change"] = "sent on each edit, with `value` set to the text",
		["event"] = "key of the message the buttons and keys send, default \"find\": { find = \"next\" } (Enter, ↓), { find = \"previous\" } (Shift-Enter, ↑), { find = \"close\" } (×, Escape)",
		["closable"] = "false hides the × (default true)",
		["focus"] = "take focus when it appears",
		["replace"] = "show the replace line",
		["replacement"] = "the replacement text",
		["replace_change"] = "sent on each edit of the replacement, with `value`",
		["replace_one"] = "sent by Replace",
		["replace_all"] = "sent by Replace all",
		["width"] = "pixels (default 340)",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".find-bar"] = { background = "card", border = "all", border_color = "border", radius = 10, padding = { 4, 6, 4, 10 }, gap = 4, shadow = "0 6px 16px -4px #00000033" },
		[".find-bar-line"] = { gap = 4 },
		[".find-bar-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 3, 0 } },
		[".find-bar-count"] = { size = "small", color = "muted-foreground", padding = { 0, 4 } },
		[".find-bar-none"] = { size = "small", color = "danger", padding = { 0, 4 } },
	},
	example = { value = "ledger", count = 12, index = 3, replace = true, replacement = "book" },
	usage = [[local find_bar = use "find-bar"

find_bar {
	value = state.query, count = state.count, index = state.match, focus = true,
	change = "query",
}
-- update: msg.msg == "query" sets state.query = msg.value and state.match = 1;
-- msg.find == "next" and "previous" move state.match round state.count;
-- "close" hides it. Give `state.query` and `state.match` to `ui.markdown`'s `find` and
-- `find_index`, and keep `count` from its `found` message.]],

	build = function(props: Props)
		local value = props.value or ""
		local count = props.count or 0
		local event = props.event or "find"
		-- Tables, as a field's keys and a button's click then read the same.
		local next = { [event] = "next" }
		local previous = { [event] = "previous" }
		local close = { [event] = "close" }
		local keys = {
			enter = next,
			["shift-enter"] = previous,
			down = next,
			up = previous,
			escape = close,
		}
		local status = nil
		if value ~= "" then
			status = if count == 0
				then ui.text { "No results", class = "find-bar-none", wrap = false }
				else ui.text { `{props.index or 1} of {count}`, class = "find-bar-count", wrap = false }
		end
		local stepping = count > 0
		local find = ui.row {
			class = "find-bar-line",
			align = "center",
			ui.input {
				id = props.id or "find",
				class = "find-bar-input",
				value = props.value,
				placeholder = props.placeholder or "Find",
				change = props.change,
				keys = keys,
				focus = props.focus,
				to = props.to,
				grow = true,
			},
			status,
			icon_button { icon = "chevron-up", tooltip = "Previous", size = "small", msg = previous, to = props.to, disabled = not stepping },
			icon_button { icon = "chevron-down", tooltip = "Next", size = "small", msg = next, to = props.to, disabled = not stepping },
			props.closable ~= false and icon_button { icon = "close", tooltip = "Close", size = "small", msg = close, to = props.to },
		}
		local replace = nil
		if props.replace then
			replace = ui.row {
				class = "find-bar-line",
				align = "center",
				ui.input {
					id = `{props.id or "find"}-replace`,
					class = "find-bar-input",
					value = props.replacement,
					placeholder = "Replace",
					change = props.replace_change,
					keys = { escape = close },
					to = props.to,
					grow = true,
				},
				ui.button { "Replace", small = true, variant = "ghost", msg = props.replace_one, to = props.to, disabled = not stepping },
				ui.button { "All", small = true, variant = "ghost", msg = props.replace_all, to = props.to, disabled = not stepping },
			}
		end
		return ui.column { class = "find-bar", width = props.width or 340, find, replace }
	end,
}
