// `app.repo`: the checkout a mod points it at with `{ at = path }`.
//
//   { at, git, branch, head, base, files, compare, diff, tree, open, file,
//     found, worktrees, busy, error, message }
//
// Messages: { at }, { compare = "uncommitted"|"branch"|"range", from?, to? },
// { pick = path } (the diff shown), { stage = path }, { unstage = path },
// { discard = path }, { commit = message }, { open_dir = path },
// { read = path }, { write = path, text, version }, { close = true },
// { find = query, root? }, { remove_worktree = path }, { refresh = true }.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { basename, join, resolve, sep } from "node:path";
import { StringDecoder } from "node:string_decoder";

const POLL = 2000;
const MAX_FILES = 300;
const MAX_DIFF_LINES = 1500;
const MAX_LINE = 400;
const MAX_FILE = 1024 * 1024;
const MAX_FOUND = 40;
// The host ends a provider whose line passes 1 MiB or 10,000 values.
const MAX_BYTES = 1_000_000;
const MAX_VALUES = 9_000;

const state = {
	at: null,
	git: false,
	branch: null,
	head: null,
	base: null,
	files: [],
	compare: { mode: "uncommitted", from: null, to: null },
	picked: null,
	diff: null,
	open: [""],
	tree: {},
	file: null,
	found: { query: "", results: [] },
	worktrees: [],
	busy: null,
	error: null,
	message: null,
};
let listing = null;
let last = "";

function countValues(value) {
	if (typeof value !== "object" || value === null) return 1;
	let total = 1;
	for (const item of Object.values(value)) total += countValues(item);
	return total;
}

const tooLarge = (line) => Buffer.byteLength(line) > MAX_BYTES;

// What to give up, in order, until the line is one the host takes.
const SHEDS = [
	(view) => (view.file?.text ? { ...view, file: { path: view.file.path, tooLarge: true } } : view),
	(view) => (view.diff ? { ...view, diff: { ...view.diff, hunks: [], truncated: true } } : view),
	(view) => ({ ...view, tree: { "": view.tree[""] ?? [] }, open: [""] }),
	(view) => ({ ...view, files: view.files.slice(0, 100), more_files: (view.more_files ?? 0) + Math.max(0, view.files.length - 100) }),
];

// The last resort when nothing above was enough: every list and text cut
// down, every key kept, since views read them. Tighter until it fits.
function trimmed(value, limit, depth = 0) {
	if (typeof value === "string") return value.length > limit * 100 ? `${value.slice(0, limit * 100)}…` : value;
	if (typeof value !== "object" || value === null) return value;
	if (depth > 12) return null;
	if (Array.isArray(value)) return value.slice(0, limit).map((item) => trimmed(item, limit, depth + 1));
	return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, trimmed(item, limit, depth + 1)]));
}

function encoded() {
	let view = { available: true, ...state };
	let line = JSON.stringify(view);
	for (const [index, shed] of SHEDS.entries()) {
		const bytes = tooLarge(line);
		if (!bytes && countValues(view) <= MAX_VALUES) break;
		// One long file is a few values: it only goes for its bytes.
		if (index === 0 && !bytes) continue;
		view = shed(view);
		line = JSON.stringify(view);
	}
	for (const limit of [50, 10, 2]) {
		if (!tooLarge(line) && countValues(view) <= MAX_VALUES) break;
		view = trimmed(view, limit);
		line = JSON.stringify(view);
	}
	return line;
}

function publish() {
	const line = encoded();
	if (line === last) return;
	last = line;
	process.stdout.write(`${line}\n`);
}

function git(args, { cwd = state.at, input } = {}) {
	return new Promise((done, fail) => {
		const child = execFile("git", args, { cwd, maxBuffer: 64 * 1024 * 1024, timeout: 60_000 }, (error, stdout, stderr) =>
			error ? fail(new Error(stderr.trim() || error.message)) : done(stdout),
		);
		if (input !== undefined) child.stdin.end(input);
	});
}

// What to diff against: nothing for the working tree's own changes.
function range() {
	const { mode, from, to } = state.compare;
	if (mode === "range" && from && to) return [from, to];
	if (mode === "branch" && state.base) return [state.base];
	return ["HEAD"];
}

async function baseBranch() {
	for (const name of ["origin/HEAD", "origin/main", "origin/master", "main", "master"]) {
		const ref = await git(["rev-parse", "--verify", "-q", `${name}^{commit}`]).catch(() => null);
		if (!ref) continue;
		const short = name === "origin/HEAD" ? (await git(["rev-parse", "--abbrev-ref", name]).catch(() => name)).trim() : name;
		if (short === state.branch) continue;
		const merge = await git(["merge-base", "HEAD", ref.trim()]).catch(() => null);
		if (merge) return { name: short, sha: merge.trim() };
	}
	return null;
}

