// The pi provider end to end: the real pi, the mock model. Needs pi on PATH.
// `node --test examples/pilot/tests/*.test.mjs`
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { start } from "./harness.mjs";

let app;
before(async () => {
	app = await start();
	await app.until((v) => v.runtime?.status === "ready" && v.models.length > 0, "pi and its models");
});
after(() => app.stop());

const items = (kind) => (app.value.thread?.items ?? []).filter((item) => item.k === kind);

test("finds pi and its models", () => {
	assert.equal(app.value.models[0].id, "mock-1");
	assert.match(app.value.runtime.version, /\d+\.\d+/);
});

test("a workspace appears with its branch", async () => {
	app.send({ add_workspace: app.project });
	const workspace = await app.until((v) => v.workspaces.find((w) => w.branch === "main"), "workspace");
	assert.equal(workspace.name, "tidy-cart");
	assert.equal(workspace.git, true);
});

// What the composer really sends: the host turns an empty Luau table into `{}`.
test("an empty attachment list from the host is no attachments", async () => {
	app.send({ set_model: { provider: "mock", id: "mock-1" } });
	app.send({ new_thread: { workspace: app.project, mode: "local", prompt: "hello there", images: {} } });
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "user" && i.text === "hello there"), "new thread sent");
	app.send({ prompt: "and again", images: {} });
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "user" && i.text === "and again"), "prompt sent");
	app.send({ delete_thread: app.value.selected });
	await app.until((v) => !v.thread, "cleared");
});

test("a thread runs pi: tools, an answer, a turn's changes, a title", async () => {
	app.send({ set_model: { provider: "mock", id: "mock-1" } });
	app.send({ new_thread: { workspace: app.project, mode: "local", prompt: "fix the price please" } });
	await app.until((v) => v.thread?.status === "running", "running");
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "changes"), "finished");
	const tools = items("tool");
	assert.deepEqual(tools.map((t) => t.tool), ["read", "edit", "bash"]);
	assert.equal(tools[0].title, "Read src/price.js");
	assert.equal(tools[1].added, 1);
	assert.equal(tools[1].deleted, 1);
	assert.ok(tools.every((t) => t.status === "done"));
	const answer = items("text").at(-1);
	assert.match(answer.text, /Fixed/);
	assert.equal(answer.answer, true);
	const [changes] = items("changes");
	assert.equal(changes.count, 1);
	assert.equal(changes.files[0].path, "src/price.js");
	assert.match(items("run")[0].summary, /Explored 1 file, edited 1 file, ran 1 command/);
	assert.match(readFileSync(join(app.project, "src/price.js"), "utf8"), /padStart/);
	await app.until((v) => v.thread.title === "Fix The Price Please", "generated title");
	assert.equal(app.value.events.at(-1).kind, "finished");
});

test("session files are pi's own, and survive a fresh load", async () => {
	const id = app.value.selected;
	const before = items("tool").length;
	app.send({ select: false });
	await app.until((v) => v.thread === null, "deselected");
	app.send({ select: id });
	await app.until((v) => v.thread?.items.filter((i) => i.k === "tool").length === before, "reloaded");
	const dirs = readdirSync(join(app.agent, "sessions"));
	assert.equal(dirs.length, 1);
	assert.equal(items("user")[0].text, "fix the price please");
	assert.ok(items("user")[0].entry, "a prompt has its entry id for forking");
});

test("expanding a tool row brings its output", async () => {
	const edit = items("tool")[1];
	app.send({ expand: edit.id });
	const open = await app.until((v) => v.thread.items.find((i) => i.id === edit.id && i.open), "open");
	assert.match(open.patch, /padStart/);
	app.send({ expand: edit.id });
});

test("a follow-up queues during a run, and stop stops", async () => {
	app.send({ prompt: "go slow now" });
	await app.until((v) => v.thread.running && v.thread.items.some((i) => i.k === "text" && i.streaming), "streaming");
	app.send({ prompt: "and then this", behavior: "follow_up" });
	await app.until((v) => v.thread.queued.followUp.length === 1, "queued");
	app.send({ clear_queue: true });
	await app.until((v) => v.thread.queued.followUp.length === 0, "cleared");
	app.send({ abort: true });
	await app.until((v) => !v.thread.running, "stopped");
	await app.until((v) => v.thread.items.some((i) => i.k === "note" && i.text === "Stopped"), "note");
});

