-- A quiet footer line of keyboard hints, "↑↓ choose · ↵ open · esc close",
-- from a list of { keys, label }. Keys are drawn as small glyphs (or caps).

local kbd = use "kbd"

type Hint = {
	keys: any,
	label: string,
}

type Props = {
	hints: { Hint },
	-- Draw each key as a bordered cap instead of a plain glyph.
	caps: boolean?,
	-- Separator between hints; default "·".
	separator: string?,
	justify: string?,
}

return component {
	title = "Key hints",
	category = "Feedback",
	icon = "command",
	description = "A muted footer line of keyboard hints, such as ↑↓ choose · ↵ open · esc close.",
	when = "Under a palette, picker, list or dialog to say which keys work there. For a full reference of shortcuts use `shortcuts`; for one shortcut beside a menu item or button use `kbd`.",
	props = {
		["hints"] = "list of { keys = \"up down\" or \"enter\" or { \"↑\", \"↓\" }, label }",
		["caps"] = "bordered key caps instead of plain glyphs",
		["separator"] = "text between hints; default \"·\"",
		["justify"] = "`start` (default), `center`, `end` or `between`",
	},
	styles = {
		[".key-hints"] = { gap = 8, size = "small", color = "muted-foreground", padding = { 4, 0 } },
		[".key-hints-hint"] = { gap = 5 },
		[".key-hints-keys"] = { gap = 2, color = "foreground" },
		[".key-hints-label"] = { size = "small", color = "muted-foreground" },
		[".key-hints-separator"] = { size = "small", color = "muted-foreground/60" },
	},
	example = {
		hints = {
			{ keys = "up down", label = "choose" },
			{ keys = "enter", label = "open" },
			{ keys = "cmd-enter", label = "open in new tab" },
			{ keys = "escape", label = "close" },
		},
	},

	build = function(props: Props)
		local children: { any } = {}
		for index, hint in ipairs(props.hints or {}) do
			if index > 1 then
				children[#children + 1] = ui.text { props.separator or "·", class = "key-hints-separator" }
			end
			-- "up down" is two keys pressed apart, not a chord: one cap set each.
			local caps: { any } = {}
			if type(hint.keys) == "string" and string.find(hint.keys, " ") and not string.find(hint.keys, "-") then
				for name in string.gmatch(hint.keys, "%S+") do
					caps[#caps + 1] = kbd { keys = name, small = true, plain = not props.caps }
				end
			else
				caps[1] = kbd { keys = hint.keys, small = true, plain = not props.caps }
			end
			children[#children + 1] = ui.row {
				class = "key-hints-hint",
				align = "center",
				ui.row { class = "key-hints-keys", table.unpack(caps) },
				ui.text { hint.label, class = "key-hints-label" },
			}
		end
		return ui.row {
			class = "key-hints",
			align = "center",
			justify = (props.justify or "start") :: any,
			wrap = true,
			table.unpack(children),
		}
	end,
}
