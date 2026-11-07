// The messages mods send `provider:pi`, each by its one distinguishing key.
// Anything that fails comes back as a toast in `app.pi.events`.
import { randomUUID } from "node:crypto";
import { existsSync, readFileSync, rmSync, statSync } from "node:fs";
import { extname } from "node:path";
import { removeWorktree } from "./git.mjs";
import { PiProcess } from "./rpc.mjs";
import { forgetKey, removeEndpoint, saveEndpoint, saveKey, setExtension, setSkill } from "./resources.mjs";
import { nextRun } from "./schedule.mjs";

const IMAGE_TYPES = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif", ".webp": "image/webp" };
const MAX_IMAGE = 8 * 1024 * 1024;

// The host only sends a Luau table as a list when it has entries: an empty one
// arrives as `{}`, so anything that may be empty goes through here.
const asList = (value) => (Array.isArray(value) ? value : []);

function images(paths) {
	const list = [];
	for (const path of asList(paths)) {
		const mimeType = IMAGE_TYPES[extname(path).toLowerCase()];
		if (!mimeType) throw new Error(`pi takes pictures, and ${path} is not one.`);
		if (statSync(path).size > MAX_IMAGE) throw new Error(`${path} is over 8 MB.`);
		list.push({ type: "image", mimeType, data: readFileSync(path).toString("base64") });
	}
	return list;
}

// Commands pi's own terminal handles itself, done here over RPC.
async function builtin(engine, id, text) {
	const [, name, rest = ""] = /^\/(\S+)\s*([\s\S]*)$/.exec(text) ?? [];
	switch (name) {
		case "compact":
			await (await engine.ensureLive(id)).send("compact", rest ? { customInstructions: rest } : {}, 300_000);
			engine.loadEntries(id);
			return true;
		case "name":
			if (rest) await (await engine.ensureLive(id)).send("set_session_name", { name: rest.trim() });
			return true;
		case "reload":
			engine.stopLive(id);
			await engine.ensureLive(id);
			return true;
		default:
			return false;
	}
}

// What the composer's text means: a shell line, a command, or a prompt.
async function submit(engine, id, { text, paths, behavior }) {
	try {
		const attached = images(paths);
		if (text.startsWith("!")) return await shell(engine, id, text);
		if (text.startsWith("/") && (await builtin(engine, id, text))) return;
		await engine.prompt(id, { text, images: attached, behavior });
	} catch (error) {
		// The composer emptied itself on sending: the words go back to this
		// thread's, whichever is open by the time a slow failure comes.
		// Mods remember the last id across launches, so a count won't do.
		if (text) engine.returned.set(id, { text, id: randomUUID() });
		throw error;
	}
}

async function shell(engine, id, text) {
	const quiet = text.startsWith("!!");
	const command = text.slice(quiet ? 2 : 1).trim();
	const thread = await engine.ensureLive(id);
	const result = await thread.send("bash", { command, excludeFromContext: quiet }, 600_000);
	await thread.refresh();
	const meta = engine.data.threads[id];
	if (meta.file && existsSync(meta.file)) engine.loadEntries(id);
	// pi writes nothing until the model first answers, so before that the
	// output is only here. Once the file exists it holds this too.
	else engine.entries.get(id)?.push({ id: randomUUID(), message: { role: "bashExecution", command, exitCode: result?.exitCode, output: result?.output ?? "", excludeFromContext: quiet } });
	engine.changed();
}

// A fork is a new session from an earlier prompt; a pi of its own makes it,
// so the thread it comes from can go on running.
async function fork(engine, { thread: id, entry, worktree }) {
	const meta = engine.data.threads[id];
	const child = new PiProcess(engine.pi.path, ["--session", meta.file], meta.cwd);
	try {
		const done = await child.send("fork", { entryId: entry });
		if (done?.cancelled) return;
		const state = await child.send("get_state");
		const forked = randomUUID();
		engine.data.threads[forked] = { id: forked, cwd: meta.cwd, workspace: meta.workspace, file: state.sessionFile, title: `Fork of ${engine.titleOf(id)}`, worktree: meta.worktree ?? null };
		if (worktree) await moveToWorktree(engine, engine.data.threads[forked]);
		engine.store.save();
		engine.scan();
		const adopted = Object.values(engine.data.threads).find((item) => item.file === state.sessionFile && item.id !== forked);
		if (adopted) {
			Object.assign(adopted, { title: engine.data.threads[forked].title, worktree: engine.data.threads[forked].worktree, workspace: meta.workspace });
			delete engine.data.threads[forked];
		}
		engine.select(adopted?.id ?? forked);
		engine.prefill = { text: done?.text ?? "", seq: Date.now() };
	} finally {
		child.stop();
	}
}

async function moveToWorktree(engine, meta) {
	const { addWorktree } = await import("./git.mjs");
	meta.worktree = await addWorktree(meta.workspace, meta.title);
	const child = new PiProcess(engine.pi.path, ["--fork", meta.file], meta.worktree.path);
	try {
		const state = await child.send("get_state");
		meta.file = state.sessionFile;
		meta.cwd = meta.worktree.path;
	} finally {
		child.stop();
	}
}

function saveSchedule(engine, task) {
	const list = engine.data.schedules;
	const saved = list.find((item) => item.id === task.id);
	const merged = { enabled: true, cadence: "daily", time: "09:00", weekday: 1, ...saved, ...task };
	merged.id ??= randomUUID();
	merged.next = nextRun(merged, Date.now() / 1000);
	if (saved) Object.assign(saved, merged);
	else list.push(merged);
	engine.store.save();
}