test("a shell command goes through pi", async () => {
	app.send({ prompt: "!echo hello from a shell" });
	const bash = await app.until((v) => v.thread.items.find((i) => i.k === "bash"), "bash item");
	assert.match(bash.output, /hello from a shell/);
	assert.equal(bash.code, 0);
});

test("forking makes a new thread from an earlier prompt", async () => {
	const source = app.value.selected;
	const entry = items("user")[0].entry;
	app.send({ fork: { thread: source, entry } });
	const forked = await app.until((v) => v.selected !== source && v.thread, "forked");
	assert.match(forked.title, /Fork of/);
	assert.equal(app.value.threads.filter((t) => t.id === source).length, 1);
	app.send({ select: source });
	await app.until((v) => v.selected === source, "back");
});

test("a worktree thread runs in a checkout of its own", async () => {
	app.send({ new_thread: { workspace: app.project, mode: "worktree", prompt: "hello there" } });
	const thread = await app.until((v) => v.thread?.worktree && v.thread, "worktree thread");
	assert.match(thread.branch, /^pilot\//);
	assert.notEqual(thread.cwd, app.project);
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "text"), "answered");
	const list = execFileSync("git", ["worktree", "list"], { cwd: app.project, encoding: "utf8" });
	assert.match(list, /pilot\//);
});

test("a thread started with the CLI shows up", async () => {
	const session = { type: "session", version: 3, id: "99999999-0000-4000-8000-000000000000", timestamp: new Date().toISOString(), cwd: app.project };
	const message = { type: "message", id: "aaaa0001", parentId: null, timestamp: new Date().toISOString(), message: { role: "user", content: "from the cli", timestamp: Date.now() } };
	const dir = join(app.agent, "sessions", readdirSync(join(app.agent, "sessions")).find((name) => name.includes("tidy-cart")));
	mkdirSync(dir, { recursive: true });
	writeFileSync(join(dir, "cli.jsonl"), `${JSON.stringify(session)}\n${JSON.stringify(message)}\n`);
	await app.until((v) => v.threads.find((t) => t.title === "from the cli"), "adopted", 8000);
});

test("pin, archive, rename and delete", async () => {
	const id = app.value.threads.find((t) => t.title === "from the cli").id;
	app.send({ pin: { id, on: true } });
	await app.until((v) => v.threads.find((t) => t.id === id)?.pinned, "pinned");
	app.send({ archive: { id, on: true } });
	await app.until((v) => v.threads.find((t) => t.id === id)?.archived, "archived");
	app.send({ delete_thread: id });
	await app.until((v) => !v.threads.some((t) => t.id === id), "deleted");
});

test("a key lands in pi's auth.json, private, and comes back out", async () => {
	app.send({ save_key: { provider: "groq", key: "gsk-test" } });
	await app.until((v) => v.providers.known.find((p) => p.id === "groq")?.connected, "connected", 5000);
	const auth = JSON.parse(readFileSync(join(app.agent, "auth.json"), "utf8"));
	assert.deepEqual(auth.groq, { type: "api_key", key: "gsk-test" });
	assert.ok(!JSON.stringify(app.value).includes("gsk-test"), "a key is never published");
	app.send({ forget_key: "groq" });
	await app.until((v) => !v.providers.known.find((p) => p.id === "groq")?.connected, "forgotten", 5000);
});

test("a schedule runs its prompt as a new thread", async () => {
	app.send({ schedule: { name: "Nightly", prompt: "hello nightly", workspace: app.project, cadence: "daily", time: "03:00" } });
	const task = await app.until((v) => v.schedules[0], "saved");
	assert.equal(task.when, "Every day at 03:00");
	assert.ok(task.next > app.value.now);
	app.send({ run_schedule: task.id });
	await app.until((v) => v.threads.find((t) => t.title.includes("Nightly") || t.title.includes("hello nightly")), "ran");
	await app.until((v) => v.schedules[0].last_run, "stamped");
});

