-- The agent's panel. This is a scripted demo, not a model: it reads the
-- deals the provider shares and answers a few kinds of question by rules
-- (what to do first, what is at risk, what will close), saying so. A real
-- agent would be a provider that talks to one and publishes its replies.

local chat_message = use "chat-message"
local thinking = use "thinking"
local record_card = use "record-card"
local numbered_list = use "numbered-list"
local composer = use "composer"
local icon_button = use "icon-button"

type Turn = { from: string, text: string, kind: string? }

type State = { turns: { Turn }, draft: string, open: boolean }

local function money(value: number): string
	if value >= 1000000 then
		return string.format("$%.2fM", value / 1000000)
	end
	return string.format("$%dK", math.floor(value / 1000 + 0.5))
end

local function amount(value: number): string
	local text = tostring(math.floor(value))
	local grouped = string.reverse((string.gsub(string.reverse(text), "(%d%d%d)", "%1,")))
	return "$" .. (string.gsub(grouped, "^,", ""))
end

-- Late deals with no contact in over ten days, largest first.
local function stale(deals: { any }): { any }
	local found = {}
	for _, deal in deals do
		if (deal.stage == "negotiation" or deal.stage == "proposal") and (deal.touched or 0) > 10 then
			table.insert(found, deal)
		end
	end
	table.sort(found, function(a, b)
		return (if a.status == "risk" then 1 else 0) > (if b.status == "risk" then 1 else 0)
			or ((a.status == "risk") == (b.status == "risk") and a.amount > b.amount)
	end)
	return found
end

-- What kind of answer a question gets.
local function kind_of(text: string): string
	local lower = string.lower(text)
	if string.find(lower, "risk") then
		return "risk"
	elseif string.find(lower, "close") or string.find(lower, "win") or string.find(lower, "won") then
		return "close"
	elseif string.find(lower, "first") or string.find(lower, "priorit") or string.find(lower, "focus") then
		return "first"
	end
	return "other"
end

local FIRST: Turn = { from = "user", text = "Which one should I prioritize first?", kind = nil }

return mod {
	region = "right",
	title = "Agent",

	styles = {
		[".agent"] = { padding = { 18, 18, 14, 18 }, gap = 16 },
		[".agent-title"] = { weight = "bold" },
		[".agent-turns"] = { gap = 18 },
		[".agent-note"] = { size = "small", color = "muted-foreground" },
	},

	init = function(previous: State?): State
		return previous or { turns = { FIRST, { from = "agent", text = "", kind = "first" } }, draft = "", open = false }
	end,

	update = function(state: State, msg)
		if msg == "reset" then
			state.turns = {}
			state.draft = ""
			return
		elseif msg == "thought" then
			state.open = not state.open
			return
		elseif msg == "send" then
			msg = { msg = "ask", value = state.draft }
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.msg == "typing" then
			state.draft = msg.value
		elseif msg.msg == "ask" and string.match(msg.value or "", "%S") then
			table.insert(state.turns, { from = "user", text = msg.value, kind = nil })
			table.insert(state.turns, { from = "agent", text = "", kind = kind_of(msg.value) })
			state.draft = ""
		end
	end,

	view = function(state: State, app)
		local data = app.deals
		local deals = (data and data.deals) or {}
		local late = stale(deals)
		local total = 0
		for _, deal in late do
			total += deal.amount
		end

		local function answer(kind: string?)
			if kind == "first" and late[1] then
				local top = late[1]
				local steps = {
					`Send {top.company} the revised pricing they asked for`,
					"Book a 30-minute call with their new decision maker",
					"Offer a two-year term to soften the price change",
					"Ask customer success for recent usage wins to share",
				}
				return chat_message {
					author = "Sales agent · demo",
					thinking {
						seconds = 6,
						open = state.open,
						msg = "thought",
						text = `Looked at {#late} late-stage deals without contact in over ten days and ranked them by risk, then size.`,
					},
					ui.text { `Start with {top.company}. It is the largest deal at risk and closes soonest ({top.date}).`, wrap = true },
					record_card {
						title = `{top.company} · {top.summary}`,
						detail = `Deal · Negotiation · {amount(top.amount)}`,
						icon = "building-2",
					},
					ui.text { "Suggested next steps:" },
					numbered_list { items = steps },
					ui.text { `Should I draft the follow-up to {top.company} now?`, wrap = true },
				}
			elseif kind == "risk" then
				local names = {}
				for _, deal in deals do
					if deal.status == "risk" then
						table.insert(names, `{deal.company} ({amount(deal.amount)})`)
					end
				end
				return chat_message {
					author = "Sales agent · demo",
					text = if #names == 0 then "Nothing is flagged at risk right now." else "Flagged at risk:",
					#names > 0 and numbered_list { items = names },
				}
			elseif kind == "close" then
				local likely = {}
				for _, deal in deals do
					if deal.stage ~= "won" and deal.chance >= 70 then
						table.insert(likely, `{deal.company}: {amount(deal.amount)} at {deal.chance}%`)
					end
				end
				return chat_message {
					author = "Sales agent · demo",
					text = if #likely == 0 then "No open deal is above 70% yet." else "Most likely to close this quarter:",
					#likely > 0 and numbered_list { items = likely },
				}
			end
			return chat_message {
				author = "Sales agent · demo",
				text = "I am a scripted demo: I can say what to do first, what is at risk, and what is likely to close. A real agent would plug in as a provider.",
			}
		end

		local turns = {}
		if data then
			table.insert(turns, ui.text {
				if #late > 0
					then `{#late} late-stage deals haven't had a touchpoint in over 10 days. Together they're worth {money(total)} of this quarter's forecast.`
					else "Every late-stage deal has had a touchpoint in the last 10 days.",
				wrap = true,
			})
		end
		for _, turn in state.turns do
			if turn.from == "user" then
				table.insert(turns, chat_message { from = "user", text = turn.text })
			else
				table.insert(turns, answer(turn.kind))
			end
		end

		return ui.column {
			class = "agent",
			grow = true,
			ui.row {
				align = "center",
				ui.text { "Pipeline review · Q4", class = "agent-title" },
				ui.spacer(),
				icon_button { icon = "ellipsis", tooltip = "New chat", msg = "reset", size = "small" },
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { class = "agent-turns", shrink = false, table.unpack(turns) },
			},
			composer {
				agent = "Sales agent · demo",
				placeholder = "Ask the sales agent anything…",
				value = state.draft,
				change = "typing",
				submit = "ask",
				send = "send",
				attach = "thought",
			},
		}
	end,
}
