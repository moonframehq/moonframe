// Runs the pi provider as the app does, against the real pi and the mock
// model, in folders of its own: send it messages, wait on what it publishes.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { serve } from "./mock-llm.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));

export function fixtureProject(root) {
	const project = join(root, "tidy-cart");
	mkdirSync(join(project, "src"), { recursive: true });
	writeFileSync(join(project, "package.json"), '{"type":"module"}\n');
	writeFileSync(
		join(project, "src/price.js"),
		"export function formatPrice(cents) {\n  const dollars = Math.floor(cents / 100);\n  const rest = cents % 100;\n  return `$${dollars}.${rest}`;\n}\n",
	);
	writeFileSync(
		join(project, "price.test.js"),
		'import test from "node:test";\nimport assert from "node:assert";\nimport { formatPrice } from "./src/price.js";\ntest("pads", () => assert.equal(formatPrice(1205), "$12.05"));\n',
	);
	const run = (...args) => execFileSync("git", args, { cwd: project });
	run("init", "-q", "-b", "main");
	run("add", "-A");
	run("-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qm", "Start");
	return project;
}

export async function mockAgentDir(root, port) {
	const dir = join(root, "agent");
	mkdirSync(dir, { recursive: true });
	writeFileSync(
		join(dir, "models.json"),
		JSON.stringify({
			providers: {
				mock: {
					baseUrl: `http://127.0.0.1:${port}/v1`,
					api: "openai-completions",
					apiKey: "mock",
					models: [
						{ id: "mock-1", name: "Mock One", reasoning: true, input: ["text", "image"], contextWindow: 200000, maxTokens: 8000, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } },
					],
				},
			},
		}),
	);
	return dir;
}

export async function start() {
	const server = await serve();
	const root = mkdtempSync(join(tmpdir(), "pilot-test-"));
	const agent = await mockAgentDir(root, server.address().port);
	const project = fixtureProject(root);
	const child = spawn("bash", ["run.sh"], {
		cwd: join(HERE, "../providers/pi"),
		stdio: ["pipe", "pipe", "inherit"],
		env: { ...process.env, PI_CODING_AGENT_DIR: agent, PILOT_DATA: join(root, "data"), PI_OFFLINE: "1" },
	});
	const provider = { root, agent, project, value: null, sizes: [] };
	const waiters = new Set();
	let buffer = "";
	child.stdout.setEncoding("utf8");
	child.stdout.on("data", (text) => {
		buffer += text;
		for (let end = buffer.indexOf("\n"); end >= 0; end = buffer.indexOf("\n")) {
			provider.sizes.push(end);
			provider.value = JSON.parse(buffer.slice(0, end));
			buffer = buffer.slice(end + 1);
			for (const waiter of waiters) waiter();
		}
	});
	provider.send = (message) => child.stdin.write(`${JSON.stringify({ from: "test", message })}\n`);
	provider.until = (test, label = "condition", timeout = 25_000) =>
		new Promise((resolve, reject) => {
			const timer = setTimeout(() => {
				waiters.delete(check);
				reject(new Error(`timed out on ${label}: ${JSON.stringify(provider.value)?.slice(0, 1500)}`));
			}, timeout);
			const check = () => {
				const found = provider.value && test(provider.value);
				if (!found) return;
				clearTimeout(timer);
				waiters.delete(check);
				resolve(found);
			};
			waiters.add(check);
			check();
		});
	provider.stop = () => {
		child.stdin.end();
		server.close();
		server.closeAllConnections();
	};
	return provider;
}
