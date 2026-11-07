// `app.pi`: everything the window draws, in one small plain value. A process
// provider's line is capped at 1 MiB and 10,000 values, so long lists stop
// short and a long conversation shows its end, with more on request.
import { shortPath } from "./paths.mjs";
import { listExtensions, listProviders, listSkills } from "./resources.mjs";
import { describe } from "./schedule.mjs";
import { contextUse, timeline, totalCost } from "./timeline.mjs";

const NO_PROJECT = "No project";

function workspaceName(engine, path) {
	if (path === engine.chats) return NO_PROJECT;
	return engine.data.workspaces.find((item) => item.path === path)?.name ?? path.split("/").pop();
}

const MAX_THREADS = 120;
const MAX_MODELS = 160;
const MAX_COMMANDS = 80;
const SHOWN_ITEMS = 160;
const TEXT_BUDGET = 500_000;
// The host ends a provider whose line passes 1 MiB or 10,000 values.
const MAX_BYTES = 1_000_000;
const MAX_VALUES = 9_000;
const LONGEST_TEXT = 100_000;

function threadStatus(engine, meta) {
	const live = engine.live.get(meta.id);
	if (live?.dialogs.length) return "waiting";
	if (live?.running || live?.sending) return "running";
	if (meta.failed) return "failed";
	return "idle";
}

function liveLine(engine, id) {
	const live = engine.live.get(id);
	if (!live?.running) return null;
	const list = engine.entries.get(id) ?? [];
	for (let i = list.length - 1; i >= 0; i--) {
		const block = list[i].message?.content?.findLast?.((part) => part.type === "toolCall");
		if (block) return `${block.name} ${block.arguments?.path ?? block.arguments?.command ?? ""}`.trim().slice(0, 80);
	}
	return "Working…";
}

function threadRows(engine) {
	const rows = [];
	for (const meta of Object.values(engine.data.threads)) {
		const info = engine.summaries.get(meta.id);
		const status = threadStatus(engine, meta);
		// A thread with only `!` output has no file yet, so no summary.
		const empty = !info && !engine.entries.get(meta.id)?.length;
		if (empty && status === "idle" && meta.id !== engine.data.selected) continue;
		rows.push({
			id: meta.id,
			title: engine.titleOf(meta.id),
			workspace: workspaceName(engine, meta.workspace),
			path: meta.workspace,
			branch: meta.worktree?.branch ?? null,
			status,
			unread: meta.unread === true,
			pinned: meta.pinned === true,
			archived: meta.archived === true,
			updated: Math.round(info?.updated ?? meta.created ?? 0),
			preview: liveLine(engine, meta.id) ?? info?.preview ?? "",
		});
	}
	rows.sort((a, b) => b.updated - a.updated);
	return rows.slice(0, MAX_THREADS);
}

function modelOf(engine, meta, live) {
	const summary = engine.summaries.get(meta.id);
	const known = live?.state?.model ?? meta.model ?? summary?.model ?? engine.data.prefs.last_model;
	if (!known) return null;
	const full = engine.catalogue.models.find((model) => model.provider === known.provider && model.id === known.id);
	return {
		provider: known.provider,
		id: known.id,
		name: known.name ?? full?.name ?? known.id,
		window: known.contextWindow ?? full?.contextWindow ?? null,
		reasoning: known.reasoning ?? full?.reasoning ?? false,
		images: (known.input ?? full?.input ?? []).includes("image"),
	};
}

// The end of the conversation, as much as fits, whole items only.
function visibleItems(items, limit) {
	let text = 0;
	let first = items.length;
	while (first > 0 && items.length - first < limit) {
		const item = items[first - 1];
		text += Buffer.byteLength(item.text ?? "") + Buffer.byteLength(item.output ?? "") + Buffer.byteLength(item.patch ?? "");
		if (text > TEXT_BUDGET && first < items.length) break;
		first--;
	}
	// Tool arguments are for the titles; the window has no use for them.
	return { earlier: first, items: items.slice(first).map(({ args, ...item }) => item) };
}