test("turning a skill off reaches pi", async () => {
	const skill = join(app.agent, "skills", "tidy-up");
	mkdirSync(skill, { recursive: true });
	writeFileSync(join(skill, "SKILL.md"), "---\nname: tidy-up\ndescription: Tidies things up.\n---\nTidy.\n");
	app.send({ refresh: true });
	const listed = await app.until((v) => v.skills.find((s) => s.name === "tidy-up"), "listed");
	assert.equal(listed.enabled, true);
	app.send({ set_skill: { id: listed.id, enabled: true } });
	await app.until((v) => v.commands.some((c) => c.name === "skill:tidy-up"), "a command while on");
	app.send({ set_skill: { id: listed.id, enabled: false } });
	await app.until((v) => v.skills.find((s) => s.name === "tidy-up")?.enabled === false, "off");
	await app.until((v) => !v.commands.some((c) => c.name === "skill:tidy-up"), "no command when off");
	app.send({ set_skill: { id: listed.id, enabled: true } });
});

test("steering goes in during a run", async () => {
	app.send({ prompt: "go slow again" });
	await app.until((v) => v.thread.running && v.thread.items.some((i) => i.k === "text" && i.streaming), "streaming");
	app.send({ prompt: "actually stop soon", behavior: "steer" });
	await app.until((v) => v.thread.queued.steering.length === 1, "steering");
	app.send({ clear_queue: true });
	app.send({ abort: true });
	await app.until((v) => !v.thread.running, "stopped");
});

test("a picture goes with a prompt", async () => {
	const png = join(app.root, "dot.png");
	writeFileSync(png, Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64"));
	app.send({ prompt: "what is this", images: [png] });
	await app.until((v) => v.thread.items.some((i) => i.k === "user" && i.images === 1 && i.text === "what is this"), "attached");
	await app.until((v) => !v.thread.running, "answered");
});

test("something not a picture is refused with a reason", async () => {
	const seen = app.value.events.at(-1)?.seq ?? 0;
	app.send({ prompt: "and this", images: [join(app.project, "package.json")] });
	const toast = await app.until((v) => v.events.find((e) => e.seq > seen && e.kind === "toast"), "toast");
	assert.match(toast.body, /not one/);
});

test("the start page's choices are for the next thread, not the selected one", async () => {
	const level = app.value.thread.thinking === "high" ? "low" : "high";
	const before = app.value.thread.thinking;
	app.send({ set_thinking: level, draft: true });
	await app.until((v) => v.prefs.last_thinking === level, "remembered");
	assert.equal(app.value.thread.thinking, before);
});

test("an extension's question waits on the user, and the answer reaches it", async () => {
	mkdirSync(join(app.agent, "extensions"), { recursive: true });
	writeFileSync(
		join(app.agent, "extensions", "ask.ts"),
		`export default function (pi) {
	pi.registerCommand("ask", {
		description: "Asks something",
		handler: async (_args, ctx) => {
			const answer = await ctx.ui.select("Pick a colour", ["Red", "Blue"]);
			ctx.ui.notify("You picked " + answer, "info");
			ctx.ui.setStatus("ask", "picked " + answer);
		},
	});
}
`,
	);
	app.send({ prompt: "/reload" });
	await app.until((v) => v.commands.some((c) => c.name === "ask"), "extension loaded");
	app.send({ prompt: "/ask" });
	const dialog = await app.until((v) => v.thread.dialog, "dialog");
	assert.equal(dialog.title, "Pick a colour");
	assert.deepEqual(dialog.options, ["Red", "Blue"]);
	assert.equal(app.value.thread.status, "waiting");
	app.send({ answer: { dialog: dialog.id, value: "Blue" } });
	await app.until((v) => v.events.some((e) => e.body === "You picked Blue"), "answered");
	await app.until((v) => v.thread.statuses.includes("picked Blue"), "status");
	assert.equal(app.value.thread.dialog, null);
});

test("what is published stays under the host's limits", () => {
	assert.ok(Math.max(...app.sizes) < 1024 * 1024);
	const count = (value) => (value && typeof value === "object" ? 1 + Object.values(value).reduce((sum, item) => sum + count(item), 0) : 1);
	assert.ok(count(app.value) < 10_000);
});
