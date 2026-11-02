-- Plan usage in the status bar, and a sheet with the details when it is
-- clicked: each AI service signed in here (Claude, Codex, Copilot, ...),
-- from the `usage` provider (`app.usage`). Configured services and host
-- errors stay reachable even before there are numbers.

local usage_bar = use "usage-bar"

local SHEET_WIDTH = 340
-- Leave room for the status bar and popover padding in a 400px window.
local SHEET_MAX_HEIGHT = 300
-- The sheet's padding on each side, from the `popover` rule in style.luau.
local SHEET_PADDING = 16
local INNER = SHEET_WIDTH - 2 * SHEET_PADDING
-- Windows a status bar widget names at most; the sheet lists the rest.
local BAR_WINDOWS = 2

type State = { open: boolean, tab: string? }

local function percent(fraction: number): string
	return `{math.floor(fraction * 100 + 0.5)}%`
end

-- A thin track filled to `fraction`, drawn from rows of fixed width.
local function bar(fraction: number, width: number, hot: boolean?)
	return ui.row {
		class = "track",
		width = width,
		height = 4,
		gap = "none",
		ui.row {
			class = if hot then { "fill", "fill-hot" } else "fill",
			width = math.clamp(fraction, 0, 1) * width,
			height = 4,
		},
	}
end

local function find(usage: any, id: string?): any
	for _, provider in usage.providers do
		if provider.id == id then
			return provider
		end
	end
	return nil
end

local function mark(provider: any)
	if provider.icon then
		return ui.icon { name = provider.icon, class = "status-text" }
	end
	return ui.text { provider.name, class = "status-label" }
end

-- One service in the status bar: its logo, a bar for its tightest window,
-- and its first windows' use and reset.
local function widget(provider: any)
	local parts = {}
	for index, window in provider.windows do
		if index > BAR_WINDOWS then
			break
		end
		parts[index] = window.label
	end
	local balance = provider.balances[1]
	local label = if #parts > 0
		then table.concat(parts, " · ")
		elseif balance then `{balance.label}: {balance.text}`
		elseif provider.access then "Allow access"
		elseif provider.error then "Unavailable"
		else "No usage yet"
	return ui.row {
		class = "usage-widget",
		align = "center",
		gap = "small",
		msg = { open = provider.id },
		tooltip = provider.error,
		mark(provider),
		provider.used and bar(provider.used, 28, provider.severity ~= "normal"),
		ui.text { label, class = "status-text" },
	}
end

local function window(entry: any)
	return usage_bar {
		title = entry.title,
		left = entry.left,
		pace = entry.pace,
		resets = entry.resets_in and `Resets in {entry.resets_in}`,
		note = entry.pace_note,
		width = INNER,
	}
end

local function pair(label: string, value: string)
	return ui.row {
		ui.text { label, class = "muted" },
		ui.spacer(),
		ui.text { value },
	}
end

local function section(entry: any)
	if entry.kind == "limit" then
		return window(entry.window)
	end
	local rows = {}
	if entry.kind == "facts" then
		for index, fact in entry.facts do
			rows[index] = pair(fact.label, fact.value)
		end
	else
		for index, share in entry.shares do
			rows[index] = ui.row {
				align = "center",
				gap = "small",
				ui.row { width = 110, ui.text { share.label, class = "muted", truncate = true } },
				bar(share.share, INNER - 160),
				ui.spacer(),
				ui.text { percent(share.share), class = "muted" },
			}
		end
	end
	return ui.column { gap = "small", ui.text { entry.title, weight = "medium" }, table.unpack(rows) }
end

local function updated(usage: any): string
	if usage.busy then
		return "Updating…"
	elseif usage.age == nil then
		return "Not updated yet"
	elseif usage.age < 1 then
		return "Updated just now"
	elseif usage.age < 60 then
		return `Updated {usage.age}m ago`
	end
	return `Updated {usage.age // 60}h ago`
end

