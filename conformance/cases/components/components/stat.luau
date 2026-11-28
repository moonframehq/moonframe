-- A labelled number.
return component {
	title = "Stat",
	-- What Moonframe's component browser previews it with.
	example = { label = "Open", value = 12 },
	build = function(props)
		return ui.column {
			class = "stat",
			gap = "small",
			ui.text { props.label, class = "stat-label", tone = "muted", size = "small" },
			ui.text { tostring(props.value), weight = "bold", size = "title" },
		}
	end,
}
