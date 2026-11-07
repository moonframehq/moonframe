// A conversation as what the window shows: a flat list of items, each small
// and plain, from pi's messages. A run is what follows a prompt; its tool
// rows and thinking fold away under one line once it is over.
import { isAbsolute, relative } from "node:path";

const OUTPUT_LIMIT = 24_000;
const SHOWN_COMMAND = 140;

function shortened(text, limit) {
	const line = String(text ?? "").replace(/\s+/g, " ").trim();
	return line.length > limit ? `${line.slice(0, limit - 1)}…` : line;
}

function pathIn(cwd, path) {
	if (!path) return "";
	if (!isAbsolute(path)) return path;
	const inside = relative(cwd, path);
	return inside.startsWith("..") ? path : inside;
}

function titleOf(name, args, cwd, status) {
	const path = pathIn(cwd, args?.path ?? args?.file_path);
	const running = status === "running";
	switch (name) {
		case "read":
			return `${running ? "Reading" : "Read"} ${path}`;
		case "edit":
			return `${running ? "Editing" : "Edited"} ${path}`;
		case "write":
			return `${running ? "Writing" : "Wrote"} ${path}`;
		case "bash":
			return `${running ? "Running" : "Ran"} ${shortened(args?.command, SHOWN_COMMAND)}`;
		case "grep":
			return `Searched for ${shortened(args?.pattern, 60)}`;
		case "find":
			return `Found ${shortened(args?.pattern, 60)}`;
		case "ls":
			return `Listed ${path || "."}`;
		default:
			return `${name}${args && Object.keys(args).length ? ` ${shortened(JSON.stringify(args), 80)}` : ""}`;
	}
}

// Lines added and removed, from a unified patch pi's edit gives.
function countPatch(patch) {
	let added = 0;
	let deleted = 0;
	for (const line of String(patch).split("\n")) {
		if (line.startsWith("+") && !line.startsWith("+++")) added++;
		else if (line.startsWith("-") && !line.startsWith("---")) deleted++;
	}
	return { added, deleted };
}

function textOf(content) {
	if (typeof content === "string") return content;
	return (content ?? [])
		.filter((block) => block.type === "text")
		.map((block) => block.text)
		.join("\n");
}

function imagesIn(content) {
	return Array.isArray(content) ? content.filter((block) => block.type === "image").length : 0;
}

function plural(count, one, many = `${one}s`) {
	return `${count} ${count === 1 ? one : many}`;
}

function runSummary(tools) {
	const count = (names) => tools.filter((tool) => names.includes(tool.tool)).length;
	const parts = [];
	const read = count(["read", "grep", "find", "ls"]);
	const edited = count(["edit", "write"]);
	const ran = count(["bash"]);
	const other = tools.length - read - edited - ran;
	if (read) parts.push(`explored ${plural(read, "file")}`);
	if (edited) parts.push(`edited ${plural(edited, "file")}`);
	if (ran) parts.push(`ran ${plural(ran, "command")}`);
	if (other) parts.push(`used ${plural(other, "tool")}`);
	const text = parts.join(", ");
	return text ? text[0].toUpperCase() + text.slice(1) : "";
}

