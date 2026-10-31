# Icons

The icons Moonframe adds to gpui-kit's bundled set, for `ui.icon`. The
app embeds this folder (`crates/moonframe/src/assets.rs`) and looks here
before gpui-kit's own icons. Each one is a single-color 24×24 SVG; GPUI
draws it as a mask in the text color, so the stylesheet's `color` tints it.

| Icons | Source | License |
| --- | --- | --- |
| Glyphs: every name without a prefix except `gitlab` | [Lucide](https://lucide.dev) 1.43.0, as gpui-kit-assets 0.8.1 ships it, unchanged. `trash-2`, `filter`, `history`, `circle-help`, `unlock` and `split-square-*` are Lucide's deprecated aliases of `trash`, `funnel`, `rotate-ccw-clock`, `circle-question-mark`, `lock-open` and `square-split-*`, under their old names | ISC, with Feather-derived icons under MIT: [LICENSE-Lucide.txt](LICENSE-Lucide.txt) |
| `agent-claude`, `agent-codex`, `agent-copilot`, `agent-cursor`, `agent-gemini`, `agent-grok`, `agent-kiro`, `agent-opencode`, `agent-amp`, `agent-qwen`, `brand-openai` | [Lobe Icons](https://github.com/lobehub/lobe-icons) (`claude`, `openai`, `githubcopilot`, `cursor`, `gemini`, `grok`, `kiro`, `opencode`, `amp`, `qwen`), reduced to a 24px `currentColor` mask; paths unchanged | MIT: [LICENSE-LobeHub.txt](LICENSE-LobeHub.txt) |
| `brand-anthropic`, `brand-mistral`, `brand-ollama`, `brand-gitlab` (and `gitlab`), `brand-github` | [Simple Icons](https://simpleicons.org) (`anthropic`, `mistralai`, `ollama`, `gitlab`, `github`), with the title removed and the fill set to `currentColor` | CC0 1.0 |
| `agent-droid` | an original lettermark (Factory Droid has no redistributable mark) | the project's license |

Brand and agent marks are their owners' trademarks and identify the product
only; these licenses grant no trademark rights.

`agent-*` names an AI coding agent (the CLI you run), `brand-*` a company or
service. Add an icon by putting its SVG here and its name in `ICONS`
(`crates/moonframe-core/src/node.rs`, sorted) and `IconName`
(`types/moonframe.d.luau`); a test checks every name has a file.
