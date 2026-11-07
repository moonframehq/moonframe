// One `pi --mode rpc` child: JSONL both ways, split on LF only, as pi's
// protocol requires, with commands matched to their responses by id.
import { spawn } from "node:child_process";
import { EventEmitter } from "node:events";
import { StringDecoder } from "node:string_decoder";

const COMMAND_TIMEOUT = 60_000;

export class PiProcess extends EventEmitter {
	constructor(command, args, cwd) {
		super();
		this.pending = new Map();
		this.nextId = 1;
		this.stderr = "";
		this.exited = false;
		this.child = spawn(command, ["--mode", "rpc", ...args], {
			cwd,
			stdio: ["pipe", "pipe", "pipe"],
			env: { ...process.env, PI_SKIP_VERSION_CHECK: "1" },
		});
		this.child.on("error", (error) => this.finish(error.message));
		this.child.on("exit", (code, signal) => this.finish(signal ? `stopped by ${signal}` : `exit ${code}`));
		this.child.stdin.on("error", () => {});
		this.readLines(this.child.stdout);
		this.child.stderr.setEncoding("utf8");
		this.child.stderr.on("data", (text) => {
			this.stderr = (this.stderr + text).slice(-4000);
		});
	}

	readLines(stream) {
		const decoder = new StringDecoder("utf8");
		let buffer = "";
		stream.on("data", (chunk) => {
			buffer += decoder.write(chunk);
			for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
				const line = buffer.slice(0, end).replace(/\r$/, "");
				buffer = buffer.slice(end + 1);
				if (line) this.receive(line);
			}
		});
	}

	receive(line) {
		let record;
		try {
			record = JSON.parse(line);
		} catch {
			console.error(`pilot: pi printed something else: ${line.slice(0, 200)}`);
			return;
		}
		if (record.type === "response" && record.id && this.pending.has(record.id)) {
			const { resolve, reject, timer } = this.pending.get(record.id);
			this.pending.delete(record.id);
			clearTimeout(timer);
			if (record.success) resolve(record.data);
			else reject(new Error(record.error || `${record.command} failed`));
			return;
		}
		this.emit("event", record);
	}

	finish(reason) {
		if (this.exited) return;
		this.exited = true;
		for (const { reject, timer } of this.pending.values()) {
			clearTimeout(timer);
			reject(new Error(`pi ended (${reason})`));
		}
		this.pending.clear();
		this.emit("exit", reason, this.stderr.trim());
	}

	write(record) {
		if (!this.exited) this.child.stdin.write(`${JSON.stringify(record)}\n`);
	}

	send(type, fields = {}, timeout = COMMAND_TIMEOUT) {
		if (this.exited) return Promise.reject(new Error("pi is not running"));
		const id = `c${this.nextId++}`;
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => {
				this.pending.delete(id);
				reject(new Error(`pi did not answer ${type}`));
			}, timeout);
			this.pending.set(id, { resolve, reject, timer });
			this.write({ id, type, ...fields });
		});
	}

	// Closing stdin is pi's orderly stop; a child that stays gets a signal.
	stop() {
		if (this.exited) return;
		this.child.stdin.end();
		const timer = setTimeout(() => this.child.kill("SIGTERM"), 1500);
		timer.unref();
	}
}
