// The pi provider: lines from the app on stdin, `app.pi` on stdout.
import { StringDecoder } from "node:string_decoder";
import { handle } from "./lib/commands.mjs";
import { Engine } from "./lib/engine.mjs";
import { snapshotOf } from "./lib/publish.mjs";

// About eleven a second: each publish redraws the conversation.
const FRAME = 90;

let timer = null;
let last = "";

function publish() {
	timer = null;
	const line = JSON.stringify(snapshotOf(engine));
	if (line === last) return;
	last = line;
	process.stdout.write(`${line}\n`);
}

const engine = new Engine(() => {
	timer ??= setTimeout(publish, FRAME);
});

const decoder = new StringDecoder("utf8");
let buffer = "";
process.stdin.on("data", (chunk) => {
	buffer += decoder.write(chunk);
	for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
		const line = buffer.slice(0, end);
		buffer = buffer.slice(end + 1);
		if (!line.trim()) continue;
		try {
			const { message } = JSON.parse(line);
			handle(engine, message);
		} catch (error) {
			console.error(`pilot: cannot read a message: ${error.message}`);
		}
	}
});
process.stdin.on("end", () => {
	engine.shutdown();
	setTimeout(() => process.exit(0), 300);
});
// The app gone without a word: nothing is left to publish to.
process.stdout.on("error", () => {
	engine.shutdown();
	process.exit(0);
});
process.on("uncaughtException", (error) => {
	console.error(`pilot: ${error.stack ?? error}`);
});

engine.boot().catch((error) => engine.fail(error));
