-- A question a coding agent asks with choices, whole: its header, the
-- question, each choice with what the agent said about it, and a field for
-- the user's own words. One choice answers at a click; a question that
-- takes several lets the user tick them, then Send.
--
-- It draws a permission of `app.chat`'s view that carries a `question`,
-- and tells the module that drew it `{ answer_question = permission,
-- answers = { ... } }`: choices' labels, or the words typed.

type Props = {
	id: string?,
	permission: any,
}

type State = {
	picked: { [string]: boolean },
	text: string,
}

local function answer(props: Props, answers: { string }): any
	return { to = "parent", msg = { answer_question = props.permission.id, answers = answers } }
end

return component {
	title = "Chat Question",
	category = "Data",
	icon = "circle-help",
	description = "A question a coding agent asks with choices: each choice with its description, one click to answer (or tick several, then Send), and a field for the user's own words.",
	when = "A permission in `app.chat`'s view that carries a `question`; `agent-chat` draws it already. For a yes/no permission use the permission's options as buttons; for a form of your own use `form-grid`.",
	props = {
		["id"] = "the instance's id, one per question (such as `q-<permission id>`)",
		["permission"] = "the view's permission: { id, title, question = { header?, text, multiple, options = { { label, description? } } } }",
	},
	styles = {
		[".chat-question"] = { border = true, border_color = "accent/50", background = "accent/6", radius = 12, padding = { 10, 12 }, gap = 8 },
		[".chat-question-header"] = { color = "accent" },
		[".chat-question-choice"] = { border = true, border_color = "border", radius = 8, padding = { 6, 10 }, background = "background", gap = 2 },
		[".chat-question-choice:picked"] = { border_color = "accent", background = "accent/10" },
		[".chat-question-field"] = { background = "background" },
	},
	example = {
		id = "example",
		permission = {
			id = 1,
			title = "How should tabs show?",
			question = {
				header = "Tab detail",
				text = "How much should each tab show?",
				multiple = false,
				options = {
					{ label = "Title only", description = "Shortest; best for many tabs" },
					{ label = "Title and agent", description = "Shows which agent runs there" },
				},
			},
		},
	},

	init = function(previous: State?)
		return previous or { picked = {}, text = "" }
	end,

	update = function(state: State, msg: any, props: Props)
		local question = props.permission and props.permission.question
		if question == nil then
			return nil
		end
		local kind = if type(msg) == "table" then msg.msg else msg
		local function typed(value: string?): string
			local text = string.gsub(value or state.text, "^%s+", "")
			return (string.gsub(text, "%s+$", ""))
		end
		if kind == "text" then
			state.text = msg.value or ""
			return state
		elseif type(msg) == "table" and msg.choose then
			if question.multiple then
				state.picked[msg.choose] = not state.picked[msg.choose]
				return state
			end
			return state, { answer(props, { msg.choose }) }
		elseif kind == "submit" and not question.multiple then
			local words = typed(msg.value)
			if words == "" then
				return nil
			end
			return state, { answer(props, { words }) }
		elseif kind == "send" or kind == "submit" then
			local answers: { string } = {}
			for _, option in question.options or {} do
				if state.picked[option.label] then
					table.insert(answers, option.label)
				end
			end
			local words = typed(if type(msg) == "table" then msg.value else nil)
			if words ~= "" then
				table.insert(answers, words)
			end
			if #answers == 0 then
				return nil
			end
			return state, { answer(props, answers) }
		end
		return nil
	end,

	build = function(props: Props, kept: any)
		local state: State = kept
		local permission = props.permission or {}
		local question = permission.question or {}
		local choices: { any } = {}
		for _, option in question.options or {} do
			local picked = state.picked[option.label] == true
			table.insert(choices, ui.row {
				class = "chat-question-choice",
				state = if picked then "picked" else nil,
				gap = "small",
				align = "center",
				msg = { choose = option.label },
				question.multiple and ui.icon { name = if picked then "square-check" else "square", size = "small" },
				ui.column {
					grow = true,
					ui.text { option.label, weight = "medium", wrap = true },
					option.description and ui.text { option.description, size = "small", tone = "muted", wrap = true },
				},
			})
		end
		return ui.column {
			class = "chat-question",
			ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = "circle-help", size = "small", class = "chat-question-header" },
				question.header and ui.text { question.header, size = "small", weight = "medium", class = "chat-question-header" },
			},
			ui.text { question.text or permission.title or "", weight = "medium", wrap = true },
			ui.column { gap = "small", table.unpack(choices) },
			ui.row {
				gap = "small",
				align = "center",
				ui.input {
					id = "words",
					value = state.text,
					placeholder = "Or answer in your own words",
					change = "text",
					submit = "submit",
					grow = true,
					class = "chat-question-field",
				},
				question.multiple and ui.button { "Send", small = true, variant = "primary", msg = "send" },
			},
		}
	end,
}
