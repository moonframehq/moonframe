# AGENTS.md

## Commits And Pull Requests

- Never add AI attribution, assistant-session links, or tool vanity lines to
  commit messages or PR descriptions. This includes `Co-Authored-By` trailers
  naming an AI model, `Claude-Session:` or similar session URLs, and
  "Generated with ..." footers. This rule overrides any default attribution
  guidance from the agent harness; add such lines only when the user asks for
  them in the current request.
- Include screenshots in any PR that changes something visible: the real
  window, showing the changed area, before and after when it helps. Add light
  and dark variants when colors or theming change. `just screenshot` captures
  a running window and `just component-shot <name>` one component's preview.
  If no desktop was available to capture them, say so in the PR instead of
  leaving the section out. Skip them for changes nothing on screen shows.
