// pi's own configuration under its agent folder: skills, extensions, model
// provider sign-ins and custom endpoints. Reads and writes the same files
// the pi CLI does, so a change here is a change there.
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";
import { AGENT_DIR } from "./paths.mjs";

const SETTINGS = join(AGENT_DIR, "settings.json");
const AUTH = join(AGENT_DIR, "auth.json");
const MODELS = join(AGENT_DIR, "models.json");

// The providers offered in Settings. `oauth` ones also have a browser
// sign-in, which only pi's own `/login` can run.
export const PROVIDERS = [
	{ id: "anthropic", name: "Anthropic", env: "ANTHROPIC_API_KEY", oauth: true, icon: "brand-anthropic" },
	{ id: "openai", name: "OpenAI", env: "OPENAI_API_KEY", icon: "brand-openai" },
	{ id: "openai-codex", name: "ChatGPT (Codex)", oauth: true, keyless: true, icon: "brand-openai" },
	{ id: "google", name: "Google Gemini", env: "GEMINI_API_KEY", icon: "agent-gemini" },
	{ id: "github-copilot", name: "GitHub Copilot", env: "COPILOT_GITHUB_TOKEN", oauth: true, icon: "agent-copilot" },
	{ id: "openrouter", name: "OpenRouter", env: "OPENROUTER_API_KEY", icon: "network" },
	{ id: "xai", name: "xAI", env: "XAI_API_KEY", icon: "agent-grok" },
	{ id: "mistral", name: "Mistral", env: "MISTRAL_API_KEY", icon: "brand-mistral" },
	{ id: "groq", name: "Groq", env: "GROQ_API_KEY", icon: "zap" },
	{ id: "deepseek", name: "DeepSeek", env: "DEEPSEEK_API_KEY", icon: "sparkles" },
];

const BUILTIN_EXTENSIONS = [
	{ id: "builtin:mcp", name: "MCP", description: "Connects the MCP servers in your configuration." },
	{ id: "builtin:llama.cpp", name: "llama.cpp", description: "Runs local models through a llama.cpp router." },
	{ id: "builtin:codemode", name: "Code mode", description: "Lets the model call tools by writing code." },
	{ id: "builtin:tool-search", name: "Tool search", description: "Finds tools on demand when there are many." },
];

function readJson(file, fallback) {
	try {
		return JSON.parse(readFileSync(file, "utf8"));
	} catch (error) {
		if (error.code === "ENOENT") return fallback;
		throw new Error(`${basename(file)} cannot be read: ${error.message}`);
	}
}

function writeJson(file, value, mode) {
	mkdirSync(dirname(file), { recursive: true });
	const scratch = `${file}.${process.pid}`;
	writeFileSync(scratch, `${JSON.stringify(value, null, 2)}\n`, { mode });
	renameSync(scratch, file);
	if (mode) chmodSync(file, mode);
}