// Staged and unstaged per file, from porcelain v1.
function parseStatus(text, prefix) {
	const found = new Map();
	const parts = text.split("\0");
	for (let i = 0; i < parts.length; i++) {
		const part = parts[i];
		if (part.length < 4) continue;
		const x = part[0];
		const y = part[1];
		if (x === "R" || x === "C") i++;
		// Status gives paths from the repository's top; everything else here
		// works from the project folder, which may be below it.
		if (!part.startsWith(prefix, 3)) continue;
		const path = part.slice(3 + prefix.length);
		found.set(path, {
			path,
			status: x === "?" ? "?" : x === "U" || y === "U" ? "U" : x !== " " ? x : y,
			staged: x !== " " && x !== "?",
			unstaged: y !== " ",
			added: 0,
			deleted: 0,
		});
	}
	return found;
}

function addCounts(files, numstat) {
	for (const line of numstat.split("\n")) {
		const [plus, minus, path] = line.split("\t");
		if (!path) continue;
		const file = files.get(path);
		if (file) {
			file.added = Number(plus) || 0;
			file.deleted = Number(minus) || 0;
		}
	}
}

async function readStatus() {
	if (state.compare.mode === "uncommitted") {
		const prefix = (await git(["rev-parse", "--show-prefix"])).trim();
		const files = parseStatus(await git(["status", "--porcelain=v1", "-z", "--untracked-files=all", "--", "."]), prefix);
		addCounts(files, await git(["diff", "--numstat", "--no-renames", "--relative", "HEAD"]).catch(() => ""));
		return [...files.values()];
	}
	const [from, to] = range();
	const args = ["diff", "--no-renames", "--relative", "--name-status", from];
	if (to) args.push(to);
	const files = new Map();
	for (const line of (await git(args)).split("\n")) {
		const [status, path] = line.split("\t");
		if (path) files.set(path, { path, status: status[0], staged: false, unstaged: false, added: 0, deleted: 0 });
	}
	addCounts(files, await git(["diff", "--numstat", "--no-renames", "--relative", from, ...(to ? [to] : [])]));
	return [...files.values()];
}

function parseDiff(text) {
	const hunks = [];
	let hunk = null;
	let oldLine = 0;
	let newLine = 0;
	let count = 0;
	let truncated = false;
	for (const raw of text.split("\n")) {
		if (raw.startsWith("@@")) {
			const match = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(raw);
			if (!match) continue;
			oldLine = Number(match[1]);
			newLine = Number(match[2]);
			hunk = { header: raw.slice(0, 120), lines: [] };
			hunks.push(hunk);
		} else if (hunk) {
			if (count >= MAX_DIFF_LINES) {
				truncated = true;
				break;
			}
			const sign = raw[0];
			const body = raw.slice(1, MAX_LINE);
			if (sign === "+") hunk.lines.push({ kind: "add", new: newLine++, text: body });
			else if (sign === "-") hunk.lines.push({ kind: "del", old: oldLine++, text: body });
			else if (sign === " ") hunk.lines.push({ kind: "ctx", old: oldLine++, new: newLine++, text: body });
			else continue;
			count++;
		}
	}
	return { hunks, truncated };
}

async function readDiff(path) {
	const file = state.files.find((item) => item.path === path);
	if (!file) return null;
	if (file.status === "?") {
		// `--no-index` exits 1 when files differ, which is the point here.
		const out = await new Promise((done) =>
			execFile("git", ["diff", "--no-index", "--", "/dev/null", path], { cwd: state.at, maxBuffer: 16 * 1024 * 1024 }, (_error, stdout) => done(stdout)),
		);
		return { path, status: "A", ...parseDiff(out) };
	}
	const text = await git(["diff", "--no-renames", "--relative", ...range(), "--", path]);
	return { path, status: file.status, binary: /^Binary files /m.test(text), ...parseDiff(text) };
}

async function readWorktrees() {
	const list = [];
	let item = null;
	for (const line of (await git(["worktree", "list", "--porcelain"])).split("\n")) {
		if (line.startsWith("worktree ")) {
			item = { path: line.slice(9), name: basename(line.slice(9)), branch: null, current: false };
			list.push(item);
		} else if (line.startsWith("branch ") && item) item.branch = line.slice(7).replace("refs/heads/", "");
	}
	for (const worktree of list) worktree.current = worktree.path === state.at;
	return list;
}

