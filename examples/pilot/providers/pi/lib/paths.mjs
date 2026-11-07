// Where pi and this app keep things.
import { homedir } from "node:os";
import { join, resolve } from "node:path";

export const AGENT_DIR = process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");
export const DATA_DIR = process.env.PILOT_DATA || join(process.env.XDG_DATA_HOME || join(homedir(), ".local", "share"), "moonframe", "pilot");
export const WORKTREES_DIR = join(DATA_DIR, "worktrees");
export const RUNTIME_DIR = join(DATA_DIR, "runtime");

// The folder pi keeps a working directory's sessions in, as pi names it.
export function sessionDir(cwd) {
	const safe = `--${resolve(cwd).replace(/^[/\\]/, "").replace(/[/\\:]/g, "-")}--`;
	return join(AGENT_DIR, "sessions", safe);
}

export function shortPath(path) {
	const home = homedir();
	return path === home || path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}
