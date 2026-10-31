-- Model usage in the status bar, and a sheet with the details when it is
-- clicked. The numbers are demo data: the host does not read provider
-- accounts yet, so they live here, shaped as `app.usage` would be.

type Quota = { label: string, used: number, resets: string, note: string? }
type Provider = {
	name: string,
	glyph: string,
	account: string,
	plan: string,
	source: string,
	quotas: { Quota },
	extra: { title: string, label: string, value: string },
}

local PROVIDERS: { [string]: Provider } = {
	claude = {
		name = "Claude",
		glyph = "✳",
		account = "penso",
		plan = "Max",
		source = "Local",
		quotas = {
			{ label = "Session", used = 0.03, resets = "4h 35m" },
			{ label = "Week", used = 0.34, resets = "3d 9h", note = "Opus included · Resets weekly" },
		},
		extra = { title = "Extra usage", label = "Spent this month", value = "$0.00" },
	},
	copilot = {
		name = "Copilot",
		glyph = "◉",
		account = "penso",
		plan = "Individual",
		source = "Local",
		quotas = {
			{ label = "Premium", used = 0.02, resets = "16h 4m", note = "96% in reserve · Lasts until reset" },
		},
		extra = { title = "Credits", label = "Credits used", value = "25" },
	},
}
local ORDER = { "claude", "copilot" }
local SHEET_WIDTH = 340
-- The sheet's padding on each side, from the `popover` rule in style.luau.
local SHEET_PADDING = 16

type State = { open: boolean, tab: string, refreshed: { [string]: number } }

local function percent(fraction: number): string
	return `{math.floor(fraction * 100 + 0.5)}%`
end

-- A thin track filled to `fraction`, drawn from rows of fixed width.
local function bar(fraction: number, width: number, thickness: number?)
	local height = thickness or 4
	local filled = math.clamp(fraction, 0, 1) * width
	return ui.row {
		class = "track",
		width = width,
		height = height,
		gap = "none",
		ui.row { class = "fill", width = filled, height = height },
	}
end

-- One provider in the status bar: its glyph, a bar for its first quota, and
-- each quota's use and reset.
local function widget(key: string, provider: Provider)
	local parts = {}
	for index, quota in provider.quotas do
		parts[index] = `{percent(quota.used)} used {quota.resets}`
	end
	local label = table.concat(parts, " · ")
	if #provider.quotas == 1 then
		label = `{percent(provider.quotas[1].used)} used {provider.quotas[1].label}`
	end
	return ui.row {
		class = "usage-widget",
		align = "center",
		gap = "small",
		msg = { open = key },
		ui.text { provider.glyph, class = "status-text" },
		bar(provider.quotas[1].used, 28),
		ui.text { label, class = "status-text" },
	}
end

local function quota(entry: Quota)
	local width = SHEET_WIDTH - 2 * SHEET_PADDING
	return ui.column {
		gap = "small",
		ui.row {
			align = "center",
			ui.text { `{entry.label} {percent(1 - entry.used)} left`, weight = "medium" },
			ui.spacer(),
			ui.text { `Resets in {entry.resets}`, class = "muted" },
		},
		bar(entry.used, width, 6),
		entry.note and ui.text { entry.note, class = "muted" },
	}
end

local function sheet(state: State)
	local provider = PROVIDERS[state.tab]
	local refreshed = state.refreshed[state.tab]
	local updated = if refreshed then "Updated just now" else "Updated 8m ago"
	local quotas = {}
	for index, entry in provider.quotas do
		quotas[index] = quota(entry)
	end
	local tabs = {}
	for index, key in ORDER do
		tabs[index] = ui.column {
			class = if key == state.tab then "usage-tab usage-tab-active" else "usage-tab",
			grow = true,
			align = "center",
			gap = "none",
			msg = { tab = key },
			ui.text { PROVIDERS[key].glyph },
			ui.text { PROVIDERS[key].name, class = "usage-tab-label" },
		}
	end
	return ui.column {
		gap = "normal",
		ui.row {
			align = "center",
			ui.text { provider.name, class = "sheet-title" },
			ui.spacer(),
			ui.text { provider.account, class = "muted" },
		},
		ui.row {
			class = "sheet-subtitle",
			ui.text { `{updated} · {provider.source}`, class = "muted" },
			ui.spacer(),
			ui.text { provider.plan, class = "muted" },
		},
		ui.divider(),
		ui.column { gap = "large", table.unpack(quotas) },
		ui.divider(),
		ui.text { provider.extra.title, weight = "medium" },
		ui.row {
			ui.text { provider.extra.label, class = "muted" },
			ui.spacer(),
			ui.text { provider.extra.value },
		},
		ui.divider(),
		ui.column {
			gap = "none",
			ui.item { "↻   Refresh", msg = "refresh" },
			ui.item { "↗   Usage Dashboard", msg = "dashboard" },
			ui.item { "⌁   Status Page", msg = "status" },
		},
		ui.row { class = "usage-tabs", gap = "small", table.unpack(tabs) },
	}
end

return mod {
	region = "status",

	init = function(previous: State?): State
		return previous or { open = false, tab = "claude", refreshed = {} }
	end,

	update = function(state: State, msg: any)
		if type(msg) == "table" and msg.open then
			-- Clicking the open provider's widget again closes the sheet.
			state.open = not (state.open and state.tab == msg.open)
			state.tab = msg.open
		elseif type(msg) == "table" and msg.tab then
			state.tab = msg.tab
		elseif msg == "close" then
			state.open = false
		elseif msg == "refresh" then
			state.refreshed[state.tab] = 1
		end
	end,

	view = function(state: State)
		local widgets = {}
		for index, key in ORDER do
			widgets[index] = widget(key, PROVIDERS[key])
		end
		return ui.popover {
			open = state.open,
			dismiss = "close",
			side = "top",
			align = "start",
			width = SHEET_WIDTH,
			content = sheet(state),
			ui.row { gap = "large", align = "center", table.unpack(widgets) },
		}
	end,
}
