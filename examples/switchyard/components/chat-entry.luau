-- One entry of an agent chat's transcript, as the `chat` provider shares
-- it (herdr's shape): what the user said, the agent's answer in markdown,
-- its reasoning in a fold, a tool call as one quiet line that opens, how a
-- turn ended, a notice, or a picture's placeholder.

local chat_markdown = use "chat-markdown"
local thinking = use "thinking"

local TOOL_ICONS = {
	read = "file-text", edit = "square-pen", delete = "trash-2", move = "files",
	search = "search", execute = "terminal", think = "sparkles", fetch = "globe",
	switch_mode = "zap", subagent = "bot", other = "code",
}

local END_WORDS = {
	done = "Done", cancelled = "Stopped", limit = "Hit a limit", refused = "Refused",
	failed = "Failed", other = "Ended",
}

-- "950ms", "12s", "2m 25s".
local function duration(ms: number): string
	if ms < 1000 then
		return `{math.floor(ms)}ms`
	elseif ms < 60000 then
		return `{math.floor(ms / 1000)}s`
	end
	return `{math.floor(ms / 60000)}m {math.floor(ms % 60000 / 1000)}s`
end

-- "980", "12.3k", "1.2M".
local function count(n: number): string
	if n >= 1000000 then
		return string.format("%.1fM", n / 1000000)
	elseif n >= 1000 then
		return string.format("%.1fk", n / 1000)
	end
	return tostring(n)
end

local function tool_status(status: string?): any
	if status == "in_progress" or status == "pending" then
		return ui.icon { name = "loader", size = "small", spin = true, class = "entry-muted" }
	elseif status == "failed" then
		return ui.icon { name = "close", size = "small", tone = "danger" }
	end
	return ui.icon { name = "check", size = "small", class = "entry-muted" }
end

local function tool_content(tool: any): { any }
	local parts = {}
	for _, location in tool.locations or {} do
		table.insert(parts, ui.text { location, class = "entry-path", truncate = true })
	end
	for _, part in tool.content or {} do
		if part.type == "text" and part.text ~= "" then
			table.insert(parts, ui.text { part.text, class = "entry-tool-text", wrap = true })
		elseif part.type == "diff" then
			table.insert(parts, ui.row { gap = "small", align = "center",
				ui.icon { name = "square-pen", size = "small", class = "entry-muted" },
				ui.text { part.path, class = "entry-path", truncate = true },
				ui.text { `{part.old_lines or 0} → {part.new_lines or 0} lines`, class = "entry-muted", size = "small" },
			})
		elseif part.type == "image" then
			table.insert(parts, ui.text { "[image]", class = "entry-muted" })
		elseif part.type == "terminal" then
			table.insert(parts, ui.text { `terminal {part.id}`, class = "entry-muted" })
		end
	end
	return parts
end

return component {
	title = "Chat Entry",
	category = "Data",
	icon = "message-square",
	description = "One entry of an agent chat's transcript: the user's words, the agent's markdown answer, its reasoning, a tool call, a turn's end, a notice.",
	when = "Each entry of a `chat` provider transcript, stacked in the chat's thread. For a conversation you write yourself use `chat-message`.",
	props = {
		entry = "the entry: { id, kind = { type = \"user\" | \"agent\" | \"thought\" | \"tool\" | \"turn\" | \"notice\" | \"image\", ... } }",
		open = "show a thought's or a tool's details",
		toggle = "message a click on a thought or tool sends",
		streaming = "the answer is still being written (a dot after it)",
	},
	styles = {
		[".entry-user"] = { background = "secondary", radius = 14, padding = { 10, 14 } },
		[".entry-agent"] = { padding = { 2, 2 } },
		[".entry-tool"] = { padding = { 3, 6 }, radius = 8, gap = 8 },
		[".entry-tool:hover"] = { background = "list-hover" },
		[".entry-tool-title"] = { color = "muted-foreground" },
		[".entry-tool-body"] = { padding = { 2, 8, 6, 30 }, gap = 4 },
		[".entry-tool-text"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".entry-path"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".entry-muted"] = { color = "muted-foreground" },
		[".entry-turn"] = { color = "muted-foreground", size = "small", padding = { 2, 2, 6, 2 } },
		[".entry-notice"] = { padding = { 6, 10 }, radius = 8, background = "secondary", gap = 8 },
	},
	example = {
		entry = { id = 1, kind = { type = "tool", kind = "read", title = "Read crates/moonframe/src/lib.rs", status = "completed", locations = {}, content = {} } },
	},

	build = function(props)
		local kind: any = props.entry.kind or {}
		local kind_type = kind.type
		if kind_type == "user" then
			return ui.row { justify = "end",
				ui.column { class = "entry-user", hug = true, max_width = 620, ui.text { kind.text or "", wrap = true } },
			}
		elseif kind_type == "agent" then
			return ui.column { class = "entry-agent", chat_markdown { text = kind.text or "", cursor = props.streaming } }
		elseif kind_type == "thought" then
			return thinking { label = if props.streaming then "Thinking…" else "Thought", open = props.open, msg = props.toggle, text = kind.text or "" }
		elseif kind_type == "tool" then
			local title = kind.title
			if not title or title == "" then
				title = kind.kind or "tool"
			end
			return ui.column { gap = "none",
				ui.row { class = "entry-tool", align = "center", msg = props.toggle,
					ui.icon { name = (TOOL_ICONS[kind.kind or "other"] or "code") :: any, size = "small", class = "entry-muted" },
					ui.text { title, class = "entry-tool-title", truncate = true },
					ui.spacer(),
					tool_status(kind.status),
				},
				props.open and ui.column { class = "entry-tool-body", table.unpack(tool_content(kind)) },
			}
		elseif kind_type == "turn" then
			local words = { END_WORDS[kind["end"] or "other"] or "Ended", duration(kind.duration_ms or 0) }
			if kind.tokens and kind.tokens.total then
				table.insert(words, `{count(kind.tokens.total)} tokens`)
			end
			return ui.text { table.concat(words, " · "), class = "entry-turn" }
		elseif kind_type == "notice" then
			local tone = if kind.level == "error" then "danger" elseif kind.level == "warning" then "warning" else "muted"
			return ui.row { class = "entry-notice", align = "center",
				ui.icon { name = if kind.level == "info" then "circle-alert" else "circle-alert", size = "small", tone = tone :: any },
				ui.text { kind.text or "", tone = tone :: any, wrap = true },
			}
		elseif kind_type == "image" then
			return ui.text { "[image]", class = "entry-muted" }
		end
		return ui.text { `[{kind_type or "entry"}]`, class = "entry-muted" }
	end,
}
