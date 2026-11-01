-- The sidebar: the product's name, New chat, "Your dot" with its count,
-- then Pinned (chats and folders), Projects (folders that open to show
-- their chats, five at a time) and Recents, the open chat highlighted.
-- Which folders are open is this mod's own state; the chats themselves come
-- from `app.chats`, and picking one asks the provider to open it.

type State = { open: { [string]: boolean? }, more: { [string]: boolean? } }

local SHOWN = 5

local function chat_row(item: any, current: string?, leading: any)
	return ui.row {
		class = "pl-side-row",
		align = "center",
		selected = item.id == current,
		msg = { open = item.id },
		to = "provider:chats",
		leading,
		ui.text { item.title, class = "pl-side-label", truncate = true },
		item.status == "pending" and ui.icon { name = "clock", class = "pl-side-status", tooltip = "Waiting on you" },
	}
end

local function indent()
	return ui.row { width = 16, shrink = false }
end

return mod {
	title = "Sidebar",

	styles = {
		[".pl-side"] = { background = "sidebar", radius = { 10, 0, 0, 10 } },
		[".pl-side-focus"] = { width = 2, background = "accent" },
		[".pl-side-head"] = { padding = { 0, 18, 0, 16 }, gap = 16, color = "#c8c8c8" },
		[".pl-side-brand"] = { gap = 6, radius = 8 },
		[".pl-side-name"] = { size = "large", weight = "bold", color = "foreground" },
		[".pl-side-body"] = { padding = { 0, 8, 16, 8 }, gap = 0 },
		[".pl-side-row"] = { height = 31, padding = { 0, 8 }, gap = 8, radius = 8 },
		[".pl-side-row:hover"] = { background = "list-hover" },
		[".pl-side-row:selected"] = { background = "frame" },
		[".pl-side-label"] = { color = "foreground" },
		[".pl-side-icon"] = { color = "#d6d6d6" },
		[".pl-side-status"] = { color = "muted-foreground" },
		[".pl-side-more"] = { color = "muted-foreground" },
		[".pl-side-heading"] = { color = "muted-foreground", padding = { 18, 8, 8, 8 } },
		[".pl-side-first"] = { padding = { top = 2 } },
		[".pl-side-gap"] = { height = 14 },
		[".pl-side-count"] = { background = "#2f3641", radius = 7, padding = { 2, 8 } },
		[".pl-side-count-text"] = { color = "accent", size = "small", weight = "medium" },
		[".pl-dot-outer"] = { background = "#4aa8f5", radius = 9 },
		[".pl-dot-ring"] = { background = "#ffffff", radius = 4 },
		[".pl-dot-core"] = { background = "#2b7fe0", radius = 2 },
	},

	init = function(previous: State?): State
		return previous or { open = { ["p-moonframe"] = true, ["p-ledger"] = true, ["p-notes"] = true }, more = {} }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.fold then
			state.open[msg.fold] = if state.open[msg.fold] then nil else true
		elseif msg.more then
			state.more[msg.more] = if state.more[msg.more] then nil else true
		end
	end,

	view = function(state: State, app)
		local chats = app.chats
		local current = chats and chats.current
		local rows = {}

		local function folder(group: any)
			local open = state.open[group.id] == true
			table.insert(rows, ui.row {
				class = "pl-side-row",
				align = "center",
				msg = { fold = group.id },
				ui.icon { name = if open then "folder-open" else "folder", class = "pl-side-icon" },
				ui.text { group.title, class = "pl-side-label", truncate = true },
			})
			if open then
				local all = state.more[group.id] == true
				for index, item in group.chats do
					if index > SHOWN and not all then
						break
					end
					table.insert(rows, chat_row(item, current, indent()))
				end
				if #group.chats > SHOWN then
					table.insert(rows, ui.row {
						class = "pl-side-row",
						align = "center",
						msg = { more = group.id },
						indent(),
						ui.text { if all then "Show less" else "Show more", class = "pl-side-more" },
					})
				end
			end
		end

		if chats then
			table.insert(rows, ui.text { "Pinned", class = "pl-side-heading pl-side-first" })
			for _, item in chats.pinned do
				table.insert(rows, chat_row(item, current, ui.icon { name = "message-square", class = "pl-side-icon" }))
			end
			for _, group in chats.folders do
				folder(group)
			end
			table.insert(rows, ui.text { "Projects", class = "pl-side-heading" })
			for index, group in chats.projects do
				folder(group)
				-- An open project with chats keeps a little room after it.
				if state.open[group.id] and #group.chats > 0 and index < #chats.projects then
					table.insert(rows, ui.row { class = "pl-side-gap" })
				end
			end
			table.insert(rows, ui.text { "Recents", class = "pl-side-heading" })
			for _, item in chats.recents do
				table.insert(rows, chat_row(item, current, nil))
			end
		end

		return ui.row {
			class = "pl-side",
			grow = true,
			gap = "none",
			ui.column {
				grow = true,
				gap = "none",
				ui.row {
					class = "pl-side-head",
					height = 54,
					shrink = false,
					align = "center",
					ui.row {
						class = "pl-side-brand",
						align = "center",
						tooltip = "Scripted demo: no model is connected",
						ui.text { "Parlance", class = "pl-side-name" },
						ui.icon { name = "chevron-down", size = "small" },
					},
					ui.spacer(),
					ui.icon { name = "bell", tooltip = "Notifications" },
					ui.icon { name = "search", tooltip = "Search chats" },
				},
				ui.column {
					class = "pl-side-body",
					shrink = false,
					ui.row {
						class = "pl-side-row",
						align = "center",
						msg = { new = true },
						to = "provider:chats",
						ui.icon { name = "pencil", class = "pl-side-icon" },
						ui.text { "New chat", class = "pl-side-label" },
					},
					ui.row {
						class = "pl-side-row",
						align = "center",
						ui.row {
							class = "pl-dot-outer",
							width = 18,
							height = 18,
							align = "center",
							justify = "center",
							ui.row {
								class = "pl-dot-ring",
								width = 8,
								height = 8,
								align = "center",
								justify = "center",
								ui.row { class = "pl-dot-core", width = 4, height = 4 },
							},
						},
						ui.text { "Your dot", class = "pl-side-label" },
						ui.spacer(),
						chats and chats.dot > 0 and ui.row {
							class = "pl-side-count",
							ui.text { tostring(chats.dot), class = "pl-side-count-text" },
						},
					},
				},
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.column { class = "pl-side-body", shrink = false, table.unpack(rows) },
				},
			},
			ui.row { class = "pl-side-focus" },
		}
	end,
}
