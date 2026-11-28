-- AI Assistant pane (right card): live planner-aware assistant inspired by
-- Woday's Ask Wodie panel, with quick suggestion chips, structured answers
-- computed from the user's real planner data, and one-click planner actions.

local chat_message = use "chat-message"
local icon_button = use "icon-button"

export type AssistantAction = {
	label: string,
	icon: string?,
	msg: any,
}

export type AssistantTurn = {
	from: string,
	text: string,
	actions: { AssistantAction }?,
}

type Props = {
	messages: { AssistantTurn },
	draft: string,
}

local SUGGESTIONS = {
	"What's on my plate today?",
	"Plan my week",
	"What's overdue? Help me reschedule",
	"Summarize my habits & streaks",
	"Create a note with a packing list",
}

return component {
	title = "Daybook Assistant",
	category = "Daybook",
	icon = "sparkles",
	description = "Planner-aware assistant pane with prompt chips, live summaries, and one-click actions.",
	styles = {
		[".asst"] = { padding = { 24, 22, 54, 22 }, gap = 14 },
		[".asst-title"] = { size = "title", weight = "bold" },
		[".asst-new"] = { background = "surface-soft", radius = 14, padding = { 4, 11 }, size = "small", weight = "medium", color = "muted-foreground" },
		[".asst-new:hover"] = { background = "list-hover", color = "foreground" },
		[".asst-hero"] = { padding = { 32, 8, 12, 8 }, gap = 14 },
		[".asst-badge"] = { radius = 22, gradient = "135deg, #ff5f4d, #e53935", color = "#ffffff", shadow = "0 6px 18px -4px #e5393566" },
		[".asst-ask"] = { size = "large", weight = "bold" },
		[".asst-sub"] = { size = "small", color = "muted-foreground" },
		[".asst-chip"] = { background = "surface-soft", radius = 16, padding = { 7, 14 }, border = "all", border_color = "border/60" },
		[".asst-chip:hover"] = { background = "list-hover", border_color = "foreground/25" },
		[".asst-action"] = { background = "accent/12", color = "accent", radius = 10, padding = { 5, 10 }, gap = 6 },
		[".asst-action:hover"] = { background = "accent/20" },
		[".asst-composer"] = { background = "surface-soft", border = true, border_color = "border", radius = 16, padding = { 8, 10, 8, 14 }, gap = 8 },
		[".asst-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".asst-send"] = { background = "foreground", color = "background", radius = 14 },
		[".asst-send:hover"] = { opacity = 0.85 },
		[".asst-thread"] = { gap = 14, padding = { 4, 0 } },
	},
	example = { messages = {}, draft = "" },

	build = function(props: Props)
		local messages = props.messages or {}

		local header = ui.row {
			align = "center",
			gap = "small",
			ui.text { "AI Assistant", class = "asst-title" },
			ui.spacer(),
			#messages > 0 and ui.row {
				class = "asst-new",
				msg = "asst-clear",
				ui.text { "New Chat", size = "small", weight = "medium" },
			},
			icon_button { icon = "inbox", tooltip = "Switch to Inbox", size = "small", msg = { mode = "inbox" } },
			icon_button { icon = "panel-right", tooltip = "Hide right pane", size = "small", msg = "toggle-right", to = "shell" },
		}

		local body: Element
		if #messages == 0 then
			local chips: { Element } = {}
			for _, prompt in SUGGESTIONS do
				table.insert(chips, ui.row {
					class = "asst-chip",
					hug = true,
					msg = { asst_prompt = prompt },
					ui.text { prompt, size = "small", weight = "medium" },
				})
			end
			body = ui.column {
				grow = true,
				scroll = true,
				align = "center",
				justify = "center",
				ui.column {
					class = "asst-hero",
					align = "center",
					ui.row {
						class = "asst-badge",
						width = 56,
						height = 56,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles", size = "title" },
					},
					ui.column {
						align = "center",
						gap = "none",
						ui.text { "Ask Daybook", class = "asst-ask" },
						ui.text { "Live answers & actions over your planner", class = "asst-sub" },
					},
					ui.column {
						align = "center",
						gap = "small",
						table.unpack(chips),
					},
				},
			}
		else
			local turns: { Element } = {}
			for _, turn in messages do
				if turn.from == "user" then
					table.insert(turns, chat_message { from = "user", text = turn.text })
				else
					local action_row: Element? = nil
					if turn.actions and #turn.actions > 0 then
						local btns: { Element } = {}
						for _, act in turn.actions do
							table.insert(btns, ui.row {
								class = "asst-action",
								align = "center",
								hug = true,
								msg = act.msg,
								act.icon and ui.icon { name = act.icon :: any, size = "small" },
								ui.text { act.label, size = "small", weight = "medium" },
							})
						end
						action_row = ui.row { gap = "small", wrap = true, table.unpack(btns) }
					end
					table.insert(turns, chat_message {
						from = "assistant",
						author = "Daybook",
						icon = "sparkles",
						text = turn.text,
						action_row,
					})
				end
			end
			body = ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { class = "asst-thread", shrink = false, table.unpack(turns) },
			}
		end

		local composer = ui.row {
			class = "asst-composer",
			align = "center",
			ui.input {
				id = "asst-input",
				value = props.draft,
				placeholder = "Ask, plan, create…",
				change = "asst-typing",
				submit = "asst-submit",
				grow = true,
				class = "asst-input",
			},
			ui.row {
				class = "asst-send",
				width = 28,
				height = 28,
				align = "center",
				justify = "center",
				shrink = false,
				tooltip = "Send (Enter)",
				msg = "asst-send",
				ui.icon { name = "arrow-up", size = "small" },
			},
		}

		return ui.column {
			class = "asst",
			grow = true,
			header,
			body,
			composer,
		}
	end,
}