function listDir(path, root = state.at) {
	const dir = join(root, path);
	try {
		return readdirSync(dir, { withFileTypes: true })
			.filter((entry) => entry.name !== ".git" && entry.name !== ".DS_Store")
			.map((entry) => ({ name: entry.name, dir: entry.isDirectory() }))
			.sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
			.slice(0, 400);
	} catch {
		return [];
	}
}

function readTree() {
	const tree = {};
	for (const path of state.open) tree[path] = listDir(path);
	state.tree = tree;
}

let refreshing = false;
// A refresh clears its own failures, not those of what the user just did.
let refreshFailed = false;
async function refresh() {
	if (!state.at || refreshing) return;
	refreshing = true;
	const at = state.at;
	try {
		state.git = await git(["rev-parse", "--is-inside-work-tree"]).then(() => true, () => false);
		readTree();
		if (state.git) {
			state.branch = (await git(["symbolic-ref", "--short", "-q", "HEAD"]).catch(() => "")).trim() || null;
			state.head = (await git(["rev-parse", "--short", "HEAD"]).catch(() => "")).trim() || null;
			const base = await baseBranch();
			state.base = base?.sha ?? null;
			state.base_name = base?.name ?? null;
			const files = await readStatus();
			if (at !== state.at) return;
			state.files = files.slice(0, MAX_FILES);
			state.more_files = Math.max(0, files.length - MAX_FILES);
			if (state.picked && !state.files.some((file) => file.path === state.picked)) state.picked = null;
			if (!state.picked && state.files[0]) state.picked = state.files[0].path;
			state.diff = state.picked ? await readDiff(state.picked) : null;
			state.worktrees = await readWorktrees();
		} else {
			Object.assign(state, { files: [], diff: null, branch: null, head: null, worktrees: [] });
		}
		if (refreshFailed) state.error = null;
		refreshFailed = false;
	} catch (error) {
		state.error = error.message;
		refreshFailed = true;
	} finally {
		refreshing = false;
	}
	publish();
}

// A path inside the checkout, through links too.
function inside(path) {
	const root = realpathSync(state.at);
	const full = resolve(root, path);
	const real = realpathSync(full);
	if (real !== root && !real.startsWith(root + sep)) throw new Error("That file is outside this checkout.");
	return real;
}

function versionOf(text) {
	return createHash("sha1").update(text).digest("hex").slice(0, 12);
}

function openFile(path) {
	const full = inside(path);
	const { size } = statSync(full);
	if (size > MAX_FILE) {
		state.file = { path, tooLarge: true };
		return;
	}
	const buffer = readFileSync(full);
	if (buffer.subarray(0, 8000).includes(0)) {
		state.file = { path, binary: true };
		return;
	}
	const text = buffer.toString("utf8");
	state.file = { path, text, version: versionOf(text) };
}

function saveFile({ write: path, text, version }) {
	const full = inside(path);
	const current = readFileSync(full, "utf8");
	if (version && versionOf(current) !== version) {
		// What's on disk now, so a second save knowingly replaces it.
		state.file = { path, text: current, version: versionOf(current), conflict: true };
		return;
	}
	writeFileSync(full, text);
	state.file = { path, text, version: versionOf(text), saved: true };
	state.message = `Saved ${path}`;
}

async function allFiles(root) {
	if (listing?.at === root && Date.now() - listing.time < 15_000) return listing.files;
	let files = [];
	const tracked = await git(["ls-files", "-co", "--exclude-standard"], { cwd: root }).catch(() => null);
	if (tracked !== null) {
		files = tracked.split("\n").filter(Boolean);
	} else {
		const walk = (dir, depth) => {
			for (const entry of listDir(dir, root)) {
				const path = dir ? `${dir}/${entry.name}` : entry.name;
				if (entry.dir && depth < 4 && !entry.name.startsWith(".") && entry.name !== "node_modules") walk(path, depth + 1);
				else if (!entry.dir) files.push(path);
			}
		};
		walk("", 0);
	}
	listing = { at: root, time: Date.now(), files };
	return files;
}

// Characters in order, the closer together and the nearer the name the
// better, as a quick open does.
function score(path, query) {
	const lower = path.toLowerCase();
	let at = 0;
	let total = 0;
	let previous = -2;
	for (const char of query) {
		const found = lower.indexOf(char, at);
		if (found < 0) return null;
		total += found === previous + 1 ? 0 : found - at + 1;
		previous = found;
		at = found + 1;
	}
	const name = lower.lastIndexOf("/") + 1;
	return total + (lower.includes(query, name) ? -100 : 0) + path.length / 100;
}

