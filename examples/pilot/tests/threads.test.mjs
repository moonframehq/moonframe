// Threads and what the provider publishes: the real pi, the mock model.
import assert from "node:assert/strict";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { encode } from "../providers/pi/lib/publish.mjs";
import { nextRun, skipMissed } from "../providers/pi/lib/schedule.mjs";
import { start } from "./harness.mjs";

let app;
before(async () => {
	app = await start();
	await app.until((v) => v.runtime?.status === "ready" && v.models.length > 0, "pi and its models");
	app.send({ set_model: { provider: "mock", id: "mock-1" } });
	app.send({ add_workspace: app.project });
	await app.until((v) => v.workspaces.length === 1, "project");
});
after(() => app.stop());

test("a new thread with only a picture sends it", async () => {
	const png = join(app.root, "dot.png");
	writeFileSync(png, Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64"));
	app.send({ new_thread: { workspace: app.project, mode: "local", prompt: "", images: [png] } });
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "user" && i.images === 1), "picture sent");
});

test("a new thread's !command runs in the shell, not as a prompt", async () => {
	app.send({ new_thread: { workspace: app.project, mode: "local", prompt: "!echo from the start page", images: {} } });
	const bash = await app.until((v) => v.thread?.items.find((i) => i.k === "bash"), "ran");
	assert.match(bash.output, /from the start page/);
	assert.equal(app.value.thread.items.some((i) => i.k === "user"), false);
	app.send({ prompt: "hello there", images: {} });
	await app.until((v) => v.thread?.status === "idle" && v.thread.items.some((i) => i.k === "text"), "answered");
	assert.equal(app.value.thread.items.filter((i) => i.k === "bash").length, 1, "the file takes over, without a copy");
});

test("a worktree outlives its first thread while a fork uses it", async () => {
	app.send({ new_thread: { workspace: app.project, mode: "worktree", prompt: "hello there", images: {} } });
	const original = await app.until((v) => v.thread?.worktree && v.thread.status === "idle" && v.thread.items.some((i) => i.entry) && v.thread, "worktree thread");
	const { entry } = original.items.find((i) => i.entry);
	app.send({ fork: { thread: original.id, entry } });
	const fork = await app.until((v) => v.selected !== original.id && v.thread, "forked");
	assert.equal(fork.cwd, original.cwd);

	app.send({ delete_thread: original.id });
	await app.until((v) => !v.threads.some((row) => row.id === original.id), "original gone");
	assert.ok(existsSync(original.cwd), "the fork still has its checkout");

	app.send({ delete_thread: fork.id });
	await app.until((v) => !v.threads.some((row) => row.id === fork.id), "fork gone");
	await app.until(() => !existsSync(original.cwd), "the last thread removes it");
});

test("a task that came due while the app was closed waits for its next time", () => {
	const now = Date.UTC(2026, 10, 7, 12) / 1000;
	const missed = { cadence: "daily", time: "09:00", next: now - 3600 };
	const coming = { cadence: "daily", time: "09:00", next: now + 3600 };
	assert.equal(skipMissed([missed, coming], now), true);
	assert.equal(missed.next, nextRun(missed, now));
	assert.ok(missed.next > now);
	assert.equal(coming.next, now + 3600);
	assert.equal(skipMissed([coming], now), false);
});

const MIB = 1024 * 1024;
const countValues = (item) => (typeof item === "object" && item !== null ? Object.values(item).reduce((total, inner) => total + countValues(inner), 1) : 1);
const hostTakes = (line) => Buffer.byteLength(line) <= MIB && countValues(JSON.parse(line)) <= 10_000;

test("what is published always fits the host", () => {
	const text = (id) => ({ k: "text", id, text: '"\n'.repeat(200_000) });
	const many = { thread: { earlier: 0, items: [text("a"), text("b"), text("c"), text("end")] } };
	const line = encode(many);
	assert.ok(hostTakes(line));
	assert.equal(JSON.parse(line).thread.items.at(-1).id, "end", "the newest stays");
	assert.ok(JSON.parse(line).thread.earlier > 0);

	const alone = encode({ thread: { earlier: 0, items: [{ k: "tool", id: "t", output: "é".repeat(2_000_000) }] } });
	assert.ok(hostTakes(alone), "one huge item is cut");
	assert.match(JSON.parse(alone).thread.items[0].output, /cut, too long/);

	const wide = encode({ thread: { earlier: 0, items: Array.from({ length: 4000 }, (_, id) => ({ k: "note", id, text: "x" })) } });
	assert.ok(hostTakes(wide), "many small items too");
});
