-- Contracts: the register. Where you are, the title and what it holds,
-- Import and Add, five headline figures, then the contracts card: filter
-- pills by regime, a search over contracts and suppliers, an AI insight,
-- and the table, whose Open shows a contract in a side sheet. Add contract
-- opens a dialog; added contracts stay for the session. From `app.duly`
-- (sample data).

local kpi_meter = use "kpi-meter"
local insight_note = use "insight-note"
local dashboard_card = use "dashboard-card"
local sheet = use "sheet"
local dialog = use "dialog"
local key_values = use "key-values"

type State = {
	filter: string,
	query: string,
	selected: string?,
	open: string?,
	adding: boolean,
	name: string,
	supplier: string,
	added: { any },
}

local REGIMES = {
	act = { label = "Act", color = "blue" },
	earlier = { label = "Earlier rules", color = "grey" },
	missing = { label = "Missing data", color = "red" },
}

return mod {
	title = "Contracts",

	styles = {
		[".d-page"] = { padding = { 22, 30, 30, 30 }, gap = 12 },
		[".crumb"] = { size = "large", color = "muted-foreground" },
		[".crumb-on"] = { size = "large" },
		[".d-page-title"] = { size = "display", weight = "medium" },
		[".d-muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".outline-button"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 7, 12 }, gap = 6, shadow = "0 1px 1px #00000008" },
		[".outline-button:hover"] = { background = "secondary" },
		[".dark-button"] = { background = "foreground", color = "background", radius = 8, padding = { 7, 12 }, gap = 6 },
		[".strip"] = { background = "secondary", border = true, border_color = "border", radius = 12, gap = 0 },
		[".strip-rule"] = { background = "border" },
		[".d-pill"] = { border = true, border_color = "border", radius = 14, padding = { 4, 11 }, gap = 6, background = "background" },
		[".d-pill:hover"] = { background = "secondary" },
		[".d-pill:selected"] = { background = "foreground", color = "background", border_color = "foreground" },
		[".pill-count"] = { size = "small", color = "muted-foreground" },
		[".pill-count-on"] = { size = "small", color = "background" },
		[".search-box"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 1, 10 }, gap = 6 },
		[".search-input"] = { background = "#00000000", border = "none", padding = { 4, 0 } },
		[".head"] = { padding = { 6, 4 }, border = "bottom", border_color = "hairline" },
		[".heading"] = { size = "small", color = "muted-foreground" },
		[".row"] = { padding = { 5, 4 }, border = "bottom", border_color = "hairline", radius = 4 },
		[".row:hover"] = { background = "list-hover" },
		[".row:selected"] = { background = "accent/8" },
		[".regime"] = { background = "#00000010", radius = 5, padding = { 2, 7 }, gap = 6 },
		[".regime-dot"] = { radius = 3 },
		[".late"] = { color = "danger", size = "small" },
		[".open-button"] = { background = "background", border = true, border_color = "border", radius = 7, padding = { 4, 10 } },
		[".open-button:hover"] = { background = "secondary" },
		[".field-label"] = { size = "small", color = "muted-foreground" },
		[".field"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 6, 10 } },
	},

	init = function(previous: State?): State
		local state: State = previous or { filter = "all", query = "", adding = false, name = "", supplier = "", added = {} }
		state.query = ""
		state.open = nil
		state.adding = false
		return state
	end,

	update = function(state: State, msg)
		if msg == "add" then
			state.adding, state.name, state.supplier = true, "", ""
		elseif msg == "close-add" then
			state.adding = false
		elseif msg == "save-add" then
			if state.name ~= "" then
				table.insert(state.added, 1, {
					id = `new-{#state.added + 1}`,
					contract = state.name,
					supplier = if state.supplier == "" then "Supplier to confirm" else state.supplier,
					value = "-",
					regime = "missing",
					signed = "-",
					next = "Value needed",
					late = true,
				})
			end
			state.adding = false
		elseif msg == "close-sheet" then
			state.open = nil
		elseif type(msg) == "table" then
			if msg.filter then
				state.filter = msg.filter
			elseif msg.msg == "query" then
				state.query = msg.value
			elseif msg.msg == "clear" then
				state.query = ""
			elseif msg.msg == "name" then
				state.name = msg.value
			elseif msg.msg == "supplier" then
				state.supplier = msg.value
			elseif msg.select then
				state.selected = msg.select
			elseif msg.open then
				state.selected, state.open = msg.open, msg.open
			elseif msg.go then
				return state, { { to = "sidebar", msg = { page = msg.go } } }
			end
		end
	end,

	view = function(state: State, app)
		local data = app.duly
		if not data or data.available == false then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { if data then data.reason or "No data" else "Loading…", class = "d-muted" } }
		end
		local register = data.contracts

		local head = ui.column {
			gap = "none",
			ui.row {
				drag = true,
				align = "center",
				gap = "normal",
				height = 36,
				ui.icon { name = "file-text", class = "d-muted" },
				ui.text { "Contracts", class = "crumb" },
				ui.text { "/", class = "crumb" },
				ui.text { "Register", class = "crumb-on" },
			},
			ui.row { height = 22 },
			ui.row {
				align = "end",
				ui.column {
					gap = "small",
					ui.text { "Contract register", class = "d-page-title" },
					ui.text { register.subtitle, class = "d-muted" },
				},
				ui.spacer(),
				ui.row {
					gap = "normal",
					ui.row { class = "outline-button", align = "center", msg = { go = "contracts" }, ui.icon { name = "upload", size = "small" }, ui.text { "Import list", weight = "medium" } },
					ui.row { class = "dark-button", align = "center", msg = "add", ui.icon { name = "plus", size = "small" }, ui.text { "Add contract", weight = "medium" } },
				},
			},
		}

		local cells = {}
		for index, kpi in register.kpis do
			if index > 1 then
				table.insert(cells, ui.row { class = "strip-rule", width = 1 })
			end
			local filter = ({ nil, "act", "earlier", nil, "missing" } :: { any })[index]
			table.insert(cells, kpi_meter {
				title = kpi.title,
				dot = kpi.dot,
				value = kpi.value,
				caption = kpi.caption,
				caption_color = if kpi.caption_tone == "red" then "danger" else nil,
				segments = kpi.bar,
				width = 184,
				msg = { filter = filter or "all" },
			})
		end

		-- Filter pills and the search.
		local pills = {}
		for _, filter in register.filters do
			local on = state.filter == filter.id
			table.insert(pills, ui.row {
				class = "d-pill",
				align = "center",
				selected = on,
				msg = { filter = filter.id },
				ui.text { filter.label, size = "small", weight = "medium" },
				ui.text { tostring(filter.count), class = if on then "pill-count-on" else "pill-count" },
			})
		end

		local query = string.lower(state.query)
		local shown = {}
		local all = {}
		for _, row in state.added do
			table.insert(all, row)
		end
		for _, row in register.rows do
			table.insert(all, row)
		end
		for _, row in all do
			local keep = state.filter == "all" or row.regime == state.filter
			if keep and query ~= "" then
				keep = string.find(string.lower(row.contract .. " " .. row.supplier), query, 1, true) ~= nil
			end
			if keep then
				table.insert(shown, row)
			end
		end

		local rows = {}
		for _, row in shown do
			local regime = REGIMES[row.regime] or REGIMES.act
			table.insert(rows, ui.row {
				class = "row",
				align = "center",
				selected = state.selected == row.id,
				msg = { select = row.id },
				ui.row { grow = true, ui.text { row.contract, size = "small", truncate = true } },
				ui.row { width = 230, shrink = false, ui.text { row.supplier, size = "small", class = "d-muted", truncate = true } },
				ui.row { width = 64, shrink = false, justify = "end", ui.text { row.value, size = "small", class = "d-muted" } },
				ui.row { width = 12 },
				ui.row {
					width = 126,
					shrink = false,
					ui.row {
						class = "regime",
						hug = true,
						align = "center",
						ui.row { class = "regime-dot", width = 5, height = 5, style = { background = regime.color } },
						ui.text { regime.label, size = "small", class = "d-muted" },
					},
				},
				ui.row { width = 100, shrink = false, ui.text { row.signed, size = "small", class = "d-muted" } },
				ui.row { width = 190, shrink = false, ui.text { row.next, size = "small", class = if row.late then "late" else "d-muted", truncate = true } },
				ui.row { width = 56, shrink = false, justify = "end", ui.row { class = "open-button", msg = { open = row.id }, ui.text { "Open", size = "small", weight = "medium" } } },
			})
		end

		local function heading(title: string, width: number?, right: boolean?)
			return ui.row {
				width = width,
				grow = width == nil,
				shrink = false,
				justify = if right then "end" else "start",
				ui.text { title, class = "heading" },
			}
		end

		local card = dashboard_card {
			title = "Contracts",
			icon = "file-text",
			menu = "contracts-menu",
			ui.row {
				align = "center",
				gap = "normal",
				ui.row { gap = "small", table.unpack(pills) },
				ui.spacer(),
				ui.row {
					class = "search-box",
					width = 244,
					align = "center",
					ui.icon { name = "search", size = "small", class = "d-muted" },
					ui.input { id = "contract-search", class = "search-input", value = state.query, placeholder = "Search contracts or suppliers", change = "query", keys = { escape = "clear" }, grow = true },
				},
			},
			insight_note { text = register.insight },
			ui.column {
				gap = "none",
				ui.row {
					class = "head",
					align = "center",
					heading("Contract"),
					heading("Supplier", 230),
					heading("Value", 64, true),
					ui.row { width = 12 },
					heading("Regime", 126),
					heading("Signed", 100),
					heading("Next notice", 190),
					ui.row { width = 56 },
				},
				#rows == 0 and ui.row { padding = "large", ui.text { "No contracts match", class = "d-muted" } },
				table.unpack(rows),
			},
		}

		-- The open contract, in a side sheet.
		local open
		for _, row in all do
			if row.id == state.open then
				open = row
			end
		end
		local details = open and sheet {
			open = true,
			dismiss = "close-sheet",
			title = open.contract,
			subtitle = open.supplier,
			icon = "file-text",
			width = 400,
			key_values {
				width = 120,
				rows = {
					{ "Value", open.value },
					{ "Regime", (REGIMES[open.regime] or REGIMES.act).label },
					{ "Signed", open.signed },
					{ "Next notice", open.next },
				},
			},
			ui.row { height = 8 },
			if open.regime == "missing"
				then insight_note { text = "No value or end date in the register. The matching purchase order suggests one; confirm it to see which notices this contract owes." }
				else insight_note { label = "Notices", icon = "bell", text = if open.late then `{open.next}. Draft it from the rating template and send it to the contract owner.` else `Nothing late. {open.next}.` },
		}

		local adding = dialog {
			open = state.adding,
			dismiss = "close-add",
			title = "Add contract",
			subtitle = "It joins the register as missing data until a value and dates are in.",
			icon = "plus",
			size = "small",
			submit = "save-add",
			buttons = {
				{ label = "Cancel", msg = "close-add", variant = "ghost" },
				{ label = "Add contract", msg = "save-add", variant = "primary", disabled = state.name == "" },
			},
			ui.text { "Contract", class = "field-label" },
			ui.input { id = "new-contract", class = "field", value = state.name, placeholder = "School catering", change = "name", submit = "save-add", focus = state.adding },
			ui.text { "Supplier", class = "field-label" },
			ui.input { id = "new-supplier", class = "field", value = state.supplier, placeholder = "Supplier name", change = "supplier", submit = "save-add" },
		}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "d-page",
				shrink = false,
				head,
				ui.row { height = 4 },
				ui.row { class = "strip", align = "stretch", table.unpack(cells) },
				card,
			},
			details,
			adding,
		}
	end,
}