function threadDetail(engine, id, now) {
	const meta = engine.data.threads[id];
	if (!meta) return null;
	const live = engine.live.get(id);
	const entries = engine.entries.get(id) ?? [];
	const items = timeline(entries, {
		cwd: meta.cwd,
		live,
		turns: meta.turns,
		expanded: engine.expanded.get(id),
		now,
	});
	if (live?.sending) items.push({ k: "user", id: "sending", text: live.sending });
	const limit = engine.shown.get(id) ?? SHOWN_ITEMS;
	const model = modelOf(engine, meta, live);
	const branch = engine.branches.get(meta.cwd);
	return {
		id,
		title: engine.titleOf(id),
		workspace: workspaceName(engine, meta.workspace),
		path: meta.workspace,
		cwd: meta.cwd,
		branch: meta.worktree?.branch ?? branch?.branch ?? null,
		worktree: meta.worktree != null,
		git: meta.workspace !== engine.chats && (branch?.git ?? true),
		status: threadStatus(engine, meta),
		updated: Math.round(engine.summaries.get(id)?.updated ?? meta.created ?? now),
		ready: live?.state != null,
		running: live?.running === true,
		compacting: live?.compacting === true,
		pinned: meta.pinned === true,
		model,
		thinking: live?.state?.thinkingLevel ?? meta.thinking ?? engine.summaries.get(id)?.thinking ?? engine.data.prefs.last_thinking ?? "medium",
		levels: live?.levels ?? [],
		context: contextUse(entries, model?.window),
		cost: totalCost(entries),
		retry: live?.retry ?? null,
		queued: live?.queued ?? { steering: [], followUp: [] },
		dialog: live?.dialogs[0] ?? null,
		statuses: Object.values(live?.statuses ?? {}),
		widgets: Object.values(live?.widgets ?? {}).flat().slice(0, 20),
		editor_text: live?.editorText ?? null,
		...visibleItems(items, limit),
	};
}

function countValues(value) {
	if (typeof value !== "object" || value === null) return 1;
	let total = 1;
	for (const item of Object.values(value)) total += countValues(item);
	return total;
}

const fits = (line, snapshot) => Buffer.byteLength(line) <= MAX_BYTES && countValues(snapshot) <= MAX_VALUES;

// The last resort when nothing above was enough: every list and text cut
// down, every key kept, since views read them. Tighter until it fits.
function trimmed(value, limit, depth = 0) {
	if (typeof value === "string") return value.length > limit * 100 ? `${value.slice(0, limit * 100)}…` : value;
	if (typeof value !== "object" || value === null) return value;
	if (depth > 12) return null;
	if (Array.isArray(value)) return value.slice(0, limit).map((item) => trimmed(item, limit, depth + 1));
	return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, trimmed(item, limit, depth + 1)]));
}

function shortened(item) {
	const cut = {};
	for (const [key, value] of Object.entries(item)) {
		cut[key] = typeof value === "string" && value.length > LONGEST_TEXT ? `${value.slice(0, LONGEST_TEXT)}\n… cut, too long to show` : value;
	}
	return cut;
}

// The line for a snapshot, one the host takes: what it can't hold of the
// conversation goes, the oldest first, as older entries do when they scroll off.
export function encode(snapshot) {
	let line = JSON.stringify(snapshot);
	const thread = snapshot.thread;
	while (thread && !fits(line, snapshot) && thread.items.length > 1) {
		const dropped = Math.ceil(thread.items.length / 4);
		thread.items = thread.items.slice(dropped);
		thread.earlier += dropped;
		line = JSON.stringify(snapshot);
	}
	if (thread && !fits(line, snapshot)) {
		thread.items = thread.items.map(shortened);
		line = JSON.stringify(snapshot);
	}
	if (thread && !fits(line, snapshot)) {
		thread.earlier += thread.items.length;
		thread.items = [];
		line = JSON.stringify(snapshot);
	}
	let view = snapshot;
	for (const limit of [50, 10, 2]) {
		if (fits(line, view)) break;
		view = trimmed(view, limit);
		line = JSON.stringify(view);
	}
	return line;
}

