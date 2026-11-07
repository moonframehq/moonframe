-- Skills and extensions, as pi finds them in its folders and your project.
-- Turning one off writes pi's own settings, so the CLI agrees.

local empty_state = use "empty-state"
local search_field = use "search-field"
local switch = use "switch"

type Props = { kind: string? }
type State = { query: string, serial: number }

local TEXT = {
	skills = { title = "Skills", about = "Instructions pi reads when a task calls for them. Use one with /skill:name." },
	extensions = { title = "Extensions", about = "Tools and commands added to pi. Changes apply as each thread starts again." },
}

local function card(item: any, kind: string): any
	return ui.row {
		class = "page-card",
		align = "center",
		ui.column {
			grow = true,
			gap = "none",
			ui.row { align = "center", gap = "small", ui.text { item.name, weight = "medium", truncate = true }, ui.text { item.scope, class = "scope-chip" } },
			item.description ~= "" and ui.text { item.description, tone = "muted", size = "small", lines = 2 },
		},
		kind == "skills" and item.enabled and ui.button { "Try", small = true, variant = "ghost", msg = { try = item.name } },
		not item.fixed and switch {
			on = item.enabled,
			to = "provider:pi",
			msg = if kind == "skills" then { set_skill = { id = item.id, enabled = not item.enabled } } else { set_extension = { id = item.id, enabled = not item.enabled } },
		},
	}
end

return mod {
	init = function(previous: State?): (State, { any })
		return previous or { query = "", serial = 0 }, { { to = "provider:pi", msg = { refresh = true } } }
	end,

	update = function(state: State, msg: any)
		if type(msg) ~= "table" then
			if msg == "clear" then
				state.query = ""
			end
			return
		end
		if msg.msg == "query" then
			state.query = msg.value
		elseif msg.try then
			state.serial += 1
			local sends: { any } = {
				{ to = "topic:pilot/compose", msg = { seq = state.serial, text = `/skill:{msg.try} ` } },
				{ to = "shell", msg = { page = "new" } },
			}
			return state, sends
		end
	end,

	view = function(state: State, app: any, props: Props?)
		if props == nil then
			return ui.column { width = 0, height = 0 }
		end
		local kind = (props and props.kind) or "skills"
		local text = TEXT[kind] or TEXT.skills
		local pi = app.pi
		local list: { any } = (pi and (if kind == "skills" then pi.skills else pi.extensions)) or {}
		local wanted = string.lower(state.query)
		local cards: { any } = {}
		for _, item in list do
			if wanted == "" or string.find(string.lower(`{item.name} {item.description}`), wanted, 1, true) then
				table.insert(cards, card(item, kind))
			end
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row { drag = true, height = 52, shrink = false },
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					class = "page",
					shrink = false,
					ui.column { gap = "none", ui.text { text.title, class = "page-title" }, ui.text { text.about, tone = "muted" } },
					#list > 6 and search_field { id = "resource-search", value = state.query, change = "query", placeholder = `Search {string.lower(text.title)}` },
					#cards == 0 and empty_state { icon = "layers", title = `No {string.lower(text.title)}`, message = "pi looks in ~/.pi/agent and the project's .pi folder." },
					table.unpack(cards),
				},
			},
		}
	end,
}
