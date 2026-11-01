-- The canvas: a new session (Cinder on the import pill over an empty page,
-- the composer at the bottom), a session's transcript, or the demo agent's
-- conversation.
--
-- Transcripts are your coding-agent CLI sessions, read-only, from the
-- history provider. The demo agent is scripted: it answers from rules in
-- this file, says so, and never calls a model or writes anything.

local sprite = use "cinder-sprite"
local menu = use "menu"

type Tool = { name: string, detail: string }
type Part = { kind: string, text: string, tools: { Tool } }
type Turn = { from: string, parts: { Part } }

type State = {
	mode: string,
	open: string,
	draft: string,
	turns: { Turn },
	unfolded: { [string]: boolean },
	popup: string,
	folder: string,
	effort: string,
	auto: boolean,
}

local EFFORTS: { any } = {
	{ id = "Low", label = "Low", detail = "quick answers" },
	{ id = "Medium", label = "Medium", detail = "the default" },
	{ id = "High", label = "High", detail = "thinks longer" },
}

local function text_part(text: string): Part
	return { kind = "text", text = text, tools = {} }
end

local function tools_part(list: { { string } }): Part
	local tools = {}
	for _, pair in list do
		table.insert(tools, { name = pair[1], detail = pair[2] })
	end
	return { kind = "tools", text = "", tools = tools }
end

-- The demo agent: a scripted answer by what the request mentions.
local function reply(request: string, folder: string): Turn
	local lower = string.lower(request)
	local place = if folder ~= "" then folder else "this computer"
	if string.find(lower, "test") then
		return { from = "assistant", parts = {
			text_part(`I'll run the tests in {place} first.`),
			tools_part({ { "Bash", "Run the test suite" }, { "Read", "tests/store_test.rs" } }),
			text_part("Two tests fail on a stale fixture:\n\n- `store_keeps_order` expects the old sort\n- `store_reloads` reads a file the setup no longer writes\n\nThis is the scripted demo, so nothing ran. A real agent would fix the fixture and run them again."),
		} }
	elseif string.find(lower, "bug") or string.find(lower, "fix") or string.find(lower, "crash") then
		return { from = "assistant", parts = {
			text_part("Let me find where it breaks."),
			tools_part({ { "Grep", "panic|unwrap" }, { "Read", "src/main.rs" }, { "Bash", "Reproduce with the failing input" } }),
			text_part("It unwraps a value that is missing on first launch. The fix is to fall back to the default:\n\n```rust\nlet config = load().unwrap_or_default();\n```\n\n(Scripted demo: no file was read or changed.)"),
		} }
	end
	return { from = "assistant", parts = {
		text_part(`I'll start by looking around {place}.`),
		tools_part({ { "Bash", "List the project files" }, { "Read", "README.md" }, { "Grep", "TODO" } }),
		text_part(`Here is how I would go about "{request}":\n\n- Read how it works today and note what it touches\n- Make the change in small steps, with a test for each\n- Run the checks, then summarise what changed\n\nThis is Cinder's scripted demo agent: it calls no model and changes nothing. Ask about tests or a bug for other answers.`),
	} }
end

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
				class = "em-code",
				scroll = "x",
				ui.text { body, wrap = false, class = "em-code-text" },
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
								ui.text { "•", class = "em-bullet" },
								ui.row { grow = true, ui.text { plain(item), class = "em-prose" } },
							})
						else
							table.insert(prose, line)
						end
					end
					if #prose > 0 then
						table.insert(out, ui.text { plain(table.concat(prose, "\n")), class = "em-prose" })
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

