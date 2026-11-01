-- The conversation: the user's messages as bubbles on the right with copy,
-- share and edit under them; the assistant's answers as rich markdown with
-- an action row; and at the foot the note and the composer (+, the field,
-- the mode, the mic, and send, which turns into stop while an answer is
-- being written). The answers come from a scripted demo agent in
-- `providers/chats`, which streams them word by word.

local markdown = use "markdown"
local dropdown = use "dropdown"

type State = { draft: string, mode: string, mode_open: boolean, copied: number? }

local MODES = {
	{ id = "instant", label = "Instant", detail = "Answers right away" },
	{ id = "thinking", label = "Thinking", detail = "Takes a moment first" },
	{ id = "research", label = "Research", detail = "Reads around the topic" },
}

local function action(icon: string, tooltip: string, extra: any)
	return ui.row {
		class = "pl-chat-action",
		align = "center",
		justify = "center",
		width = 28,
		height = 28,
		tooltip = tooltip,
		copy = extra and extra.copy,
		msg = extra and extra.msg,
		to = extra and extra.to,
		ui.icon { name = icon :: any },
	}
end

return mod {
	title = "Chat",

	styles = {
		[".pl-chat"] = { background = "background", radius = { 0, 10, 10, 0 } },
		[".pl-chat-scroll"] = { padding = { 32, 28, 24, 28 } },
		[".pl-chat-column"] = { gap = 0 },
		[".pl-chat-user"] = { padding = { 0, 32, 0, 0 }, gap = 6 },
		[".pl-chat-bubble"] = { background = "bubble", radius = 20, padding = { 11, 16 } },
		[".pl-chat-user-actions"] = { gap = 2, padding = { 0, 0, 30, 0 } },
		[".pl-chat-answer"] = { gap = 12, padding = { 0, 0, 36, 0 } },
		[".pl-chat-action"] = { radius = 7, color = "#b4b4b4" },
		[".pl-chat-action:hover"] = { background = "list-hover", color = "foreground" },
		[".pl-chat-actions"] = { gap = 2 },
		[".pl-chat-empty"] = { size = "title", weight = "medium" },
		[".pl-chat-foot"] = { padding = { 6, 28, 18, 28 }, gap = 10 },
		[".pl-chat-note"] = { size = "small", color = "#b8b8b8" },
		[".pl-composer"] = { background = "field", radius = 22, padding = { 0, 8, 0, 14 }, gap = 8 },
		[".pl-composer-icon"] = { radius = 15, color = "#e6e6e6" },
		[".pl-composer-icon:hover"] = { background = "#ffffff14" },
		[".pl-send"] = { background = "#a8a8a8", radius = 15, color = "#2a2a2a" },
		[".pl-send:hover"] = { background = "#c4c4c4" },
		[".pl-send-ready"] = { background = "#f2f2f2", color = "#1f1f1f" },
		[".pl-stop"] = { background = "#f2f2f2", radius = 15, color = "#1f1f1f" },
		[".pl-stop-square"] = { background = "#1f1f1f", radius = 2 },
		[".dropdown-trigger"] = { background = "#00000000", border = "none", padding = { 4, 8 }, color = "#d6d6d6", gap = 8 },
		[".dropdown-trigger:hover"] = { background = "#ffffff12" },
		[".dropdown-trigger:selected"] = { background = "#ffffff12" },
		[".dropdown-chevron"] = { color = "#b4b4b4" },
		[".pl-thinking"] = { color = "muted-foreground", gap = 8 },
	},

	init = function(previous: State?): State
		return {
			draft = "",
			mode = if previous and previous.mode then previous.mode else "instant",
			mode_open = false,
		}
	end,

	update = function(state: State, msg)
		if msg == "toggle-mode" then
			state.mode_open = not state.mode_open
			return
		elseif msg == "close-mode" then
			state.mode_open = false
			return
		elseif msg == "stop" then
			return state, { { to = "provider:chats", msg = { stop = true } } }
		elseif msg == "send" then
			msg = { msg = "submit", value = state.draft }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.pick then
			state.mode = msg.pick
			state.mode_open = false
		elseif msg.msg == "typing" then
			state.draft = msg.value or ""
		elseif msg.msg == "submit" and string.match(msg.value or "", "%S") then
			local text = msg.value
			state.draft = ""
			return state, { { to = "provider:chats", msg = { send = text } } }
		elseif msg.forward then
			-- From the menus and the title bar: pass on to the provider.
			return state, { { to = "provider:chats", msg = msg.forward } }
		end
	end,

	view = function(state: State, app)
		local chats = app.chats
		local chat = chats and chats.chat
		local streaming = chats ~= nil and chats.streaming == true
		local messages = if chat then chat.messages else {}
		local turns = {}
		for index, message in messages do
			local last = index == #messages
			if message.role == "user" then
				table.insert(turns, ui.column {
					class = "pl-chat-user",
					align = "end",
					ui.row {
						justify = "end",
						ui.column {
							class = "pl-chat-bubble",
							max_width = 514,
							ui.text { message.text, wrap = true },
						},
					},
					ui.row {
						class = "pl-chat-user-actions",
						action("copy", "Copy", { copy = message.text }),
						action("upload", "Share", nil),
						action("pencil", "Edit message", nil),
					},
				})
			else
				local writing = streaming and last
				local body = if writing
					then markdown { text = message.text, cursor = true }
					else ui.memo(`answer-{index}`, { message.text }, function()
						return markdown { text = message.text }
					end)
				table.insert(turns, ui.column {
					class = "pl-chat-answer",
					body,
					not writing and ui.row {
						class = "pl-chat-actions",
						action("copy", "Copy", { copy = message.text }),
						action("upload", "Share", nil),
						action("circle-play", "Read aloud", nil),
						last and action("refresh-cw", "Regenerate", { msg = { regenerate = true }, to = "provider:chats" })
							or action("refresh-cw", "Regenerate", nil),
						action("thumbs-up", "Good answer", nil),
						action("ellipsis", "More", nil),
					},
				})
			end
		end

		local content
		if chats == nil then
			content = ui.column { grow = true }
		elseif #messages == 0 then
			content = ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.text { "What's on your mind today?", class = "pl-chat-empty" },
			}
		else
			content = ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.row {
					class = "pl-chat-scroll",
					shrink = false,
					justify = "center",
					ui.column {
						class = "pl-chat-column",
						grow = true,
						max_width = 770,
						table.unpack(turns),
					},
				},
			}
		end

		local ready = string.match(state.draft, "%S") ~= nil
		local send = if streaming
			then ui.row {
				class = "pl-stop",
				width = 30,
				height = 30,
				align = "center",
				justify = "center",
				shrink = false,
				tooltip = "Stop",
				msg = "stop",
				ui.row { class = "pl-stop-square", width = 10, height = 10 },
			}
			else ui.row {
				class = if ready then "pl-send pl-send-ready" else "pl-send",
				width = 30,
				height = 30,
				align = "center",
				justify = "center",
				shrink = false,
				tooltip = "Send",
				msg = "send",
				ui.icon "arrow-up",
			}

		return ui.column {
			class = "pl-chat",
			grow = true,
			gap = "none",
			content,
			ui.column {
				class = "pl-chat-foot",
				shrink = false,
				align = "center",
				ui.text { "Parlance is a scripted demo and can make mistakes. Check important info.", class = "pl-chat-note" },
				ui.row {
					self_align = "stretch",
					justify = "center",
					ui.row {
						class = "pl-composer",
						grow = true,
						max_width = 736,
						height = 44,
						align = "center",
						ui.row {
							class = "pl-composer-icon",
							width = 30,
							height = 30,
							align = "center",
							justify = "center",
							shrink = false,
							tooltip = "Add photos and files",
							ui.icon { name = "plus", size = "large" },
						},
						ui.input {
							id = "composer",
							class = "pl-input",
							value = state.draft,
							placeholder = "Ask Parlance",
							change = "typing",
							submit = "submit",
							keys = { escape = "stop" },
							grow = true,
							focus = true,
						},
						dropdown {
							value = nil,
							selected = state.mode,
							open = state.mode_open,
							toggle = "toggle-mode",
							dismiss = "close-mode",
							options = MODES,
							side = "top",
							align = "end",
							width = 240,
						},
						ui.row {
							class = "pl-composer-icon",
							width = 30,
							height = 30,
							align = "center",
							justify = "center",
							shrink = false,
							tooltip = "Dictate",
							ui.icon "mic",
						},
						send,
					},
				},
			},
		}
	end,
}
