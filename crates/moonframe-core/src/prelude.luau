-- The `ui` library and `mod`, set up in every module's state before it is
-- sealed, so a module can call them but not change them. Each `ui` function
-- tags a table with its element type; the host checks the rest.

-- The line of the module that is calling, which the host puts here.
local caller_line = __caller_line

local function element(kind: string, primary: string?)
	return function(props: any)
		if props == nil then
			props = {}
		elseif type(props) == "string" and primary then
			props = { [primary] = props }
		elseif type(props) ~= "table" then
			error(`ui.{kind} takes a table`, 2)
		end
		-- `ui.text { "hi", tone = "muted" }` reads as `text = "hi"`.
		if primary and props[1] ~= nil and props[primary] == nil then
			props[primary] = props[1]
			props[1] = nil
		end
		props.type = kind
		props.__line = caller_line()
		return props
	end
end

local ui = {}
for _, kind in { "row", "column", "panel", "hsplit", "vsplit", "pane", "divider", "spacer", "popover", "overlay", "input", "line", "paths", "hatch", "map", "list" } do
	ui[kind] = element(kind)
end
ui.text = element("text", "text")
ui.badge = element("badge", "text")
ui.icon = element("icon", "name")
ui.image = element("image", "src")
ui.terminal = element("terminal", "id")
ui.browser = element("browser", "url")
ui.video = element("video", "source")
ui.markdown = element("markdown", "text")
ui.view = element("view", "name")
ui.button = element("button", "label")
ui.item = element("item", "label")
ui.region = element("region", "name")
ui.mount = element("mount", "mod")

-- `ui.memo(key, deps, build)`: what `build()` returned the last time this
-- view asked for `key` with equal `deps`, or `build()` now. Dependencies
-- compare with `==`, so a table counts as the same while it is the same
-- table, even if changed inside.
local memos = {}
local memo_count = 0
local MAX_MEMOS = 512

local function same(a: { any }, b: { any }): boolean
	if #a ~= #b then
		return false
	end
	for index = 1, #a do
		if a[index] ~= b[index] then
			return false
		end
	end
	return true
end

function ui.memo(key: string, deps: { any }, build: () -> any)
	if type(key) ~= "string" then
		error("ui.memo takes a key (a string), its dependencies (a list), and a function", 2)
	end
	if type(deps) ~= "table" or type(build) ~= "function" then
		error("ui.memo takes a key, its dependencies (a list), and a function", 2)
	end
	local kept = memos[key]
	if kept and same(kept.deps, deps) then
		return kept.tree
	end
	local tree = build()
	if not kept then
		-- Keys made from data could grow without end: start over instead.
		if memo_count >= MAX_MEMOS then
			memos = {}
			memo_count = 0
		end
		memo_count += 1
	end
	memos[key] = { deps = table.clone(deps), tree = tree }
	return tree
end

local function define(definition)
	if type(definition) ~= "table" then
		error("mod takes a table", 2)
	end
	if type(definition.view) ~= "function" then
		error("mod needs a `view` function", 2)
	end
	return definition
end

local function stylesheet(sheet)
	if type(sheet) ~= "table" then
		error("style takes a table", 2)
	end
	return sheet
end

local function component(definition)
	if type(definition) ~= "table" then
		error("component takes a table", 2)
	end
	if type(definition.build) ~= "function" then
		error("component needs a `build` function", 2)
	end
	return definition
end

return ui, define, stylesheet, component
