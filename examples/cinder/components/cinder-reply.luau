-- One agent turn as Cinder shows it: prose with bullet lists and fenced
-- code, and tool calls folded into one line each group until clicked.
-- The same look for a CLI session's transcript (from the history provider)
-- and a live agent's reply (from the acp provider's `parts`).

type Tool = { name: string, detail: string, status: string? }
type Part = { kind: string, text: string?, tools: { Tool }? }

-- Inline marks the view can't draw: bold markers go, backticks stay.
local function plain(text: string): string
	return (string.gsub(text, "%*%*", ""))
end

-- Prose and fenced code: paragraphs, bullet lists, code in a mono box.
local function render_text(text: string): { any }
	local out = {}
	local segments = string.split(text, "```")
	for index, segment in segments do
		if index % 2 == 0 then
			-- Code: drop the language on the fence's line.
			local body = string.gsub(segment, "^[%w_+-]*\n", "")
			body = string.gsub(body, "\n$", "")
			table.insert(out, ui.column {
				class = "cinder-reply-code",
				scroll = "x",
				ui.text { body, wrap = false, class = "cinder-reply-code-text" },
			})
		else
			for _, para in string.split(segment, "\n\n") do
				local trimmed = string.match(para, "^%s*(.-)%s*$") or ""
				if trimmed ~= "" then
					local bullets = {}
					local prose = {}
					for _, line in string.split(trimmed, "\n") do
						local item = string.match(line, "^%s*[-*] (.*)$") or string.match(line, "^%s*%d+%. (.*)$")
						if item then
							table.insert(bullets, ui.row {
								gap = "normal",
								ui.text { "•", class = "cinder-reply-bullet" },
								ui.row { grow = true, ui.text { plain(item), class = "cinder-reply-prose" } },
							})
						else
							table.insert(prose, line)
						end
					end
					if #prose > 0 then
						table.insert(out, ui.text { plain(table.concat(prose, "\n")), class = "cinder-reply-prose" })
					end
					if #bullets > 0 then
						table.insert(out, ui.column { gap = "small", table.unpack(bullets) })
					end
				end
			end
		end
	end
	return out
end

local TOOL_STATUS = {
	pending = { icon = "circle-dashed", class = "cinder-reply-faint" },
	in_progress = { icon = "loader-circle", class = "cinder-reply-faint", spin = true },
	failed = { icon = "circle-x", class = "cinder-reply-failed" },
}

-- Tool calls, folded into one line until opened.
local function render_tools(key: string, tools: { Tool }, open: boolean)
	local first = tools[1]
	local summary = if first then `{first.name} {first.detail}` else ""
	local working = false
	for _, tool in tools do
		working = working or tool.status == "in_progress" or tool.status == "pending"
	end
	local head = ui.row {
		class = "cinder-reply-tools-head",
		align = "center",
		msg = { unfold = key },
		ui.icon { name = if open then "chevron-down" else "chevron-right", size = "small" },
		ui.text { if #tools == 1 then "1 tool call" else `{#tools} tool calls`, weight = "medium" },
		working and ui.icon { name = "loader-circle", size = "small", spin = true },
		not open and ui.row { grow = true, ui.text { summary, truncate = true, class = "cinder-reply-tools-summary" } },
	}
	if not open then
		return head
	end
	local rows = {}
	for _, tool in tools do
		local look = tool.status and TOOL_STATUS[tool.status]
		table.insert(rows, ui.row {
			class = "cinder-reply-tool-row",
			align = "center",
			ui.text { tool.name, weight = "medium", class = "cinder-reply-tool-name" },
			ui.row { grow = true, ui.text { tool.detail, truncate = true, class = "cinder-reply-tools-summary", tooltip = tool.detail } },
			look and ui.icon { name = look.icon :: any, size = "small", class = look.class, spin = look.spin },
		})
	end
	return ui.column { gap = "none", head, ui.column { class = "cinder-reply-tool-list", table.unpack(rows) } }
end

return component {
	title = "Cinder Reply",
	category = "Data",
	icon = "message-square",
	description = "An agent's turn: prose, bullet lists and code blocks, with tool calls folded into a line per group.",
	when = "Showing what a coding agent said in a transcript, in Cinder's look. For a run's status, plan and permission question use `agent-run`; for chat bubbles use `chat`.",
	props = {
		parts = "list of `{ kind = \"text\", text }` or `{ kind = \"tools\", tools = { { name, detail, status? } } }`, in order",
		key = "a string naming this turn; a tool group's click sends `{ unfold = key .. \".\" .. index }` to the mod",
		unfolded = "map from those keys to true for the groups shown open",
	},
	styles = {
		[".cinder-reply"] = { gap = 12 },
		[".cinder-reply-prose"] = { color = "foreground" },
		[".cinder-reply-bullet"] = { color = "muted-foreground" },
		[".cinder-reply-code"] = { background = "#111111", border = true, border_color = "border", radius = 10, padding = { 10, 12 } },
		[".cinder-reply-code-text"] = { size = "small", font = "mono", color = "#d8d8d8" },
		[".cinder-reply-tools-head"] = { padding = { 4, 8 }, gap = 8, radius = 8, color = "muted-foreground" },
		[".cinder-reply-tools-head:hover"] = { background = "list-hover", color = "foreground" },
		[".cinder-reply-tools-summary"] = { color = "muted-foreground", font = "mono", size = "small" },
		[".cinder-reply-tool-list"] = { padding = { 4, 0, 4, 22 }, gap = 2, border = "left", border_color = "border", margin = { left = 14 } },
		[".cinder-reply-tool-row"] = { padding = { 3, 8 }, gap = 10 },
		[".cinder-reply-tool-name"] = { color = "foreground", size = "small" },
		[".cinder-reply-faint"] = { color = "muted-foreground" },
		[".cinder-reply-failed"] = { color = "danger" },
	},
	example = {
		key = "example",
		unfolded = {},
		parts = ({
			{ kind = "text", text = "Let me look at the tests first." },
			{ kind = "tools", tools = { { name = "Run", detail = "cargo test", status = "completed" }, { name = "Read", detail = "src/store.rs", status = "completed" } } },
			{ kind = "text", text = "Two tests fail on a stale fixture:\n\n- `store_keeps_order`\n- `store_reloads`" },
		} :: { any }),
	},
	build = function(props)
		local key: string = props.key or "reply"
		local unfolded: { [string]: boolean } = props.unfolded or {}
		local out = {}
		local parts: { any } = props.parts or {}
		for index, part in parts do
			if part.kind == "tools" then
				local k = `{key}.{index}`
				if part.tools and #part.tools > 0 then
					table.insert(out, render_tools(k, part.tools, unfolded[k] == true))
				end
			else
				for _, el in render_text(part.text or "") do
					table.insert(out, el)
				end
			end
		end
		return ui.column { class = "cinder-reply", table.unpack(out) }
	end,
}
