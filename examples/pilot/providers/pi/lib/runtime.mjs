// Finding pi, and the environment it should run in. A window opened from the
// Dock has a bare environment, so PATH and provider keys come from the
// user's login shell: what pi's tools run needs them as much as pi does.
import { execFile, spawnSync } from "node:child_process";
import { accessSync, constants, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { RUNTIME_DIR } from "./paths.mjs";

const PACKAGE = "@earendil-works/pi-coding-agent";
const CREDENTIALS = /(_API_KEY|_AUTH_TOKEN|_OAUTH_TOKEN|_GITHUB_TOKEN|^HF_TOKEN|_BASE_URL)$|^AWS_|^GOOGLE_/;

function executable(path) {
	try {
		accessSync(path, constants.X_OK);
		return true;
	} catch {
		return false;
	}
}

export function adoptLoginEnvironment() {
	const shell = process.env.SHELL || "/bin/zsh";
	const result = spawnSync(shell, ["-ilc", "env -0"], {
		encoding: "utf8",
		timeout: 6000,
		stdio: ["ignore", "pipe", "ignore"],
		maxBuffer: 4 * 1024 * 1024,
	});
	if (result.status !== 0 || !result.stdout) return;
	for (const line of result.stdout.split("\0")) {
		const at = line.indexOf("=");
		if (at <= 0) continue;
		const name = line.slice(0, at);
		const value = line.slice(at + 1);
		if (name === "PATH") process.env.PATH = `${process.env.PATH}${delimiter}${value}`;
		else if (CREDENTIALS.test(name) && !(name in process.env)) process.env[name] = value;
	}
}

// Where pi might be, best first. A version manager's shim comes last: mise's
// picks a version by the folder it runs in, so it would fail in any project
// whose mise.toml it refuses, which is exactly where pi has to run.
function candidates() {
	const home = homedir();
	const onPath = (process.env.PATH ?? "").split(delimiter).filter(Boolean).map((dir) => join(dir, "pi"));
	const isShim = (path) => /[/\\]shims[/\\]/.test(path);
	const mise = join(process.env.MISE_DATA_DIR || join(home, ".local/share/mise"), "installs/pi/latest/pi/pi");
	return [
		process.env.PILOT_PI,
		...onPath.filter((path) => !isShim(path)),
		mise,
		"/opt/homebrew/bin/pi",
		"/usr/local/bin/pi",
		join(home, ".npm-global/bin/pi"),
		join(home, ".bun/bin/pi"),
		join(RUNTIME_DIR, "node_modules/.bin/pi"),
		...onPath.filter(isShim),
		join(home, ".local/share/mise/shims/pi"),
	].filter((path) => path && executable(path));
}

function versionOf(path) {
	const result = spawnSync(path, ["--version"], { encoding: "utf8", timeout: 15_000, cwd: homedir() });
	if (result.status === 0) return result.stdout.trim().split("\n").pop();
	console.error(`pilot: ${path} --version: ${result.error?.message ?? result.stderr}`);
	return null;
}

// The first pi that answers, as `{ path, version }`.
export function findPi() {
	for (const path of new Set(candidates())) {
		const version = versionOf(path);
		if (version) return { path, version };
	}
	return null;
}

// Puts pi in a folder of this app's own, touching nothing else.
export function installPi() {
	mkdirSync(RUNTIME_DIR, { recursive: true });
	return new Promise((resolve, reject) => {
		execFile(
			"npm",
			["install", "--prefix", RUNTIME_DIR, "--no-fund", "--no-audit", PACKAGE],
			{ timeout: 300_000, maxBuffer: 8 * 1024 * 1024 },
			(error, _stdout, stderr) => (error ? reject(new Error(stderr.trim().split("\n").pop() || error.message)) : resolve()),
		);
	});
}
