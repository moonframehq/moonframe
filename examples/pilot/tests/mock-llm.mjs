// A stand-in for a model: an OpenAI-compatible chat completions server that
// streams scripted answers, so the real pi runs end to end with no account.
// `node mock-llm.mjs [port]` prints its port. What it answers depends on the
// words in the last prompt:
//   "fix"    reads price.js, edits it, runs the tests, then sums up
//   "slow"   streams slowly, for steering and stopping a run
//   "title"  a thread's title (what the app asks for after a first run)
//   else     a short markdown answer
import { createServer } from "node:http";

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

function lastPrompt(messages) {
	const text = (message) =>
		typeof message.content === "string"
			? message.content
			: (message.content ?? []).map((part) => part.text ?? "").join(" ");
	const users = messages.filter((message) => message.role === "user");
	return text(users[users.length - 1] ?? { content: "" });
}

// The tool calls already answered since the last prompt.
function toolsSincePrompt(messages) {
	let count = 0;
	for (let i = messages.length - 1; i >= 0 && messages[i].role !== "user"; i--) {
		if (messages[i].role === "tool") count++;
	}
	return count;
}

function script(messages) {
	const prompt = lastPrompt(messages).toLowerCase();
	const done = toolsSincePrompt(messages);
	if (prompt.includes("thread title") || prompt.includes("short title")) {
		const asked = /"text":"([^"]*)/.exec(lastPrompt(messages))?.[1] ?? "New thread";
		const words = asked.split(/\s+/).slice(0, 4);
		return { text: words.map((word) => word[0].toUpperCase() + word.slice(1)).join(" ") };
	}
	if (prompt.includes("fix")) {
		const steps = [
			{ think: "The cents lose their leading zero.", tool: "read", args: { path: "src/price.js" } },
			{
				tool: "edit",
				args: {
					path: "src/price.js",
					edits: [{ oldText: "${rest}", newText: '${String(rest).padStart(2, "0")}' }],
				},
			},
			{ tool: "bash", args: { command: "node --test" } },
		];
		if (done < steps.length) return steps[done];
		return {
			text: "Fixed `formatPrice` to pad cents to two digits.\n\n`node --test` passes.",
		};
	}
	if (prompt.includes("slow")) {
		return { text: "Thinking this through one word at a time. ".repeat(40), delay: 60 };
	}
	return {
		text: "Here is a **short** answer with `code`.\n\n- one\n- two\n\n```js\nconsole.log('hi');\n```\n",
	};
}

function chunk(model, delta, finish = null) {
	return `data: ${JSON.stringify({
		id: "mock",
		object: "chat.completion.chunk",
		created: 0,
		model,
		choices: [{ index: 0, delta, finish_reason: finish }],
	})}\n\n`;
}

async function answer(request, response) {
	let body = "";
	for await (const part of request) body += part;
	const { messages = [], model = "mock" } = JSON.parse(body || "{}");
	response.writeHead(200, { "content-type": "text/event-stream" });
	const step = script(messages);
	const delay = step.delay ?? 15;
	let closed = false;
	response.on("close", () => (closed = true));
	const write = (text) => closed || response.write(text);
	write(chunk(model, { role: "assistant", content: "" }));
	for (const word of (step.think ?? "").split(/(?<= )/).filter(Boolean)) {
		write(chunk(model, { reasoning_content: word }));
		await pause(delay);
	}
	for (const word of (step.text ?? "").split(/(?<= )/).filter(Boolean)) {
		if (closed) return;
		write(chunk(model, { content: word }));
		await pause(delay);
	}
	if (step.tool) {
		write(
			chunk(model, {
				tool_calls: [
					{
						index: 0,
						id: `call_${Date.now()}`,
						type: "function",
						function: { name: step.tool, arguments: JSON.stringify(step.args) },
					},
				],
			}),
		);
	}
	write(chunk(model, {}, step.tool ? "tool_calls" : "stop"));
	write(
		`data: ${JSON.stringify({
			id: "mock",
			object: "chat.completion.chunk",
			created: 0,
			model,
			choices: [],
			usage: { prompt_tokens: 1200, completion_tokens: 80, total_tokens: 1280 },
		})}\n\n`,
	);
	write("data: [DONE]\n\n");
	response.end();
}

export function serve(port = 0) {
	const server = createServer((request, response) => {
		answer(request, response).catch((error) => {
			console.error(error);
			response.destroy();
		});
	});
	return new Promise((done) => server.listen(port, "127.0.0.1", () => done(server)));
}

if (import.meta.url === `file://${process.argv[1]}`) {
	const server = await serve(Number(process.argv[2] ?? 0));
	console.log(server.address().port);
}
