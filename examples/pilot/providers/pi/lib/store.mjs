// What this app remembers that pi does not: the workspaces, each thread's
// place and flags, schedules and preferences. Conversations stay in pi's own
// session files.
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DATA_DIR } from "./paths.mjs";

const FILE = join(DATA_DIR, "state.json");

const blank = () => ({
	version: 1,
	workspaces: [],
	threads: {},
	schedules: [],
	selected: null,
	prefs: { notifications: true, sounds: true, auto_title: true, group: "time" },
});

export class Store {
	constructor() {
		this.data = blank();
		try {
			const saved = JSON.parse(readFileSync(FILE, "utf8"));
			this.data = { ...blank(), ...saved, prefs: { ...blank().prefs, ...saved.prefs } };
		} catch (error) {
			if (error.code !== "ENOENT") console.error(`pilot: cannot read ${FILE}: ${error.message}`);
		}
		this.timer = null;
	}

	save() {
		this.timer ??= setTimeout(() => this.flush(), 200);
	}

	flush() {
		clearTimeout(this.timer);
		this.timer = null;
		mkdirSync(DATA_DIR, { recursive: true });
		const scratch = `${FILE}.${process.pid}`;
		writeFileSync(scratch, JSON.stringify(this.data, null, "\t"));
		renameSync(scratch, FILE);
	}
}
