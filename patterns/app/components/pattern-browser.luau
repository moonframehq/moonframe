-- Browser chrome is tooling, not a pattern to ship in an application.
local code_view = use "code-view"
local segmented = use "segmented"

local function matches(entry: any, category: string, query: string): boolean
	local haystack = string.lower(entry.title .. " " .. entry.category .. " " .. entry.description .. " " .. table.concat(entry.parts, " "))
	return (category == "All patterns" or entry.category == category) and string.find(haystack, string.lower(query), 1, true) ~= nil
end

return component {
	title = "Pattern browser",
	category = "Tooling",
	icon = "layout-dashboard",
	description = "Searchable, independent workspace for exploring UI patterns and their source.",
	when = "The standalone pattern showcase. Copy a pattern's source into your app rather than the browser itself.",
	props = { ["entries"] = "Generated catalog.", ["state"] = "Browser navigation and search state." },
	example = {},
	build = function(props: any)
		local state = props.state or { category = "All patterns", query = "", code = "example" }
		local entries = props.entries or {}
		local categories = { "All patterns" }
		local known = {}
		local shown = {}
		local current = nil
		for _, entry in entries do
			if not known[entry.category] then table.insert(categories, entry.category); known[entry.category] = true end
			if matches(entry, state.category, state.query) then table.insert(shown, entry) end
			if entry.id == state.selected then current = entry end
		end
		local nav = {}
		for _, category in categories do
			table.insert(nav, ui.row { class = "patterns-nav", selected = state.category == category, msg = { category = category }, ui.text(category) })
		end
		local body
		if current then
			local source = if state.code == "source" then current.source else current.example
			local wide = current.id == "workspace-shell"
			local function layout(children: any)
				return if wide then ui.column(children) else ui.row(children)
			end
			body = ui.column { class = "patterns-page", grow = true, scroll = true,
				ui.row { align = "center", ui.button { "All patterns", icon = "arrow-left", msg = "back", variant = "ghost" }, ui.spacer(), ui.text { current.category, tone = "muted" } },
				ui.text { current.title, class = "patterns-heading" }, ui.text { current.description, tone = "muted" },
				layout { gap = "large", shrink = false,
					ui.column { grow = not wide, gap = "normal", width = if wide then nil else 420,
						ui.text { "Live preview · sample data", size = "small", tone = "muted" },
						ui.column { class = "patterns-preview", ui.mount { `preview-{current.id}`, id = "detail" } },
					},
					ui.column { grow = not wide, gap = "normal", width = if wide then nil else 380,
						segmented { selected = state.code, event = "code", options = { { id = "example", label = "Example" }, { id = "source", label = "Pattern source" } } },
						code_view { text = source, height = 420 },
						ui.row { justify = "end", ui.button { "Copy code", icon = "copy", copy = source, small = true } },
					},
				},
				ui.text { "Why it works", weight = "medium" }, ui.text { current.when, tone = "muted" },
				ui.text { "Built from: " .. table.concat(current.parts, ", "), size = "small", tone = "muted" },
				ui.text { `Copy patterns/app/components/{current.id}.luau to your app's components/ folder.`, size = "small", tone = "muted" },
			}
		else
			local cards = {}
			for _, entry in shown do
				table.insert(cards, ui.column { class = "patterns-tile", width = 350, shrink = false, gap = "normal",
					ui.row { gap = "small", align = "center", ui.icon { name = entry.icon }, ui.text { entry.category, tone = "muted", size = "small" } },
					ui.text { entry.title, size = "large", weight = "medium" }, ui.text { entry.description, tone = "muted", size = "small" },
					ui.button { "Explore pattern", icon = "arrow-right", msg = { select = entry.id }, variant = "outline" },
				})
			end
			body = ui.column { class = "patterns-page", grow = true, scroll = true,
				ui.text { "Good UI, already composed.", class = "patterns-heading" },
				ui.text { "Thoughtful starting points. Real Luau. Yours to change.", tone = "muted" },
				ui.text { `{#shown} patterns`, tone = "muted", size = "small" },
				ui.row { gap = "large", wrap = true, align = "start", shrink = false, table.unpack(cards) },
				#shown == 0 and ui.text { "No patterns match. Try a different search or category.", tone = "muted" },
			}
		end
		return ui.row { grow = true, gap = "none",
			ui.column { class = "patterns-sidebar", width = 220,
				ui.text { "moonframe", weight = "semibold", size = "large" }, ui.text { "PATTERNS", tone = "muted", size = "small" },
				ui.input { id = "pattern-search", value = state.query, placeholder = "Search patterns…", change = "query" },
				ui.column { gap = "small", table.unpack(nav) }, ui.spacer(),
				ui.text { "A companion to your app.\nThe preview owns its own state.", tone = "muted", size = "small" },
			},
			body,
		}
	end,
}
