-- Changed files as a tree of their folders, as a review shows them: a
-- folder holding a single folder joins it ("cmd/ryui"), and each file has
-- its status letter at the right. It draws `app.projects.current.files`
-- ("<status>\t<added>\t<deleted>\t<path>"); folders start open, and the mod
-- keeps the ones folded.

local file_icon = use "file-icon"

type Props = {
	files: { string }?,
	selected: string?,
	folded: { [string]: boolean? }?,
	fold: string?,
	pick: string?,
	to: string?,
	indent: number?,
}

type Folder = { name: string, path: string, folders: { [string]: Folder }, files: { any } }

local TONES = { M = "warning", A = "success", D = "danger", R = "accent" }

local function folder(name: string, path: string): Folder
	return { name = name, path = path, folders = {}, files = {} }
end

-- The changed files, nested by folder.
local function nest(lines: { string }): Folder
	local root = folder("", "")
	for _, line in lines do
		local status, added, deleted, path = string.match(line, "^(%a)\t(%d+)\t(%d+)\t(.+)$")
		if path then
			local at = root
			local parts = string.split(path, "/")
			for index = 1, #parts - 1 do
				local part = parts[index]
				local below = at.folders[part]
				if not below then
					below = folder(part, if at.path == "" then part else `{at.path}/{part}`)
					at.folders[part] = below
				end
				at = below
			end
			table.insert(at.files, {
				name = parts[#parts],
				path = path,
				status = status,
				added = tonumber(added),
				deleted = tonumber(deleted),
			})
		end
	end
	return root
end

local function sorted_folders(at: Folder): { Folder }
	local list = {}
	for _, below in at.folders do
		table.insert(list, below)
	end
	table.sort(list, function(a, b)
		return string.lower(a.name) < string.lower(b.name)
	end)
	return list
end

return component {
	title = "Change Tree",
	category = "Projects",
	icon = "git-compare",
	description = "Changed files nested in their folders, single-child folders joined, with A, M or D at the right.",
	when = "The Changes view of a project or review: what `app.projects.current.files` lists. For every file use `file-tree`; for a file's lines use `diff-view`.",
	props = {
		["files"] = "`app.projects.current.files`: \"<status>\\t<added>\\t<deleted>\\t<path>\"",
		["selected"] = "the chosen file's path",
		["folded"] = "folders folded shut: { [path] = true }",
		["fold"] = "key of the message a folder sends, default \"fold\": { fold = path }",
		["pick"] = "key of the message a file sends, default \"pick\": { pick = path }",
		["to"] = "the mod to send them to, if not the drawing one",
		["indent"] = "pixels per level (default 14)",
	},
	styles = {
		[".change-tree-row"] = { padding = { 3, 8 }, radius = 8, gap = 6 },
		[".change-tree-row:hover"] = { background = "list-hover" },
		[".change-tree-row:selected"] = { background = "card", shadow = "0 1px 2px #0000000f" },
		[".change-tree-chevron"] = { color = "muted-foreground" },
		[".change-tree-mark"] = { size = "small", weight = "bold", font = "mono" },
	},
	example = {
		files = {
			"A\t40\t0\tappleauth/appleauth.go",
			"M\t12\t3\tcmd/ryui/android.go",
			"M\t2\t1\tcmd/ryui/config.go",
			"M\t5\t0\tplatform/android/app/build.gradle.kts",
		},
		selected = "cmd/ryui/config.go",
	},

	build = function(props: Props)
		local folded = props.folded or {}
		local fold = props.fold or "fold"
		local pick = props.pick or "pick"
		local indent = props.indent or 14
		local rows = {}

		local function add(at: Folder, depth: number)
			for _, below in sorted_folders(at) do
				-- A folder holding only one folder joins it.
				local label = below.name
				while #below.files == 0 do
					local only = sorted_folders(below)
					if #only ~= 1 then
						break
					end
					below = only[1]
					label = `{label}/{below.name}`
				end
				local open = not folded[below.path]
				table.insert(rows, ui.row {
					class = "change-tree-row",
					align = "center",
					msg = { [fold] = below.path },
					to = props.to,
					ui.row { width = indent * depth },
					ui.icon {
						name = if open then "chevron-down" else "chevron-right",
						class = "change-tree-chevron",
						size = "small",
					},
					file_icon { name = label, dir = true, open = open },
					ui.text { label, truncate = true },
				})
				if open then
					add(below, depth + 1)
				end
			end
			table.sort(at.files, function(a, b)
				return string.lower(a.name) < string.lower(b.name)
			end)
			for _, file in at.files do
				table.insert(rows, ui.row {
					class = "change-tree-row",
					align = "center",
					selected = file.path == props.selected,
					msg = { [pick] = file.path },
					to = props.to,
					tooltip = `+{file.added} −{file.deleted}`,
					ui.row { width = indent * depth + 14 },
					file_icon { name = file.name },
					ui.text { file.name, truncate = true },
					ui.spacer(),
					ui.text {
						file.status,
						class = "change-tree-mark",
						tone = (TONES[file.status] or "muted") :: any,
					},
				})
			end
		end

		add(nest(props.files or {}), 0)
		return ui.column { class = "change-tree", gap = "none", shrink = false, table.unpack(rows) }
	end,
}