async function deleteThread(engine, id) {
	const meta = engine.data.threads[id];
	if (!meta) return;
	engine.stopLive(id);
	if (meta.file) rmSync(meta.file, { force: true });
	if (meta.worktree) {
		// A fork can share the checkout: the last thread in it removes it.
		const heir = Object.values(engine.data.threads).find((other) => other.id !== id && other.cwd === meta.cwd);
		if (heir) heir.worktree ??= meta.worktree;
		else await removeWorktree(meta.workspace, meta.worktree.path).catch((error) => console.error(`pilot: ${error.message}`));
	}
	delete engine.data.threads[id];
	engine.returned.delete(id);
	engine.summaries.delete(id);
	engine.entries.delete(id);
	if (engine.data.selected === id) engine.data.selected = null;
	engine.store.save();
}

const HANDLERS = {
	install_pi: (engine) => engine.install(),
	add_workspace: (engine, msg) => engine.addWorkspace(msg.add_workspace),
	remove_workspace: (engine, msg) => engine.removeWorkspace(msg.remove_workspace),
	select: (engine, msg) => engine.select(msg.select === false ? null : msg.select),
	new_thread: async (engine, msg) => {
		const options = msg.new_thread;
		const model = options.model ?? engine.data.prefs.last_model;
		const paths = asList(options.images);
		await engine.createThread({ ...options, model, images: paths, deliver: (id, { text }) => submit(engine, id, { text, paths }) });
	},
	prompt: async (engine, msg) => {
		const id = msg.thread ?? engine.data.selected;
		await submit(engine, id, { text: msg.prompt, paths: msg.images, behavior: msg.behavior });
	},
	returned_taken: (engine, msg) => {
		const { thread, id } = msg.returned_taken;
		if (engine.returned.get(thread)?.id === id) engine.returned.delete(thread);
	},
	abort: async (engine, msg) => {
		const live = engine.live.get(msg.thread ?? engine.data.selected);
		if (!live) return;
		await live.send("abort");
	},
	clear_queue: async (engine) => {
		await engine.live.get(engine.data.selected)?.send("clear_queue");
	},
	set_model: async (engine, msg) => {
		const { provider, id } = msg.set_model;
		engine.data.prefs.last_model = { provider, id };
		// From the start page it is for the next thread, not the one selected.
		const meta = msg.draft ? null : engine.data.threads[engine.data.selected];
		if (meta) {
			meta.model = { provider, id };
			const live = engine.live.get(meta.id);
			if (live) {
				await live.send("set_model", { provider, modelId: id });
				await live.refresh();
				live.levels = (await live.send("get_available_thinking_levels"))?.levels ?? [];
			}
		}
		engine.store.save();
	},
	set_thinking: async (engine, msg) => {
		engine.data.prefs.last_thinking = msg.set_thinking;
		const meta = msg.draft ? null : engine.data.threads[engine.data.selected];
		if (meta) {
			meta.thinking = msg.set_thinking;
			const live = engine.live.get(meta.id);
			if (live) {
				await live.send("set_thinking_level", { level: msg.set_thinking });
				await live.refresh();
			}
		}
		engine.store.save();
	},
	rename: async (engine, msg) => {
		const { id, title } = msg.rename;
		(await engine.ensureLive(id)).send("set_session_name", { name: title });
	},
	pin: (engine, msg) => flag(engine, msg.pin, "pinned"),
	archive: (engine, msg) => flag(engine, msg.archive, "archived"),
	delete_thread: (engine, msg) => deleteThread(engine, msg.delete_thread),
	fork: (engine, msg) => fork(engine, msg.fork),
	expand: (engine, msg) => {
		const id = engine.data.selected;
		const set = engine.expanded.get(id) ?? new Set();
		if (!set.delete(msg.expand)) set.add(msg.expand);
		engine.expanded.set(id, set);
	},
	show_more: (engine) => {
		const id = engine.data.selected;
		engine.shown.set(id, (engine.shown.get(id) ?? 160) + 160);
	},
	answer: (engine, msg) => {
		const { dialog, ...answer } = msg.answer;
		engine.live.get(engine.data.selected)?.answer(dialog, answer);
	},
	schedule: (engine, msg) => saveSchedule(engine, msg.schedule),
	delete_schedule: (engine, msg) => {
		engine.data.schedules = engine.data.schedules.filter((task) => task.id !== msg.delete_schedule);
		engine.store.save();
	},
	run_schedule: async (engine, msg) => {
		const task = engine.data.schedules.find((item) => item.id === msg.run_schedule);
		if (task) await engine.runTask(task);
	},
	set_skill: (engine, msg) => {
		setSkill(msg.set_skill.id, msg.set_skill.enabled);
		engine.configChanged();
	},
	set_extension: (engine, msg) => {
		setExtension(msg.set_extension.id, msg.set_extension.enabled);
		engine.configChanged();
	},
	save_key: (engine, msg) => saveKey(msg.save_key.provider, msg.save_key.key),
	forget_key: (engine, msg) => forgetKey(msg.forget_key),
	save_endpoint: (engine, msg) => saveEndpoint(msg.save_endpoint),
	remove_endpoint: (engine, msg) => removeEndpoint(msg.remove_endpoint),
	set_pref: (engine, msg) => {
		Object.assign(engine.data.prefs, msg.set_pref);
		engine.store.save();
	},
	refresh: (engine) => {
		engine.resourcesAsked = (engine.resourcesAsked ?? 0) + 1;
		engine.scan();
	},
};

function flag(engine, { id, on }, name) {
	const meta = engine.data.threads[id];
	if (!meta) return;
	meta[name] = on;
	engine.store.save();
}

export async function handle(engine, message) {
	if (typeof message !== "object" || message === null) return;
	const name = Object.keys(HANDLERS).find((key) => key in message);
	if (!name) return;
	try {
		await HANDLERS[name](engine, message);
	} catch (error) {
		engine.fail(error);
	}
	engine.changed();
}