async function find(query, root) {
	const files = await allFiles(root);
	const wanted = query.toLowerCase();
	const scored = [];
	for (const path of files) {
		const value = wanted ? score(path, wanted) : path.length;
		if (value !== null) scored.push({ path, value });
	}
	scored.sort((a, b) => a.value - b.value);
	state.found = { query, results: scored.slice(0, MAX_FOUND).map((item) => item.path) };
}

async function run(label, action, done) {
	state.busy = label;
	state.error = null;
	publish();
	try {
		await action();
		if (done) state.message = done;
	} catch (error) {
		state.error = error.message;
	}
	state.busy = null;
	await refresh();
}

async function handle(msg) {
	if (typeof msg !== "object" || msg === null) return;
	if (msg.at !== undefined) {
		if (msg.at === state.at) return;
		Object.assign(state, {
			at: msg.at,
			files: [],
			diff: null,
			picked: null,
			file: null,
			open: [""],
			compare: { mode: "uncommitted", from: null, to: null },
			found: { query: "", results: [] },
			message: null,
			error: null,
		});
		publish();
		return refresh();
	}
	// A draft searches the project it was given, whatever is open here.
	if (msg.find !== undefined) {
		const root = msg.root ?? state.at;
		if (!root) return;
		await find(msg.find, root).catch((error) => (state.error = error.message));
		return publish();
	}
	if (!state.at) return;
	if (msg.compare) {
		state.compare = { mode: msg.compare, from: msg.from ?? null, to: msg.to ?? null };
		state.picked = msg.pick ?? null;
		return refresh();
	}
	if (msg.pick !== undefined) {
		state.picked = msg.pick;
		state.diff = await readDiff(msg.pick).catch(() => null);
		return publish();
	}
	if (msg.stage) return run("stage", () => git(["add", "--", msg.stage]));
	if (msg.unstage) return run("unstage", () => git(["reset", "-q", "HEAD", "--", msg.unstage]));
	if (msg.stage_all) return run("stage", () => git(["add", "-A", "--", "."]));
	if (msg.discard) {
		const file = state.files.find((item) => item.path === msg.discard);
		return run("discard", () =>
			file?.status === "?" ? git(["clean", "-fq", "--", msg.discard]) : git(["checkout", "HEAD", "--", msg.discard]),
		);
	}
	if (msg.commit) {
		return run(
			"commit",
			async () => {
				// A commit takes everything staged, and this list only shows
				// the project: what's staged beyond it must not go in unseen.
				const prefix = (await git(["rev-parse", "--show-prefix"])).trim();
				const staged = (await git(["diff", "--cached", "--name-only", "-z"])).split("\0").filter(Boolean);
				const outside = staged.filter((path) => !path.startsWith(prefix));
				if (outside.length > 0) throw new Error(`Staged outside this project: ${outside.slice(0, 3).join(", ")}. Unstage it first.`);
				if (staged.length === 0) await git(["add", "-A", "--", "."]);
				await git(["commit", "-q", "-F", "-"], { input: msg.commit });
			},
			"Committed",
		);
	}
	if (msg.open_dir !== undefined) {
		const at = state.open.indexOf(msg.open_dir);
		if (at >= 0) state.open.splice(at, 1);
		else state.open.push(msg.open_dir);
		readTree();
		return publish();
	}
	if (msg.read) {
		try {
			openFile(msg.read);
		} catch (error) {
			state.file = { path: msg.read, missing: true, reason: error.message };
		}
		return publish();
	}
	if (msg.write) {
		try {
			saveFile(msg);
		} catch (error) {
			state.error = error.message;
		}
		return publish();
	}
	if (msg.close) {
		state.file = null;
		return publish();
	}
	if (msg.remove_worktree) return run("worktree", () => git(["worktree", "remove", "--force", msg.remove_worktree]));
	return refresh();
}

let chain = Promise.resolve();
const decoder = new StringDecoder("utf8");
let buffer = "";
process.stdin.on("data", (chunk) => {
	buffer += decoder.write(chunk);
	for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
		const line = buffer.slice(0, end);
		buffer = buffer.slice(end + 1);
		if (!line.trim()) continue;
		chain = chain.then(() => handle(JSON.parse(line).message)).catch((error) => console.error(`repo: ${error.stack ?? error}`));
	}
});
process.stdin.on("end", () => process.exit(0));
setInterval(() => refresh().catch(() => {}), POLL).unref();
publish();
