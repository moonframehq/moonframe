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
for _, kind in { "row", "column", "panel", "hsplit", "vsplit", "pane", "divider", "spacer", "popover", "overlay", "input", "line", "hatch" } do
	ui[kind] = element(kind)
end
ui.text = element("text", "text")
ui.badge = element("badge", "text")
ui.icon = element("icon", "name")
ui.image = element("image", "src")
ui.terminal = element("terminal", "id")
ui.browser = element("browser", "url")
ui.button = element("button", "label")
ui.item = element("item", "label")
ui.region = element("region", "name")
ui.mount = element("mount", "mod")

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
