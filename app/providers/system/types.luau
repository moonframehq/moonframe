-- What this provider publishes, as `app.system` (`just provider-types`
-- copies it into types/moonframe.d.luau). published.json is an example the
-- check and the host's tests hold it to.

-- Nil until the first sample arrives.
export type Published = {
	-- Fractions from 0 to 1.
	cpu: number,
	-- Oldest first, ending with `cpu`.
	cpu_history: { number },
	memory: number,
	memory_used: number,
	memory_total: number,
	-- Bytes per second.
	received: number,
	transmitted: number,
}

-- `send "provider:system" "clear"` empties the sparkline.
export type Message = "clear"
