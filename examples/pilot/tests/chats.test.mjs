// A thread needs no project: this app has none, so nothing here adds one.
import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { start } from "./harness.mjs";

let app;
before(async () => {
	app = await start();
	await app.until((v) => v.runtime?.status === "ready" && v.models.length > 0, "pi and its models");
	app.send({ set_model: { provider: "mock", id: "mock-1" } });
});
after(() => app.stop());

const finished = (text) => (v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "user" && i.text === text);

test("with no project, a thread runs in the app's own folder", async () => {
	assert.equal(app.value.workspaces.length, 0);
	const { path, name, git } = app.value.chats;
	assert.equal(name, "No project");
	assert.equal(git, false);
	assert.ok(existsSync(path));

	app.send({ new_thread: { workspace: path, mode: "local", prompt: "just a question", images: {} } });
	await app.until(finished("just a question"), "chat answered");
	const { thread } = app.value;
	assert.equal(thread.cwd, path);
	assert.equal(thread.workspace, "No project");
	assert.equal(thread.git, false, "a chat offers no fork into a worktree");
	assert.equal(app.value.threads.find((row) => row.id === app.value.selected).workspace, "No project");
	const sessions = readdirSync(join(app.agent, "sessions"));
	assert.equal(sessions.length, 1, "pi keeps its session as usual");
});

test("a thread that names no folder at all also works", async () => {
	app.send({ new_thread: { mode: "local", prompt: "no folder named", images: {} } });
	await app.until(finished("no folder named"), "answered");
	assert.equal(app.value.thread.cwd, app.value.chats.path);
});

test("a worktree choice left over from a project is ignored", async () => {
	app.send({ new_thread: { workspace: app.value.chats.path, mode: "worktree", prompt: "stale mode", images: {} } });
	await app.until(finished("stale mode"), "answered");
	assert.equal(app.value.thread.cwd, app.value.chats.path);
});

test("a scheduled task can have no project", async () => {
	const before = app.value.threads.length;
	app.send({ schedule: { name: "daily note", prompt: "write a note", workspace: app.value.chats.path, worktree: true } });
	await app.until((v) => v.schedules.length === 1, "saved");
	assert.equal(app.value.schedules[0].workspace_name, "No project");
	app.send({ run_schedule: app.value.schedules[0].id });
	await app.until((v) => v.threads.length > before, "ran");
});