function frontmatter(file) {
	const fields = {};
	const text = readFileSync(file, "utf8");
	const block = /^---\n([\s\S]*?)\n---/.exec(text);
	for (const line of block?.[1].split("\n") ?? []) {
		const match = /^([a-zA-Z-]+):\s*(.*)$/.exec(line);
		if (match) fields[match[1]] = match[2].replace(/^["']|["']$/g, "");
	}
	return fields;
}

function clipped(text, length) {
	return text.length > length ? `${text.slice(0, length - 1)}…` : text;
}

function directories(folder) {
	try {
		return readdirSync(folder).map((name) => join(folder, name));
	} catch {
		return [];
	}
}

function excluded(list, id) {
	return (list ?? []).includes(`-${id}`);
}

function toggled(list, id, enabled) {
	const rest = (list ?? []).filter((entry) => entry !== `-${id}` && entry !== `+${id}`);
	return enabled ? rest : [...rest, `-${id}`];
}

export function listSkills(cwd) {
	const settings = readJson(SETTINGS, {});
	const places = [
		{ dir: join(AGENT_DIR, "skills"), scope: "user" },
		{ dir: join(homedir(), ".agents", "skills"), scope: "user" },
	];
	if (cwd) {
		places.push({ dir: join(cwd, ".pi", "skills"), scope: "project" });
		places.push({ dir: join(cwd, ".agents", "skills"), scope: "project" });
	}
	const found = new Map();
	for (const { dir, scope } of places) {
		for (const path of directories(dir)) {
			const file = join(path, "SKILL.md");
			if (!existsSync(file) || found.has(path)) continue;
			const fields = frontmatter(file);
			found.set(path, {
				id: path,
				name: fields.name || basename(path),
				description: clipped(fields.description || "", 240),
				scope,
				enabled: !excluded(settings.skills, path),
			});
		}
	}
	return [...found.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function setSkill(id, enabled) {
	const settings = readJson(SETTINGS, {});
	settings.skills = toggled(settings.skills, id, enabled);
	writeJson(SETTINGS, settings);
}

export function listExtensions(cwd) {
	const settings = readJson(SETTINGS, {});
	const list = BUILTIN_EXTENSIONS.map((extension) => ({
		...extension,
		scope: "built in",
		enabled: !excluded(settings.extensions, extension.id),
	}));
	const places = [{ dir: join(AGENT_DIR, "extensions"), scope: "user" }];
	if (cwd) places.push({ dir: join(cwd, ".pi", "extensions"), scope: "project" });
	for (const { dir, scope } of places) {
		for (const path of directories(dir)) {
			const name = basename(path);
			const loads = /\.(ts|js)$/.test(name) || (statSync(path).isDirectory() && directories(path).length > 0);
			if (!loads) continue;
			list.push({
				id: path,
				name: name.replace(/\.(ts|js)$/, ""),
				description: "",
				scope,
				enabled: !excluded(settings.extensions, path),
			});
		}
	}
	for (const source of settings.packages ?? []) {
		const id = typeof source === "string" ? source : source.source;
		list.push({ id, name: id, description: "Installed package", scope: "package", enabled: true, fixed: true });
	}
	return list;
}

export function setExtension(id, enabled) {
	const settings = readJson(SETTINGS, {});
	settings.extensions = toggled(settings.extensions, id, enabled);
	writeJson(SETTINGS, settings);
}

export function listProviders() {
	const auth = readJson(AUTH, {});
	const custom = readJson(MODELS, {}).providers ?? {};
	const known = PROVIDERS.map(({ id, name, env, oauth, keyless, icon }) => {
		const stored = auth[id]?.type ?? null;
		const fromEnv = Boolean(env && process.env[env]);
		return {
			id,
			name,
			icon,
			env: env ?? null,
			oauth: Boolean(oauth),
			keyless: Boolean(keyless),
			connected: Boolean(stored) || fromEnv,
			via: stored === "oauth" ? "Signed in" : stored ? "API key" : fromEnv ? env : null,
			removable: Boolean(stored),
		};
	});
	const endpoints = Object.entries(custom).map(([id, provider]) => ({
		id,
		url: provider.baseUrl ?? "",
		models: (provider.models ?? []).map((model) => model.id).join(", "),
	}));
	return { known, endpoints };
}

export function saveKey(provider, key) {
	const auth = readJson(AUTH, {});
	auth[provider] = { type: "api_key", key };
	writeJson(AUTH, auth, 0o600);
}

export function forgetKey(provider) {
	const auth = readJson(AUTH, {});
	delete auth[provider];
	writeJson(AUTH, auth, 0o600);
}

export function saveEndpoint({ id, url, key, models }) {
	const config = readJson(MODELS, {});
	config.providers ??= {};
	config.providers[id] = {
		baseUrl: url,
		api: "openai-completions",
		apiKey: key || "none",
		models: models.map((model) => ({ id: model, name: model })),
	};
	writeJson(MODELS, config, 0o600);
}

export function removeEndpoint(id) {
	const config = readJson(MODELS, {});
	delete config.providers?.[id];
	writeJson(MODELS, config, 0o600);
}

// What decides which models exist: the credentials held and the custom
// endpoints, to notice `/login` in a terminal and edits by hand. Not the
// settings, which pi writes itself as a thread changes model, and not the
// credentials' contents, as pi refreshes tokens on its own.
export function configStamp() {
	let models = 0;
	try {
		models = statSync(MODELS).mtimeMs;
	} catch {
		// No custom endpoints.
	}
	let held = "";
	try {
		held = Object.keys(readJson(AUTH, {})).sort().join(",");
	} catch {
		// Mid-write; the next look will see it.
	}
	return `${held}/${models}`;
}