function schedules(engine) {
	return engine.data.schedules.map(({ id, name, prompt, workspace, cadence, time, weekday, enabled, worktree, next, last_run, last_thread }) => ({
		id,
		name,
		prompt,
		workspace,
		workspace_name: workspace ? workspaceName(engine, workspace) : "",
		cadence,
		time,
		weekday: weekday ?? 1,
		enabled,
		worktree: worktree === true,
		next: next ? Math.round(next) : null,
		last_run: last_run ? Math.round(last_run) : null,
		last_thread: last_thread ?? null,
		when: describe({ cadence, time, weekday }),
	}));
}

function commandsFor(engine, selected) {
	const live = engine.live.get(selected);
	const list = live?.commands.length ? live.commands : engine.catalogue.commands;
	return list.slice(0, MAX_COMMANDS).map(({ name, description, source }) => ({
		name,
		description: (description ?? "").slice(0, 120),
		source,
	}));
}

// Reading skills means touching the disk, so it happens when asked for or
// when pi's configuration changes, not on each streamed word.
function resources(engine) {
	const stamp = `${engine.stamp}|${engine.resourcesAsked ?? 0}|${engine.data.selected}`;
	if (engine.resources?.stamp !== stamp) {
		const cwd = engine.data.threads[engine.data.selected]?.cwd ?? engine.data.workspaces[0]?.path;
		let found;
		try {
			found = { skills: listSkills(cwd), extensions: listExtensions(cwd), providers: listProviders(), error: null };
		} catch (error) {
			found = { skills: [], extensions: [], providers: { known: [], endpoints: [] }, error: error.message };
		}
		engine.resources = { stamp, found };
	}
	return engine.resources.found;
}

// A short mark that changes when an item does, so a view can keep what it
// built for all the others while one streams.
function itemVersion(item) {
	return `${item.text?.length ?? 0}.${item.status ?? ""}.${item.open ?? ""}.${item.output?.length ?? 0}.${item.label ?? ""}.${item.summary ?? ""}.${item.count ?? ""}.${item.streaming ?? ""}`;
}

function startOfDay() {
	const day = new Date();
	day.setHours(0, 0, 0, 0);
	return Math.floor(day.getTime() / 1000);
}

// Rows change far less often than a conversation streams.
function versioned(engine, rows) {
	const text = JSON.stringify(rows);
	if (text !== engine.rowsText) {
		engine.rowsText = text;
		engine.rowsVersion = (engine.rowsVersion ?? 0) + 1;
	}
	return engine.rowsVersion;
}

export function snapshotOf(engine) {
	const now = Math.floor(Date.now() / 1000);
	const selected = engine.data.selected;
	const found = resources(engine);
	const threads = threadRows(engine);
	const thread = selected ? threadDetail(engine, selected, now) : null;
	for (const item of thread?.items ?? []) item.v = itemVersion(item);
	return {
		available: true,
		now,
		day_start: startOfDay(),
		threads_version: versioned(engine, threads),
		runtime: { ...engine.pi, path: engine.pi.path && shortPath(engine.pi.path) },
		prefs: engine.data.prefs,
		chats: { path: engine.chats, name: NO_PROJECT, short: shortPath(engine.chats), git: false },
		workspaces: engine.data.workspaces.map((workspace) => ({
			...workspace,
			short: shortPath(workspace.path),
			branch: engine.branches.get(workspace.path)?.branch ?? null,
			git: engine.branches.get(workspace.path)?.git ?? false,
		})),
		threads,
		selected,
		thread,
		prefill: engine.prefill ?? null,
		models: engine.catalogue.models.slice(0, MAX_MODELS).map(({ provider, id, name, reasoning }) => ({
			provider,
			id,
			name,
			reasoning: reasoning === true,
		})),
		commands: commandsFor(engine, selected),
		skills: found.skills,
		extensions: found.extensions,
		providers: found.providers,
		config_error: found.error,
		schedules: schedules(engine),
		events: engine.events,
	};
}
