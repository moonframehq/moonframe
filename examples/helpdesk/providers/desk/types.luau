-- What this provider publishes, as `app.desk`, and what it takes from
-- `send "provider:desk"`. `just types examples` checks the app's mods
-- against them; published.json is an example of the top level that
-- `just provider-types-check` and this crate's tests hold both sides to.

export type Published = {
	-- True once it publishes.
	available: boolean,
	assistant: string,
	workspace: { team: string, company: string },
	me: { name: string, email: string },
	week: Week,
	-- Newest first.
	tickets: { Ticket },
	counts: { all: number, open: number, mine: number, waiting: number },
}

export type Week = {
	total: number,
	-- Percent the assistant resolved.
	share: number,
	first_reply: string,
	split: { { id: string, label: string, count: number } },
}

-- As in sample.json; the nested records are left `any`.
export type Ticket = {
	id: string,
	kind: string,
	priority: string,
	age: string,
	minutes: number,
	title: string,
	requester: any,
	source: string,
	-- "open", "waiting" or "resolved".
	status: string,
	status_label: string,
	mine: boolean,
	created: string,
	device: any,
	summary: string,
	sla: { any },
	watchers: { string },
	thread: { any },
	check: any?,
	earlier: { any }?,
}

-- One key each.
export type Message =
	-- A ticket is on screen: a check step still running finishes a few
	-- seconds later.
	{ watch: string }
	-- Run the check's suggestion (its primary button).
	| { act: string }
	-- Add a reply, or an internal note, to the ticket's thread.
	| { reply: { id: string, text: string, internal: boolean? } }
	-- Open a blank ticket at the top of the inbox.
	| { new: boolean }
