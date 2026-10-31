-- Key caps for a keyboard shortcut. Takes a list of keys or a string such
-- as "cmd-shift-p" (chords separated by spaces: "cmd-k cmd-s"), and draws
-- macOS glyphs for the modifiers and named keys.

type Props = {
	keys: any,
	small: boolean?,
	plain: boolean?,
}

-- Names in a shortcut string, to the glyphs macOS shows for them.
local GLYPHS = {
	cmd = "⌘",
	command = "⌘",
	super = "⌘",
	secondary = "⌘",
	shift = "⇧",
	alt = "⌥",
	option = "⌥",
	opt = "⌥",
	ctrl = "⌃",
	control = "⌃",
	enter = "↩",
	["return"] = "↩",
	escape = "esc",
	esc = "esc",
	tab = "⇥",
	backspace = "⌫",
	delete = "⌦",
	up = "↑",
	down = "↓",
	left = "←",
	right = "→",
	space = "Space",
	pageup = "⇞",
	pagedown = "⇟",
	home = "↖",
	["end"] = "↘",
}

local function glyph(name: string): string
	local known = GLYPHS[string.lower(name)]
	if known then
		return known
	end
	if #name == 1 then
		return string.upper(name)
	end
	-- f1-f24 and anything else: upper-case the first letter.
	return string.upper(string.sub(name, 1, 1)) .. string.sub(name, 2)
end

-- A shortcut string to chords, each a list of key labels.
local function parse(keys: any): { { string } }
	if type(keys) == "table" then
		return { keys }
	end
	local chords = {}
	for chord in string.gmatch(tostring(keys or ""), "%S+") do
		local caps = {}
		-- "cmd--" means cmd and the minus key.
		local rest = chord
		while #rest > 0 do
			local dash = string.find(rest, "-", 2, true)
			if dash then
				caps[#caps + 1] = glyph(string.sub(rest, 1, dash - 1))
				rest = string.sub(rest, dash + 1)
			else
				caps[#caps + 1] = glyph(rest)
				rest = ""
			end
		end
		chords[#chords + 1] = caps
	end
	return chords
end

return component {
	title = "Kbd",
	category = "Data",
	icon = "keyboard",
	description = "Key caps for a shortcut, from a list of keys or a string like \"cmd-shift-p\" drawn with macOS glyphs.",
	when = "Show a keyboard shortcut: in a menu row, a palette result, a tooltip line or a help sheet. Inside menus `menu` already draws one from an item's `shortcut`; for a word-like tag use `ui.badge`.",
	props = {
		["keys"] = "list of key labels ({ \"⌘\", \"⇧\", \"P\" }), or a string \"cmd-shift-p\"; spaces separate chords (\"cmd-k cmd-s\"). Names map to macOS glyphs: cmd/secondary ⌘, shift ⇧, alt/option ⌥, ctrl ⌃, enter ↩, tab ⇥, backspace ⌫, delete ⌦, arrows ↑↓←→, escape esc; single letters are upper-cased",
		["small"] = "smaller caps, for menu rows",
		["plain"] = "glyphs only, no cap borders (as in a macOS menu)",
	},
	styles = {
		[".kbd"] = { gap = 6 },
		[".kbd-chord"] = { gap = 3 },
		[".kbd-key"] = {
			padding = { 1, 5 },
			radius = 5,
			border = "all",
			border_color = "border",
			background = "muted/60",
			color = "muted-foreground",
			size = "small",
			shadow = "0 1 0 #00000014",
		},
		[".kbd-small"] = { padding = { 0, 4 }, radius = 4 },
		[".kbd-plain"] = { padding = 0, border = "none", background = "#00000000", shadow = "none" },
	},
	example = { keys = "cmd-shift-p" },
	usage = [[local kbd = use "kbd"

kbd { keys = "cmd-shift-p" }          -- ⌘ ⇧ P
kbd { keys = { "⌘", "K" } }
kbd { keys = "cmd-k cmd-s", small = true }
kbd { keys = "ctrl-tab", plain = true }]],

	build = function(props: Props)
		local chords = {}
		for _, caps in parse(props.keys) do
			local cells = {}
			for _, cap in caps do
				local class = { "kbd-key" }
				if props.small then
					table.insert(class, "kbd-small")
				end
				if props.plain then
					table.insert(class, "kbd-plain")
				end
				cells[#cells + 1] = ui.row {
					class = class,
					align = "center",
					justify = "center",
					ui.text { cap },
				}
			end
			chords[#chords + 1] = ui.row { class = "kbd-chord", align = "center", table.unpack(cells) }
		end
		return ui.row { class = "kbd", align = "center", table.unpack(chords) }
	end,
}
