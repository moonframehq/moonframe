// The little git a thread needs: worktrees, and a snapshot of the working
// tree at each end of a run, so a run's changes can be shown on their own.
import { execFile } from "node:child_process";
import { copyFileSync, mkdirSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { WORKTREES_DIR } from "./paths.mjs";

export function git(cwd, args, env = {}) {
	return new Promise((resolve, reject) => {
		execFile(
			"git",
			args,
			{ cwd, env: { ...process.env, GIT_OPTIONAL_LOCKS: "0", ...env }, maxBuffer: 32 * 1024 * 1024, timeout: 60_000 },
			(error, stdout, stderr) => (error ? reject(new Error(stderr.trim() || error.message)) : resolve(stdout)),
		);
	});
}

export async function repoRoot(cwd) {
	try {
		return (await git(cwd, ["rev-parse", "--show-toplevel"])).trim();
	} catch {
		return null;
	}
}

export async function currentBranch(cwd) {
	try {
		return (await git(cwd, ["symbolic-ref", "--short", "-q", "HEAD"])).trim() || null;
	} catch {
		return null;
	}
}

// The tree the working folder would commit as, ignored files aside, through
// a throwaway index so the user's staging stays as it is.
export async function snapshot(cwd) {
	if (!(await repoRoot(cwd))) return null;
	const index = join(tmpdir(), `pilot-index-${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2)}`);
	try {
		const real = (await git(cwd, ["rev-parse", "--git-path", "index"])).trim();
		try {
			copyFileSync(join(cwd, real), index);
		} catch {
			copyFileSync(real, index);
		}
	} catch {
		// A repository with nothing staged yet has no index to start from.
	}
	const env = { GIT_INDEX_FILE: index };
	try {
		await git(cwd, ["add", "-A"], env);
		return (await git(cwd, ["write-tree"], env)).trim();
	} catch {
		return null;
	} finally {
		rmSync(index, { force: true });
		rmSync(`${index}.lock`, { force: true });
	}
}

export async function changesBetween(cwd, from, to) {
	const files = [];
	let added = 0;
	let deleted = 0;
	const out = await git(cwd, ["diff", "--numstat", "--no-renames", "--relative", from, to]);
	for (const line of out.split("\n")) {
		const [plus, minus, path] = line.split("\t");
		if (!path) continue;
		const file = { path, added: Number(plus) || 0, deleted: Number(minus) || 0 };
		files.push(file);
		added += file.added;
		deleted += file.deleted;
	}
	return { files, added, deleted };
}

function slugged(text) {
	return (
		text
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/^-+|-+$/g, "")
			.slice(0, 40)
			.replace(/-+$/, "") || "thread"
	);
}

// A worktree on a new branch, off what the workspace has checked out.
export async function addWorktree(workspace, words) {
	const root = await repoRoot(workspace);
	if (!root) throw new Error("A worktree needs a git repository, and this folder is not one.");
	const slug = `${slugged(words)}-${Math.random().toString(36).slice(2, 6)}`;
	const branch = `pilot/${slug}`;
	const path = join(WORKTREES_DIR, basename(root), slug);
	mkdirSync(join(WORKTREES_DIR, basename(root)), { recursive: true });
	await git(root, ["worktree", "add", "-b", branch, path]);
	// pi names a session folder after the real path, links resolved.
	return { path: realpathSync(path), branch };
}

export async function removeWorktree(workspace, path) {
	await git(workspace, ["worktree", "remove", "--force", path]);
}
