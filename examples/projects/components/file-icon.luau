-- A file's or folder's icon, coloured by its type as editors colour them:
-- by its whole name first, then its extension.

type Props = {
	name: string,
	dir: boolean?,
	open: boolean?,
}

-- Icon and colour by name, then by extension.
local BY_NAME = {
	["Cargo.toml"] = { "settings-2", "#c76f3b" },
	["Cargo.lock"] = { "lock", "#c76f3b" },
	["go.mod"] = { "settings-2", "#00a5c8" },
	["go.sum"] = { "lock", "#00a5c8" },
	["package.json"] = { "settings-2", "#8bb54a" },
	["LICENSE"] = { "file-text", "#c2a83e" },
	["README.md"] = { "book-open", "#3f7fbf" },
	["AGENTS.md"] = { "book-open", "#3f7fbf" },
	["CLAUDE.md"] = { "agent-claude", "#d97757" },
	["justfile"] = { "terminal", "#5f9a35" },
	["Makefile"] = { "terminal", "#5f9a35" },
	["Dockerfile"] = { "layers", "#2496ed" },
	[".gitignore"] = { "git-branch", "#e0533a" },
	[".gitattributes"] = { "git-branch", "#e0533a" },
	[".gitmodules"] = { "git-branch", "#e0533a" },
}
local BY_EXTENSION = {
	go = { "code", "#00a5c8" },
	rs = { "code", "#c76f3b" },
	swift = { "code", "#f05138" },
	kt = { "code", "#a074c4" },
	kts = { "code", "#a074c4" },
	java = { "code", "#b07219" },
	c = { "code", "#5577aa" },
	h = { "code", "#a074c4" },
	cpp = { "code", "#f34b7d" },
	py = { "code", "#3572a5" },
	rb = { "code", "#cc342d" },
	js = { "code", "#c9a400" },
	mjs = { "code", "#c9a400" },
	jsx = { "code", "#1f9fc4" },
	ts = { "code", "#3178c6" },
	tsx = { "code", "#1f9fc4" },
	luau = { "moon", "#5b6cff" },
	lua = { "moon", "#5b6cff" },
	sh = { "terminal", "#5f9a35" },
	zsh = { "terminal", "#5f9a35" },
	toml = { "settings", "#8a8f98" },
	yml = { "settings", "#cb4b16" },
	yaml = { "settings", "#cb4b16" },
	json = { "code", "#b8a300" },
	lock = { "lock", "#8a8f98" },
	md = { "book-open", "#3f7fbf" },
	txt = { "file-text", "#8a8f98" },
	html = { "code", "#e34c26" },
	css = { "code", "#563d7c" },
	svg = { "frame", "#a074c4" },
	png = { "frame", "#a074c4" },
	jpg = { "frame", "#a074c4" },
	jpeg = { "frame", "#a074c4" },
	gif = { "frame", "#a074c4" },
	wit = { "plug", "#4fb3cf" },
}
local FILE = { "file", "#8a8f98" }
local FOLDER = "#6b7280"

return component {
	title = "File Icon",
	category = "Projects",
	icon = "file",
	description = "A file's or folder's icon, coloured by its type, as an editor's file tree shows it.",
	when = "Beside a file name in a tree, a tab or a list of changes. For a generic icon use `ui.icon`.",
	props = {
		["name"] = "the file's name, which picks the icon and colour",
		["dir"] = "a folder",
		["open"] = "an open folder",
	},
	example = { name = "app.go" },

	build = function(props: Props)
		if props.dir then
			return ui.icon {
				name = if props.open then "folder-open" else "folder",
				class = "file-icon",
				style = { color = FOLDER },
			}
		end
		local known = BY_NAME[props.name]
		if not known then
			local extension = string.match(props.name, "%.([%w]+)$")
			known = extension and BY_EXTENSION[string.lower(extension)] or FILE
		end
		return ui.icon { name = known[1] :: any, class = "file-icon", style = { color = known[2] } }
	end,
}