local function body(provider: any)
	local blocks = {}
	local function add(element)
		if #blocks > 0 then
			table.insert(blocks, ui.divider())
		end
		table.insert(blocks, element)
	end
	if provider.access then
		add(ui.column {
			gap = "small",
			ui.text {
				if provider.access == "denied"
					then "macOS refused access to this sign-in."
					else `{provider.name}'s sign-in is in the Keychain; macOS asks before it is read.`,
				class = "muted",
			},
			ui.button { "Allow", to = "provider:usage", msg = { allow = provider.id } },
		})
	end
	if provider.error then
		add(ui.text {
			if provider.stale then `Showing older numbers: {provider.error}` else provider.error,
			class = "muted",
			tone = "warning",
		})
	end
	if #provider.windows > 0 then
		local windows = {}
		for index, entry in provider.windows do
			windows[index] = window(entry)
		end
		add(ui.column { gap = "large", table.unpack(windows) })
	end
	if #provider.balances > 0 then
		local rows = {}
		for index, balance in provider.balances do
			rows[index] = ui.column {
				gap = "small",
				pair(balance.label, balance.text),
				balance.fraction and bar(balance.fraction, INNER),
			}
		end
		add(ui.column { gap = "small", table.unpack(rows) })
	end
	for _, entry in provider.sections do
		add(section(entry))
	end
	for _, account in provider.accounts or {} do
		local first = account.windows[1]
		add(ui.column {
			gap = "small",
			ui.row {
				ui.text { account.name, weight = "medium" },
				ui.spacer(),
				account.note and ui.text { account.note, class = "muted" },
			},
			first and pair(first.title, `{percent(first.left)} left`),
			first and bar(first.left, INNER),
		})
	end
	if #blocks == 0 then
		add(ui.text { "Nothing to show yet.", class = "muted" })
	end
	return blocks
end

local function sheet(usage: any, provider: any)
	local tabs = {}
	for index, entry in usage.providers do
		tabs[index] = ui.column {
			class = if provider and entry.id == provider.id then "usage-tab usage-tab-active" else "usage-tab",
			grow = true,
			align = "center",
			gap = "none",
			msg = { tab = entry.id },
			entry.icon and ui.icon { name = entry.icon },
			ui.text { entry.name, class = "usage-tab-label" },
		}
	end
	local blocks = if provider then body(provider) else {}
	return ui.column {
		max_height = SHEET_MAX_HEIGHT,
		scroll = "y",
		ui.column {
			-- Preserve the content's height so long reports scroll, not squeeze.
			shrink = false,
			gap = "normal",
			ui.row {
				align = "center",
				ui.text { if provider then provider.name else "Usage", class = "sheet-title" },
				ui.spacer(),
				provider and provider.email and ui.text { provider.email, class = "muted" },
			},
			ui.row {
				class = "sheet-subtitle",
				ui.text { `{updated(usage)} · {usage.host or "local"}`, class = "muted" },
				ui.spacer(),
				provider and provider.plan and ui.text { provider.plan, class = "muted" },
			},
			ui.divider(),
			usage.error and ui.text { usage.error, tone = "warning" },
			ui.column { gap = "normal", table.unpack(blocks) },
			ui.divider(),
			ui.item { "↻   Refresh", to = "provider:usage", msg = "refresh" },
			#tabs > 1 and ui.row { class = "usage-tabs", gap = "small", wrap = true, table.unpack(tabs) },
		},
	}
end

return mod {
	region = "status",

	init = function(previous: State?): State
		return previous or { open = false }
	end,

	update = function(state: State, msg: any)
		if type(msg) == "table" and msg.open then
			-- Clicking the open provider's widget again closes the sheet.
			state.open = not (state.open and state.tab == msg.open)
			state.tab = msg.open
		elseif type(msg) == "table" and msg.tab then
			state.tab = msg.tab
		elseif msg == "open" then
			state.open = not state.open
		elseif msg == "close" then
			state.open = false
		end
	end,

	view = function(state: State, app)
		local usage = app.usage
		if usage == nil or (#usage.providers == 0 and not usage.error) then
			return ui.row {}
		end
		local widgets = {}
		for _, id in usage.headline do
			local provider = find(usage, id)
			if provider then
				table.insert(widgets, widget(provider))
			end
		end
		local shown = find(usage, state.tab) or find(usage, usage.headline[1]) or usage.providers[1]
		if #widgets == 0 then
			table.insert(widgets, if shown then widget(shown) else ui.row {
				class = "usage-widget",
				msg = "open",
				tooltip = usage.error,
				ui.text { "Usage unavailable", class = "status-text" },
			})
		end
		return ui.popover {
			open = state.open,
			dismiss = "close",
			side = "top",
			align = "start",
			width = SHEET_WIDTH,
			content = sheet(usage, shown),
			ui.row { gap = "large", align = "center", table.unpack(widgets) },
		}
	end,
}
