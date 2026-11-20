-- What this provider publishes, as `app.focus`, and what it takes from
-- `send "provider:focus"`. published.json is an example of the top level
-- that `just provider-types-check` and tests.rs hold both sides to.

export type Published = {
	-- Seconds left of a session `length` seconds long.
	left: number,
	length: number,
	-- `left` as "mm:ss".
	clock: string,
	running: boolean,
	-- Sessions finished.
	sessions: number,
	-- Which build publishes it: "wasm" or "native".
	build: string,
}

-- "status" only asks for the timer again; `minutes` (1 to 240) sets a new
-- length and stops it.
export type Message = "start" | "pause" | "reset" | "status" | { minutes: number }
