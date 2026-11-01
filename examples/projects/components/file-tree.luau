-- A project's folders as an editor's file tree: folders first, chevrons,
-- icons by type, ignored entries dimmed, and changed files marked. It draws
-- what the `projects` provider lists in `app.projects.dirs` (entries as
-- "<kind><name>": d a folder, f a file, D and F when git ignores them);
-- the mod keeps which folders are open and tells the provider.

local file_icon = use "file-icon"

type Props = {
	dirs: { [string]: { string } }?,
	open: { [string]: boolean? }?,
	selected: string?,
	marks: { [string]: string }?,
	toggle: string?,
	pick: string?,
	to: string?,
	indent: number?,
}

local TONES = { M = "warning", A = "success", D = "danger", R = "accent" }

return component {
	title = "File Tree",
	category = "Projects",
	icon = "list-tree",
	description = "A project's folders and files, folders first, with chevrons, icons by type, dimmed ignored entries and change marks.",
	when = "The Files view of a project: what `app.projects.dirs` lists, with the open folders in the mod's state. For only the changed files use `change-tree`; for any other nesting use `tree`.",
	props = {
		["dirs"] = "`app.projects.dirs`: entries by folder path, \"\" for the root",
		["open"] = "open folders: { [path] = true }",
		["selected"] = "the chosen file's path",
		["marks"] = "status letters by path (M, A, D), shown at the right",
		["toggle"] = "key of the message a folder sends, default \"toggle\": { toggle = path }",
		["pick"] = "key of the message a file sends, default \"pick\": { pick = path }",
		["to"] = "the mod to send them to, if not the drawing one",
		["indent"] = "pixels per level (default 14)",
	},
	styles = {
		[".file-tree"] = { gap = 0 },
		[".file-tree-row"] = { padding = { 3, 8 }, radius = 8, gap = 6 },
		[".file-tree-row:hover"] = { background = "list-hover" },
		[".file-tree-row:selected"] = { background = "card", shadow = "0 1px 2px #0000000f" },
		[".file-tree-ignored"] = { opacity = 0.45 },
		[".file-tree-chevron"] = { color = "muted-foreground" },
		[".file-tree-mark"] = { size = "small", weight = "bold", font = "mono" },
	},
	example = {
		dirs = {
			[""] = { "dcmd", "Dnode_modules", "f.gitignore", "fapp.go", "fREADME.md" },
			["cmd"] = { "fmain.go" },
		},
		open = { cmd = true },
		selected = "app.go",
		marks = { ["app.go"] = "M", ["cmd/main.go"] = "A" },
	},

	build = function(props: Props)
		local dirs = props.dirs or {}
		local open = props.open or {}
		local marks = props.marks or {}
		local toggle = props.toggle or "toggle"
		local pick = props.pick or "pick"
		local indent = props.indent or 14
		local rows = {}

		local function add(dir: string, depth: number)
			local entries = dirs[dir]
			if not entries then
				table.insert(rows, ui.row {
					class = "file-tree-row",
					ui.row { width = indent * depth + 14 },
					ui.text { "…", tone = "muted", size = "small" },
				})
				return
			end
			for _, entry in entries do
				local kind = string.sub(entry, 1, 1)
				local name = string.sub(entry, 2)
				local folder = kind == "d" or kind == "D"
				local path = if dir == "" then name else `{dir}/{name}`
				local expanded = folder and open[path] == true
				local mark = marks[path]
				local classes = { "file-tree-row" }
				if kind == "D" or kind == "F" then
					table.insert(classes, "file-tree-ignored")
				end
				local msg = if folder then { [toggle] = path } else { [pick] = path }
				table.insert(rows, ui.row {
					class = classes,
					align = "center",
					selected = path == props.selected,
					msg = msg,
					to = props.to,
					ui.row { width = indent * depth },
					if folder
						then ui.icon {
							name = if expanded then "chevron-down" else "chevron-right",
							class = "file-tree-chevron",
							size = "small",
						}
						else ui.row { width = 14 },
					file_icon { name = name, dir = folder, open = expanded },
					ui.text { name, truncate = true },
					ui.spacer(),
					mark and ui.text { mark, class = "file-tree-mark", tone = (TONES[mark] or "muted") :: any },
				})
				if expanded then
					add(path, depth + 1)
				end
			end
		end

		add("", 0)
		return ui.column { class = "file-tree", gap = "none", shrink = false, table.unpack(rows) }
	end,
}
