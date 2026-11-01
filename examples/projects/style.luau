-- Colours the `code` component paints with, for light and dark, merged into
-- the app's stylesheet. An app's own style.luau can redefine any of them.
return style {
	colors = {
		["code-keyword"] = { light = "#c4314b", dark = "#ff7b72" },
		["code-type"] = { light = "#1f5fbf", dark = "#79c0ff" },
		["code-function"] = { light = "#6f42c1", dark = "#d2a8ff" },
		["code-string"] = { light = "#a3401e", dark = "#a5d6ff" },
		["code-number"] = { light = "#0b7a75", dark = "#79c0ff" },
		["code-comment"] = { light = "#6e7781", dark = "#8b949e" },
	},
}