export function duration(seconds) {
	const whole = Math.max(1, Math.round(seconds));
	if (whole < 60) return `${whole}s`;
	const minutes = Math.floor(whole / 60);
	if (minutes < 60) return `${minutes}m ${whole % 60}s`;
	return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

// `entries` are `{ id?, message }` or `{ id, note, tone }`, oldest first.
// `live` is what is happening now: the message streaming in, the tools
// running, whether a run is going. `turns` are the snapshots of finished
// runs by their prompt's timestamp; `expanded` the tool rows showing output.
export function timeline(entries, { cwd, live, turns = {}, expanded = new Set(), now = Date.now() / 1000 }) {
	const items = [];
	const tools = new Map();
	let run = null;

	const closeRun = (ended) => {
		if (!run) return;
		const rowTools = items.filter((item) => item.k === "tool" && item.run === run.id);
		const header = items[run.header];
		header.summary = runSummary(rowTools);
		header.steps = rowTools.length;
		const end = ended ?? run.last;
		header.label = `Worked for ${duration(end - run.started)}`;
		// The last words stay when the steps fold away.
		for (let i = items.length - 1; i > run.header; i--) {
			if (items[i].k === "text") {
				items[i].answer = true;
				break;
			}
		}
		const turn = turns[run.key];
		if (turn?.files?.length) {
			items.push({
				k: "changes",
				id: `${run.id}-changes`,
				run: run.id,
				files: turn.files.slice(0, 12),
				count: turn.files.length,
				added: turn.added,
				deleted: turn.deleted,
				from: turn.before,
				to: turn.after,
			});
		}
		run = null;
	};

	const openRun = (id, key, started) => {
		run = { id: `run-${id}`, key, started, last: started, header: items.length };
		items.push({ k: "run", id: run.id, label: "", summary: "", steps: 0, running: false });
	};

	const addAssistant = (message, id, streaming) => {
		if (!run) openRun(id, null, (message.timestamp ?? now * 1000) / 1000);
		if (message.timestamp) run.last = message.timestamp / 1000;
		(message.content ?? []).forEach((block, index) => {
			const key = `${id}-${index}`;
			if (block.type === "text" && block.text) {
				items.push({ k: "text", id: key, run: run.id, text: block.text, streaming: streaming || undefined });
			} else if (block.type === "thinking" && block.thinking?.trim()) {
				items.push({ k: "think", id: key, run: run.id, text: block.thinking, streaming: streaming || undefined });
			} else if (block.type === "toolCall") {
				const status = "running";
				const item = {
					k: "tool",
					id: block.id,
					run: run.id,
					tool: block.name,
					title: titleOf(block.name, block.arguments, cwd, status),
					args: block.arguments,
					status,
					open: expanded.has(block.id) || undefined,
				};
				tools.set(block.id, item);
				items.push(item);
			}
		});
		if (message.stopReason === "error" && message.errorMessage) {
			items.push({ k: "error", id: `${id}-error`, run: run.id, text: message.errorMessage });
		} else if (message.stopReason === "aborted") {
			items.push({ k: "note", id: `${id}-stopped`, run: run.id, text: "Stopped", tone: "muted" });
		}
	};

	const addResult = (message) => {
		const item = tools.get(message.toolCallId);
		if (!item) return;
		finishTool(item, message.content, message.details, message.isError);
		if (message.timestamp && run) run.last = message.timestamp / 1000;
	};

	const finishTool = (item, content, details, isError) => {
		item.status = isError ? "error" : "done";
		item.title = titleOf(item.tool, item.args, cwd, item.status);
		if (details?.patch) Object.assign(item, countPatch(details.patch));
		else if (item.tool === "write" && item.args?.content != null) {
			item.added = String(item.args.content).split("\n").length;
			item.deleted = 0;
		}
		if (item.open) {
			item.output = textOf(content).slice(0, OUTPUT_LIMIT);
			if (details?.patch) item.patch = String(details.patch).slice(0, OUTPUT_LIMIT);
		}
	};

	for (const entry of entries) {
		if (entry.note) {
			items.push({ k: "note", id: entry.id, text: entry.note, tone: entry.tone ?? "muted" });
			continue;
		}
		const message = entry.message;
		if (message.role === "user") {
			closeRun();
			const id = entry.id ?? `u${message.timestamp}`;
			items.push({
				k: "user",
				id,
				entry: entry.id,
				text: textOf(message.content),
				images: imagesIn(message.content) || undefined,
			});
			openRun(id, String(message.timestamp), message.timestamp / 1000);
		} else if (message.role === "assistant") {
			addAssistant(message, entry.id ?? `a${message.timestamp}`, false);
		} else if (message.role === "toolResult") {
			addResult(message);
		} else if (message.role === "bashExecution") {
			closeRun();
			items.push({
				k: "bash",
				id: entry.id ?? `b${message.timestamp}`,
				command: message.command,
				code: message.exitCode ?? null,
				output: String(message.output ?? "").slice(-OUTPUT_LIMIT),
			});
		} else if (message.role === "custom" && message.display) {
			items.push({ k: "note", id: entry.id, text: textOf(message.content), tone: "muted" });
		}
	}

	if (live?.partial) addAssistant(live.partial, "live", true);
	for (const [id, state] of live?.tools ?? []) {
		const item = tools.get(id);
		if (!item) continue;
		if (state.done) finishTool(item, state.content, state.details, state.isError);
		else if (item.open && state.content) item.output = textOf(state.content).slice(-OUTPUT_LIMIT);
	}

	if (run && live?.running) {
		const header = items[run.header];
		const rowTools = items.filter((item) => item.k === "tool" && item.run === run.id);
		header.running = true;
		header.label = `Working for ${duration(now - run.started)}`;
		header.summary = runSummary(rowTools);
		header.steps = rowTools.length;
	} else {
		closeRun();
	}

	// A run with no steps needs no line of its own.
	return items.filter((item) => item.k !== "run" || item.steps > 0 || item.running);
}

// What the context holds, from the last answer's usage.
export function contextUse(entries, window) {
	for (let i = entries.length - 1; i >= 0; i--) {
		const message = entries[i].message;
		if (message?.role !== "assistant" || !message.usage) continue;
		const { input = 0, output = 0, cacheRead = 0, cacheWrite = 0 } = message.usage;
		const tokens = input + output + cacheRead + cacheWrite;
		if (!tokens) continue;
		return { tokens, window: window ?? null, used: window ? Math.min(1, tokens / window) : null };
	}
	return null;
}

export function totalCost(entries) {
	let cost = 0;
	for (const { message } of entries) {
		if (message?.role === "assistant") cost += message.usage?.cost?.total ?? 0;
	}
	return Math.round(cost * 10000) / 10000;
}
