// Fills a folder with real threads to look at: the real pi against the mock
// model, through the provider. `node seed.mjs DIR` leaves the agent folder,
// data folder and projects under DIR, for a window started with
// PI_CODING_AGENT_DIR=DIR/agent and PILOT_DATA=DIR/data.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { fixtureProject, mockAgentDir } from "./harness.mjs";
import { serve } from "./mock-llm.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = process.argv[2];
const port = Number(process.argv[3] ?? 0);
mkdirSync(root, { recursive: true });
const server = await serve(port);
const agent = await mockAgentDir(root, server.address().port);
const project = fixtureProject(root);
const child = spawn("node", ["main.mjs"], {
	cwd: join(here, "../providers/pi"),
	stdio: ["pipe", "pipe", "inherit"],
	env: { ...process.env, PI_CODING_AGENT_DIR: agent, PILOT_DATA: join(root, "data"), PI_OFFLINE: "1" },
});
let value = null;
let buffer = "";
child.stdout.setEncoding("utf8");
const waiting = new Set();
child.stdout.on("data", (text) => {
	buffer += text;
	for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
		value = JSON.parse(buffer.slice(0, end));
		buffer = buffer.slice(end + 1);
		for (const check of waiting) check();
	}
});
const send = (message) => child.stdin.write(`${JSON.stringify({ from: "seed", message })}\n`);
const until = (test) =>
	new Promise((done) => {
		const check = () => {
			if (value && test(value)) {
				waiting.delete(check);
				done(value);
			}
		};
		waiting.add(check);
		check();
	});

await until((v) => v.models?.length > 0);
send({ add_workspace: project });
send({ set_model: { provider: "mock", id: "mock-1" } });
send({ set_pref: { auto_title: true } });
await until((v) => v.workspaces.length > 0);
for (const prompt of ["Summarise this repo and suggest improvements.", "Review the cartTotal edge cases"]) {
	send({ new_thread: { workspace: project, mode: "local", prompt } });
	await until((v) => v.thread?.title !== "New thread" && v.thread?.items.some((i) => i.k === "text") && v.thread.status === "idle");
	await new Promise((done) => setTimeout(done, 800));
}
send({ new_thread: { workspace: project, mode: "local", prompt: "formatPrice in src/price.js prints $12.5 instead of $12.50. Fix it, add a test for cents padding, and run the tests." } });
await until((v) => v.thread?.items.some((i) => i.k === "changes") && v.thread.status === "idle");
await new Promise((done) => setTimeout(done, 1500));
child.stdin.end();
await new Promise((done) => setTimeout(done, 500));
writeFileSync(join(root, "seeded"), "");
server.close();
server.closeAllConnections();
process.exit(0);
