-- A country's two-letter code in a small mono chip, where a flag would go.

type Props = { code: string }

return component {
	title = "Country Code",
	category = "Data",
	icon = "globe",
	description = "A country's ISO code (\"CA\") in a small bordered mono chip.",
	when = "Beside a country's name in lists of locations or settings, in place of a flag (no flag images ship). For any short label use `ui.badge` or `pill`.",
	props = { ["code"] = "the two-letter code" },
	styles = {
		[".country-code"] = { radius = 5, border = true, border_color = "border", padding = { 1, 5 }, background = "muted-foreground/6" },
		[".country-code-text"] = { size = "small", font = "mono", color = "muted-foreground" },
	},
	example = { code = "CA" },

	build = function(props: Props)
		return ui.row {
			class = "country-code",
			width = 30,
			justify = "center",
			shrink = false,
			ui.text { string.upper(props.code), class = "country-code-text" },
		}
	end,
}
