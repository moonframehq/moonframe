// Reading pi's session files: JSONL entries in a tree by parentId, the last
// entry being where the conversation stands. These files are the truth;
// nothing here writes them.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { sessionDir } from "./paths.mjs";

const summaries = new Map();

export function parse(file) {
	const entries = [];
	for (const line of readFileSync(file, "utf8").split("\n")) {
		if (!line) continue;
		try {
			entries.push(JSON.parse(line));
		} catch {
			// A line still being written.
		}
	}
	return entries;
}

function textOf(content) {
	if (typeof content === "string") return content;
	return (content ?? [])
		.filter((block) => block.type === "text")
		.map((block) => block.text)
		.join("\n");
}

// The entries from the root to where the session stands, oldest first.
export function activeBranch(entries) {
	const byId = new Map();
	let leaf = null;
	for (const entry of entries) {
		if (entry.id) {
			byId.set(entry.id, entry);
			leaf = entry;
		}
	}
	const path = [];
	for (let at = leaf; at; at = at.parentId ? byId.get(at.parentId) : null) path.push(at);
	return path.reverse();
}

// One line to read in a list, without its markdown marks.
function firstLine(text, length) {
	const line = text.trim().split("\n")[0].replace(/[*`_#]+/g, "").replace(/\s+/g, " ").trim();
	return line.length > length ? `${line.slice(0, length - 1)}…` : line;
}

function summarise(file, entries) {
	const header = entries.find((entry) => entry.type === "session");
	if (!header) return null;
	const summary = {
		id: header.id,
		cwd: header.cwd,
		file,
		parent: header.parentSession ?? null,
		created: Date.parse(header.timestamp) / 1000,
		updated: Date.parse(header.timestamp) / 1000,
		name: null,
		prompt: null,
		preview: "",
		messages: 0,
		model: null,
		thinking: null,
	};
	for (const entry of activeBranch(entries)) {
		if (entry.timestamp) summary.updated = Date.parse(entry.timestamp) / 1000;
		if (entry.type === "session_info") summary.name = entry.name ?? null;
		if (entry.type === "model_change") summary.model = { provider: entry.provider, id: entry.modelId };
		if (entry.type === "thinking_level_change") summary.thinking = entry.thinkingLevel;
		if (entry.type !== "message") continue;
		const message = entry.message;
		if (message.role === "user") {
			summary.messages++;
			summary.prompt ??= firstLine(textOf(message.content), 80);
		} else if (message.role === "assistant") {
			const text = textOf(message.content);
			if (text.trim()) summary.preview = firstLine(text, 90);
			if (message.stopReason === "error" && message.errorMessage) summary.preview = firstLine(message.errorMessage, 90);
			if (message.provider && message.model) summary.model = { provider: message.provider, id: message.model };
		}
	}
	return summary;
}

// A file's summary, read again only when it changed.
export function summary(file) {
	let stat;
	try {
		stat = statSync(file);
	} catch {
		summaries.delete(file);
		return null;
	}
	const cached = summaries.get(file);
	if (cached && cached.mtime === stat.mtimeMs && cached.size === stat.size) return cached.summary;
	const fresh = summarise(file, parse(file));
	summaries.set(file, { mtime: stat.mtimeMs, size: stat.size, summary: fresh });
	return fresh;
}

export function sessionsIn(cwd) {
	const dir = sessionDir(cwd);
	if (!existsSync(dir)) return [];
	return readdirSync(dir)
		.filter((name) => name.endsWith(".jsonl"))
		.map((name) => summary(join(dir, name)))
		.filter(Boolean);
}
