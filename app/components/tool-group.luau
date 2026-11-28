-- A run of an agent's tool calls folded to one line: a status icon, what
-- the run did ("Read 3 files, ran 2 commands") and the lines it changed.
-- Open, each call on a line of its own with the end of its output, the
-- files it changed and a sub-agent's step count; thoughts between calls
-- show as quiet lines. It draws a `ToolGroup` from `app.chat`'s view.

type Props = {
	group: any,
	open: boolean?,
	msg: any,
	to: string?,
	diagrams: { any }?,
	-- Which group draws each diagram's card: its id to that group's key.
	cards: { [string]: string }?,
}

-- Lucide icons for herdr's tool kinds.
local KIND_ICONS: { [string]: string } = {
	read = "file-text",
	edit = "pencil",
	delete = "trash-2",
	move = "replace",
	search = "search",
	execute = "terminal",
	think = "brain",
	fetch = "globe",
	switch_mode = "sliders-horizontal",
	subagent = "bot",
	other = "settings-2",
}

local function status_icon(status: string): any
	if status == "running" or status == "pending" then
		return ui.icon { name = "loader-circle", size = "small", spin = true, class = "tool-group-running" }
	elseif status == "failed" then
		return ui.icon { name = "circle-x", size = "small", class = "tool-group-failed" }
	end
	return ui.icon { name = "check", size = "small", class = "tool-group-done" }
end

-- "+12 −3", or nothing without counts.
local function counts(file: any): any
	if file.added == nil and file.removed == nil then
		return nil
	end
	return ui.row {
		gap = "small",
		hug = true,
		(file.added or 0) > 0 and ui.text { `+{file.added}`, size = "small", class = "tool-group-added" },
		(file.removed or 0) > 0 and ui.text { `−{file.removed}`, size = "small", class = "tool-group-removed" },
	}
end

-- The lines a group added and removed, over every file.
local function totals(files: { any }): (number, number)
	local added, removed = 0, 0
	for _, file in files do
		added += file.added or 0
		removed += file.removed or 0
	end
	return added, removed
end

local function call_line(tool: any): any
	local files: { any } = {}
	for _, file in tool.files or {} do
		table.insert(files, ui.row {
			class = "tool-group-file",
			gap = "small",
			align = "center",
			ui.icon { name = if file.new then "plus" else "file", size = "small" },
			ui.text { file.path, size = "small", truncate = true },
			counts(file),
		})
	end
	return ui.column {
		class = "tool-group-call",
		gap = "small",
		ui.row {
			gap = "small",
			align = "center",
			ui.icon { name = (KIND_ICONS[tool.kind] or "settings-2") :: any, size = "small", class = "tool-group-kind" },
			ui.text { tool.title, size = "small", class = "tool-group-call-title", truncate = true },
			(tool.steps or 0) > 0 and ui.text {
				`{tool.steps} {if tool.steps == 1 then "step" else "steps"}`,
				size = "small",
				tone = "muted",
			},
			status_icon(tool.status),
		},
		tool.detail and ui.text { tool.detail, class = "tool-group-detail", size = "small", wrap = false },
		#files > 0 and ui.column { gap = "small", table.unpack(files) },
	}
end

local diagram_card = use "diagram-card"

-- Whether a call is one of `moonframe mcp diagrams`' tools.
local function is_diagram_call(item: any): boolean
	return string.find(item.title or "", "diagram_", 1, true) ~= nil
end

-- The diagram a call names first in its answer ("Showing `plan` ...",
-- "`plan`: ..."), for the group's calls still listed.
local function named_diagram(item: any): string?
	return item.detail and string.match(item.detail, "`([%w_-]+)`")
end

-- The diagrams the group's `diagram_*` calls showed, `{ id, run? }`:
-- from `group.diagrams`, which keeps them past its trimmed `items`.
local function shown_diagrams(group: any): { any }
	if group.diagrams then
		return group.diagrams
	end
	local shown: { any } = {}
	local seen: { [string]: boolean } = {}
	for _, item in group.items or {} do
		local id = item.type == "tool" and item.status ~= "failed" and is_diagram_call(item) and named_diagram(item)
		if id and not seen[id] then
			seen[id] = true
			table.insert(shown, { id = id, run = string.match(item.detail, "%[run ([%w_-]+)%]%s*$") })
		end
	end
	return shown
end

-- Cards for the diagrams the group showed that it draws (the first group
-- of a chat to show one, by `cards`), each as it stands now in
-- `app.diagrams`, while its latest writer is still the run that showed it
-- here (another session may reuse the id); and which ids those are.
local function diagram_cards(group: any, diagrams: { any }?, owners: { [string]: string }?): ({ any }, { [string]: boolean })
	local cards: { any } = {}
	local drawn: { [string]: boolean } = {}
	if not diagrams then
		return cards, drawn
	end
	for _, shown in shown_diagrams(group) do
		local id = shown.id
		local owner = owners and owners[id]
		if not drawn[id] and (owner == nil or owner == group.key) then
			for _, d in diagrams do
				if d.id == id and d.from == "agent" and (shown.run == nil or d.run == shown.run) then
					drawn[id] = true
					table.insert(cards, diagram_card { diagram = d })
					break
				end
			end
		end
	end
	return cards, drawn
end

