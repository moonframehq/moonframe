// A thread with pi running: follows pi's event stream and keeps what the
// window needs between session writes: the message streaming in, the tools
// at work, what is queued, and what extensions are asking for.
import { PiProcess } from "./rpc.mjs";

function applyDelta(partial, update) {
	const { type, contentIndex: index } = update;
	const content = partial.content;
	if (type === "text_start") content[index] = { type: "text", text: "" };
	else if (type === "text_delta") content[index].text += update.delta;
	else if (type === "text_end") content[index].text = update.content;
	else if (type === "thinking_start") content[index] = { type: "thinking", thinking: "" };
	else if (type === "thinking_delta") content[index].thinking += update.delta;
	else if (type === "thinking_end") content[index].thinking = update.content;
	else if (type === "toolcall_start") {
		content[index] = { type: "toolCall", id: update.id, name: update.toolName, arguments: {} };
	} else if (type === "toolcall_end") content[index] = update.toolCall;
}

export class LiveThread {
	// `hooks` are the engine's: changed, entry, turnStarted, settled, ended,
	// toast, renamed.
	constructor(id, command, args, cwd, hooks) {
		this.id = id;
		this.cwd = cwd;
		this.hooks = hooks;
		this.running = false;
		this.partial = null;
		this.tools = new Map();
		this.queued = { steering: [], followUp: [] };
		this.dialogs = [];
		this.statuses = {};
		this.widgets = {};
		this.retry = null;
		this.compacting = false;
		this.sending = null;
		this.editorText = null;
		this.state = null;
		this.levels = [];
		this.commands = [];
		this.touched = Date.now();
		this.process = new PiProcess(command, args, cwd);
		this.process.on("event", (event) => this.receive(event));
		this.process.on("exit", (reason, stderr) => this.hooks.ended(this, reason, stderr));
		this.ready = this.load();
	}

	async load() {
		await this.refresh();
		const [levels, commands] = await Promise.all([
			this.process.send("get_available_thinking_levels").catch(() => null),
			this.process.send("get_commands").catch(() => null),
		]);
		this.levels = levels?.levels ?? [];
		this.commands = commands?.commands ?? [];
		this.hooks.changed(this);
	}

	async refresh() {
		this.state = await this.process.send("get_state");
		this.hooks.changed(this);
	}

	send(type, fields, timeout) {
		this.touched = Date.now();
		return this.process.send(type, fields, timeout);
	}

	receive(event) {
		this.touched = Date.now();
		switch (event.type) {
			case "agent_start":
				this.running = true;
				break;
			case "message_start":
				this.messageStart(event.message);
				break;
			case "message_update":
				if (this.partial) applyDelta(this.partial, event.assistantMessageEvent);
				break;
			case "message_end":
				if (event.message.role === "assistant") this.partial = null;
				if (event.message.role !== "system") this.hooks.entry(this, event.message);
				break;
			case "tool_execution_start":
				this.tools.set(event.toolCallId, {});
				break;
			case "tool_execution_update":
				this.tools.set(event.toolCallId, { content: event.partialResult?.content });
				break;
			case "tool_execution_end":
				this.tools.set(event.toolCallId, {
					done: true,
					content: event.result?.content,
					details: event.result?.details,
					isError: event.isError,
				});
				break;
			case "queue_update":
				this.queued = { steering: event.steering ?? [], followUp: event.followUp ?? [] };
				break;
			case "compaction_start":
				this.compacting = true;
				break;
			case "compaction_end":
				this.compacting = false;
				this.hooks.reload(this);
				break;
			case "auto_retry_start":
				this.retry = { attempt: event.attempt, max: event.maxAttempts, error: event.errorMessage };
				break;
			case "auto_retry_end":
				this.retry = null;
				break;
			case "agent_settled":
				this.settle(event);
				break;
			case "session_info_changed":
				this.hooks.renamed(this, event.name ?? null);
				break;
			case "thinking_level_changed":
				if (this.state) this.state.thinkingLevel = event.level;
				break;
			case "extension_ui_request":
				this.extensionRequest(event);
				break;
			case "extension_error":
				this.hooks.toast(this, "error", "An extension failed", event.error);
				break;
		}
		this.hooks.changed(this);
	}

	messageStart(message) {
		if (message.role === "assistant") this.partial = { ...message, content: [] };
		else if (message.role === "user") {
			this.sending = null;
			this.running = true;
			this.hooks.turnStarted(this, message);
		}
	}

	settle(event) {
		this.running = false;
		this.partial = null;
		this.retry = null;
		this.sending = null;
		this.tools.clear();
		this.hooks.settled(this, event.aborted === true);
	}

	extensionRequest(request) {
		switch (request.method) {
			case "select":
			case "confirm":
			case "input":
			case "editor":
				this.dialogs.push({
					id: request.id,
					kind: request.method,
					title: request.title ?? "",
					message: request.message ?? "",
					options: request.options ?? [],
					placeholder: request.placeholder ?? "",
					prefill: request.prefill ?? "",
				});
				this.hooks.waiting(this, request.title ?? "An extension needs you");
				if (request.timeout) setTimeout(() => this.dropDialog(request.id), request.timeout).unref();
				break;
			case "notify":
				this.hooks.toast(this, request.notifyType ?? "info", "pi", request.message);
				break;
			case "setStatus":
				if (request.statusText) this.statuses[request.statusKey] = request.statusText;
				else delete this.statuses[request.statusKey];
				break;
			case "setWidget":
				if (request.widgetLines) this.widgets[request.widgetKey] = request.widgetLines;
				else delete this.widgets[request.widgetKey];
				break;
			case "set_editor_text":
				this.editorText = { text: request.text, seq: Date.now() };
				break;
		}
	}

	dropDialog(id) {
		this.dialogs = this.dialogs.filter((dialog) => dialog.id !== id);
		this.hooks.changed(this);
	}

	answer(id, answer) {
		this.process.write({ type: "extension_ui_response", id, ...answer });
		this.dropDialog(id);
	}

	stop() {
		this.process.stop();
	}
}
