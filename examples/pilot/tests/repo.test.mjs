// The repo provider against a real repository.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { fixtureProject } from "./harness.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
let child;
let value;
const waiters = new Set();
const send = (message) => child.stdin.write(`${JSON.stringify({ from: "test", message })}\n`);
const until = (check, label) =>
	new Promise((done, fail) => {
		const timer = setTimeout(() => fail(new Error(`timed out on ${label}: ${JSON.stringify(value)?.slice(0, 800)}`)), 8000);
		const look = () => {
			const found = value && check(value);
			if (found) {
				clearTimeout(timer);
				waiters.delete(look);
				done(found);
			}
		};
		waiters.add(look);
		look();
	});

let project;
before(async () => {
	project = fixtureProject(mkdtempSync(join(tmpdir(), "pilot-repo-")));
	child = spawn("node", ["main.mjs"], { cwd: join(HERE, "../providers/repo"), stdio: ["pipe", "pipe", "inherit"] });
	let buffer = "";
	child.stdout.setEncoding("utf8");
	child.stdout.on("data", (text) => {
		buffer += text;
		for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
			value = JSON.parse(buffer.slice(0, end));
			buffer = buffer.slice(end + 1);
			for (const waiter of waiters) waiter();
		}
	});
	send({ at: project });
	await until((v) => v.git && v.branch === "main", "repository");
});
after(() => child.stdin.end());

test("a change shows with its diff, an untracked file too", async () => {
	writeFileSync(join(project, "src/price.js"), `${readFileSync(join(project, "src/price.js"), "utf8")}// more\n`);
	writeFileSync(join(project, "notes.txt"), "one\ntwo\n");
	send({ refresh: true });
	await until((v) => v.files.length === 2 && v.diff, "changes");
	send({ pick: "src/price.js" });
	const diff = await until((v) => v.diff?.path === "src/price.js" && v.diff, "diff");
	assert.ok(diff.hunks[0].lines.some((line) => line.kind === "add" && line.text === "// more"));
	send({ pick: "notes.txt" });
	const added = await until((v) => v.diff?.path === "notes.txt" && v.diff, "untracked diff");
	assert.equal(added.hunks[0].lines.length, 2);
});

test("staging and unstaging a file", async () => {
	send({ stage: "notes.txt" });
	await until((v) => v.files.find((f) => f.path === "notes.txt")?.staged, "staged");
	send({ unstage: "notes.txt" });
	await until((v) => !v.files.find((f) => f.path === "notes.txt")?.staged, "unstaged");
});

test("files: listing, opening, saving, refusing what changed underneath", async () => {
	assert.ok(value.tree[""].some((entry) => entry.name === "src" && entry.dir));
	send({ open_dir: "src" });
	await until((v) => v.tree.src?.some((entry) => entry.name === "price.js"), "folder");
	send({ read: "notes.txt" });
	const file = await until((v) => v.file?.text && v.file, "read");
	send({ write: "notes.txt", text: "three\n", version: file.version });
	await until((v) => v.file?.saved, "saved");
	assert.equal(readFileSync(join(project, "notes.txt"), "utf8"), "three\n");
	writeFileSync(join(project, "notes.txt"), "changed elsewhere\n");
	send({ write: "notes.txt", text: "mine\n", version: value.file.version });
	await until((v) => v.file?.conflict, "conflict");
	assert.equal(readFileSync(join(project, "notes.txt"), "utf8"), "changed elsewhere\n");
	send({ read: "../outside" });
	await until((v) => v.file?.missing, "outside refused");
});

test("finding a file by a few letters", async () => {
	send({ find: "pric" });
	const found = await until((v) => v.found.query === "pric" && v.found, "found");
	assert.ok(found.results.slice(0, 2).includes("src/price.js"));
});

test("a commit takes everything when nothing is staged", async () => {
	send({ commit: "Tidy up" });
	await until((v) => v.message === "Committed" && v.files.length === 0, "committed");
});
