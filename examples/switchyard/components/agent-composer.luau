-- The composer under an agent chat: the message field, with completions
-- for `/` commands and, in shell mode (`!`), for the workspace's paths;
-- outputs added to the next message as chips; prompts waiting behind the
-- running turn; and a row of the chat's settings (model, thinking, mode,
-- auto-accept) beside a send button that stops a running turn.
--
-- It keeps nothing: the mod holds the text and what is open, and gets
-- `{ msg = "draft", value }` as the text changes, `{ msg = "send", value }`
-- on Enter, `{ msg = "newline", value }` on Shift-Enter, "escape", `{ highlight = n }`,
-- `{ complete = text }`, `{ menu = key }`, "close-menu",
-- `{ auto_accept = on }`, "stop", and each
-- setting's pick as `{ ["pick_" .. key] = id }`.

local dropdown = use "dropdown"

return component {
	title = "Agent Composer",
	category = "Forms",
	icon = "send",
	description = "The message box under an agent chat: completions, a shell mode for `!` commands, added outputs, queued prompts, and the chat's settings beside send or stop.",
	when = "The bottom of a chat with a coding agent that has settings (model, mode) and runs shell commands. For a plain chat use `composer`.",
	props = {
		draft = "the text in the field",
		placeholder = "the field's text while it is empty",
		shell = "the text is a shell command (it starts with `!`)",
		completions = "list of { label, detail?, icon?, insert } shown over the field; Tab or a click takes one",
		highlighted = "index of the highlighted completion",
		working = "a turn runs: the send button stops it",
		settings = "list of { key, value, icon?, options, selected, open } dropdowns, `options` as `menu` items",
		auto_accept = "true or false: the auto-accept chip, on or off; nil hides it",
		usage = "short text on the right, such as the context used and the cost",
		context = "list of { id, label }: outputs going with the next message; a chip's ✕ sends { uncontext = id }",
		queued = "list of { id, text }: prompts waiting; ✕ sends { unqueue = id }",
		escape = "message Esc sends (default \"escape\")",
	},
	styles = {
		[".composer-box"] = { background = "card", radius = 16, border = true, border_color = "border", padding = { 12, 14, 8, 14 }, gap = 6, shadow = "0 1px 2px #0000000a" },
		[".composer-box:shell"] = { border_color = "accent/70", shadow = "0 0 0 3px accent/12" },
		[".composer-hint"] = { padding = { 0, 6, 4, 6 }, gap = 6 },
		[".composer-hint-title"] = { weight = "medium" },
		[".composer-hint-text"] = { color = "muted-foreground" },
		[".composer-field"] = { background = "card", border = "none", padding = { 2, 2 } },
		[".composer-chip"] = { background = "secondary", radius = 8, padding = { 3, 8 }, gap = 6 },
		[".composer-chip-x"] = { color = "muted-foreground", radius = 4 },
		[".composer-chip-x:hover"] = { color = "foreground", background = "list-hover" },
		[".composer-queued"] = { color = "muted-foreground", size = "small" },
		[".composer-setting"] = { color = "muted-foreground" },
		[".composer-auto"] = { padding = { 3, 8 }, radius = 8, gap = 5, color = "muted-foreground" },
		[".composer-auto:hover"] = { background = "list-hover" },
		[".composer-auto:on"] = { color = "accent", background = "accent/10" },
		[".composer-usage"] = { color = "muted-foreground", size = "small" },
		[".composer-send"] = { radius = 14, width = 28, height = 28, background = "secondary", color = "muted-foreground" },
		[".composer-send:ready"] = { background = "accent", color = "accent-foreground" },
		[".composer-send:working"] = { background = "foreground", color = "card" },
		[".completion"] = { padding = { 6, 10 }, radius = 8, gap = 10 },
		[".completion:hover"] = { background = "list-hover" },
		[".completion:selected"] = { background = "accent/12" },
		[".completion-detail"] = { color = "muted-foreground", size = "small" },
		[".completions"] = { padding = 4, gap = 0 },
	},
	example = {
		draft = "!git log --oneline",
		shell = true,
		settings = {
			{ key = "model", value = "Opus 5.5", icon = "agent-claude", options = { { id = "opus", label = "Opus 5.5" } }, selected = "opus" },
			{ key = "mode", value = "Default", icon = "zap", options = { { id = "default", label = "Default" } }, selected = "default" },
		},
		auto_accept = false,
		usage = "29.8k / 1M",
	},

	build = function(given)
		-- Its props are whatever the mod passes, not the example's shape.
		local props: any = given
		local completions: { any } = props.completions or {}
		local keys: { [string]: any } = { enter = "send", ["shift-enter"] = "newline", escape = props.escape or "escape" }
		if #completions > 0 then
			local at = props.highlighted or 1
			keys.up = { highlight = if at <= 1 then #completions else at - 1 }
			keys.down = { highlight = if at >= #completions then 1 else at + 1 }
			keys.tab = { complete = completions[math.clamp(at, 1, #completions)].insert }
		end

		local rows = {}
		for index, completion in completions do
			table.insert(rows, ui.row { class = "completion", align = "center", selected = index == (props.highlighted or 1),
				msg = { complete = completion.insert },
				ui.icon { name = (completion.icon or "file") :: any, size = "small" },
				ui.text { completion.label, truncate = true },
				ui.spacer(),
				completion.detail and ui.text { completion.detail, class = "completion-detail", truncate = true },
			})
		end

		local chips = {}
		for _, chip in props.context or {} do
			table.insert(chips, ui.row { class = "composer-chip", align = "center", hug = true,
				ui.icon { name = "square-terminal", size = "small" },
				ui.row { max_width = 260, ui.text { chip.label, size = "small", truncate = true } },
				ui.row { class = "composer-chip-x", msg = { uncontext = chip.id }, tooltip = "Leave it out", ui.icon { name = "close", size = "small" } },
			})
		end
		for _, queued in props.queued or {} do
			table.insert(chips, ui.row { class = "composer-chip", align = "center", hug = true,
				ui.icon { name = "history", size = "small" },
				ui.text { `Queued: {queued.text}`, class = "composer-queued", truncate = true, max_width = 320 },
				ui.row { class = "composer-chip-x", msg = { unqueue = queued.id }, tooltip = "Don't send it", ui.icon { name = "close", size = "small" } },
			})
		end

		local settings = {}
		for _, setting in props.settings or {} do
			table.insert(settings, dropdown {
				open = setting.open,
				value = setting.value,
				icon = setting.icon,
				options = setting.options,
				selected = setting.selected,
				toggle = { menu = setting.key },
				dismiss = "close-menu",
				event = `pick_{setting.key}`,
				side = "top",
				width = 260,
			})
		end

		local ready = (props.draft or "") ~= ""
		local field = ui.input {
			id = "composer",
			class = "composer-field",
			value = props.draft or "",
			placeholder = props.placeholder or "Type a message, or ! for a shell command…",
			change = "draft",
			multiline = true,
			rows = 2,
			mono = props.shell,
			-- The popover's anchor is a row: take its width.
			grow = true,
			focus = true,
			keys = keys,
		}

		return ui.column { gap = "small",
			#chips > 0 and ui.row { gap = "small", wrap = true, table.unpack(chips) },
			props.shell and ui.row { class = "composer-hint", align = "center",
				ui.icon { name = "square-terminal", size = "small" },
				ui.text { "Shell command", class = "composer-hint-title" },
				ui.text { "Runs in this workspace. Use Add to chat on the result to show it to the agent.", class = "composer-hint-text", truncate = true },
			},
			ui.column { class = "composer-box", state = if props.shell then "shell" else nil,
				ui.popover { open = #rows > 0, side = "top", align = "start", width = 360, gap = 6,
					content = ui.column { class = "completions", table.unpack(rows) },
					field,
				},
				ui.row { align = "center", gap = "small",
					ui.row { align = "center", gap = "none", table.unpack(settings) },
					props.auto_accept ~= nil and ui.row { class = "composer-auto", align = "center", msg = { auto_accept = not props.auto_accept },
						state = if props.auto_accept then "on" else nil,
						tooltip = if props.auto_accept then "Auto-accept is on: the agent's requests are allowed as they come" else "Allow the agent's requests without asking",
						ui.icon { name = "shield", size = "small" },
						ui.text { "Auto", size = "small" },
					},
					ui.spacer(),
					props.usage and ui.text { props.usage, class = "composer-usage" },
					ui.row { class = "composer-send", align = "center", justify = "center",
						state = if props.working and not ready then "working" elseif ready then "ready" else nil,
						msg = if props.working and not ready then "stop" else "send",
						tooltip = if props.working and not ready then "Stop the agent (Esc)" elseif props.shell then "Run it (Enter)" else "Send (Enter)",
						ui.icon { name = if props.working and not ready then "square" else "arrow-up", size = "small" },
					},
				},
			},
		}
	end,
}