return component {
	title = "Tool Group",
	category = "Data",
	icon = "workflow",
	description = "A run of an agent's tool calls folded to one line with its status and changed lines; open, each call with its output's end and its files.",
	when = "Inside a chat with a coding agent, for each `tools` row of `app.chat`'s view (`agent-chat` draws them). For one agent run's whole progress use `agent-run`; for a plain fold use `collapsible`.",
	props = {
		["group"] = "a `ToolGroup` from `app.chat`: { key, title, status (\"running\" \"done\" \"failed\"), count, failed, current?, files = { { path, added?, removed?, new } }, more, diagrams? = { { id, run? } }, items = { { type = \"tool\", id, kind, title, status, detail?, files, steps } or { type = \"thought\", gist, text } } }",
		["open"] = "show its calls",
		["msg"] = "message a click on the header sends (the mod flips `open`)",
		["to"] = "mod to send it to, if not the drawing one",
		["diagrams"] = "`app.diagrams.diagrams`: the diagrams its `diagram_*` calls showed (`group.diagrams`, each drawn while the run that showed it here is its latest writer) draw as cards, open or folded, in place of the calls",
		["cards"] = "which group draws each diagram's card, its id to the group's `key`: a chat passes the first group to show it, so a diagram draws once however many groups change it",
	},
	styles = {
		[".tool-group"] = { border = true, border_color = "border", radius = 10, gap = 0 },
		[".tool-group-head"] = { padding = { 7, 10 }, gap = 8, radius = 10 },
		[".tool-group-head:hover"] = { background = "list-hover" },
		[".tool-group-title"] = { size = "small", weight = "medium" },
		[".tool-group-current"] = { size = "small", color = "muted-foreground" },
		[".tool-group-cards"] = { padding = { 4, 10, 10, 10 }, gap = 8 },
		[".tool-group-body"] = { padding = { 4, 10, 10, 10 }, gap = 8, border = "top", border_color = "border" },
		[".tool-group-kind"] = { color = "muted-foreground" },
		[".tool-group-call-title"] = { font = "mono" },
		[".tool-group-detail"] = { font = "mono", color = "muted-foreground", background = "muted", radius = 6, padding = { 4, 8 } },
		[".tool-group-file"] = { padding = { 0, 0, 0, 20 }, color = "muted-foreground" },
		[".tool-group-thought"] = { color = "muted-foreground" },
		[".tool-group-running"] = { color = "accent" },
		[".tool-group-done"] = { color = "success" },
		[".tool-group-failed"] = { color = "danger" },
		[".tool-group-added"] = { color = "success" },
		[".tool-group-removed"] = { color = "danger" },
	},
	example = {
		open = true,
		group = {
			key = "e2",
			title = "Read 2 files, edited 1 file",
			status = "done",
			count = 3,
			failed = 0,
			more = 0,
			files = { { path = "src/lib.rs", added = 12, removed = 3, new = false } },
			items = {
				{ type = "thought", gist = "Looking at the parser first", text = "" },
				{ type = "tool", id = "a", kind = "read", title = "Read src/parse.rs", status = "done", steps = 0, files = {} },
				{ type = "tool", id = "b", kind = "execute", title = "cargo test parse", status = "done", steps = 0, files = {}, detail = "test result: ok. 14 passed" },
				{ type = "tool", id = "c", kind = "edit", title = "Edit src/lib.rs", status = "done", steps = 0, files = { { path = "src/lib.rs", added = 12, removed = 3, new = false } } },
			},
		},
	},

	build = function(props: Props)
		local group = props.group or {}
		local items: { any } = {}
		local cards, drawn = diagram_cards(group, props.diagrams, props.cards)
		if props.open then
			if (group.more or 0) > 0 then
				table.insert(items, ui.text { `{group.more} earlier calls`, size = "small", tone = "muted" })
			end
			for _, item in group.items or {} do
				local id = item.type == "tool" and item.status ~= "failed" and is_diagram_call(item) and named_diagram(item)
				if id and drawn[id] then
					-- Drawn as its card instead; a failed call, or one whose
					-- diagram draws elsewhere or not at all, stays a line.
				elseif item.type == "tool" then
					table.insert(items, call_line(item))
				elseif item.gist and item.gist ~= "" then
					table.insert(items, ui.row {
						class = "tool-group-thought",
						gap = "small",
						align = "center",
						ui.icon { name = "brain", size = "small" },
						ui.text { item.gist, size = "small", truncate = true },
					})
				end
			end
		end
		local added, removed = totals(group.files or {})
		local running = group.status == "running"
		return ui.column {
			class = "tool-group",
			ui.row {
				class = "tool-group-head",
				align = "center",
				msg = props.msg,
				to = props.to,
				ui.icon { name = if props.open then "chevron-down" else "chevron-right", size = "small", class = "tool-group-kind" },
				status_icon(group.status or "done"),
				ui.column {
					grow = true,
					gap = "none",
					ui.text { group.title or "", class = "tool-group-title", truncate = true },
					running and group.current and group.current ~= group.title
						and ui.text { group.current, class = "tool-group-current", truncate = true },
				},
				(group.failed or 0) > 0 and ui.text { `{group.failed} failed`, size = "small", tone = "danger" },
				(added > 0 or removed > 0) and counts({ added = added, removed = removed }),
				(group.count or 0) > 1 and ui.text { tostring(group.count), size = "small", tone = "muted" },
			},
			#cards > 0 and ui.column { class = "tool-group-cards", table.unpack(cards) },
			#items > 0 and ui.column { class = "tool-group-body", table.unpack(items) },
		}
	end,
}
