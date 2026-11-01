-- The reader: the thread's subject, its labels and size, then its messages.
-- Email threads read like Gmail's: earlier messages fold to one line (a
-- click opens one), the newest is open, as a card with the sender, the
-- recipients, the body and the attachments. Chats are bubbles: the others'
-- on the left under their names, yours on the right in the accent.

local avatar = use "vault-avatar"

type State = { thread: number, open: { [string]: boolean? }, all: boolean }

local function fresh(): State
	return { thread = 0, open = {}, all = false }
end

return mod {
	region = "main",
	title = "Reader",
	subscribe = { "provider/vault" },

	styles = {
		[".rd-scroll"] = { padding = { 18, 32, 48, 28 } },
		[".rd-body"] = { gap = 14 },
		[".rd-head"] = { gap = 6 },
		[".rd-subject"] = { size = "title", weight = "bold" },
		[".rd-meta"] = { size = "small", color = "muted-foreground" },
		[".rd-tool"] = { radius = 6, color = "foreground" },
		[".rd-tool:hover"] = { background = "list-hover" },
		[".rd-fold"] = { border = true, border_color = "border", radius = 10, padding = { 0, 16 }, gap = 12 },
		[".rd-fold:hover"] = { background = "list-hover" },
		[".rd-card"] = { border = true, border_color = "border", radius = 10, background = "background", shadow = "0 1px 2px #0000000d" },
		[".rd-card-head"] = { padding = 16, gap = 12 },
		[".rd-card-head:hover"] = { background = "list-hover" },
		[".rd-name"] = { size = "large", weight = "bold" },
		[".rd-muted"] = { color = "muted-foreground" },
		[".rd-small"] = { size = "small", color = "muted-foreground" },
		[".rd-text"] = { padding = { 0, 16, 20, 68 } },
		[".rd-files"] = { padding = { 0, 16, 16, 68 }, gap = 8 },
		[".rd-file"] = { border = true, border_color = "border", radius = 8, background = "muted/50", padding = { 6, 4, 6, 12 }, gap = 10 },
		[".rd-file:hover"] = { background = "muted" },
		[".rd-file-tile"] = { background = "background", radius = 6, color = "accent" },
		[".rd-star"] = { color = "warning" },
		[".rd-sender"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".rd-group"] = { padding = { 12, 0, 2, 0 } },
		[".rd-bubble"] = { radius = 16, padding = { 6, 12 }, background = "muted" },
		[".rd-bubble-mine"] = { background = "accent", color = "accent-foreground" },
		[".rd-empty-tile"] = { background = "muted", radius = 999, color = "muted-foreground" },
	},

	init = function(previous: State?): State
		return previous or fresh()
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" and msg ~= "expand-all" then
			return
		end
		if msg == "expand-all" then
			state.all = not state.all
			state.open = {}
		elseif msg.topic == "provider/vault" then
			local thread = msg.value and msg.value.thread
			local id = thread and thread.id or 0
			if id ~= state.thread then
				state.thread = id
				state.open = {}
				state.all = false
			end
		elseif msg.toggle then
			local key = tostring(msg.toggle)
			state.open[key] = not state.open[key] or nil
		end
	end,

	view = function(state: State, app)
		local vault = app.vault
		local thread = vault and vault.available and vault.thread
		if not thread then
			return ui.column {
				grow = true,
				align = "center",
				justify = "center",
				gap = "normal",
				ui.row {
					class = "rd-empty-tile",
					width = 56,
					height = 56,
					align = "center",
					justify = "center",
					ui.icon { name = "mail", size = "large" },
				},
				ui.text { "No message selected", weight = "bold" },
				ui.text { "Pick a conversation from the list, or search with / and ⌘F.", tone = "muted", size = "small" },
			}
		end

		local messages = thread.messages
		local items: { any } = {}
		if thread.chat then
			local previous: string? = nil
			for _, m in messages do
				local who = if m.mine then "\0you" else m.from
				local first = who ~= previous
				previous = who
				if first then
					table.insert(items, ui.row {
						class = "rd-group",
						justify = if m.mine then "end" else "start",
						ui.row { width = if m.mine then 0 else 28, shrink = false },
						ui.text { if m.mine then "You" else m.from, class = "rd-sender" },
						ui.row { width = if m.mine then 52 else 0, shrink = false },
					})
				end
				local bubble = ui.column {
					class = if m.mine then { "rd-bubble", "rd-bubble-mine" } else "rd-bubble",
					max_width = 520,
					ui.text { m.body },
				}
				local time = ui.text { m.time, class = "rd-small" }
				if m.mine then
					table.insert(items, ui.row { align = "end", justify = "end", gap = "normal", bubble, time })
				else
					table.insert(items, ui.row {
						align = "end",
						gap = "normal",
						if first then avatar { initials = m.initials, hue = m.hue, size = 28 } else ui.row { width = 28, shrink = false },
						bubble,
						time,
					})
				end
			end
		else
			local many = #messages > 1
			for index, m in messages do
				local open = not many or index == #messages or state.all or state.open[tostring(m.id)] == true
				if not open then
					table.insert(items, ui.row {
						class = "rd-fold",
						height = 50,
						align = "center",
						msg = { toggle = m.id },
						avatar { initials = m.initials, hue = m.hue, size = 28 },
						ui.text { m.from, class = "rd-name" },
						ui.row { grow = true, ui.text { m.snippet, class = "rd-muted", truncate = true } },
						m.attachments and ui.icon { name = "paperclip", size = "small", class = "rd-muted" },
						ui.text { m.date, class = "rd-small" },
					})
				else
					local recipients = {}
					for _, part in { { "to", m.to }, { "cc", m.cc } } do
						if part[2] ~= "" then
							table.insert(recipients, ui.row {
								gap = "small",
								ui.text { part[1], class = "rd-small", weight = "medium" },
								ui.text { part[2], class = "rd-small", truncate = true },
							})
						end
					end
					local files = {}
					for _, file in m.files do
						table.insert(files, ui.row {
							class = "rd-file",
							align = "center",
							width = 288,
							tooltip = "msgvault-gpui opens this from the archive; this example shows it only",
							ui.row {
								class = "rd-file-tile",
								width = 30,
								height = 30,
								align = "center",
								justify = "center",
								shrink = false,
								ui.icon { name = file.icon :: any },
							},
							ui.column {
								grow = true,
								gap = "none",
								ui.text { file.name, weight = "medium", truncate = true },
								ui.text { file.size, class = "rd-small" },
							},
							ui.row {
								class = "rd-tool",
								width = 22,
								height = 22,
								align = "center",
								justify = "center",
								ui.icon { name = "download", size = "small" },
							},
						})
					end
					table.insert(items, ui.column {
						class = "rd-card",
						gap = "none",
						ui.row {
							class = "rd-card-head",
							align = "start",
							msg = if many then { toggle = m.id } else nil,
							avatar { initials = m.initials, hue = m.hue, size = 40 },
							ui.column {
								grow = true,
								gap = "small",
								ui.row {
									align = "end",
									gap = "normal",
									ui.text { m.from, class = "rd-name" },
									m.address ~= "" and ui.text { m.address, class = "rd-muted", truncate = true },
								},
								table.unpack(recipients),
							},
							ui.column {
								align = "end",
								gap = "small",
								shrink = false,
								ui.text { m.date, class = "rd-small" },
								m.starred and ui.icon { name = "star", size = "small", class = "rd-star" },
							},
						},
						ui.column {
							class = "rd-text",
							ui.text { if m.body ~= "" then m.body else "This message has no archived body." },
						},
						#files > 0 and ui.row { class = "rd-files", wrap = true, table.unpack(files) },
					})
				end
			end
		end

		local chips = {}
		for _, label in thread.labels do
			table.insert(chips, ui.row {
				class = "chip-round",
				align = "center",
				hug = true,
				style = { background = label.color .. "24" },
				ui.row { width = 6, height = 6, style = { background = label.color, radius = 3 } },
				ui.text { label.title, size = "small", style = { color = label.color } },
			})
		end

		local many_emails = not thread.chat and #messages > 1
		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.row {
				class = "rd-scroll",
				justify = "center",
				shrink = false,
				ui.column {
					class = "rd-body",
					grow = true,
					max_width = 880,
					ui.column {
						class = "rd-head",
						ui.row {
							align = "start",
							gap = "large",
							ui.row { grow = true, ui.text { thread.subject, class = "rd-subject" } },
							many_emails and ui.row {
								class = "rd-tool",
								width = 26,
								height = 26,
								align = "center",
								justify = "center",
								msg = "expand-all",
								tooltip = "Expand or collapse all (⌘;)",
								ui.icon { name = "mails", size = "small" },
							},
						},
						ui.row {
							align = "center",
							gap = "normal",
							wrap = true,
							#chips > 0 and ui.row { gap = "normal", table.unpack(chips) },
							ui.text { thread.meta, class = "rd-meta" },
						},
					},
					ui.column { gap = if thread.chat then "small" else "normal", table.unpack(items) },
				},
			},
		}
	end,
}
