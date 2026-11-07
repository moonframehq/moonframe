// The repo provider against a real repository.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
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

const MIB = 1024 * 1024;
const countValues = (item) => (typeof item === "object" && item !== null ? Object.values(item).reduce((total, inner) => total + countValues(inner), 1) : 1);
const hostTakes = (view) => Buffer.byteLength(JSON.stringify(view)) <= MIB && countValues(view) <= 10_000;

test("a file that JSON makes too long is refused, not published", async () => {
	writeFileSync(join(project, "quotes.txt"), '"\n'.repeat(400_000));
	send({ read: "quotes.txt" });
	await until((v) => v.file?.path === "quotes.txt", "opened");
	assert.equal(value.file.tooLarge, true);
	assert.ok(hostTakes(value));
	send({ close: true });
	await until((v) => !v.file, "closed");
});

test("many changes and a long diff stay under the host's value limit", async () => {
	for (let index = 0; index < 400; index++) writeFileSync(join(project, `many-${index}.txt`), "x\n");
	writeFileSync(join(project, "long.txt"), "line\n".repeat(3000));
	send({ refresh: true });
	await until((v) => v.more_files > 0, "listed");
	send({ pick: "long.txt" });
	await until((v) => v.diff?.path === "long.txt", "diff");
	assert.ok(hostTakes(value), `${countValues(value)} values`);
	send({ stage_all: true });
	await until((v) => v.files.every((file) => file.staged), "staged");
	send({ commit: "Lots of files" });
	await until((v) => v.message === "Committed" && v.files.length === 0, "committed");
});

test("a project below the repository's top uses paths from its own folder", async () => {
	writeFileSync(join(project, "src/price.js"), "// changed\n");
	writeFileSync(join(project, "outside.txt"), "not in this project\n");
	send({ at: join(project, "src") });
	await until((v) => v.at.endsWith("src") && v.files.length > 0, "subfolder");
	assert.deepEqual(value.files.map((file) => file.path), ["price.js"]);
	assert.ok(value.diff?.hunks.length > 0, "its diff is found");
	send({ stage: "price.js" });
	await until((v) => v.files[0]?.staged, "staged");
	send({ commit: "Only the subfolder" });
	await until((v) => v.message === "Committed" && v.files.length === 0, "committed");
	const committed = execFileSync("git", ["show", "--name-only", "--format=", "HEAD"], { cwd: project, encoding: "utf8" });
	assert.equal(committed.trim(), "src/price.js");
	send({ at: project });
	await until((v) => v.files.some((file) => file.path === "outside.txt"), "the rest is still uncommitted");
});

test("a commit refuses what's staged beyond the project", async () => {
	execFileSync("git", ["add", "outside.txt"], { cwd: project });
	writeFileSync(join(project, "src/price.js"), "// changed again\n");
	send({ at: join(project, "src") });
	await until((v) => v.at.endsWith("src") && v.files.length > 0, "subfolder");
	const before = execFileSync("git", ["rev-parse", "HEAD"], { cwd: project, encoding: "utf8" });
	send({ commit: "Should not happen" });
	await until((v) => /Staged outside this project: outside.txt/.test(v.error ?? ""), "refused");
	assert.equal(execFileSync("git", ["rev-parse", "HEAD"], { cwd: project, encoding: "utf8" }), before);
	send({ refresh: true });
	await new Promise((done) => setTimeout(done, 300));
	assert.match(value.error ?? "", /Staged outside/, "a refresh leaves it on screen");
	execFileSync("git", ["reset", "-q", "HEAD", "--", "outside.txt"], { cwd: project });
	send({ at: project });
	await until((v) => v.at === project && v.files.length > 0, "back");
});

test("a conflicting save shows what's on disk, and can replace it", async () => {
	writeFileSync(join(project, "shared.txt"), "first\n");
	send({ read: "shared.txt" });
	const { version } = await until((v) => v.file?.path === "shared.txt" && v.file, "opened");
	writeFileSync(join(project, "shared.txt"), "someone else\n");
	send({ write: "shared.txt", text: "mine\n", version });
	const conflict = await until((v) => v.file?.conflict && v.file, "conflict");
	assert.equal(conflict.text, "someone else\n");
	assert.equal(readFileSync(join(project, "shared.txt"), "utf8"), "someone else\n");
	send({ write: "shared.txt", text: "mine\n", version: conflict.version });
	await until((v) => v.file?.saved, "saved");
	assert.equal(readFileSync(join(project, "shared.txt"), "utf8"), "mine\n");
	send({ close: true });
	await until((v) => !v.file, "closed");
});

test("a search can be of another folder than the open one", async () => {
	const other = mkdtempSync(join(tmpdir(), "pilot-other-"));
	mkdirSync(join(other, "docs"));
	writeFileSync(join(other, "docs/only-here.md"), "hello");
	send({ find: "onlyhere", root: other });
	await until((v) => v.found.query === "onlyhere", "found");
	assert.deepEqual(value.found.results, ["docs/only-here.md"]);
});