-- Tool calls, folded into one line until opened.
local function render_tools(key: string, tools: { Tool }, open: boolean)
	local first = tools[1]
	local summary = if first then `{first.name} {first.detail}` else ""
	local head = ui.row {
		class = "em-tools-head",
		align = "center",
		msg = { unfold = key },
		ui.icon { name = if open then "chevron-down" else "chevron-right", size = "small" },
		ui.text { if #tools == 1 then "1 tool call" else `{#tools} tool calls`, weight = "medium" },
		not open and ui.row { grow = true, ui.text { summary, truncate = true, class = "em-tools-summary" } },
	}
	if not open then
		return head
	end
	local rows = {}
	for _, tool in tools do
		table.insert(rows, ui.row {
			class = "em-tool-row",
			align = "center",
			ui.text { tool.name, weight = "medium", class = "em-tool-name" },
			ui.row { grow = true, ui.text { tool.detail, truncate = true, class = "em-tools-summary" } },
		})
	end
	return ui.column { gap = "none", head, ui.column { class = "em-tool-list", table.unpack(rows) } }
end

local function render_turn(state: State, key: string, turn: Turn)
	if turn.from == "user" then
		local text = if turn.parts[1] then turn.parts[1].text else ""
		return ui.row {
			justify = "end",
			ui.column { class = "em-bubble", max_width = 560, ui.text { text, class = "em-prose" } },
		}
	end
	local parts = {}
	for index, part in turn.parts do
		if part.kind == "tools" then
			local k = `{key}.{index}`
			table.insert(parts, render_tools(k, part.tools, state.unfolded[k] == true))
		else
			for _, el in render_text(part.text) do
				table.insert(parts, el)
			end
		end
	end
	return ui.column { class = "em-answer", table.unpack(parts) }
end

-- A footer control: an icon or not, a label, a popover when clicked.
local function chip(icon: string?, label: string, msg: any, class: string?)
	return ui.row {
		class = `em-chip {class or ""}`,
		align = "center",
		msg = msg,
		icon and ui.icon { name = icon :: any },
		ui.text { label },
	}
end

return mod {
	title = "Canvas",
	subscribe = { "cinder/nav" },

	styles = {
		[".em-canvas"] = { padding = { 0, 24, 3, 24 } },
		[".em-import"] = {
			width = 320,
			height = 40,
			padding = { 0, 14 },
			gap = 12,
			radius = 12,
			border = true,
			border_color = "#272727",
			background = "background",
		},
		[".em-import:hover"] = { background = "#1f1f1f", border_color = "#333334" },
		[".em-pop-row"] = { padding = { 6, 10 }, radius = 8, gap = 10 },
		[".em-pop-row:hover"] = { background = "list-hover" },
		[".em-pop-head"] = { padding = { 6, 10, 4, 10 }, color = "muted-foreground", size = "small" },
		[".em-pop-detail"] = { color = "muted-foreground", size = "small" },
		[".em-composer"] = {
			height = 44,
			padding = { 0, 12, 0, 10 },
			gap = 12,
			radius = 14,
			background = "em-field",
			border = true,
			border_color = "em-field-line",
		},
		[".em-composer-icon"] = { color = "#d4d4d5", radius = 6, padding = 3 },
		[".em-composer-icon:hover"] = { background = "list-hover" },
		[".em-foot"] = { height = 34, padding = { 0, 14 }, gap = 6 },
		[".em-chip"] = { padding = { 3, 6 }, radius = 6, gap = 6, color = "foreground" },
		[".em-chip:hover"] = { background = "list-hover" },
		[".em-chip-muted"] = { color = "muted-foreground" },
		[".em-ring"] = { color = "accent", padding = { 3, 4 } },
		[".em-thread"] = { padding = { 24, 0, 24, 0 }, gap = 20 },
		[".em-banner"] = { padding = { 8, 12 }, radius = 10, gap = 10, background = "#ffffff08", color = "muted-foreground", size = "small" },
		[".em-bubble"] = { background = "card", radius = 14, padding = { 9, 14 } },
		[".em-answer"] = { gap = 12 },
		[".em-prose"] = { color = "foreground" },
		[".em-bullet"] = { color = "muted-foreground" },
		[".em-code"] = { background = "#111111", border = true, border_color = "em-line", radius = 10, padding = { 10, 12 } },
		[".em-code-text"] = { size = "small", font = "mono", color = "#d8d8d8" },
		[".em-tools-head"] = { padding = { 4, 8 }, gap = 8, radius = 8, color = "muted-foreground" },
		[".em-tools-head:hover"] = { background = "list-hover", color = "foreground" },
		[".em-tools-summary"] = { color = "em-faint", font = "mono", size = "small" },
		[".em-tool-list"] = { padding = { 4, 0, 4, 22 }, gap = 2, border = "left", border_color = "em-line", margin = { left = 14 } },
		[".em-tool-row"] = { padding = { 3, 8 }, gap = 10 },
		[".em-tool-name"] = { color = "#d4d4d5", size = "small" },
		[".em-hidden"] = { color = "em-faint", size = "small" },
	},

	init = function(previous: State?): State
		return {
			mode = "code",
			open = "",
			draft = "",
			-- The demo conversation starts over each launch.
			turns = {},
			unfolded = {},
			popup = "",
			folder = (previous and previous.folder) or "",
			effort = (previous and previous.effort) or "Medium",
			auto = if previous then previous.auto ~= false else true,
		}
	end,

	update = function(state: State, msg)
		if msg == "close-popup" then
			state.popup = ""
		elseif msg == "auto" then
			state.auto = not state.auto
		elseif msg == "send" then
			msg = { msg = "ask", value = state.draft }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.topic == "cinder/nav" and msg.value then
			state.mode = msg.value.mode or "code"
			state.open = msg.value.open or ""
			state.popup = ""
		elseif msg.popup then
			state.popup = if state.popup == msg.popup then "" else msg.popup
		elseif msg.unfold then
			state.unfolded[msg.unfold] = not state.unfolded[msg.unfold]
		elseif msg.folder then
			state.folder = msg.folder
			state.popup = ""
		elseif msg.pick then
			state.effort = msg.pick
			state.popup = ""
		elseif msg.msg == "typing" then
			state.draft = msg.value or ""
		elseif msg.msg == "ask" and string.match(msg.value or "", "%S") then
			table.insert(state.turns, { from = "user", parts = { text_part(msg.value) } })
			table.insert(state.turns, reply(msg.value, state.folder))
			state.draft = ""
		elseif msg.msg == "clear" then
			state.turns = {}
			state.draft = ""
		elseif msg.open then
			state.popup = ""
			return state, { { to = "shell", msg = { open = msg.open } } }
		end
	end,

	view = function(state: State, app)
		local history = app.history
		local reading = state.open ~= ""
		local chat = state.mode == "chat"

		-- What fills the page.
		local page
		if reading then
			local t = history and history.transcript
			if not t or t.id ~= state.open then
				page = ui.column {
					grow = true,
					align = "center",
					justify = "center",
					ui.row { gap = "normal", align = "center", ui.icon { name = "loader-circle", spin = true, tone = "muted" }, ui.text { "Reading the session…", tone = "muted" } },
				}
			else
				local items = {}
				table.insert(items, ui.row {
					class = "em-banner",
					align = "center",
					ui.icon { name = "history", size = "small" },
					ui.text { `A CLI session in {t.project}{if t.branch ~= "" then " · " .. t.branch else ""}, read-only`, size = "small" },
				})
				if t.hidden > 0 then
					table.insert(items, ui.text { `{t.hidden} earlier turns are not shown.`, class = "em-hidden" })
				end
				for index, turn in t.turns do
					table.insert(items, render_turn(state, `{t.id}.{index}`, turn))
				end
				page = ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.row {
						justify = "center",
						shrink = false,
						ui.column { class = "em-thread", grow = true, max_width = 760, table.unpack(items) },
					},
				}
			end
		elseif #state.turns > 0 and not chat then
			local items = {}
			table.insert(items, ui.row {
				class = "em-banner",
				align = "center",
				ui.icon { name = "sparkles", size = "small" },
				ui.text { "Demo agent: scripted replies. No model is called and nothing is written.", size = "small" },
				ui.spacer(),
				ui.row { class = "em-chip", msg = { msg = "clear" }, ui.text { "Clear", size = "small" } },
			})
			for index, turn in state.turns do
				table.insert(items, render_turn(state, `demo.{index}`, turn))
			end
			page = ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.row {
					justify = "center",
					shrink = false,
					ui.column { class = "em-thread", grow = true, max_width = 760, table.unpack(items) },
				},
			}
		else
			-- The new session: Cinder sitting on the import pill.
			local recent = {}
			local count = 0
			if history then
				count = history.total or 0
				table.insert(recent, ui.text { "Recent CLI sessions", class = "em-pop-head" })
				local shown = 0
				for _, group in history.groups or {} do
					for _, session in group.sessions do
						if shown < 10 then
							shown += 1
							table.insert(recent, ui.row {
								class = "em-pop-row",
								align = "center",
								msg = { open = session.id },
								ui.icon { name = "terminal", size = "small", tone = "muted" },
								ui.row { grow = true, ui.text { session.title, truncate = true } },
								ui.text { `{group.name} · {session.age}`, class = "em-pop-detail" },
							})
						end
					end
				end
			end
			local label = if chat
				then "Start a chat below"
				else `Import {count} CLI session{if count == 1 then "" else "s"}`
			page = ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.popover {
					open = state.popup == "import" and not chat,
					dismiss = "close-popup",
					side = "bottom",
					width = 460,
					content = ui.column { gap = "none", table.unpack(recent) },
					ui.row {
						class = "em-import",
						align = "center",
						msg = if chat then nil else { popup = "import" },
						ui.icon { name = if chat then "message-square" else "terminal", size = "small" },
						ui.text { label },
						ui.row {
							float = { top = -30, right = 10 },
							sprite { tooltip = "Cinder" },
						},
					},
				},
			}
		end

		-- The folders to start in: the repositories sessions ran in.
		local folders: { any } = { { heading = "Start in" } }
		local seen = {}
		if history then
			for _, group in history.groups or {} do
				if group.path ~= "" and not seen[group.path] and #folders < 12 then
					seen[group.path] = true
					table.insert(folders, { id = group.path, label = group.name, icon = "folder", detail = group.path, msg = { folder = group.path } })
				end
			end
		end
		local folder_name = "Choose…"
		if state.folder ~= "" then
			folder_name = string.match(state.folder, "([^/]+)/?$") or state.folder
		end

		local placeholder = if reading
			then "This session is read-only. Start a new one to try the demo agent."
			elseif chat then "How can I help you today?"
			else "Describe a task or ask a question"

		return ui.column {
			class = "em-canvas",
			grow = true,
			gap = "none",
			page,
			ui.row {
				justify = "center",
				shrink = false,
				ui.column {
					grow = true,
					max_width = 768,
					gap = "none",
					ui.row {
						class = "em-composer",
						align = "center",
						ui.row { class = "em-composer-icon", tooltip = "Attach (not in this example)", ui.icon "plus" },
						ui.input {
							id = "prompt",
							class = "em-input",
							grow = true,
							value = state.draft,
							placeholder = placeholder,
							disabled = reading,
							change = "typing",
							submit = "ask",
						},
						ui.row { class = "em-composer-icon", tooltip = "Voice (not in this example)", ui.icon "mic" },
					},
					ui.row {
						class = "em-foot",
						align = "center",
						chip("laptop", "This computer", nil),
						ui.popover {
							open = state.popup == "folder",
							dismiss = "close-popup",
							side = "top",
							width = 360,
							content = menu { items = folders },
							chip("folder", folder_name, { popup = "folder" }),
						},
						ui.spacer(),
						ui.row {
							align = "center",
							gap = "none",
							chip(nil, "Scripted 1.0", nil),
							ui.popover {
								open = state.popup == "effort",
								dismiss = "close-popup",
								side = "top",
								align = "end",
								width = 220,
								content = menu { items = EFFORTS },
								chip(nil, state.effort, { popup = "effort" }, "em-chip-muted"),
							},
						},
						ui.row { width = 4 },
						chip(nil, if state.auto then "Auto" else "Ask", "auto"),
						ui.row { class = "em-ring", tooltip = "Context: 18% used", ui.icon { name = "loader-circle" } },
					},
				},
			},
		}
	end,
}
