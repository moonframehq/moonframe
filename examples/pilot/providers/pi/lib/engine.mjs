// The app's model: workspaces and threads, what each thread's pi is doing,
// and everything published as `app.pi`. Conversations live in pi's session
// files; the store only says where they belong. See publish.mjs for the
// shape, and commands.mjs for the messages it takes.
import { randomUUID } from "node:crypto";
import { execFile } from "node:child_process";
import { existsSync, mkdirSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { basename } from "node:path";
import { addWorktree, changesBetween, currentBranch, repoRoot, snapshot } from "./git.mjs";
import { LiveThread } from "./live.mjs";
import { CHATS_DIR } from "./paths.mjs";
import { PiProcess } from "./rpc.mjs";
import { configStamp } from "./resources.mjs";
import { adoptLoginEnvironment, findPi, installPi } from "./runtime.mjs";
import { nextRun } from "./schedule.mjs";
import { activeBranch, parse, sessionsIn, summary } from "./sessions.mjs";
import { Store } from "./store.mjs";

const IDLE_LIMIT = 10 * 60_000;
const MAX_LIVE = 12;
const KEPT_TURNS = 60;

export class Engine {
	constructor(publish) {
		this.notifyPublish = publish;
		this.store = new Store();
		mkdirSync(CHATS_DIR, { recursive: true });
		this.chats = realpathSync(CHATS_DIR);
		this.live = new Map();
		this.entries = new Map();
		this.summaries = new Map();
		this.branches = new Map();
		this.expanded = new Map();
		this.shown = new Map();
		this.events = [];
		this.seq = 0;
		this.pi = { status: "looking", path: null, version: null, error: null };
		this.catalogue = { models: [], commands: [] };
		this.stamp = "";
		this.probe = null;
	}

	get data() {
		return this.store.data;
	}

	changed() {
		this.notifyPublish();
	}

	// Starting up -----------------------------------------------------

	async boot() {
		adoptLoginEnvironment();
		this.locatePi();
		this.scan();
		if (this.data.selected) this.loadEntries(this.data.selected);
		this.changed();
		this.startProbe();
		this.stamp = configStamp();
		setInterval(() => this.tick(), 1000).unref();
		for (const workspace of this.data.workspaces) this.refreshBranch(workspace.path);
		if (this.data.selected) this.ensureLive(this.data.selected).catch((error) => this.fail(error));
	}

	locatePi() {
		const found = findPi();
		if (!found) console.error(`pilot: no working pi on ${process.env.PATH}`);
		this.pi = found
			? { status: "ready", ...found, error: null }
			: { status: "missing", path: null, version: null, error: null };
	}

	async install() {
		this.pi = { ...this.pi, status: "installing", error: null };
		this.changed();
		try {
			await installPi();
			this.locatePi();
			this.startProbe();
		} catch (error) {
			this.pi = { ...this.pi, status: "missing", error: error.message };
		}
		this.changed();
	}

	// The catalogue of models and commands comes from a pi of its own, so
	// it is there before any thread runs.
	startProbe() {
		this.probe?.stop();
		this.probe = null;
		if (this.pi.status !== "ready") return;
		const cwd = this.data.workspaces[0]?.path ?? homedir();
		const probe = new PiProcess(this.pi.path, ["--no-session"], cwd);
		this.probe = probe;
		probe.on("exit", (reason, stderr) => {
			if (this.probe !== probe) return;
			this.probe = null;
			if (stderr) console.error(`pilot: pi catalogue stopped (${reason}): ${stderr}`);
		});
		Promise.all([probe.send("get_available_models"), probe.send("get_commands")])
			.then(([models, commands]) => {
				this.catalogue = { models: models?.models ?? [], commands: commands?.commands ?? [] };
				this.changed();
			})
			.catch((error) => console.error(`pilot: pi catalogue: ${error.message}`));
	}

	tick() {
		const now = Date.now();
		if ([...this.live.values()].some((thread) => thread.running)) this.changed();
		if (now % 5000 < 1000) this.scan();
		const stamp = configStamp();
		if (stamp !== this.stamp) {
			this.stamp = stamp;
			this.configChanged();
		}
		this.reapIdle(now);
		this.runDue(now / 1000);
	}

	// pi reads its settings when it starts: idle threads start again on
	// their next use, and the catalogue at once.
	configChanged() {
		this.startProbe();
		for (const thread of this.live.values()) {
			if (!thread.running && !thread.sending) this.stopLive(thread.id);
		}
		this.resourcesAsked = (this.resourcesAsked ?? 0) + 1;
		if (this.data.selected) this.ensureLive(this.data.selected).catch(() => {});
		this.changed();
	}

	reapIdle(now) {
		for (const thread of this.live.values()) {
			const idle = now - thread.touched > IDLE_LIMIT;
			if (idle && !thread.running && !thread.dialogs.length && thread.id !== this.data.selected) {
				this.stopLive(thread.id);
			}
		}
	}

	stopLive(id) {
		const thread = this.live.get(id);
		if (!thread) return;
		this.live.delete(id);
		thread.quiet = true;
		thread.stop();
	}

	shutdown() {
		this.store.flush();
		this.probe?.stop();
		for (const thread of this.live.values()) thread.stop();
	}

	// Workspaces and the threads in them ------------------------------

	addWorkspace(path) {
		const real = realpathSync(path);
		if (!this.data.workspaces.some((workspace) => workspace.path === real)) {
			this.data.workspaces.push({ path: real, name: basename(real) });
			this.store.save();
		}
		this.refreshBranch(real);
		this.scan();
		this.changed();
	}

	removeWorkspace(path) {
		this.data.workspaces = this.data.workspaces.filter((workspace) => workspace.path !== path);
		for (const [id, meta] of Object.entries(this.data.threads)) {
			if (meta.workspace !== path) continue;
			this.stopLive(id);
			delete this.data.threads[id];
		}
		this.store.save();
		this.scan();
		this.changed();
	}

	async refreshBranch(cwd) {
		const branch = await currentBranch(cwd);
		const git = (await repoRoot(cwd)) !== null;
		this.branches.set(cwd, { branch, git });
		this.changed();
	}

	// Adopts sessions pi wrote in a workspace, the CLI's included, and keeps
	// each thread's summary current.
	scan() {
		const known = new Set(this.data.workspaces.map((workspace) => workspace.path));
		for (const meta of Object.values(this.data.threads)) known.add(meta.cwd);
		for (const cwd of known) {
			for (const session of sessionsIn(cwd)) {
				const meta = (this.data.threads[session.id] ??= {
					id: session.id,
					cwd,
					workspace: this.workspaceOf(cwd),
				});
				if (meta.file !== session.file) {
					meta.file = session.file;
					this.store.save();
				}
				this.summaries.set(session.id, session);
			}
		}
		this.changed();
	}

	workspaceOf(cwd) {
		return this.data.workspaces.find((workspace) => cwd === workspace.path)?.path ?? cwd;
	}

	refreshSummary(id) {
		const meta = this.data.threads[id];
		const fresh = meta?.file && summary(meta.file);
		if (fresh) this.summaries.set(id, fresh);
	}

	// What a thread has said, as the timeline reads it.
	loadEntries(id) {
		const meta = this.data.threads[id];
		if (!meta?.file || !existsSync(meta.file)) {
			if (!this.entries.has(id)) this.entries.set(id, []);
			return;
		}
		const list = [];
		for (const entry of activeBranch(parse(meta.file))) {
			if (entry.type === "message" && entry.message.role !== "system") {
				list.push({ id: entry.id, message: entry.message });
			} else if (entry.type === "compaction") {
				list.push({ id: entry.id, note: `Compacted ${Math.round((entry.tokensBefore ?? 0) / 1000)}k tokens of context` });
			} else if (entry.type === "branch_summary") {
				list.push({ id: entry.id, note: "Came back from another branch" });
			}
		}
		this.entries.set(id, list);
	}

	select(id) {
		if (id && !this.data.threads[id]) return;
		this.data.selected = id;
		if (id) {
			const meta = this.data.threads[id];
			meta.unread = false;
			meta.failed = false;
			this.loadEntries(id);
			this.ensureLive(id).catch((error) => this.fail(error, id));
		}
		this.store.save();
		this.changed();
	}

	// Running pi ------------------------------------------------------

	async ensureLive(id) {
		const existing = this.live.get(id);
		if (existing) {
			await existing.ready;
			return existing;
		}
		const meta = this.data.threads[id];
		if (!meta) throw new Error("That thread is gone.");
		if (this.pi.status !== "ready") throw new Error("pi is not installed yet.");
		if (!existsSync(meta.cwd)) throw new Error(`Its folder is gone: ${meta.cwd}`);
		this.makeRoom();
		const args = meta.file && existsSync(meta.file) ? ["--session", meta.file] : ["--session-id", id];
		const model = meta.model ?? this.data.prefs.last_model;
		if (!(meta.file && existsSync(meta.file))) {
			if (model) args.push("--model", `${model.provider}/${model.id}`);
			const thinking = meta.thinking ?? this.data.prefs.last_thinking;
			if (thinking) args.push("--thinking", thinking);
		}
		const thread = new LiveThread(id, this.pi.path, args, meta.cwd, this.hooks());
		this.live.set(id, thread);
		await thread.ready;
		const file = thread.state?.sessionFile;
		if (file && meta.file !== file) {
			meta.file = file;
			this.store.save();
		}
		return thread;
	}

	makeRoom() {
		if (this.live.size < MAX_LIVE) return;
		const idle = [...this.live.values()]
			.filter((thread) => !thread.running && !thread.dialogs.length && thread.id !== this.data.selected)
			.sort((a, b) => a.touched - b.touched);
		if (idle[0]) this.stopLive(idle[0].id);
	}

	hooks() {
		return {
			changed: () => this.changed(),
			entry: (thread, message) => {
				this.entries.get(thread.id)?.push({ message });
				this.changed();
			},
			reload: (thread) => this.loadEntries(thread.id),
			turnStarted: (thread, message) => this.turnStarted(thread, message),
			settled: (thread, aborted) => this.settled(thread, aborted),
			waiting: (thread, title) => this.emit("waiting", thread.id, this.titleOf(thread.id), title),
			toast: (thread, level, title, body) => this.emit("toast", thread.id, title, body, level),
			renamed: (thread) => {
				this.refreshSummary(thread.id);
				this.changed();
			},
			ended: (thread, reason, stderr) => this.ended(thread, reason, stderr),
		};
	}

	ended(thread, reason, stderr) {
		const expected = thread.quiet === true;
		if (this.live.get(thread.id) === thread) this.live.delete(thread.id);
		if (expected) return;
		const detail = stderr.split("\n").filter((line) => !line.startsWith("Warning:")).pop();
		if (thread.running) {
			this.markFailed(thread.id);
			this.emit("toast", thread.id, "pi stopped", detail || reason, "error");
		}
		console.error(`pilot: pi for ${thread.id} ended: ${reason}`);
		this.changed();
	}

	// A run's "before" is taken ahead of sending its prompt, as a quick run
	// could otherwise edit first. A queued prompt has no such moment: its
	// start is the end of the run before it, and the best there is.
	turnStarted(thread, message) {
		thread.turnReady = (async () => {
			const before = thread.nextBefore ?? (await snapshot(thread.cwd));
			thread.nextBefore = null;
			await this.closeTurn(thread.id, before);
			thread.turn = String(message.timestamp);
			thread.before = before;
		})();
	}

	async closeTurn(id, after) {
		const thread = this.live.get(id);
		const meta = this.data.threads[id];
		if (!thread?.turn || !meta || !thread.before || !after) return;
		const key = thread.turn;
		const before = thread.before;
		thread.turn = null;
		if (before === after) return;
		const changes = await changesBetween(thread.cwd, before, after).catch(() => null);
		if (!changes?.files.length) return;
		meta.turns ??= {};
		meta.turns[key] = { ...changes, before, after };
		const keys = Object.keys(meta.turns);
		if (keys.length > KEPT_TURNS) delete meta.turns[keys[0]];
		this.store.save();
	}

	async settled(thread, aborted) {
		const id = thread.id;
		const meta = this.data.threads[id];
		if (!meta) return;
		await thread.turnReady;
		await this.closeTurn(id, await snapshot(thread.cwd));
		if (thread.state) await thread.refresh().catch(() => {});
		const file = thread.state?.sessionFile;
		if (file) meta.file = file;
		this.loadEntries(id);
		this.refreshSummary(id);
		const last = this.entries.get(id)?.findLast((entry) => entry.message?.role === "assistant")?.message;
		const failed = last?.stopReason === "error";
		const title = this.titleOf(id);
		if (failed) {
			this.markFailed(id);
			this.emit("failed", id, title, last.errorMessage);
		} else if (!aborted) {
			if (id !== this.data.selected) meta.unread = true;
			this.emit("finished", id, title, this.summaries.get(id)?.preview ?? "");
		}
		this.store.save();
		this.changed();
		this.nameThread(thread).catch((error) => console.error(`pilot: naming a thread: ${error.message}`));
	}

	markFailed(id) {
		const meta = this.data.threads[id];
		if (!meta) return;
		meta.failed = id !== this.data.selected;
		this.store.save();
	}

	// A first run's thread gets a short title from its own model.
	async nameThread(thread) {
		const info = this.summaries.get(thread.id);
		if (!this.data.prefs.auto_title || info?.name || !info?.prompt || info.messages > 1) return;
		const model = thread.state?.model;
		if (!model) return;
		const words = (this.entries.get(thread.id) ?? []).find((entry) => entry.message?.role === "user");
		const prompt = `Write a short title of at most five words for this request. Answer with the title alone.\n\n${JSON.stringify(words?.message?.content).slice(0, 1500)}`;
		const title = await new Promise((resolve) => {
			const child = execFile(
				this.pi.path,
				["-p", "--no-session", "--no-tools", "--no-extensions", "--no-skills", "--no-context-files",
					"--no-prompt-templates", "--thinking", "off", "--model", `${model.provider}/${model.id}`, prompt],
				{ timeout: 45_000, cwd: thread.cwd },
				(error, stdout) => resolve(error ? null : stdout.trim().split("\n")[0]),
			);
			// `pi -p` reads piped stdin as part of the prompt, to its end.
			child.stdin.end();
		});
		const clean = title?.replace(/^["'#\s]+|["'.\s]+$/g, "").slice(0, 60);
		if (clean && this.live.get(thread.id) === thread) await thread.send("set_session_name", { name: clean });
	}

	titleOf(id) {
		const info = this.summaries.get(id);
		const meta = this.data.threads[id];
		return info?.name || meta?.title || info?.prompt || "New thread";
	}

	// Asking pi -------------------------------------------------------

	async createThread({ workspace, mode, prompt, title, images, select = true, model, thinking }) {
		const given = workspace ?? this.data.workspaces[0]?.path ?? this.chats;
		// pi names a session folder after the real path, links resolved.
		const path = realpathSync(given);
		const id = randomUUID();
		let cwd = path;
		let worktree = null;
		// A choice left over from a project: chats are not a repository.
		if (mode === "worktree" && path !== this.chats) {
			worktree = await addWorktree(path, title || prompt || "thread");
			cwd = worktree.path;
			this.refreshBranch(cwd);
		}
		this.data.threads[id] = {
			id,
			cwd,
			workspace: path,
			worktree,
			title: title ?? firstWords(prompt),
			created: Date.now() / 1000,
			model: model ?? null,
			thinking: thinking ?? null,
		};
		this.entries.set(id, []);
		this.store.save();
		if (select) this.select(id);
		if (prompt) await this.prompt(id, { text: prompt, images });
		return id;
	}

	async prompt(id, { text, images, behavior }) {
		const thread = await this.ensureLive(id);
		thread.touched = Date.now();
		const fields = { message: text };
		if (images?.length) fields.images = images;
		if (thread.running) {
			fields.streamingBehavior = behavior === "steer" ? "steer" : "followUp";
		} else {
			thread.sending = text;
			this.changed();
			thread.nextBefore = await snapshot(thread.cwd);
		}
		this.changed();
		try {
			await thread.send("prompt", fields);
		} catch (error) {
			thread.sending = null;
			throw error;
		}
	}

	emit(kind, thread, title, body, level) {
		this.seq++;
		this.events.push({ seq: this.seq, kind, thread, title, body: String(body ?? "").slice(0, 300), level: level ?? "info" });
		if (this.events.length > 12) this.events.shift();
		this.changed();
	}

	fail(error, thread) {
		console.error(`pilot: ${error.stack ?? error}`);
		this.emit("toast", thread ?? null, "That didn't work", error.message, "error");
	}

	// Scheduled tasks -------------------------------------------------

	runDue(now) {
		for (const task of this.data.schedules) {
			if (!task.enabled) continue;
			task.next ??= nextRun(task, now);
			if (task.next <= now) {
				task.next = nextRun(task, now);
				this.runTask(task).catch((error) => this.fail(error));
			}
		}
	}

	async runTask(task) {
		task.last_run = Date.now() / 1000;
		task.last_thread = await this.createThread({
			workspace: task.workspace,
			mode: task.worktree ? "worktree" : "local",
			prompt: task.prompt,
			title: task.name,
			select: false,
		});
		this.store.save();
		this.changed();
	}
}

function firstWords(text) {
	const line = String(text ?? "").trim().split("\n")[0];
	return line.length > 60 ? `${line.slice(0, 59)}…` : line || null;
}
