-- Changed files as a tree of their folders, as a review shows them: a
-- folder holding a single folder joins it ("cmd/ryui"), and each file has
-- its status letter at the right. It draws a list of changed files
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
	counts: boolean?,
	mark: string?,
	plain: boolean?,
}

type Folder = { name: string, path: string, folders: { [string]: Folder }, files: { any } }

-- Each status letter's class, which the stylesheet colours.
local KINDS = { M = "modified", A = "added", D = "deleted", R = "renamed", ["?"] = "added", U = "conflict" }

local function folder(name: string, path: string): Folder
	return { name = name, path = path, folders = {}, files = {} }
end

-- The changed files, nested by folder.
local function nest(lines: { string }): Folder
	local root = folder("", "")
	for _, line in lines do
		local status, added, deleted, path = string.match(line, "^(%S)\t(%d+)\t(%d+)\t(.+)$")
		if path then
			local at = root
			-- A new folder git lists whole (`docs/new/`) is one entry,
			-- named with its slash.
			local whole = string.sub(path, -1) == "/"
			local parts = string.split(if whole then string.sub(path, 1, -2) else path, "/")
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
				name = if whole then parts[#parts] .. "/" else parts[#parts],
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
	category = "Git",
	icon = "git-compare",
	description = "Changed files nested in their folders, single-child folders joined, with A, M or D at the right.",
	when = "The changed files of a repository or a review, as the right pane's Changes shows `app.git.files`. For every file in a folder use `tree`; for a file's lines use `diff-view`.",
	props = {
		["files"] = "lines of \"<status>\\t<added>\\t<deleted>\\t<path>\" (`app.projects.current.files`, or made from `app.git.files`)",
		["selected"] = "the chosen file's path",
		["folded"] = "folders folded shut: { [path] = true }",
		["fold"] = "key of the message a folder sends, default \"fold\": { fold = path }",
		["pick"] = "key of the message a file sends, default \"pick\": { pick = path }",
		["to"] = "the mod to send them to, if not the drawing one",
		["indent"] = "pixels per level (default 14)",
		["counts"] = "show each file's +added −deleted at the right",
		["mark"] = "where the status letter goes: \"end\" (default, at the right) or \"start\" (before the icon)",
		["plain"] = "outline folder and file icons instead of coloured ones by type",
	},
	styles = {
		[".change-tree-row"] = { padding = { 3, 8 }, radius = 8, gap = 6 },
		[".change-tree-row:hover"] = { background = "list-hover" },
		[".change-tree-row:selected"] = { background = "card", shadow = "0 1px 2px #0000000f" },
		[".change-tree-chevron"] = { color = "muted-foreground" },
		[".change-tree-mark"] = { size = "small", weight = "bold", font = "mono", color = "muted-foreground" },
		[".change-tree-mark-modified"] = { color = "warning" },
		[".change-tree-mark-added"] = { color = "success" },
		[".change-tree-mark-deleted"] = { color = "danger" },
		[".change-tree-mark-renamed"] = { color = "accent" },
		[".change-tree-mark-conflict"] = { color = "danger" },
		[".change-tree-icon"] = { color = "muted-foreground" },
		[".change-tree-added"] = { size = "small", weight = "medium", font = "mono", color = "success" },
		[".change-tree-deleted"] = { size = "small", weight = "medium", font = "mono", color = "danger" },
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
					if props.plain
						then ui.icon { name = "folder", class = "change-tree-icon", size = "small" }
						else file_icon { name = label, dir = true, open = open },
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
				local mark = ui.text {
					file.status,
					class = { "change-tree-mark", `change-tree-mark-{KINDS[file.status] or "other"}` },
				}
				local start = props.mark == "start"
				table.insert(rows, ui.row {
					class = "change-tree-row",
					align = "center",
					selected = file.path == props.selected,
					msg = { [pick] = file.path },
					to = props.to,
					tooltip = if props.counts then nil else `+{file.added} −{file.deleted}`,
					ui.row { width = indent * depth + 14 },
					start and mark,
					if props.plain
						then ui.icon { name = "file-text", class = "change-tree-icon", size = "small" }
						else file_icon { name = file.name },
					ui.text { file.name, truncate = true },
					ui.spacer(),
					props.counts and ui.text { `+{file.added}`, class = "change-tree-added" },
					props.counts and ui.text { `−{file.deleted}`, class = "change-tree-deleted" },
					not start and mark,
				})
			end
		end

		add(nest(props.files or {}), 0)
		return ui.column { class = "change-tree", gap = "none", shrink = false, table.unpack(rows) }
	end,
}
