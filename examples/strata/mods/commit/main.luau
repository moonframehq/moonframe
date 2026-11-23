-- The right pane: the commit the timeline picked, as the git provider
-- publishes it in `app.git.commit`: its subject and message, who made it and
-- when, its parents and refs, then every file it changed as a diff, one open
-- at a time. A header opens or closes its file; the open one's diff comes
-- from git as `commit.diff` once this mod asks for it with `show_diff`. A
-- merge shows what it brought against its first parent.

local diff_counts = use "diff-counts"
local diff_view = use "diff-view"
local empty_state = use "empty-state"
local relative_time = use "relative-time"

type State = {
	-- The commit on show, and its file whose diff is open.
	sha: string?,
	open: string?,
	-- Old and new side by side.
	split: boolean,
}

type File = { status: string, added: number, deleted: number, path: string }

-- "M\t12\t3\tsrc/a.rs" as its parts.
local function file_of(line: string): File?
	local status, added, deleted, path = string.match(line, "^(%a)\t(%d+)\t(%d+)\t(.+)$")
	if not status or not path then
		return nil
	end
	return { status = status, added = tonumber(added) or 0, deleted = tonumber(deleted) or 0, path = path }
end

local function open_file(state: State, path: string?): { any }
	state.open = path
	return { { to = "provider:git", msg = { show_diff = path or false } } }
end

-- The file's diff, if git has sent it, and a note when it cannot show one.
local function diff_of(commit: any, path: string): (any, string?)
	local diff = commit.diff
	if not diff or diff.path ~= path then
		return nil, "Reading the diff…"
	end
	if diff.binary then
		return diff, "A binary file: no lines to show."
	end
	if #(diff.hunks or {}) == 0 then
		return diff, "No lines changed: a mode change, or an empty file."
	end
	return diff, if diff.truncated then `Only the first {diff.lines} lines are shown.` else nil
end

local function header(commit: any, now: number?)
	local seconds = if now and commit.time then math.max(0, now - commit.time) else nil
	local body = commit.body or ""
	return ui.column {
		gap = "small",
		class = "commit-head",
		ui.text { commit.subject or "", size = "large", weight = "semibold" },
		ui.row {
			gap = "small",
			align = "center",
			ui.icon { "user", tone = "muted" },
			ui.text { commit.author or "", tone = "muted", tooltip = commit.email },
			seconds and ui.text { "·", tone = "muted" } or nil,
			seconds and relative_time { seconds = seconds, size = "normal" } or nil,
			ui.text { "·", tone = "muted" },
			ui.text { commit.short or "", class = "commit-sha", tooltip = "Copy the full id", copy = commit.sha },
			ui.spacer(),
			diff_counts { files = commit.count, added = commit.added, removed = commit.deleted },
		},
		body ~= "" and ui.text { body, class = "commit-body" } or nil,
	}
end

-- Parents to step back to, and the branches and tags on the commit.
local function lineage(commit: any)
	local parents: { any } = {}
	for _, sha in commit.parents or {} do
		table.insert(parents, ui.button {
			string.sub(sha, 1, 7),
			icon = "git-commit-horizontal",
			small = true,
			variant = "ghost",
			tooltip = "Show this parent",
			to = "timeline",
			msg = { pick = sha },
		})
	end
	local refs: { any } = {}
	for _, name in commit.refs or {} do
		table.insert(refs, ui.badge { name })
	end
	return ui.column {
		gap = "small",
		class = "commit-lineage",
		ui.row {
			gap = "small",
			align = "center",
			ui.text { if #parents > 1 then "Merge of" elseif #parents == 1 then "Parent" else "First commit", class = "commit-label" },
			table.unpack(parents),
		},
		#refs > 0 and ui.row { gap = "small", align = "center", wrap = true, ui.text { "On", class = "commit-label" }, table.unpack(refs) } or nil,
	}
end

return mod {
	title = "Commit",
	subscribe = { "provider/git" },

	styles = {
		[".commit-head"] = { padding = { 14, 18, 6, 18 } },
		[".commit-sha"] = { font = "mono", color = "muted-foreground" },
		[".commit-body"] = { color = "muted-foreground", padding = { 4, 0, 0, 0 } },
		[".commit-lineage"] = { padding = { 0, 18, 10, 18 } },
		[".commit-label"] = { size = "small", color = "muted-foreground", width = 64 },
		[".commit-files"] = { padding = { 0, 14, 14, 14 } },
		[".commit-note"] = { size = "small", color = "muted-foreground", padding = { 8, 12 } },
	},

	init = function(previous: State?): State
		return { split = previous ~= nil and previous.split or false }
	end,

	update = function(state: State, msg: any)
		if msg == "split" then
			state.split = not state.split
			return state
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/git" then
			local commit = type(msg.value) == "table" and msg.value.commit
			local sha = commit and commit.sha
			if sha == state.sha then
				return
			end
			state.sha = sha
			-- Another commit: its first file opens.
			local first = commit and commit.files and commit.files[1]
			local file = first and file_of(first)
			return state, open_file(state, if file then file.path else nil)
		elseif msg.toggle then
			return state, open_file(state, if msg.toggle == state.open then nil else msg.toggle)
		elseif msg.switch then
			state.split = msg.switch == "split"
			return state
		end
		return
	end,

	view = function(state: State, app)
		local git = app.git
		local commit = git and git.commit
		if not commit then
			return empty_state {
				icon = "git-commit-horizontal",
				title = "No commit picked",
				message = "Pick one in the history to read its message and diff.",
			}
		end
		local files: { any } = {}
		local listed = 0
		for _, line in commit.files do
			local file = file_of(line)
			if file then
				listed += 1
				local open = file.path == state.open
				local diff, note = nil, nil
				if open then
					diff, note = diff_of(commit, file.path)
				end
				table.insert(files, diff_view {
					path = file.path,
					status = file.status,
					collapsed = not open,
					split = state.split,
					switch = if open then "switch" else nil,
					hunks = diff and diff.hunks or {},
					-- An open diff counts its own lines in its header.
					actions = if open then nil else { diff_counts { added = file.added, removed = file.deleted } },
				})
				if note then
					table.insert(files, ui.text { note, class = "commit-note" })
				end
			end
		end
		if commit.truncated then
			table.insert(files, ui.text { `{commit.count - listed} more files are not listed.`, class = "commit-note" })
		end
		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				gap = "none",
				shrink = false,
				header(commit, app.projects and app.projects.now),
				lineage(commit),
				ui.column { gap = "small", class = "commit-files", table.unpack(files) },
			},
		}
	end,
}
